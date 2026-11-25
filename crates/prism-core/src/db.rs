//! Persistence layer.
//!
//! Profiles, proxies, settings and launch history live in one JSON document
//! that is written atomically (temp file + fsync + rename). This keeps the
//! dependency graph free of C libraries, makes backups trivially inspectable,
//! and survives a crash mid-write: either the previous or the new document is
//! on disk, never a partial one.
//!
//! Secrets are never stored in clear text: proxy passwords are AES-256-GCM
//! blobs (see `crypto`), and the in-memory model only exposes plaintext through
//! the `Vault` handle the caller passes in.

use crate::crypto::Vault;
use crate::error::{AppError, Result};
use crate::model::*;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

/// Launch history is capped so the store cannot grow without bound.
const HISTORY_LIMIT: usize = 2000;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredProxy {
    id: String,
    label: String,
    protocol: String,
    host: String,
    port: u16,
    #[serde(default)]
    username: String,
    #[serde(default)]
    password: String,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    last_test: Option<ProxyTest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Snapshot {
    #[serde(default = "schema_version")]
    version: u32,
    #[serde(default)]
    profiles: Vec<Profile>,
    #[serde(default)]
    proxies: Vec<StoredProxy>,
    #[serde(default)]
    settings: Settings,
    #[serde(default)]
    history: Vec<HistoryEntry>,
    #[serde(default)]
    next_history_id: i64,
}

fn schema_version() -> u32 {
    1
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            version: schema_version(),
            profiles: Vec::new(),
            proxies: Vec::new(),
            settings: Settings::default(),
            history: Vec::new(),
            next_history_id: 1,
        }
    }
}

pub struct Database {
    path: Option<PathBuf>,
    state: Mutex<Snapshot>,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let snapshot = match std::fs::read(path) {
            Ok(bytes) if !bytes.is_empty() => match serde_json::from_slice::<Snapshot>(&bytes) {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    // Keep the damaged file for recovery instead of silently
                    // starting over with an empty workspace.
                    let backup = path.with_extension(format!(
                        "corrupt-{}.json",
                        chrono::Utc::now().format("%Y%m%d%H%M%S")
                    ));
                    let _ = std::fs::copy(path, &backup);
                    tracing::error!(
                        error = %error,
                        backup = %backup.display(),
                        "the data store could not be parsed; starting with an empty workspace"
                    );
                    Snapshot::default()
                }
            },
            Ok(_) => Snapshot::default(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Snapshot::default(),
            Err(error) => return Err(AppError::Io(error)),
        };
        tracing::debug!(version = snapshot.version, profiles = snapshot.profiles.len(), "data store loaded");
        let db = Self {
            path: Some(path.to_path_buf()),
            state: Mutex::new(snapshot),
        };
        db.persist(&db.lock())?;
        Ok(db)
    }

    /// In-memory store used by unit tests.
    pub fn open_in_memory() -> Result<Self> {
        Ok(Self {
            path: None,
            state: Mutex::new(Snapshot::default()),
        })
    }

    fn lock(&self) -> MutexGuard<'_, Snapshot> {
        self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn persist(&self, snapshot: &Snapshot) -> Result<()> {
        let Some(path) = self.path.as_ref() else {
            return Ok(());
        };
        let temporary = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(snapshot)?;
        {
            let mut file = std::fs::File::create(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
        }
        std::fs::rename(&temporary, path)?;
        Ok(())
    }

    /// Runs a mutation and writes the document once it succeeds. A failed
    /// mutation leaves the previous document untouched.
    fn mutate<T>(&self, change: impl FnOnce(&mut Snapshot) -> Result<T>) -> Result<T> {
        let mut snapshot = self.lock();
        let outcome = change(&mut snapshot)?;
        self.persist(&snapshot)?;
        Ok(outcome)
    }

    /* ---------------------------------------------------------- proxies */

    fn to_proxy(stored: &StoredProxy, vault: &Vault) -> Proxy {
        Proxy {
            id: stored.id.clone(),
            label: stored.label.clone(),
            protocol: ProxyProtocol::parse(&stored.protocol).unwrap_or(ProxyProtocol::Http),
            host: stored.host.clone(),
            port: stored.port,
            username: stored.username.clone(),
            password: vault.open(&stored.password).unwrap_or_default(),
            created_at: stored.created_at.clone(),
            updated_at: stored.updated_at.clone(),
            last_test: stored.last_test.clone(),
        }
    }

    pub fn proxies(&self, vault: &Vault) -> Result<Vec<Proxy>> {
        let snapshot = self.lock();
        let mut list: Vec<Proxy> = snapshot
            .proxies
            .iter()
            .map(|stored| Self::to_proxy(stored, vault))
            .collect();
        list.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
        Ok(list)
    }

    pub fn proxy(&self, vault: &Vault, id: &str) -> Result<Option<Proxy>> {
        let snapshot = self.lock();
        Ok(snapshot
            .proxies
            .iter()
            .find(|stored| stored.id == id)
            .map(|stored| Self::to_proxy(stored, vault)))
    }

    pub fn upsert_proxy(&self, vault: &Vault, input: &ProxyInput) -> Result<Proxy> {
        let host = input.host.trim().to_string();
        if host.is_empty() {
            return Err(AppError::validation("proxy host is required"));
        }
        if input.port == 0 {
            return Err(AppError::validation("proxy port must be between 1 and 65535"));
        }
        let label = if input.label.trim().is_empty() {
            format!("{host}:{}", input.port)
        } else {
            input.label.trim().to_string()
        };
        let sealed = vault.seal(&input.password)?;
        let now = now_iso();

        let stored = self.mutate(|snapshot| {
            if let Some(existing) = input
                .id
                .as_ref()
                .and_then(|id| snapshot.proxies.iter_mut().find(|proxy| &proxy.id == id))
            {
                existing.label = label.clone();
                existing.protocol = input.protocol.as_str().to_string();
                existing.host = host.clone();
                existing.port = input.port;
                existing.username = input.username.trim().to_string();
                existing.password = sealed.clone();
                existing.updated_at = now.clone();
                return Ok(existing.clone());
            }
            let stored = StoredProxy {
                id: uuid::Uuid::new_v4().to_string(),
                label: label.clone(),
                protocol: input.protocol.as_str().to_string(),
                host: host.clone(),
                port: input.port,
                username: input.username.trim().to_string(),
                password: sealed.clone(),
                created_at: now.clone(),
                updated_at: now.clone(),
                last_test: None,
            };
            snapshot.proxies.push(stored.clone());
            Ok(stored)
        })?;
        Ok(Self::to_proxy(&stored, vault))
    }

    pub fn delete_proxy(&self, id: &str) -> Result<()> {
        self.mutate(|snapshot| {
            snapshot.proxies.retain(|proxy| proxy.id != id);
            for profile in snapshot.profiles.iter_mut() {
                if profile.proxy_id.as_deref() == Some(id) {
                    profile.proxy_id = None;
                }
            }
            Ok(())
        })
    }

    pub fn record_proxy_test(&self, id: &str, test: &ProxyTest) -> Result<()> {
        self.mutate(|snapshot| {
            if let Some(proxy) = snapshot.proxies.iter_mut().find(|proxy| proxy.id == id) {
                proxy.last_test = Some(test.clone());
                proxy.updated_at = now_iso();
            }
            Ok(())
        })
    }

    #[cfg(test)]
    fn raw_password(&self, id: &str) -> Option<String> {
        self.lock()
            .proxies
            .iter()
            .find(|proxy| proxy.id == id)
            .map(|proxy| proxy.password.clone())
    }

    /* --------------------------------------------------------- profiles */

    pub fn profiles(&self, query: &ProfileQuery) -> Result<Vec<Profile>> {
        let snapshot = self.lock();
        let search = query.search.trim().to_lowercase();
        let tag = query.tag.trim();
        let proxy = query.proxy_id.trim();
        let folder = query.folder.trim();

        let mut list: Vec<Profile> = snapshot
            .profiles
            .iter()
            .filter(|profile| {
                if !search.is_empty() {
                    let haystack = format!(
                        "{}\n{}\n{}\n{}",
                        profile.name,
                        profile.notes,
                        profile.tags.join(" "),
                        profile.folder
                    )
                    .to_lowercase();
                    if !haystack.contains(&search) {
                        return false;
                    }
                }
                match query.status.as_str() {
                    "active" if profile.status != ProfileStatus::Active => return false,
                    "archived" if profile.status != ProfileStatus::Archived => return false,
                    _ => {}
                }
                if !tag.is_empty()
                    && !profile
                        .tags
                        .iter()
                        .any(|value| value.eq_ignore_ascii_case(tag))
                {
                    return false;
                }
                match proxy {
                    "" => {}
                    "none" => {
                        if profile.proxy_id.is_some() {
                            return false;
                        }
                    }
                    id => {
                        if profile.proxy_id.as_deref() != Some(id) {
                            return false;
                        }
                    }
                }
                if !folder.is_empty() && !profile.folder.eq_ignore_ascii_case(folder) {
                    return false;
                }
                true
            })
            .cloned()
            .collect();

        match query.sort.as_str() {
            "created" => list.sort_by(|a, b| b.created_at.cmp(&a.created_at)),
            "lastLaunch" => list.sort_by(|a, b| match (&b.last_launched_at, &a.last_launched_at) {
                (Some(left), Some(right)) => left.cmp(right),
                (Some(_), None) => std::cmp::Ordering::Greater,
                (None, Some(_)) => std::cmp::Ordering::Less,
                (None, None) => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            }),
            "launchCount" => list.sort_by(|a, b| {
                b.launch_count
                    .cmp(&a.launch_count)
                    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            }),
            _ => list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
        }
        Ok(list)
    }

    pub fn profile(&self, id: &str) -> Result<Option<Profile>> {
        Ok(self
            .lock()
            .profiles
            .iter()
            .find(|profile| profile.id == id)
            .cloned())
    }

    pub fn insert_profile(&self, profile: &Profile) -> Result<()> {
        self.mutate(|snapshot| {
            if snapshot.profiles.iter().any(|item| item.id == profile.id) {
                return Err(AppError::validation(
                    "a profile with this identifier already exists",
                ));
            }
            snapshot.profiles.push(profile.clone());
            Ok(())
        })
    }

    pub fn update_profile(&self, profile: &Profile) -> Result<()> {
        let updated = Profile {
            updated_at: now_iso(),
            ..profile.clone()
        };
        self.mutate(|snapshot| {
            match snapshot.profiles.iter_mut().find(|item| item.id == profile.id) {
                Some(existing) => {
                    *existing = updated.clone();
                    Ok(())
                }
                None => Err(AppError::not_found("profile")),
            }
        })
    }

    pub fn delete_profile(&self, id: &str) -> Result<()> {
        self.mutate(|snapshot| {
            snapshot.profiles.retain(|profile| profile.id != id);
            snapshot.history.retain(|entry| entry.profile_id != id);
            Ok(())
        })
    }

    pub fn record_launch(&self, id: &str) -> Result<i64> {
        let now = now_iso();
        self.mutate(|snapshot| {
            let name = snapshot
                .profiles
                .iter_mut()
                .find(|profile| profile.id == id)
                .map(|profile| {
                    profile.last_launched_at = Some(now.clone());
                    profile.launch_count = profile.launch_count.saturating_add(1);
                    profile.name.clone()
                })
                .ok_or_else(|| AppError::not_found("profile"))?;

            let entry_id = snapshot.next_history_id;
            snapshot.next_history_id += 1;
            snapshot.history.push(HistoryEntry {
                id: entry_id,
                profile_id: id.to_string(),
                profile_name: name,
                started_at: now,
                ended_at: None,
                exit_code: None,
                error: None,
            });
            if snapshot.history.len() > HISTORY_LIMIT {
                let overflow = snapshot.history.len() - HISTORY_LIMIT;
                snapshot.history.drain(0..overflow);
            }
            Ok(entry_id)
        })
    }

    pub fn close_launch(
        &self,
        history_id: i64,
        exit_code: Option<i32>,
        error: Option<&str>,
    ) -> Result<()> {
        let finished = now_iso();
        self.mutate(|snapshot| {
            if let Some(entry) = snapshot.history.iter_mut().find(|entry| entry.id == history_id) {
                entry.ended_at = Some(finished.clone());
                entry.exit_code = exit_code;
                entry.error = error.map(|value| value.to_string());
            }
            Ok(())
        })
    }

    pub fn history(&self, profile_id: Option<&str>, limit: u32) -> Result<Vec<HistoryEntry>> {
        let snapshot = self.lock();
        let mut entries: Vec<HistoryEntry> = snapshot
            .history
            .iter()
            .filter(|entry| profile_id.map(|id| entry.profile_id == id).unwrap_or(true))
            .cloned()
            .collect();
        entries.sort_by(|a, b| b.id.cmp(&a.id));
        entries.truncate(limit as usize);
        Ok(entries)
    }

    pub fn open_history(&self) -> Result<Vec<HistoryEntry>> {
        let snapshot = self.lock();
        Ok(snapshot
            .history
            .iter()
            .filter(|entry| entry.ended_at.is_none())
            .cloned()
            .collect())
    }

    /* --------------------------------------------------------- settings */

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        debug_assert_eq!(key, "app", "only the settings document is stored");
        let snapshot = self.lock();
        Ok(serde_json::to_string(&snapshot.settings).ok())
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        if key != "app" {
            return Err(AppError::validation(format!("unknown setting key \"{key}\"")));
        }
        let settings: Settings = serde_json::from_str(value)?;
        self.mutate(|snapshot| {
            snapshot.settings = settings;
            Ok(())
        })
    }

    pub fn settings(&self) -> Result<Settings> {
        Ok(self.lock().settings.clone())
    }

    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        self.mutate(|snapshot| {
            snapshot.settings = settings.clone();
            Ok(())
        })
    }

    /* ------------------------------------------------------------ stats */

    pub fn count_profiles(&self) -> Result<i64> {
        Ok(self.lock().profiles.len() as i64)
    }

    pub fn count_proxies(&self) -> Result<i64> {
        Ok(self.lock().proxies.len() as i64)
    }

    pub fn clear_history(&self) -> Result<()> {
        self.mutate(|snapshot| {
            snapshot.history.clear();
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vault() -> (tempfile::TempDir, Vault) {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::load_or_create(&dir.path().join("k")).unwrap();
        (dir, vault)
    }

    #[test]
    fn profiles_round_trip_with_filters() {
        let db = Database::open_in_memory().unwrap();
        let mut a = Profile::new("Alpha");
        a.tags = vec!["marketing".into()];
        let mut b = Profile::new("Beta");
        b.status = ProfileStatus::Archived;
        db.insert_profile(&a).unwrap();
        db.insert_profile(&b).unwrap();

        assert_eq!(db.profiles(&ProfileQuery::default()).unwrap().len(), 2);
        assert_eq!(
            db.profiles(&ProfileQuery {
                status: "active".into(),
                ..Default::default()
            })
            .unwrap()
            .len(),
            1
        );
        let tagged = db
            .profiles(&ProfileQuery {
                tag: "marketing".into(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(tagged.len(), 1);
        assert_eq!(tagged[0].name, "Alpha");
        assert_eq!(
            db.profiles(&ProfileQuery {
                search: "bet".into(),
                ..Default::default()
            })
            .unwrap()
            .len(),
            1
        );
    }

    #[test]
    fn proxy_passwords_are_encrypted_at_rest() {
        let db = Database::open_in_memory().unwrap();
        let (_dir, vault) = vault();
        let proxy = db
            .upsert_proxy(
                &vault,
                &ProxyInput {
                    id: None,
                    label: "Residential".into(),
                    protocol: ProxyProtocol::Socks5,
                    host: "10.0.0.1".into(),
                    port: 1080,
                    username: "user".into(),
                    password: "hunter2".into(),
                },
            )
            .unwrap();
        let raw = db.raw_password(&proxy.id).unwrap();
        assert!(raw.starts_with("v1:"));
        assert!(!raw.contains("hunter2"));
        assert_eq!(db.proxy(&vault, &proxy.id).unwrap().unwrap().password, "hunter2");
    }

    #[test]
    fn deleting_a_proxy_clears_profile_assignments() {
        let db = Database::open_in_memory().unwrap();
        let (_dir, vault) = vault();
        let proxy = db
            .upsert_proxy(
                &vault,
                &ProxyInput {
                    id: None,
                    label: String::new(),
                    protocol: ProxyProtocol::Http,
                    host: "proxy.local".into(),
                    port: 8080,
                    username: String::new(),
                    password: String::new(),
                },
            )
            .unwrap();
        let mut profile = Profile::new("With proxy");
        profile.proxy_id = Some(proxy.id.clone());
        db.insert_profile(&profile).unwrap();
        db.delete_proxy(&proxy.id).unwrap();
        assert!(db.profile(&profile.id).unwrap().unwrap().proxy_id.is_none());
    }

    #[test]
    fn history_is_recorded_per_launch() {
        let db = Database::open_in_memory().unwrap();
        let profile = Profile::new("Hist");
        db.insert_profile(&profile).unwrap();
        let id = db.record_launch(&profile.id).unwrap();
        assert_eq!(db.open_history().unwrap().len(), 1);
        db.close_launch(id, Some(0), None).unwrap();
        let entries = db.history(Some(&profile.id), 10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].exit_code, Some(0));
        assert!(db.open_history().unwrap().is_empty());
        assert_eq!(db.profile(&profile.id).unwrap().unwrap().launch_count, 1);
    }

    #[test]
    fn settings_are_persisted() {
        let db = Database::open_in_memory().unwrap();
        let mut settings = Settings::default();
        settings.max_concurrent = 4;
        db.save_settings(&settings).unwrap();
        assert_eq!(db.settings().unwrap().max_concurrent, 4);
    }

    #[test]
    fn document_survives_a_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prism.json");
        {
            let db = Database::open(&path).unwrap();
            let mut profile = Profile::new("Persisted");
            profile.notes = "stays".into();
            db.insert_profile(&profile).unwrap();
        }
        let db = Database::open(&path).unwrap();
        let profiles = db.profiles(&ProfileQuery::default()).unwrap();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].notes, "stays");
    }

    #[test]
    fn a_damaged_document_is_backed_up_not_silently_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prism.json");
        std::fs::write(&path, b"{ this is not json").unwrap();
        let db = Database::open(&path).unwrap();
        assert!(db.profiles(&ProfileQuery::default()).unwrap().is_empty());
        let backups: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().contains("corrupt"))
            .collect();
        assert_eq!(backups.len(), 1);
    }

    #[test]
    fn duplicate_ids_are_rejected() {
        let db = Database::open_in_memory().unwrap();
        let profile = Profile::new("Once");
        db.insert_profile(&profile).unwrap();
        assert!(db.insert_profile(&profile).is_err());
    }
}
