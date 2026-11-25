use crate::error::Result;
use std::path::{Path, PathBuf};

/// Resolves every directory the application writes to.
///
/// Keeping the layout in one place makes backup/restore and the "open data
/// folder" action predictable, and lets power users relocate the data root with
/// `PRISM_DATA_DIR`.
#[derive(Debug, Clone)]
pub struct Paths {
    root: PathBuf,
}

impl Paths {
    pub fn resolve() -> Result<Self> {
        let root = match std::env::var_os("PRISM_DATA_DIR") {
            Some(value) if !value.is_empty() => PathBuf::from(value),
            _ => dirs::data_dir()
                .ok_or_else(|| crate::AppError::msg("could not locate the user data directory"))?
                .join("Prism"),
        };
        let paths = Self { root };
        paths.ensure()?;
        Ok(paths)
    }

    pub fn with_root(root: impl Into<PathBuf>) -> Result<Self> {
        let paths = Self { root: root.into() };
        paths.ensure()?;
        Ok(paths)
    }

    fn ensure(&self) -> Result<()> {
        for dir in [
            self.root.clone(),
            self.profiles_root(),
            self.logs_dir(),
            self.exports_dir(),
            self.browser_extensions_dir(),
        ] {
            std::fs::create_dir_all(&dir)?;
        }
        Ok(())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn database(&self) -> PathBuf {
        self.root.join("prism.json")
    }

    pub fn key_file(&self) -> PathBuf {
        self.root.join("vault.key")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    pub fn exports_dir(&self) -> PathBuf {
        self.root.join("exports")
    }

    pub fn profiles_root(&self) -> PathBuf {
        self.root.join("profiles")
    }

    /// Per-profile directory: `profiles/<uuid>/`.
    /// `browser/` inside it is the Chromium `--user-data-dir`, so cookies,
    /// cache, IndexedDB, history and sessions never leave the profile folder.
    pub fn profile_dir(&self, id: &str) -> PathBuf {
        self.profiles_root().join(id)
    }

    pub fn profile_browser_dir(&self, id: &str) -> PathBuf {
        self.profile_dir(id).join("browser")
    }

    pub fn profile_extension_dir(&self, id: &str) -> PathBuf {
        self.profile_dir(id).join("extension")
    }

    pub fn browser_extensions_dir(&self) -> PathBuf {
        self.root.join("extensions")
    }

    /// Human readable byte size of a directory tree (used by the profile cards
    /// and the storage report in Settings).
    pub fn dir_size(path: &Path) -> u64 {
        let mut total = 0;
        let Ok(entries) = std::fs::read_dir(path) else {
            return 0;
        };
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                total += Self::dir_size(&entry.path());
            } else {
                total += meta.len();
            }
        }
        total
    }
}
