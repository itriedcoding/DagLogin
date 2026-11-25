//! Profile export/import and full backups.
//!
//! Two artefacts are produced:
//!
//! * **config export** – profiles + proxies as JSON (optionally sealed with a
//!   passphrase), small enough to move between machines;
//! * **full backup** – a ZIP containing the config and every profile's browser
//!   state directory, optionally sealed with a passphrase.
//!
//! Sealed files use a streaming AES-256-GCM format (64 KiB chunks, Argon2id key
//! derivation) so multi-gigabyte browser directories can be backed up without
//! loading them into memory.

use crate::crypto::{derive_key, random_bytes};
use crate::error::{AppError, Result};
use crate::model::{now_iso, Profile, Proxy, Settings};
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const MAGIC: &[u8; 8] = b"PRISMBK1";
const CHUNK: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub app: String,
    pub version: String,
    pub created_at: String,
    pub profiles: Vec<Profile>,
    pub proxies: Vec<Proxy>,
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub with_data: bool,
}

impl Manifest {
    pub fn new(profiles: Vec<Profile>, proxies: Vec<Proxy>, settings: Settings, with_data: bool) -> Self {
        Self {
            app: "Prism".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            created_at: now_iso(),
            profiles,
            proxies,
            settings,
            with_data,
        }
    }
}

fn nonce_for(prefix: &[u8], counter: u32) -> [u8; 12] {
    let mut nonce = [0u8; 12];
    nonce[..8].copy_from_slice(&prefix[..8]);
    nonce[8..].copy_from_slice(&counter.to_be_bytes());
    nonce
}

fn seal_stream<R: Read, W: Write>(mut reader: R, mut writer: W, passphrase: &str) -> Result<()> {
    let salt = random_bytes(16);
    let prefix = random_bytes(8);
    let key = derive_key(passphrase, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|_| AppError::Crypto("could not initialise the backup cipher".into()))?;

    writer.write_all(MAGIC)?;
    writer.write_all(&[1u8])?;
    writer.write_all(&salt)?;
    writer.write_all(&prefix)?;

    let mut buffer = vec![0u8; CHUNK];
    let mut counter: u32 = 0;
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce_for(&prefix, counter)), &buffer[..read])
            .map_err(|_| AppError::Crypto("could not encrypt the backup".into()))?;
        writer.write_all(&(ciphertext.len() as u32).to_be_bytes())?;
        writer.write_all(&ciphertext)?;
        counter = counter.checked_add(1).ok_or_else(|| {
            AppError::Crypto("the backup is too large for this format".into())
        })?;
    }
    writer.write_all(&0u32.to_be_bytes())?;
    writer.flush()?;
    Ok(())
}

fn open_stream<R: Read, W: Write>(mut reader: R, mut writer: W, passphrase: &str) -> Result<()> {
    let mut header = [0u8; 8 + 1];
    reader.read_exact(&mut header)?;
    if &header[..8] != MAGIC {
        return Err(AppError::Crypto("this file is not a Prism backup".into()));
    }
    if header[8] != 1 {
        return Err(AppError::Crypto("unsupported backup format version".into()));
    }
    let mut salt = [0u8; 16];
    let mut prefix = [0u8; 8];
    reader.read_exact(&mut salt)?;
    reader.read_exact(&mut prefix)?;
    let key = derive_key(passphrase, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|_| AppError::Crypto("could not initialise the backup cipher".into()))?;

    let mut counter: u32 = 0;
    loop {
        let mut length = [0u8; 4];
        reader.read_exact(&mut length)?;
        let length = u32::from_be_bytes(length) as usize;
        if length == 0 {
            break;
        }
        let mut buffer = vec![0u8; length];
        reader.read_exact(&mut buffer)?;
        let plaintext = cipher
            .decrypt(Nonce::from_slice(&nonce_for(&prefix, counter)), buffer.as_slice())
            .map_err(|_| {
                AppError::Crypto("the passphrase is wrong or the backup is damaged".into())
            })?;
        writer.write_all(&plaintext)?;
        counter = counter
            .checked_add(1)
            .ok_or_else(|| AppError::Crypto("the backup is corrupted".into()))?;
    }
    writer.flush()?;
    Ok(())
}

pub fn is_sealed(path: &Path) -> bool {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(_) => return false,
    };
    let mut magic = [0u8; 8];
    matches!(file.read_exact(&mut magic), Ok(_)) && &magic == MAGIC
}

fn seal_bytes(data: &[u8], passphrase: &str) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(data.len() + 64);
    seal_stream(data, &mut out, passphrase)?;
    Ok(out)
}

fn open_bytes(data: &[u8], passphrase: &str) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(data.len());
    open_stream(data, &mut out, passphrase)?;
    Ok(out)
}

fn seal_file(source: &Path, dest: &Path, passphrase: &str) -> Result<()> {
    let mut input = File::open(source)?;
    let mut output = File::create(dest)?;
    seal_stream(&mut input, &mut output, passphrase)
}

fn open_file(source: &Path, dest: &Path, passphrase: &str) -> Result<()> {
    let mut input = File::open(source)?;
    let mut output = File::create(dest)?;
    open_stream(&mut input, &mut output, passphrase)
}

/* ------------------------------------------------------------ config files */

/// Serialises a config export. Sealed with the passphrase when one is given.
pub fn write_config(dest: &Path, manifest: &Manifest, passphrase: Option<&str>) -> Result<u64> {
    let json = serde_json::to_vec_pretty(manifest)?;
    let bytes = match passphrase.map(str::trim).filter(|p| !p.is_empty()) {
        Some(passphrase) => seal_bytes(&json, passphrase)?,
        None => json,
    };
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(dest, &bytes)?;
    Ok(bytes.len() as u64)
}

pub fn read_config(source: &Path, passphrase: Option<&str>) -> Result<Manifest> {
    let bytes = std::fs::read(source)?;
    let plain = if bytes.len() >= 8 && &bytes[..8] == MAGIC {
        let passphrase = passphrase
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .ok_or_else(|| {
                AppError::msg("this export is encrypted — enter the passphrase to import it")
            })?;
        open_bytes(&bytes, passphrase)?
    } else {
        bytes
    };
    let manifest: Manifest = serde_json::from_slice(&plain)
        .map_err(|e| AppError::msg(format!("this is not a valid Prism export file: {e}")))?;
    if manifest.app != "Prism" {
        return Err(AppError::msg("this file was not created by Prism"));
    }
    Ok(manifest)
}

/* ------------------------------------------------------------------ backups */

fn collect_files(root: &Path) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    fn walk(root: &Path, current: &Path, out: &mut Vec<(PathBuf, String)>) {
        let Ok(entries) = std::fs::read_dir(current) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                walk(root, &path, out);
            } else if meta.is_file() {
                let Ok(relative) = path.strip_prefix(root) else { continue };
                let name = relative.to_string_lossy().replace('\\', "/");
                if name.starts_with("extension/") || name.ends_with(".log") {
                    continue;
                }
                out.push((path, name));
            }
        }
    }
    if root.is_dir() {
        walk(root, root, &mut out);
    }
    out
}

/// Builds the full backup archive. Returns the number of files written.
pub fn write_archive(
    dest: &Path,
    manifest: &Manifest,
    profiles_root: &Path,
    passphrase: Option<&str>,
) -> Result<u64> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = dest.with_extension("zip.tmp");
    let mut file_count = 0u64;
    {
        let file = File::create(&temp)?;
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o644);
        zip.start_file("manifest.json", options)?;
        zip.write_all(&serde_json::to_vec_pretty(manifest)?)?;
        file_count += 1;

        for profile in &manifest.profiles {
            for (path, name) in collect_files(&profiles_root.join(&profile.id)) {
                let entry = format!("profiles/{}/{}", profile.id, name);
                zip.start_file(entry, options)?;
                let mut source = File::open(&path)?;
                std::io::copy(&mut source, &mut zip)?;
                file_count += 1;
            }
        }
        zip.finish()?;
    }

    match passphrase.map(str::trim).filter(|p| !p.is_empty()) {
        Some(passphrase) => {
            seal_file(&temp, dest, passphrase)?;
            std::fs::remove_file(&temp).ok();
        }
        None => {
            std::fs::rename(&temp, dest)?;
        }
    }
    Ok(file_count)
}

/// Extracts an archive (sealed or plain) and returns its manifest.
pub fn read_archive(source: &Path, dest_root: &Path, passphrase: Option<&str>) -> Result<Manifest> {
    let mut archive_path = source.to_path_buf();
    let mut temp_plain: Option<PathBuf> = None;
    if is_sealed(source) {
        let passphrase = passphrase
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .ok_or_else(|| AppError::msg("this backup is encrypted — enter the passphrase"))?;
        let plain = dest_root.join("archive.zip");
        if let Some(parent) = plain.parent() {
            std::fs::create_dir_all(parent)?;
        }
        open_file(source, &plain, passphrase)?;
        archive_path = plain.clone();
        temp_plain = Some(plain);
    }

    let result = (|| -> Result<Manifest> {
        let file = File::open(&archive_path)?;
        let mut archive = ZipArchive::new(file)
            .map_err(|e| AppError::msg(format!("this is not a valid Prism backup: {e}")))?;
        std::fs::create_dir_all(dest_root)?;
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index)?;
            let Some(relative) = entry.enclosed_name().map(|p| p.to_path_buf()) else {
                continue;
            };
            let target = dest_root.join(&relative);
            if entry.is_dir() {
                std::fs::create_dir_all(&target)?;
                continue;
            }
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut output = File::create(&target)?;
            std::io::copy(&mut entry, &mut output)?;
        }
        let manifest_path = dest_root.join("manifest.json");
        let manifest: Manifest = serde_json::from_slice(&std::fs::read(&manifest_path)?)
            .map_err(|e| AppError::msg(format!("the backup manifest is damaged: {e}")))?;
        Ok(manifest)
    })();

    if let Some(temp) = temp_plain {
        std::fs::remove_file(temp).ok();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest(dir: &Path) -> Manifest {
        let mut profile = Profile::new("Restored profile");
        let browser = dir.join(&profile.id).join("browser").join("Default");
        std::fs::create_dir_all(&browser).unwrap();
        std::fs::write(browser.join("Cookies"), b"session-cookie-bytes").unwrap();
        profile.tags = vec!["alpha".into()];
        Manifest::new(
            vec![profile],
            vec![],
            Settings::default(),
            true,
        )
    }

    #[test]
    fn config_round_trips_in_plain_json() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = sample_manifest(dir.path());
        let dest = dir.path().join("export.json");
        write_config(&dest, &manifest, None).unwrap();
        let text = std::fs::read_to_string(&dest).unwrap();
        assert!(text.contains("\"app\": \"Prism\""));
        let read = read_config(&dest, None).unwrap();
        assert_eq!(read.profiles.len(), 1);
    }

    #[test]
    fn config_round_trips_when_sealed() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = sample_manifest(dir.path());
        let dest = dir.path().join("export.prism");
        write_config(&dest, &manifest, Some("correct horse")).unwrap();
        assert!(is_sealed(&dest));
        assert!(!std::fs::read(&dest)
            .unwrap()
            .windows(6)
            .any(|w| w == b"Prism\""));

        assert!(read_config(&dest, None).is_err());
        assert!(read_config(&dest, Some("wrong")).is_err());
        let read = read_config(&dest, Some("correct horse")).unwrap();
        assert_eq!(read.profiles[0].name, "Restored profile");
    }

    #[test]
    fn full_backup_includes_browser_state_and_restores_it() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = sample_manifest(dir.path());
        let dest = dir.path().join("backup.prism");
        let files = write_archive(&dest, &manifest, dir.path(), Some("pw")).unwrap();
        assert!(files >= 2);

        let restore_root = dir.path().join("restore");
        let read = read_archive(&dest, &restore_root, Some("pw")).unwrap();
        let id = &read.profiles[0].id;
        let cookies = restore_root
            .join("profiles")
            .join(id)
            .join("browser/Default/Cookies");
        assert_eq!(std::fs::read(cookies).unwrap(), b"session-cookie-bytes");
        assert!(read.with_data);
    }

    #[test]
    fn rejects_archives_without_a_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.zip");
        {
            let file = File::create(&path).unwrap();
            let mut zip = ZipWriter::new(file);
            zip.start_file("random.txt", SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"nope").unwrap();
            zip.finish().unwrap();
        }
        assert!(read_archive(&path, &dir.path().join("out"), None).is_err());
    }

    #[test]
    fn streaming_cipher_handles_multiple_chunks() {
        let data = vec![7u8; CHUNK * 3 + 123];
        let sealed = seal_bytes(&data, "pw").unwrap();
        assert!(sealed.len() > data.len());
        assert_eq!(open_bytes(&sealed, "pw").unwrap(), data);
    }
}
