//! Envelope encryption for secrets stored on disk (proxy passwords, notes
//! marked private, backup archives).
//!
//! The 256-bit master key lives in `<data>/vault.key`. On Windows the key blob
//! is wrapped with DPAPI (`CryptProtectData`) so it can only be unwrapped by the
//! same Windows user on the same machine. Every secret is sealed with
//! AES-256-GCM using a fresh random 96-bit nonce; the stored form is
//! `v1:<base64(nonce || ciphertext || tag)>`.

use crate::error::{AppError, Result};
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rand::RngCore;
use std::path::Path;

const PREFIX: &str = "v1:";

#[derive(Clone)]
pub struct Vault {
    cipher: Aes256Gcm,
}

impl std::fmt::Debug for Vault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Vault(<redacted>)")
    }
}

impl Vault {
    /// Loads the master key, creating and protecting it on first run.
    pub fn load_or_create(key_path: &Path) -> Result<Self> {
        let key = if key_path.exists() {
            unprotect(&std::fs::read(key_path)?)?
        } else {
            let mut material = [0u8; 32];
            rand::rngs::OsRng.fill_bytes(&mut material);
            if let Some(parent) = key_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let wrapped = protect(&material)?;
            std::fs::write(key_path, wrapped)?;
            restrict_permissions(key_path)?;
            material.to_vec()
        };

        if key.len() != 32 {
            return Err(AppError::Crypto(format!(
                "master key has an unexpected length ({})",
                key.len()
            )));
        }
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|_| AppError::Crypto("could not initialise the cipher".into()))?;
        Ok(Self { cipher })
    }

    /// Encrypts a UTF-8 secret. Empty input maps to an empty string so the
    /// database never carries a useless blob.
    pub fn seal(&self, plaintext: &str) -> Result<String> {
        if plaintext.is_empty() {
            return Ok(String::new());
        }
        let mut nonce = [0u8; 12];
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        let ciphertext = self
            .cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext.as_bytes())
            .map_err(|_| AppError::Crypto("could not encrypt the value".into()))?;
        let mut blob = Vec::with_capacity(12 + ciphertext.len());
        blob.extend_from_slice(&nonce);
        blob.extend_from_slice(&ciphertext);
        Ok(format!("{PREFIX}{}", B64.encode(blob)))
    }

    /// Decrypts a value produced by [`Vault::seal`].
    pub fn open(&self, sealed: &str) -> Result<String> {
        if sealed.is_empty() {
            return Ok(String::new());
        }
        let Some(encoded) = sealed.strip_prefix(PREFIX) else {
            // Value written by an older build: treat it as plain text instead of
            // failing so an upgrade never locks a user out of their proxies.
            return Ok(sealed.to_string());
        };
        let blob = B64
            .decode(encoded)
            .map_err(|_| AppError::Crypto("the stored secret is corrupted".into()))?;
        if blob.len() < 13 {
            return Err(AppError::Crypto("the stored secret is truncated".into()));
        }
        let (nonce, ciphertext) = blob.split_at(12);
        let plaintext = self
            .cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|_| AppError::Crypto("the stored secret could not be decrypted".into()))?;
        String::from_utf8(plaintext)
            .map_err(|_| AppError::Crypto("the stored secret is not valid UTF-8".into()))
    }
}

/// Derives a key from a user passphrase for portable, password protected
/// backups (Argon2id, 64 MiB, 3 passes).
pub fn derive_key(passphrase: &str, salt: &[u8]) -> Result<[u8; 32]> {
    let mut out = [0u8; 32];
    let params = argon2::Params::new(64 * 1024, 3, 1, Some(32))
        .map_err(|e| AppError::Crypto(e.to_string()))?;
    let argon = argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    argon
        .hash_password_into(passphrase.as_bytes(), salt, &mut out)
        .map_err(|e| AppError::Crypto(e.to_string()))?;
    Ok(out)
}

pub fn random_bytes(len: usize) -> Vec<u8> {
    let mut buf = vec![0u8; len];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    buf
}

#[cfg(windows)]
fn protect(data: &[u8]) -> Result<Vec<u8>> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CryptProtectData, CRYPT_INTEGER_BLOB};

    unsafe {
        let mut input = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };
        let ok = CryptProtectData(
            &mut input,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null(),
            0x1, // CRYPTPROTECT_UI_FORBIDDEN
            &mut output,
        );
        if ok == 0 {
            return Err(AppError::Crypto(format!(
                "DPAPI could not protect the master key: {}",
                std::io::Error::last_os_error()
            )));
        }
        let wrapped =
            std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(output.pbData as *mut core::ffi::c_void);
        Ok(wrapped)
    }
}

#[cfg(windows)]
fn unprotect(data: &[u8]) -> Result<Vec<u8>> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB};

    unsafe {
        let mut input = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };
        let ok = CryptUnprotectData(
            &mut input,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null(),
            0x1,
            &mut output,
        );
        if ok == 0 {
            return Err(AppError::Crypto(
                "the master key could not be unlocked for this Windows user".into(),
            ));
        }
        let plain = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(output.pbData as *mut core::ffi::c_void);
        Ok(plain)
    }
}

#[cfg(not(windows))]
fn protect(data: &[u8]) -> Result<Vec<u8>> {
    Ok(data.to_vec())
}

#[cfg(not(windows))]
fn unprotect(data: &[u8]) -> Result<Vec<u8>> {
    Ok(data.to_vec())
}

#[cfg(unix)]
fn restrict_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o600);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path) -> Result<()> {
    // The key blob is DPAPI protected, which already scopes it to this user.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vault() -> (tempfile::TempDir, Vault) {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::load_or_create(&dir.path().join("vault.key")).unwrap();
        (dir, vault)
    }

    #[test]
    fn round_trips_secrets() {
        let (_dir, vault) = vault();
        let sealed = vault.seal("s3cr3t-pässword").unwrap();
        assert!(sealed.starts_with("v1:"));
        assert_ne!(sealed, "s3cr3t-pässword");
        assert_eq!(vault.open(&sealed).unwrap(), "s3cr3t-pässword");
    }

    #[test]
    fn empty_values_stay_empty() {
        let (_dir, vault) = vault();
        assert_eq!(vault.seal("").unwrap(), "");
        assert_eq!(vault.open("").unwrap(), "");
    }

    #[test]
    fn ciphertext_is_not_reusable_across_keys() {
        let dir_a = tempfile::tempdir().unwrap();
        let dir_b = tempfile::tempdir().unwrap();
        let a = Vault::load_or_create(&dir_a.path().join("k")).unwrap();
        let b = Vault::load_or_create(&dir_b.path().join("k")).unwrap();
        let sealed = a.seal("token").unwrap();
        assert!(b.open(&sealed).is_err());
    }

    #[test]
    fn passphrase_derivation_is_deterministic() {
        let salt = b"0123456789abcdef";
        assert_eq!(derive_key("pw", salt).unwrap(), derive_key("pw", salt).unwrap());
        assert_ne!(derive_key("pw", salt).unwrap(), derive_key("pw2", salt).unwrap());
    }
}
