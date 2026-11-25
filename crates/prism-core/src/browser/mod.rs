//! Browser detection and process launching.
//!
//! Prism drives an installed Chromium-family browser (Chrome, Edge, Brave or
//! Chromium). Each profile gets its own `--user-data-dir`, which is what
//! actually isolates cookies, cache, IndexedDB, history and sessions: Chromium
//! itself guarantees that no two browser instances sharing a state directory
//! are isolated, and no two directories ever share state.

pub mod extension;
pub mod relay;
pub mod supervisor;

use crate::error::{AppError, Result};
use crate::model::{BrowserInfo, Fingerprint};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DetectedBrowser {
    pub path: PathBuf,
    pub name: String,
    pub version: Option<String>,
}

pub struct LaunchSpec {
    pub browser: PathBuf,
    pub browser_name: String,
    pub user_data_dir: PathBuf,
    pub extension_dir: Option<PathBuf>,
    pub cdp_port: u16,
    /// `socks5://127.0.0.1:<relay>` when a proxy is assigned.
    pub proxy: Option<String>,
    pub urls: Vec<String>,
    pub fingerprint: Fingerprint,
    pub hardware_acceleration: bool,
    pub log_file: PathBuf,
}

fn push_candidate(list: &mut Vec<(PathBuf, &'static str)>, base: Option<PathBuf>, name: &'static str, rel: &str) {
    if let Some(base) = base {
        let path = base.join(rel);
        if path.exists() {
            list.push((path, name));
        }
    }
}

/// Every place a Chromium-family browser is commonly installed, plus `PATH`.
pub fn candidates() -> Vec<(PathBuf, &'static str)> {
    let mut list: Vec<(PathBuf, &'static str)> = Vec::new();

    #[cfg(windows)]
    {
        let program_files = std::env::var_os("ProgramFiles").map(PathBuf::from);
        let program_files_x86 = std::env::var_os("ProgramFiles(x86)").map(PathBuf::from);
        let local_app_data = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);

        for base in [program_files.clone(), program_files_x86.clone(), local_app_data.clone()]
            .into_iter()
            .flatten()
        {
            push_candidate(
                &mut list,
                Some(base.clone()),
                "Google Chrome",
                "Google/Chrome/Application/chrome.exe",
            );
            push_candidate(
                &mut list,
                Some(base.clone()),
                "Microsoft Edge",
                "Microsoft/Edge/Application/msedge.exe",
            );
            push_candidate(
                &mut list,
                Some(base.clone()),
                "Brave",
                "BraveSoftware/Brave-Browser/Application/brave.exe",
            );
            push_candidate(
                &mut list,
                Some(base.clone()),
                "Chromium",
                "Chromium/Application/chrome.exe",
            );
            push_candidate(
                &mut list,
                Some(base.clone()),
                "Vivaldi",
                "Vivaldi/Application/vivaldi.exe",
            );
        }
    }

    #[cfg(not(windows))]
    {
        for (path, name) in [
            ("/usr/bin/google-chrome", "Google Chrome"),
            ("/usr/bin/google-chrome-stable", "Google Chrome"),
            ("/usr/bin/chromium", "Chromium"),
            ("/usr/bin/chromium-browser", "Chromium"),
            ("/usr/bin/microsoft-edge", "Microsoft Edge"),
            ("/opt/google/chrome/chrome", "Google Chrome"),
        ] {
            let path = PathBuf::from(path);
            if path.exists() {
                list.push((path, name));
            }
        }
    }

    // Executables available on PATH (portable installs, scoop, winget shims).
    let exe_names: &[(&str, &str)] = &[
        ("chrome", "Google Chrome"),
        ("chrome.exe", "Google Chrome"),
        ("msedge", "Microsoft Edge"),
        ("msedge.exe", "Microsoft Edge"),
        ("brave", "Brave"),
        ("brave.exe", "Brave"),
        ("chromium", "Chromium"),
        ("chromium.exe", "Chromium"),
    ];
    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            for (file, name) in exe_names {
                let candidate = dir.join(file);
                if candidate.exists() && !list.iter().any(|(p, _)| p == &candidate) {
                    list.push((candidate, name));
                }
            }
        }
    }

    list
}

/// Reads the browser's own version from the versioned install directory
/// (`.../Application/131.0.6778.86/chrome.exe`), which avoids spawning a process
/// and avoids opening a window just to ask.
pub fn detect_version(browser_path: &Path) -> Option<String> {
    let app_dir = browser_path.parent()?;
    let entries = std::fs::read_dir(app_dir).ok()?;
    let mut versions: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|name| {
            let mut parts = name.split('.');
            parts.next().map(|p| p.chars().all(|c| c.is_ascii_digit()) && !p.is_empty())
                == Some(true)
        })
        .collect();
    versions.sort_by_key(|v| {
        v.split('.')
            .next()
            .and_then(|m| m.parse::<u32>().ok())
            .unwrap_or(0)
    });
    versions.pop()
}

pub fn detect(explicit: Option<&str>) -> Option<DetectedBrowser> {
    if let Some(path) = explicit.map(str::trim).filter(|p| !p.is_empty()) {
        let path = PathBuf::from(path);
        if path.exists() {
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Browser")
                .to_string();
            return Some(DetectedBrowser {
                version: detect_version(&path),
                path,
                name,
            });
        }
        return None;
    }
    candidates()
        .into_iter()
        .map(|(path, name)| DetectedBrowser {
            version: detect_version(&path),
            path,
            name: name.to_string(),
        })
        .next()
}

pub fn browser_info(settings_path: &str) -> BrowserInfo {
    if let Some(detected) = detect(Some(settings_path)) {
        return BrowserInfo {
            path: Some(detected.path.to_string_lossy().to_string()),
            name: detected.name,
            version: detected.version,
            detected: true,
            candidates: candidates()
                .into_iter()
                .map(|(p, _)| p.to_string_lossy().to_string())
                .collect(),
        };
    }
    if settings_path.trim().is_empty() {
        if let Some(detected) = detect(None) {
            return BrowserInfo {
                path: Some(detected.path.to_string_lossy().to_string()),
                name: detected.name,
                version: detected.version,
                detected: true,
                candidates: candidates()
                    .into_iter()
                    .map(|(p, _)| p.to_string_lossy().to_string())
                    .collect(),
            };
        }
    }
    BrowserInfo {
        path: None,
        name: "Not found".into(),
        version: None,
        detected: false,
        candidates: candidates()
            .into_iter()
            .map(|(p, _)| p.to_string_lossy().to_string())
            .collect(),
    }
}

pub fn args_for(spec: &LaunchSpec) -> Vec<String> {
    let mut args: Vec<String> = Vec::with_capacity(28);
    let fp = &spec.fingerprint;

    args.push(format!(
        "--user-data-dir={}",
        spec.user_data_dir.to_string_lossy()
    ));
    args.push("--profile-directory=Default".into());
    args.push("--no-first-run".into());
    args.push("--no-default-browser-check".into());
    args.push("--disable-sync".into());
    args.push("--disable-background-mode".into());
    args.push("--disable-component-update".into());
    args.push("--disable-features=Translate,ChromeWhatsNewUI".into());
    args.push("--no-service-autorun".into());
    args.push("--disable-search-engine-choice-screen".into());
    args.push(format!("--remote-debugging-port={}", spec.cdp_port));
    args.push("--remote-allow-origins=*".into());
    args.push(format!(
        "--window-size={},{}",
        fp.screen.width, fp.screen.height
    ));

    if !fp.locale.trim().is_empty() {
        args.push(format!("--lang={}", fp.locale));
        args.push(format!("--accept-lang={}", fp.accept_language()));
    }
    if !fp.user_agent.trim().is_empty() {
        args.push(format!("--user-agent={}", fp.user_agent));
    }
    if (fp.screen.device_scale_factor - 1.0).abs() > f64::EPSILON {
        args.push(format!(
            "--force-device-scale-factor={}",
            fp.screen.device_scale_factor
        ));
    }
    if fp.headless {
        args.push("--headless=new".into());
        args.push("--disable-gpu".into());
    } else if !spec.hardware_acceleration {
        args.push("--disable-gpu".into());
    }
    if fp.do_not_track {
        args.push("--enable-features=EnableDoNotTrack".into());
    }
    if let Some(proxy) = &spec.proxy {
        args.push(format!("--proxy-server={proxy}"));
        // Fail hard instead of silently falling back to a direct connection.
        args.push("--proxy-bypass-list=<-loopback>".into());
    }
    if let Some(dir) = &spec.extension_dir {
        let value = dir.to_string_lossy().to_string();
        args.push(format!("--disable-extensions-except={value}"));
        args.push(format!("--load-extension={value}"));
    }

    if spec.urls.is_empty() {
        args.push("about:blank".into());
    } else {
        args.extend(spec.urls.iter().cloned());
    }
    args
}

pub fn spawn(spec: &LaunchSpec) -> Result<tokio::process::Child> {
    if !spec.browser.exists() {
        return Err(AppError::msg(format!(
            "the browser executable was not found at {}",
            spec.browser.display()
        )));
    }
    std::fs::create_dir_all(&spec.user_data_dir)?;
    if let Some(parent) = spec.log_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&spec.log_file)
        .ok();

    let mut command = tokio::process::Command::new(&spec.browser);
    command
        .args(args_for(spec))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null());
    command.stderr(match log {
        Some(file) => std::process::Stdio::from(file),
        None => std::process::Stdio::null(),
    });
    command.kill_on_drop(false);

    #[cfg(windows)]
    {
        // Hide the console window Chromium would otherwise attach on some
        // systems and give the profile its own process group so a stop request
        // can target the whole tree.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        command.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
    }

    command
        .spawn()
        .map_err(|e| AppError::msg(format!("could not start {}: {e}", spec.browser_name)))
}

/// Terminates a browser and all of its helper processes.
pub fn kill_tree(pid: u32) -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let output = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        match output {
            Ok(out) if out.status.success() => Ok(()),
            Ok(out) => {
                let text = String::from_utf8_lossy(&out.stderr).trim().to_string();
                // "not found" simply means the process already exited.
                if text.contains("not found") || text.contains("не найден") {
                    Ok(())
                } else {
                    Err(AppError::msg(format!("could not stop the browser: {text}")))
                }
            }
            Err(e) => Err(AppError::msg(format!("could not run taskkill: {e}"))),
        }
    }
    #[cfg(not(windows))]
    {
        let status = std::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
        match status {
            Ok(s) if s.success() => Ok(()),
            _ => Err(AppError::msg(format!("could not stop process {pid}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> LaunchSpec {
        LaunchSpec {
            browser: PathBuf::from("chrome.exe"),
            browser_name: "Chrome".into(),
            user_data_dir: PathBuf::from("C:/data/profiles/abc/browser"),
            extension_dir: Some(PathBuf::from("C:/data/profiles/abc/extension")),
            cdp_port: 9333,
            proxy: Some("socks5://127.0.0.1:49152".into()),
            urls: vec!["https://instagram.com".into()],
            fingerprint: Fingerprint {
                user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/131.0.0.0".into(),
                locale: "de-DE".into(),
                languages: vec!["de-DE".into(), "de".into()],
                do_not_track: true,
                ..Fingerprint::default()
            },
            hardware_acceleration: false,
            log_file: PathBuf::from("C:/data/profiles/abc/browser.log"),
        }
    }

    #[test]
    fn launch_arguments_isolate_and_configure_the_browser() {
        let spec = spec();
        let args = args_for(&spec);
        let joined = args.join(" ");
        assert!(joined.contains("--user-data-dir=C:/data/profiles/abc/browser"));
        assert!(joined.contains("--remote-debugging-port=9333"));
        assert!(joined.contains("--proxy-server=socks5://127.0.0.1:49152"));
        assert!(joined.contains("--lang=de-DE"));
        assert!(joined.contains("--accept-lang=de-DE,de;q=0.9"));
        assert!(joined.contains("--user-agent=Mozilla/5.0"));
        assert!(joined.contains("--disable-gpu"));
        assert!(joined.contains("--window-size=1920,1080"));
        assert!(joined.contains("--load-extension=C:/data/profiles/abc/extension"));
        assert_eq!(args.last().unwrap(), "https://instagram.com");
    }

    #[test]
    fn each_profile_gets_a_distinct_state_directory() {
        let mut first = spec();
        first.user_data_dir = PathBuf::from("C:/data/profiles/one/browser");
        let mut second = spec();
        second.user_data_dir = PathBuf::from("C:/data/profiles/two/browser");
        let first_args = args_for(&first);
        let second_args = args_for(&second);
        assert_ne!(
            first_args.first().unwrap(),
            second_args.first().unwrap(),
            "two profiles must never share a user-data-dir"
        );
        assert!(first_args[0].contains("profiles/one"));
        assert!(second_args[0].contains("profiles/two"));
    }

    #[test]
    fn blank_urls_fall_back_to_about_blank() {
        let mut spec = spec();
        spec.urls.clear();
        assert_eq!(args_for(&spec).last().unwrap(), "about:blank");
    }

    #[test]
    fn version_detection_prefers_the_highest_installed_build() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("Application");
        std::fs::create_dir_all(app.join("129.0.6668.100")).unwrap();
        std::fs::create_dir_all(app.join("131.0.6778.86")).unwrap();
        std::fs::write(app.join("chrome.exe"), b"stub").unwrap();
        assert_eq!(
            detect_version(&app.join("chrome.exe")),
            Some("131.0.6778.86".into())
        );
    }
}
