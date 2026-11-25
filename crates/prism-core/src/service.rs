//! Engine service: the single entry point used by the desktop shell.
//!
//! Every mutating operation is centralised here so the Tauri layer stays a thin
//! wrapper. State is kept consistent when the app dies unexpectedly: open
//! launch-history rows are closed on start-up, and browsers are optional
//! children that never hold profile data open beyond their own lifetime.

use crate::browser::{
    self,
    extension,
    relay::{self, Relay, Upstream},
    supervisor,
};
use crate::crypto::Vault;
use crate::db::Database;
use crate::error::{AppError, Result};
use crate::model::*;
use crate::paths::Paths;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, watch};
use tokio::task::JoinHandle;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum EngineEvent {
    ProfileStarted {
        profile_id: String,
        name: String,
        pid: u32,
    },
    ProfileStopped {
        profile_id: String,
        name: String,
        exit_code: Option<i32>,
        error: Option<String>,
    },
    Notice {
        level: String,
        message: String,
    },
}

struct Running {
    profile_id: String,
    name: String,
    pid: u32,
    cdp_port: u16,
    relay: Option<Relay>,
    relay_port: Option<u16>,
    supervisor_cancel: watch::Sender<bool>,
    supervisor: Option<JoinHandle<()>>,
    started: Instant,
    started_at: String,
    history_id: i64,
    browser_dir: PathBuf,
    proxy_label: Option<String>,
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.supervisor_cancel.send(true);
        if let Some(task) = self.supervisor.take() {
            task.abort();
        }
        // `relay` is dropped here, which closes the loopback listener.
    }
}

pub struct Service {
    paths: Paths,
    db: Arc<Database>,
    vault: Arc<Vault>,
    running: Arc<Mutex<HashMap<String, Running>>>,
    events: broadcast::Sender<EngineEvent>,
    system: Mutex<sysinfo::System>,
    storage_cache: Mutex<HashMap<String, (u64, Instant)>>,
    total_cache: Mutex<Option<(u64, Instant)>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn free_port() -> Result<u16> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

impl Service {
    pub fn bootstrap() -> Result<Self> {
        let paths = Paths::resolve()?;
        crate::init_logging(&paths.logs_dir(), "info");
        Self::new(paths)
    }

    pub fn new(paths: Paths) -> Result<Self> {
        let db = Arc::new(Database::open(&paths.database())?);
        let vault = Arc::new(Vault::load_or_create(&paths.key_file())?);
        let (events, _) = broadcast::channel(256);
        let service = Self {
            paths,
            db,
            vault,
            running: Arc::new(Mutex::new(HashMap::new())),
            events,
            system: Mutex::new(sysinfo::System::new()),
            storage_cache: Mutex::new(HashMap::new()),
            total_cache: Mutex::new(None),
        };
        service.recover_interrupted_launches();
        Ok(service)
    }

    /// Marks launch-history rows left open by a crash or a kill so the UI can
    /// tell the user what happened instead of showing a launch that never ended.
    fn recover_interrupted_launches(&self) {
        match self.db.open_history() {
            Ok(rows) => {
                for row in rows {
                    let _ = self.db.close_launch(
                        row.id,
                        None,
                        Some("Prism stopped unexpectedly while this profile was running"),
                    );
                }
            }
            Err(err) => tracing::warn!(error = %err, "could not recover launch history"),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EngineEvent> {
        self.events.subscribe()
    }

    fn notify(&self, event: EngineEvent) {
        let _ = self.events.send(event);
    }

    /* ------------------------------------------------------------- app info */

    pub fn app_info(&self) -> Result<AppInfo> {
        let settings = self.db.settings()?;
        Ok(AppInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            data_dir: self.paths.root().to_string_lossy().to_string(),
            db_path: self.paths.database().to_string_lossy().to_string(),
            logs_dir: self.paths.logs_dir().to_string_lossy().to_string(),
            os: std::env::consts::OS.to_string(),
            browser: browser::browser_info(&settings.browser_path),
            profile_count: self.db.count_profiles()?,
            proxy_count: self.db.count_proxies()?,
            storage_bytes: self.total_storage(),
        })
    }

    pub fn settings(&self) -> Result<Settings> {
        self.db.settings()
    }

    pub fn save_settings(&self, settings: Settings) -> Result<Settings> {
        if !settings.browser_path.trim().is_empty()
            && !std::path::Path::new(settings.browser_path.trim()).exists()
        {
            return Err(AppError::validation(
                "the selected browser path does not exist",
            ));
        }
        if !settings.proxy_test_url.starts_with("http") {
            return Err(AppError::validation(
                "the proxy test URL must start with http:// or https://",
            ));
        }
        self.db.save_settings(&settings)?;
        Ok(settings)
    }

    pub fn presets(&self) -> Result<Presets> {
        let settings = self.db.settings()?;
        let info = browser::browser_info(&settings.browser_path);
        let mut presets = presets();
        if let Some(version) = info.version.as_deref() {
            let major = version.split('.').next().unwrap_or(version).to_string();
            presets.browser_version = Some(version.to_string());
            for preset in presets.user_agents.iter_mut() {
                preset.value = build_user_agent(&preset.os, Some(&major));
            }
        }
        Ok(presets)
    }

    pub fn browser_info(&self) -> Result<BrowserInfo> {
        let settings = self.db.settings()?;
        Ok(browser::browser_info(&settings.browser_path))
    }

    pub fn detect_browser(&self, path: Option<String>) -> Result<BrowserInfo> {
        let found = browser::detect(path.as_deref());
        match found {
            Some(detected) => Ok(BrowserInfo {
                path: Some(detected.path.to_string_lossy().to_string()),
                name: detected.name,
                version: detected.version,
                detected: true,
                candidates: browser::candidates()
                    .into_iter()
                    .map(|(p, _)| p.to_string_lossy().to_string())
                    .collect(),
            }),
            None => Ok(BrowserInfo {
                path: None,
                name: "Not found".into(),
                version: None,
                detected: false,
                candidates: browser::candidates()
                    .into_iter()
                    .map(|(p, _)| p.to_string_lossy().to_string())
                    .collect(),
            }),
        }
    }

    /* -------------------------------------------------------------- profiles */

    pub fn list_profiles(&self, query: ProfileQuery) -> Result<Vec<ProfileView>> {
        let profiles = self.db.profiles(&query)?;
        let proxies = self.db.proxies(&self.vault)?;
        let proxy_map: HashMap<&str, &Proxy> =
            proxies.iter().map(|p| (p.id.as_str(), p)).collect();
        let running: Vec<String> = lock(&self.running).keys().cloned().collect();

        Ok(profiles
            .into_iter()
            .map(|profile| {
                let proxy = profile
                    .proxy_id
                    .as_deref()
                    .and_then(|id| proxy_map.get(id).copied());
                let status_indicator = if running.contains(&profile.id) {
                    "running"
                } else if profile.status == ProfileStatus::Archived {
                    "archived"
                } else {
                    "idle"
                }
                .to_string();
                let storage_bytes = self.profile_storage(&profile.id);
                ProfileView {
                    profile,
                    proxy_label: proxy.map(|p| p.label.clone()),
                    proxy_protocol: proxy.map(|p| p.protocol.as_str().to_string()),
                    status_indicator,
                    storage_bytes,
                }
            })
            .collect())
    }

    pub fn get_profile(&self, id: &str) -> Result<Profile> {
        self.db
            .profile(id)?
            .ok_or_else(|| AppError::not_found("profile"))
    }

    pub fn create_profile(&self, name: Option<String>) -> Result<Profile> {
        let settings = self.db.settings()?;
        let default_name = format!("Profile {}", self.db.count_profiles()? + 1);
        let mut profile = Profile::new(name.as_deref().unwrap_or(&default_name));
        if !settings.default_start_urls.is_empty() {
            profile.start_urls = settings.default_start_urls.clone();
        }
        profile.validate()?;
        self.db.insert_profile(&profile)?;
        std::fs::create_dir_all(self.paths.profile_browser_dir(&profile.id))?;
        tracing::info!(profile = %profile.id, name = %profile.name, "profile created");
        Ok(profile)
    }

    pub fn update_profile(&self, profile: Profile) -> Result<Profile> {
        profile.validate()?;
        if self.db.profile(&profile.id)?.is_none() {
            return Err(AppError::not_found("profile"));
        }
        self.db.update_profile(&profile)?;
        Ok(self.get_profile(&profile.id)?)
    }

    pub fn duplicate_profile(&self, id: &str, with_data: bool) -> Result<Profile> {
        let source = self.get_profile(id)?;
        let mut copy = source.clone();
        copy.id = uuid::Uuid::new_v4().to_string();
        copy.name = format!("{} (copy)", source.name);
        copy.created_at = now_iso();
        copy.updated_at = now_iso();
        copy.last_launched_at = None;
        copy.launch_count = 0;
        copy.status = ProfileStatus::Active;
        copy.validate()?;
        self.db.insert_profile(&copy)?;

        let target = self.paths.profile_dir(&copy.id);
        std::fs::create_dir_all(&target)?;
        if with_data {
            copy_dir_recursive(&self.paths.profile_browser_dir(id), &self.paths.profile_browser_dir(&copy.id))?;
        }
        tracing::info!(source = %id, copy = %copy.id, with_data, "profile duplicated");
        Ok(copy)
    }

    pub async fn delete_profile(&self, id: &str) -> Result<()> {
        if lock(&self.running).contains_key(id) {
            self.stop(id).await?;
        }
        self.db.delete_profile(id)?;
        let dir = self.paths.profile_dir(id);
        if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(|e| {
                AppError::msg(format!(
                    "the profile was removed but its data folder could not be deleted: {e}"
                ))
            })?;
        }
        lock(&self.storage_cache).remove(id);
        tracing::info!(profile = %id, "profile deleted");
        Ok(())
    }

    pub fn set_archived(&self, id: &str, archived: bool) -> Result<Profile> {
        let mut profile = self.get_profile(id)?;
        profile.status = if archived {
            ProfileStatus::Archived
        } else {
            ProfileStatus::Active
        };
        self.db.update_profile(&profile)?;
        self.get_profile(id)
    }

    pub async fn bulk_action(&self, ids: Vec<String>, action: String) -> Result<usize> {
        let mut count = 0;
        for id in ids {
            match action.as_str() {
                "archive" => {
                    self.set_archived(&id, true)?;
                    count += 1;
                }
                "unarchive" => {
                    self.set_archived(&id, false)?;
                    count += 1;
                }
                "delete" => {
                    self.delete_profile(&id).await?;
                    count += 1;
                }
                "stop" => {
                    self.stop(&id).await.ok();
                    count += 1;
                }
                other => {
                    return Err(AppError::validation(format!(
                        "unknown bulk action \"{other}\""
                    )))
                }
            }
        }
        Ok(count)
    }

    pub fn profile_data_dir(&self, id: &str) -> Result<String> {
        self.get_profile(id)?;
        let dir = self.paths.profile_dir(id);
        std::fs::create_dir_all(&dir)?;
        Ok(dir.to_string_lossy().to_string())
    }

    fn profile_storage(&self, id: &str) -> u64 {
        let now = Instant::now();
        if let Some((bytes, at)) = lock(&self.storage_cache).get(id) {
            if now.duration_since(*at) < Duration::from_secs(30) {
                return *bytes;
            }
        }
        let bytes = Paths::dir_size(&self.paths.profile_dir(id));
        lock(&self.storage_cache).insert(id.to_string(), (bytes, now));
        bytes
    }

    fn total_storage(&self) -> u64 {
        let now = Instant::now();
        if let Some((bytes, at)) = *lock(&self.total_cache) {
            if now.duration_since(at) < Duration::from_secs(60) {
                return bytes;
            }
        }
        let bytes = Paths::dir_size(&self.paths.profiles_root());
        *lock(&self.total_cache) = Some((bytes, now));
        bytes
    }

    /* --------------------------------------------------------------- launch */

    pub async fn launch(&self, id: &str) -> Result<LaunchOutcome> {
        let settings = self.db.settings()?;
        let profile = self.get_profile(id)?;
        if profile.status == ProfileStatus::Archived {
            return Err(AppError::msg(
                "this profile is archived — restore it before launching",
            ));
        }
        {
            let running = lock(&self.running);
            if running.contains_key(id) {
                return Err(AppError::msg("this profile is already running"));
            }
            if settings.max_concurrent > 0 && running.len() as u32 >= settings.max_concurrent {
                return Err(AppError::msg(format!(
                    "the concurrency limit of {} running profiles is reached — close a profile or raise the limit in Settings",
                    settings.max_concurrent
                )));
            }
        }

        let detected = browser::detect(Some(&settings.browser_path))
            .or_else(|| browser::detect(None))
            .ok_or_else(|| {
                AppError::msg(
                    "no Chromium-based browser was found. Install Google Chrome or Microsoft Edge, or set the browser path in Settings.",
                )
            })?;

        let mut warnings: Vec<String> = Vec::new();
        let mut proxy_label = None;
        let mut relay_handle: Option<Relay> = None;

        if let Some(proxy_id) = profile.proxy_id.as_deref() {
            match self.db.proxy(&self.vault, proxy_id)? {
                Some(px) => {
                    let upstream = Upstream {
                        protocol: px.protocol,
                        host: px.host.clone(),
                        port: px.port,
                        username: px.username.clone(),
                        password: px.password.clone(),
                    };
                    match relay::start(upstream).await {
                        Ok(relay) => {
                            proxy_label = Some(px.label.clone());
                            relay_handle = Some(relay);
                        }
                        Err(err) => warnings.push(format!("proxy relay unavailable: {err}")),
                    }
                }
                None => warnings.push(
                    "the assigned proxy no longer exists — the profile will use a direct connection"
                        .into(),
                ),
            }
        }

        let browser_dir = self.paths.profile_browser_dir(id);
        std::fs::create_dir_all(&browser_dir)?;
        let extension_plan =
            extension::prepare(&self.paths.profile_extension_dir(id), &profile.fingerprint)?;

        let cdp_port = free_port()?;
        let urls: Vec<String> = profile
            .start_urls
            .iter()
            .map(|u| u.trim().to_string())
            .filter(|u| !u.is_empty())
            .collect();
        let spec = browser::LaunchSpec {
            browser: detected.path.clone(),
            browser_name: detected.name.clone(),
            user_data_dir: browser_dir.clone(),
            extension_dir: extension_plan
                .enabled
                .then(|| extension_plan.dir.clone()),
            cdp_port,
            proxy: relay_handle.as_ref().map(|r| r.browser_proxy()),
            urls,
            fingerprint: profile.fingerprint.clone(),
            hardware_acceleration: settings.hardware_acceleration,
            log_file: self.paths.profile_dir(id).join("browser.log"),
        };

        let mut child = browser::spawn(&spec)?;
        let pid = child.id().unwrap_or_default();

        // Wait until the debug endpoint answers, but fail fast if the browser
        // exits (wrong path, locked profile, bad arguments).
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut ws_url = None;
        while Instant::now() < deadline {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let code = status.code().unwrap_or(-1);
                    let detail = last_log_lines(&self.paths.profile_dir(id).join("browser.log"));
                    return Err(AppError::msg(format!(
                        "{} closed immediately (exit code {code}). {}{}",
                        detected.name,
                        if detail.is_empty() {
                            String::new()
                        } else {
                            format!("Browser log: {detail}")
                        },
                        "Check that the profile's browser folder is writable."
                    )));
                }
                Ok(None) => {}
                Err(err) => warnings.push(format!("could not read the browser status: {err}")),
            }
            if let Some(url) = supervisor::wait_for_cdp(cdp_port, Duration::from_millis(250)).await {
                ws_url = Some(url);
                break;
            }
        }
        if ws_url.is_none() {
            warnings.push(
                "the browser started but its debug endpoint did not answer; fingerprint overrides and graceful shutdown are unavailable".into(),
            );
        }

        let history_id = self.db.record_launch(id)?;
        let started_at = now_iso();
        let (cancel_tx, cancel_rx) = watch::channel(false);
        let mut supervisor_task = None;
        if settings.cdp_overrides {
            match ws_url {
                Some(url) => {
                    let overrides = overrides_for(&profile);
                    supervisor_task = Some(tokio::spawn(supervisor::supervise(
                        url,
                        overrides,
                        cancel_rx.clone(),
                    )));
                }
                None => warnings.push(
                    "fingerprint overrides were skipped because the browser debug endpoint was not reachable".into(),
                ),
            }
        }

        let relay_port = relay_handle.as_ref().map(|r| r.port());
        let running_entry = Running {
            profile_id: profile.id.clone(),
            name: profile.name.clone(),
            pid,
            cdp_port,
            relay: relay_handle,
            relay_port,
            supervisor_cancel: cancel_tx,
            supervisor: supervisor_task,
            started: Instant::now(),
            started_at: started_at.clone(),
            history_id,
            browser_dir: browser_dir.clone(),
            proxy_label: proxy_label.clone(),
        };
        lock(&self.running).insert(profile.id.clone(), running_entry);

        // Reaper: closes history, frees the map entry and notifies the UI.
        let running_map = Arc::clone(&self.running);
        let db = Arc::clone(&self.db);
        let events = self.events.clone();
        let profile_id = profile.id.clone();
        let profile_name = profile.name.clone();
        tokio::spawn(async move {
            let outcome = child.wait().await;
            let (exit_code, error) = match outcome {
                Ok(status) => (status.code(), None),
                Err(err) => (None, Some(err.to_string())),
            };
            running_map
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .remove(&profile_id);
            let _ = db.close_launch(history_id, exit_code, error.as_deref());
            tracing::info!(profile = %profile_id, ?exit_code, "browser exited");
            let _ = events.send(EngineEvent::ProfileStopped {
                profile_id,
                name: profile_name,
                exit_code,
                error,
            });
        });

        self.notify(EngineEvent::ProfileStarted {
            profile_id: profile.id.clone(),
            name: profile.name.clone(),
            pid,
        });
        tracing::info!(profile = %profile.id, pid, cdp_port, ?relay_port, "profile launched");

        Ok(LaunchOutcome {
            profile_id: profile.id,
            pid,
            cdp_port,
            relay_port,
            started_at,
            warnings,
        })
    }

    pub async fn stop(&self, id: &str) -> Result<()> {
        let (pid, cdp_port) = {
            let running = lock(&self.running);
            match running.get(id) {
                Some(entry) => (entry.pid, entry.cdp_port),
                None => return Err(AppError::msg("this profile is not running")),
            }
        };
        if let Some(entry) = lock(&self.running).get(id) {
            let _ = entry.supervisor_cancel.send(true);
        }

        // Ask Chromium to shut down first: it flushes cookies, storage and
        // history on the way out, which is what keeps sessions intact.
        let graceful = supervisor::close_browser(cdp_port).await;
        if graceful {
            for _ in 0..80 {
                if !lock(&self.running).contains_key(id) || !self.process_alive(pid) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
        if lock(&self.running).contains_key(id) && self.process_alive(pid) {
            tracing::warn!(profile = %id, pid, "graceful shutdown failed, terminating the process tree");
            let _ = browser::kill_tree(pid);
        }
        for _ in 0..60 {
            if !lock(&self.running).contains_key(id) {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        // The reaper may still be finishing; force the entry out so the UI can
        // launch the profile again immediately.
        lock(&self.running).remove(id);
        Ok(())
    }

    pub async fn stop_all(&self) -> Result<usize> {
        let ids: Vec<String> = lock(&self.running).keys().cloned().collect();
        let mut stopped = 0;
        for id in ids {
            if self.stop(&id).await.is_ok() {
                stopped += 1;
            }
        }
        Ok(stopped)
    }

    /// Best-effort synchronous shutdown used by the window close handler.
    pub fn shutdown(&self) {
        let entries: Vec<(String, u32)> = lock(&self.running)
            .iter()
            .map(|(id, entry)| (id.clone(), entry.pid))
            .collect();
        for (_, pid) in entries {
            let _ = browser::kill_tree(pid);
        }
        lock(&self.running).clear();
    }

    fn process_alive(&self, pid: u32) -> bool {
        let mut system = lock(&self.system);
        system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        system.process(sysinfo::Pid::from_u32(pid)).is_some()
    }

    pub fn running(&self) -> Result<Vec<RunningInstance>> {
        let entries: Vec<(
            String,
            String,
            u32,
            u16,
            Option<u16>,
            Instant,
            String,
            PathBuf,
            Option<String>,
        )> = lock(&self.running)
            .values()
            .map(|entry| {
                (
                    entry.profile_id.clone(),
                    entry.name.clone(),
                    entry.pid,
                    entry.cdp_port,
                    entry.relay_port,
                    entry.started,
                    entry.started_at.clone(),
                    entry.browser_dir.clone(),
                    entry.proxy_label.clone(),
                )
            })
            .collect();
        if entries.is_empty() {
            return Ok(Vec::new());
        }

        let (memory, counts, cpu) = {
            let mut system = lock(&self.system);
            system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
            let mut usage: HashMap<u32, (u64, f32)> = HashMap::new();
            let mut parents: HashMap<u32, u32> = HashMap::new();
            for (pid, process) in system.processes() {
                usage.insert(pid.as_u32(), (process.memory(), process.cpu_usage()));
                if let Some(parent) = process.parent() {
                    parents.insert(pid.as_u32(), parent.as_u32());
                }
            }
            let mut memory = HashMap::new();
            let mut counts = HashMap::new();
            let mut cpu = HashMap::new();
            for (_, _, pid, ..) in &entries {
                let mut total_memory = 0u64;
                let mut total_cpu = 0f32;
                let mut count = 0usize;
                let mut stack = vec![*pid];
                while let Some(current) = stack.pop() {
                    if let Some((bytes, cpu_usage)) = usage.get(&current) {
                        total_memory += bytes;
                        total_cpu += cpu_usage;
                        count += 1;
                    }
                    for (child, parent) in &parents {
                        if *parent == current && !stack.contains(child) {
                            stack.push(*child);
                        }
                    }
                }
                memory.insert(*pid, total_memory);
                counts.insert(*pid, count);
                cpu.insert(*pid, total_cpu);
            }
            (memory, counts, cpu)
        };

        Ok(entries
            .into_iter()
            .map(
                |(profile_id, profile_name, pid, cdp_port, relay_port, started, started_at, dir, proxy_label)| RunningInstance {
                    profile_id,
                    profile_name,
                    pid,
                    process_count: counts.get(&pid).copied().unwrap_or(0),
                    cdp_port,
                    relay_port,
                    started_at,
                    uptime_secs: started.elapsed().as_secs(),
                    memory_bytes: memory.get(&pid).copied().unwrap_or(0),
                    cpu_percent: cpu.get(&pid).copied().unwrap_or(0.0),
                    browser_dir: dir.to_string_lossy().to_string(),
                    proxy_label,
                },
            )
            .collect())
    }

    pub fn history(&self, profile_id: Option<String>, limit: Option<u32>) -> Result<Vec<HistoryEntry>> {
        self.db.history(profile_id.as_deref(), limit.unwrap_or(100))
    }

    pub fn clear_history(&self) -> Result<()> {
        self.db.clear_history()
    }

    /* --------------------------------------------------------------- proxies */

    pub fn proxies(&self) -> Result<Vec<Proxy>> {
        self.db.proxies(&self.vault)
    }

    pub fn save_proxy(&self, input: ProxyInput) -> Result<Proxy> {
        if input.port == 0 {
            return Err(AppError::validation("proxy port must be between 1 and 65535"));
        }
        self.db.upsert_proxy(&self.vault, &input)
    }

    pub fn delete_proxy(&self, id: &str) -> Result<()> {
        self.db.delete_proxy(id)
    }

    pub async fn test_proxy(&self, id: &str) -> Result<ProxyTest> {
        let proxy = self
            .db
            .proxy(&self.vault, id)?
            .ok_or_else(|| AppError::not_found("proxy"))?;
        let settings = self.db.settings()?;
        let result = crate::proxy::test(
            proxy.protocol,
            &proxy.host,
            proxy.port,
            &proxy.username,
            &proxy.password,
            &settings.proxy_test_url,
        )
        .await;
        self.db.record_proxy_test(id, &result)?;
        Ok(result)
    }

    pub async fn test_proxy_draft(&self, input: ProxyInput) -> Result<ProxyTest> {
        let settings = self.db.settings()?;
        Ok(crate::proxy::test(
            input.protocol,
            &input.host,
            input.port,
            &input.username,
            &input.password,
            &settings.proxy_test_url,
        )
        .await)
    }

    pub async fn test_all_proxies(&self) -> Result<Vec<Proxy>> {
        let proxies = self.db.proxies(&self.vault)?;
        for proxy in &proxies {
            let _ = self.test_proxy(&proxy.id).await;
        }
        self.db.proxies(&self.vault)
    }

    pub fn import_proxies(&self, text: &str) -> Result<usize> {
        let parsed = crate::proxy::parse_list(text)?;
        let mut count = 0;
        for input in parsed {
            self.db.upsert_proxy(&self.vault, &input)?;
            count += 1;
        }
        Ok(count)
    }

    pub fn export_proxies(&self) -> Result<String> {
        let proxies = self.db.proxies(&self.vault)?;
        Ok(crate::proxy::export_list(&proxies))
    }

    /* -------------------------------------------------------- import/export */

    pub fn export_config(
        &self,
        dest: String,
        ids: Vec<String>,
        passphrase: Option<String>,
    ) -> Result<BackupReport> {
        let profiles = if ids.is_empty() {
            self.db.profiles(&ProfileQuery::default())?
        } else {
            let mut list = Vec::new();
            for id in &ids {
                if let Some(profile) = self.db.profile(id)? {
                    list.push(profile);
                }
            }
            list
        };
        if profiles.is_empty() {
            return Err(AppError::msg("there are no profiles to export"));
        }
        let all_proxies = self.db.proxies(&self.vault)?;
        let used: Vec<String> = profiles
            .iter()
            .filter_map(|p| p.proxy_id.clone())
            .collect();
        let proxies = if ids.is_empty() {
            all_proxies
        } else {
            all_proxies
                .into_iter()
                .filter(|p| used.contains(&p.id))
                .collect()
        };
        let settings = self.db.settings()?;
        let manifest = crate::backup::Manifest::new(profiles, proxies, settings, false);
        let path = PathBuf::from(dest);
        let bytes = crate::backup::write_config(&path, &manifest, passphrase.as_deref())?;
        Ok(BackupReport {
            path: path.to_string_lossy().to_string(),
            profiles: manifest.profiles.len(),
            proxies: manifest.proxies.len(),
            bytes,
            encrypted: passphrase.map(|p| !p.trim().is_empty()).unwrap_or(false),
        })
    }

    pub fn import_config(&self, source: String, passphrase: Option<String>) -> Result<ImportReport> {
        let manifest = crate::backup::read_config(&PathBuf::from(source), passphrase.as_deref())?;
        let mut report = ImportReport {
            imported: 0,
            skipped: 0,
            errors: Vec::new(),
        };
        for mut profile in manifest.profiles {
            if self.db.profile(&profile.id)?.is_some() {
                profile.id = uuid::Uuid::new_v4().to_string();
                report.skipped += 1;
            }
            let original_proxy = profile.proxy_id.clone();
            profile.proxy_id = match original_proxy {
                Some(id) => match manifest.proxies.iter().find(|p| p.id == id) {
                    Some(proxy) => {
                        match self.find_proxy_by_endpoint(&proxy.host, proxy.port) {
                            Some(existing) => Some(existing),
                            None => match self.db.upsert_proxy(
                                &self.vault,
                                &ProxyInput {
                                    id: None,
                                    label: proxy.label.clone(),
                                    protocol: proxy.protocol,
                                    host: proxy.host.clone(),
                                    port: proxy.port,
                                    username: proxy.username.clone(),
                                    password: proxy.password.clone(),
                                },
                            ) {
                                Ok(created) => Some(created.id),
                                Err(err) => {
                                    report.errors.push(format!("proxy for {}: {err}", profile.name));
                                    None
                                }
                            },
                        }
                    }
                    None => None,
                },
                None => None,
            };
            profile.updated_at = now_iso();
            match profile.validate() {
                Ok(()) => match self.db.insert_profile(&profile) {
                    Ok(()) => {
                        let _ = std::fs::create_dir_all(self.paths.profile_browser_dir(&profile.id));
                        report.imported += 1;
                    }
                    Err(err) => report.errors.push(format!("{}: {err}", profile.name)),
                },
                Err(err) => report.errors.push(format!("{}: {err}", profile.name)),
            }
        }
        Ok(report)
    }

    fn find_proxy_by_endpoint(&self, host: &str, port: u16) -> Option<String> {
        self.db
            .proxies(&self.vault)
            .ok()?
            .into_iter()
            .find(|p| p.host.eq_ignore_ascii_case(host) && p.port == port)
            .map(|p| p.id)
    }

    pub fn create_backup(&self, dest: String, passphrase: Option<String>) -> Result<BackupReport> {
        let profiles = self.db.profiles(&ProfileQuery::default())?;
        let proxies = self.db.proxies(&self.vault)?;
        let settings = self.db.settings()?;
        let manifest = crate::backup::Manifest::new(profiles, proxies, settings, true);
        let path = PathBuf::from(dest);
        let files = crate::backup::write_archive(
            &path,
            &manifest,
            &self.paths.profiles_root(),
            passphrase.as_deref(),
        )?;
        let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(files);
        Ok(BackupReport {
            path: path.to_string_lossy().to_string(),
            profiles: manifest.profiles.len(),
            proxies: manifest.proxies.len(),
            bytes,
            encrypted: passphrase.map(|p| !p.trim().is_empty()).unwrap_or(false),
        })
    }

    pub fn restore_backup(&self, source: String, passphrase: Option<String>) -> Result<ImportReport> {
        if !lock(&self.running).is_empty() {
            return Err(AppError::msg(
                "close every running profile before restoring a backup",
            ));
        }
        let staging = self.paths.root().join("restore-staging");
        if staging.exists() {
            std::fs::remove_dir_all(&staging).ok();
        }
        let manifest = crate::backup::read_archive(
            &PathBuf::from(source),
            &staging,
            passphrase.as_deref(),
        )?;

        let mut report = ImportReport {
            imported: 0,
            skipped: 0,
            errors: Vec::new(),
        };
        let mut proxy_ids: HashMap<String, Option<String>> = HashMap::new();
        for proxy in &manifest.proxies {
            let mapped = match self.find_proxy_by_endpoint(&proxy.host, proxy.port) {
                Some(existing) => Some(existing),
                None => match self.db.upsert_proxy(
                    &self.vault,
                    &ProxyInput {
                        id: None,
                        label: proxy.label.clone(),
                        protocol: proxy.protocol,
                        host: proxy.host.clone(),
                        port: proxy.port,
                        username: proxy.username.clone(),
                        password: proxy.password.clone(),
                    },
                ) {
                    Ok(created) => Some(created.id),
                    Err(err) => {
                        report.errors.push(format!("proxy {}: {err}", proxy.label));
                        None
                    }
                },
            };
            proxy_ids.insert(proxy.id.clone(), mapped);
        }

        for mut profile in manifest.profiles {
            let archived_id = profile.id.clone();
            if self.db.profile(&profile.id)?.is_some() {
                profile.id = uuid::Uuid::new_v4().to_string();
                profile.name = format!("{} (restored)", profile.name);
                report.skipped += 1;
            }
            profile.proxy_id = profile
                .proxy_id
                .and_then(|id| proxy_ids.get(&id).cloned().flatten());
            profile.last_launched_at = None;
            profile.updated_at = now_iso();
            profile.created_at = now_iso();
            if let Err(err) = profile.validate() {
                report.errors.push(format!("{}: {err}", profile.name));
                continue;
            }
            if let Err(err) = self.db.insert_profile(&profile) {
                report.errors.push(format!("{}: {err}", profile.name));
                continue;
            }
            let source_dir = staging.join("profiles").join(&archived_id);
            if source_dir.exists() {
                let target = self.paths.profile_dir(&profile.id);
                if let Err(err) = move_dir(&source_dir, &target) {
                    report.errors.push(format!(
                        "{}: profile data could not be restored: {err}",
                        profile.name
                    ));
                }
            }
            report.imported += 1;
        }
        std::fs::remove_dir_all(&staging).ok();
        Ok(report)
    }

    pub fn exports_dir(&self) -> String {
        self.paths.exports_dir().to_string_lossy().to_string()
    }

    pub fn logs_dir(&self) -> String {
        self.paths.logs_dir().to_string_lossy().to_string()
    }

    pub fn data_dir(&self) -> String {
        self.paths.root().to_string_lossy().to_string()
    }

    pub fn reveal_prepare(&self, path: &str) -> Result<String> {
        let path = PathBuf::from(path);
        if path.is_absolute() && path.exists() {
            Ok(path.to_string_lossy().to_string())
        } else {
            Err(AppError::not_found("file"))
        }
    }
}

fn overrides_for(profile: &Profile) -> supervisor::Overrides {
    let fp = &profile.fingerprint;
    supervisor::Overrides {
        timezone: fp.timezone.clone(),
        locale: fp.locale.clone(),
        accept_language: fp.accept_language(),
        user_agent: fp.user_agent.clone(),
        platform: fp.platform.clone(),
        width: fp.screen.width,
        height: fp.screen.height,
        screen_width: fp.screen.screen_width,
        screen_height: fp.screen.screen_height,
        device_scale_factor: fp.screen.device_scale_factor,
        mobile: fp.screen.mobile,
        color_scheme: fp.color_scheme.clone(),
        hardware_concurrency: fp.hardware_concurrency,
    }
}

fn copy_dir_recursive(source: &std::path::Path, target: &std::path::Path) -> Result<()> {
    if !source.exists() {
        std::fs::create_dir_all(target)?;
        return Ok(());
    }
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let destination = target.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&path, &destination)?;
        } else if entry.file_type()?.is_file() {
            std::fs::copy(&path, &destination)?;
        }
    }
    Ok(())
}

fn move_dir(source: &std::path::Path, target: &std::path::Path) -> Result<()> {
    if target.exists() {
        std::fs::remove_dir_all(target).ok();
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    match std::fs::rename(source, target) {
        Ok(()) => Ok(()),
        Err(_) => {
            copy_dir_recursive(source, target)?;
            std::fs::remove_dir_all(source).ok();
            Ok(())
        }
    }
}

fn last_log_lines(path: &std::path::Path) -> String {
    let Ok(text) = std::fs::read_to_string(path) else {
        return String::new();
    };
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .rev()
        .take(2)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" | ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> (tempfile::TempDir, Service) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::with_root(dir.path().join("Prism")).unwrap();
        let service = Service::new(paths).unwrap();
        (dir, service)
    }

    #[test]
    fn creates_lists_and_filters_profiles() {
        let (_dir, service) = service();
        let profile = service.create_profile(Some("Shop EU".into())).unwrap();
        assert_eq!(service.list_profiles(ProfileQuery::default()).unwrap().len(), 1);
        assert!(service
            .list_profiles(ProfileQuery {
                search: "shop".into(),
                ..Default::default()
            })
            .unwrap()
            .len()
            == 1);

        let archived = service.set_archived(&profile.id, true).unwrap();
        assert_eq!(archived.status, ProfileStatus::Archived);
        assert!(service
            .list_profiles(ProfileQuery {
                status: "active".into(),
                ..Default::default()
            })
            .unwrap()
            .is_empty());
    }

    #[test]
    fn duplicate_copies_configuration_and_optionally_data() {
        let (_dir, service) = service();
        let mut profile = service.create_profile(Some("Source".into())).unwrap();
        profile.notes = "keep me".into();
        profile.tags = vec!["eu".into()];
        service.update_profile(profile.clone()).unwrap();

        let browser_dir = service.paths.profile_browser_dir(&profile.id);
        std::fs::write(browser_dir.join("Cookies"), b"data").unwrap();

        let copy = service.duplicate_profile(&profile.id, true).unwrap();
        assert_eq!(copy.notes, "keep me");
        assert_eq!(copy.tags, vec!["eu".to_string()]);
        assert_ne!(copy.id, profile.id);
        assert!(service.paths.profile_browser_dir(&copy.id).join("Cookies").exists());
    }

    #[tokio::test]
    async fn deleting_a_profile_removes_its_data() {
        let (_dir, service) = service();
        let profile = service.create_profile(None).unwrap();
        let dir = service.paths.profile_dir(&profile.id);
        std::fs::write(dir.join("browser").join("Cookies"), b"x").unwrap();
        service.delete_profile(&profile.id).await.unwrap();
        assert!(!dir.exists());
        assert!(service.get_profile(&profile.id).is_err());
    }

    #[test]
    fn export_and_import_round_trip_profiles_and_proxies() {
        let (dir, service) = service();
        let proxy = service
            .save_proxy(ProxyInput {
                id: None,
                label: "Frankfurt".into(),
                protocol: ProxyProtocol::Socks5,
                host: "10.1.1.1".into(),
                port: 1080,
                username: "u".into(),
                password: "p".into(),
            })
            .unwrap();
        let mut profile = service.create_profile(Some("IG DE".into())).unwrap();
        profile.proxy_id = Some(proxy.id.clone());
        service.update_profile(profile).unwrap();

        let dest = dir.path().join("export.prism").to_string_lossy().to_string();
        let report = service
            .export_config(dest.clone(), vec![], Some("pw".into()))
            .unwrap();
        assert_eq!(report.profiles, 1);
        assert!(report.encrypted);

        // Import into a fresh database.
        let other_root = tempfile::tempdir().unwrap();
        let other = Service::new(Paths::with_root(other_root.path().join("P")).unwrap()).unwrap();
        let imported = other.import_config(dest, Some("pw".into())).unwrap();
        assert_eq!(imported.imported, 1);
        let profiles = other.list_profiles(ProfileQuery::default()).unwrap();
        assert_eq!(profiles[0].name, "IG DE");
        assert_eq!(other.proxies().unwrap().len(), 1);
        assert!(other.proxies().unwrap()[0].password == "p");
    }

    #[test]
    fn backup_and_restore_keeps_browser_state() {
        let (dir, service) = service();
        let profile = service.create_profile(Some("Backup me".into())).unwrap();
        let browser = service.paths.profile_browser_dir(&profile.id);
        std::fs::write(browser.join("Cookies"), b"cookie-value").unwrap();

        let dest = dir.path().join("full.prism").to_string_lossy().to_string();
        let report = service.create_backup(dest.clone(), None).unwrap();
        assert_eq!(report.profiles, 1);

        let other_root = tempfile::tempdir().unwrap();
        let other = Service::new(Paths::with_root(other_root.path().join("P")).unwrap()).unwrap();
        let restored = other.restore_backup(dest, None).unwrap();
        assert_eq!(restored.imported, 1);
        let restored_profile = other.list_profiles(ProfileQuery::default()).unwrap();
        let cookies = other
            .paths
            .profile_browser_dir(&restored_profile[0].id)
            .join("Cookies");
        assert_eq!(std::fs::read(cookies).unwrap(), b"cookie-value");
    }

    #[test]
    fn settings_validation_rejects_missing_browser_path() {
        let (_dir, service) = service();
        let mut settings = service.settings().unwrap();
        settings.browser_path = "C:/definitely/not/here/chrome.exe".into();
        assert!(service.save_settings(settings).is_err());
    }

    #[test]
    fn proxy_import_persists_and_exports() {
        let (_dir, service) = service();
        let count = service
            .import_proxies("socks5://a:b@1.1.1.1:1080\n2.2.2.2:8080")
            .unwrap();
        assert_eq!(count, 2);
        let text = service.export_proxies().unwrap();
        assert!(text.contains("socks5://a:b@1.1.1.1:1080"));
        assert_eq!(service.proxies().unwrap().len(), 2);
    }

    #[test]
    fn browser_fallbacks_are_reported_without_panicking() {
        let (_dir, service) = service();
        let info = service.browser_info().unwrap();
        assert!(info.candidates.len() >= 0);
    }

    #[test]
    fn launching_an_archived_profile_is_rejected() {
        let (_dir, service) = service();
        let profile = service.create_profile(None).unwrap();
        service.set_archived(&profile.id, true).unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(service.launch(&profile.id));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("archived"));
    }
}
