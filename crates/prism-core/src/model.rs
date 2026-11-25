//! Serialisable domain model shared with the desktop UI.
//!
//! Everything here is plain data: no I/O, no globals. The UI receives these
//! structures directly over Tauri IPC (camelCase JSON).

use serde::{Deserialize, Serialize};

pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/* ------------------------------------------------------------------ proxies */

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ProxyProtocol {
    #[default]
    Http,
    Https,
    Socks5,
}

impl ProxyProtocol {
    pub fn as_str(self) -> &'static str {
        match self {
            ProxyProtocol::Http => "http",
            ProxyProtocol::Https => "https",
            ProxyProtocol::Socks5 => "socks5",
        }
    }

    pub fn scheme(self) -> &'static str {
        match self {
            ProxyProtocol::Http => "http",
            ProxyProtocol::Https => "https",
            ProxyProtocol::Socks5 => "socks5",
        }
    }

    pub fn default_port(self) -> u16 {
        match self {
            ProxyProtocol::Http | ProxyProtocol::Https => 8080,
            ProxyProtocol::Socks5 => 1080,
        }
    }

    pub fn parse(value: &str) -> crate::Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "http" => Ok(ProxyProtocol::Http),
            "https" => Ok(ProxyProtocol::Https),
            "socks5" | "socks" | "socks5h" => Ok(ProxyProtocol::Socks5),
            other => Err(crate::AppError::validation(format!(
                "unsupported proxy protocol \"{other}\" (expected http, https or socks5)"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyTest {
    pub ok: bool,
    pub latency_ms: Option<u64>,
    pub exit_ip: Option<String>,
    pub country: Option<String>,
    pub error: Option<String>,
    pub tested_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Proxy {
    pub id: String,
    pub label: String,
    pub protocol: ProxyProtocol,
    pub host: String,
    pub port: u16,
    pub username: String,
    /// Decrypted in memory only; stored with AES-GCM.
    #[serde(default)]
    pub password: String,
    pub created_at: String,
    pub updated_at: String,
    pub last_test: Option<ProxyTest>,
}

impl Proxy {
    pub fn has_auth(&self) -> bool {
        !self.username.is_empty() || !self.password.is_empty()
    }

    pub fn display(&self) -> String {
        if self.username.is_empty() {
            format!("{}://{}:{}", self.protocol.as_str(), self.host, self.port)
        } else {
            format!(
                "{}://{}:***@{}:{}",
                self.protocol.as_str(),
                self.username,
                self.host,
                self.port
            )
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyInput {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub label: String,
    pub protocol: ProxyProtocol,
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
}

/* --------------------------------------------------------------- fingerprint */

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Screen {
    pub width: u32,
    pub height: u32,
    pub screen_width: u32,
    pub screen_height: u32,
    pub device_scale_factor: f64,
    pub mobile: bool,
}

impl Default for Screen {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            screen_width: 1920,
            screen_height: 1080,
            device_scale_factor: 1.0,
            mobile: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fingerprint {
    /// `windows` | `macos` | `linux`
    pub os: String,
    /// Empty means "use the installed browser's real user agent".
    pub user_agent: String,
    /// `navigator.platform` value, e.g. `Win32`.
    pub platform: String,
    pub languages: Vec<String>,
    pub timezone: String,
    pub locale: String,
    pub screen: Screen,
    pub hardware_concurrency: u32,
    pub device_memory_gb: u32,
    /// `light` | `dark` | `system`
    pub color_scheme: String,
    /// `default` | `proxy` (only the proxy's exit IP is exposed)
    pub webrtc: String,
    pub canvas_noise: bool,
    pub webgl_noise: bool,
    pub do_not_track: bool,
    /// Deterministic seed so a profile's noise stays stable between sessions.
    pub seed: u64,
    pub headless: bool,
}

impl Default for Fingerprint {
    fn default() -> Self {
        Self {
            os: "windows".into(),
            user_agent: String::new(),
            platform: "Win32".into(),
            languages: vec!["en-US".into(), "en".into()],
            timezone: "America/New_York".into(),
            locale: "en-US".into(),
            screen: Screen::default(),
            hardware_concurrency: 8,
            device_memory_gb: 8,
            color_scheme: "dark".into(),
            webrtc: "proxy".into(),
            canvas_noise: true,
            webgl_noise: true,
            do_not_track: false,
            seed: 0,
            headless: false,
        }
    }
}

impl Fingerprint {
    pub fn accept_language(&self) -> String {
        if self.languages.is_empty() {
            "en-US,en;q=0.9".into()
        } else {
            let mut parts = Vec::new();
            for (index, lang) in self.languages.iter().enumerate() {
                if lang.trim().is_empty() {
                    continue;
                }
                if index == 0 {
                    parts.push(lang.trim().to_string());
                } else {
                    let q = 1.0 - (index as f64 * 0.1);
                    parts.push(format!("{};q={:.1}", lang.trim(), q.max(0.1)));
                }
            }
            parts.join(",")
        }
    }

    pub fn validate(&self) -> crate::Result<()> {
        if self.timezone.trim().is_empty() {
            return Err(crate::AppError::validation("timezone is required"));
        }
        if self.locale.trim().is_empty() {
            return Err(crate::AppError::validation("locale is required"));
        }
        if !self.user_agent.is_empty()
            && (!self.user_agent.contains("Mozilla/") || self.user_agent.len() < 40)
        {
            return Err(crate::AppError::validation(
                "user agent must be a full browser user agent string",
            ));
        }
        if self.screen.width < 640 || self.screen.height < 480 {
            return Err(crate::AppError::validation(
                "window size must be at least 640x480",
            ));
        }
        if self.screen.screen_width < self.screen.width
            || self.screen.screen_height < self.screen.height
        {
            return Err(crate::AppError::validation(
                "screen size cannot be smaller than the window size",
            ));
        }
        if !(1..=64).contains(&self.hardware_concurrency) {
            return Err(crate::AppError::validation(
                "hardware concurrency must be between 1 and 64",
            ));
        }
        if !matches!(self.webrtc.as_str(), "default" | "proxy" | "disabled") {
            return Err(crate::AppError::validation("unknown WebRTC policy"));
        }
        Ok(())
    }
}

/* ------------------------------------------------------------------ profile */

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProfileStatus {
    Active,
    Archived,
}

impl ProfileStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ProfileStatus::Active => "active",
            ProfileStatus::Archived => "archived",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub notes: String,
    pub tags: Vec<String>,
    pub folder: String,
    pub status: ProfileStatus,
    pub color: String,
    pub proxy_id: Option<String>,
    pub start_urls: Vec<String>,
    pub fingerprint: Fingerprint,
    pub created_at: String,
    pub updated_at: String,
    pub last_launched_at: Option<String>,
    pub launch_count: u32,
}

impl Profile {
    pub fn new(name: &str) -> Self {
        let now = now_iso();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            notes: String::new(),
            tags: Vec::new(),
            folder: String::new(),
            status: ProfileStatus::Active,
            color: "#4f8cff".into(),
            proxy_id: None,
            start_urls: vec!["https://www.google.com".into()],
            fingerprint: Fingerprint {
                seed: rand::random::<u64>() & 0x0000_ffff_ffff_ffff,
                ..Fingerprint::default()
            },
            created_at: now.clone(),
            updated_at: now,
            last_launched_at: None,
            launch_count: 0,
        }
    }

    pub fn validate(&self) -> crate::Result<()> {
        if self.name.trim().is_empty() {
            return Err(crate::AppError::validation("profile name is required"));
        }
        if self.name.chars().count() > 80 {
            return Err(crate::AppError::validation(
                "profile name cannot be longer than 80 characters",
            ));
        }
        self.fingerprint.validate()
    }
}

/// A profile plus the derived fields the dashboard needs. Flattened so the UI
/// receives a single object per row.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileView {
    #[serde(flatten)]
    pub profile: Profile,
    pub proxy_label: Option<String>,
    pub proxy_protocol: Option<String>,
    pub status_indicator: String,
    pub storage_bytes: u64,
}

/* ----------------------------------------------------------------- settings */

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Empty means auto-detect.
    pub browser_path: String,
    /// 0 = unlimited.
    pub max_concurrent: u32,
    pub confirm_on_delete: bool,
    pub default_start_urls: Vec<String>,
    pub reduce_animations: bool,
    /// Apply per-profile timezone/locale/screen/user-agent through the Chrome
    /// DevTools Protocol after launch.
    pub cdp_overrides: bool,
    pub hardware_acceleration: bool,
    pub close_browsers_on_exit: bool,
    pub proxy_test_url: String,
    pub log_level: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            browser_path: String::new(),
            max_concurrent: 0,
            confirm_on_delete: true,
            default_start_urls: vec!["https://www.google.com".into()],
            reduce_animations: false,
            cdp_overrides: true,
            hardware_acceleration: true,
            close_browsers_on_exit: true,
            proxy_test_url: "https://api.ipify.org?format=json".into(),
            log_level: "info".into(),
        }
    }
}

/* ------------------------------------------------------------------ runtime */

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningInstance {
    pub profile_id: String,
    pub profile_name: String,
    pub pid: u32,
    pub process_count: usize,
    pub cdp_port: u16,
    pub relay_port: Option<u16>,
    pub started_at: String,
    pub uptime_secs: u64,
    pub memory_bytes: u64,
    pub cpu_percent: f32,
    pub browser_dir: String,
    pub proxy_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchOutcome {
    pub profile_id: String,
    pub pid: u32,
    pub cdp_port: u16,
    pub relay_port: Option<u16>,
    pub started_at: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: i64,
    pub profile_id: String,
    pub profile_name: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
}

/* ------------------------------------------------------------------ presets */

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UaPreset {
    pub label: String,
    pub os: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenPreset {
    pub label: String,
    pub width: u32,
    pub height: u32,
    pub dpr: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Presets {
    pub user_agents: Vec<UaPreset>,
    pub timezones: Vec<String>,
    pub languages: Vec<String>,
    pub locales: Vec<String>,
    pub screens: Vec<ScreenPreset>,
    pub color_schemes: Vec<String>,
    pub webrtc_modes: Vec<String>,
    pub browser_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserInfo {
    pub path: Option<String>,
    pub name: String,
    pub version: Option<String>,
    pub detected: bool,
    pub candidates: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub data_dir: String,
    pub db_path: String,
    pub logs_dir: String,
    pub os: String,
    pub browser: BrowserInfo,
    pub profile_count: i64,
    pub proxy_count: i64,
    pub storage_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub imported: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupReport {
    pub path: String,
    pub profiles: usize,
    pub proxies: usize,
    pub bytes: u64,
    pub encrypted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProfileQuery {
    #[serde(default)]
    pub search: String,
    /// `all` | `active` | `archived`
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub tag: String,
    #[serde(default)]
    pub proxy_id: String,
    #[serde(default)]
    pub folder: String,
    /// `name` | `created` | `lastLaunch` | `launchCount`
    #[serde(default)]
    pub sort: String,
}

pub fn preset_browser_version(version: Option<&str>) -> Option<String> {
    version.map(|v| v.split('.').next().unwrap_or(v).to_string())
}

pub fn build_user_agent(os: &str, major: Option<&str>) -> String {
    let major = major.unwrap_or("131");
    match os {
        "macos" => format!(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{major}.0.0.0 Safari/537.36"
        ),
        "linux" => format!(
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{major}.0.0.0 Safari/537.36"
        ),
        _ => format!(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{major}.0.0.0 Safari/537.36"
        ),
    }
}

pub fn platform_for_os(os: &str) -> &'static str {
    match os {
        "macos" => "MacIntel",
        "linux" => "Linux x86_64",
        _ => "Win32",
    }
}

pub fn presets() -> Presets {
    let timezones = [
        "America/New_York",
        "America/Chicago",
        "America/Denver",
        "America/Los_Angeles",
        "America/Phoenix",
        "America/Anchorage",
        "America/Toronto",
        "America/Vancouver",
        "America/Mexico_City",
        "America/Bogota",
        "America/Lima",
        "America/Santiago",
        "America/Sao_Paulo",
        "America/Argentina/Buenos_Aires",
        "Europe/London",
        "Europe/Dublin",
        "Europe/Lisbon",
        "Europe/Madrid",
        "Europe/Paris",
        "Europe/Brussels",
        "Europe/Amsterdam",
        "Europe/Berlin",
        "Europe/Zurich",
        "Europe/Vienna",
        "Europe/Rome",
        "Europe/Prague",
        "Europe/Warsaw",
        "Europe/Budapest",
        "Europe/Stockholm",
        "Europe/Oslo",
        "Europe/Copenhagen",
        "Europe/Helsinki",
        "Europe/Athens",
        "Europe/Bucharest",
        "Europe/Kyiv",
        "Europe/Istanbul",
        "Europe/Moscow",
        "Asia/Dubai",
        "Asia/Riyadh",
        "Asia/Karachi",
        "Asia/Kolkata",
        "Asia/Dhaka",
        "Asia/Bangkok",
        "Asia/Jakarta",
        "Asia/Singapore",
        "Asia/Kuala_Lumpur",
        "Asia/Manila",
        "Asia/Hong_Kong",
        "Asia/Shanghai",
        "Asia/Taipei",
        "Asia/Seoul",
        "Asia/Tokyo",
        "Australia/Perth",
        "Australia/Brisbane",
        "Australia/Sydney",
        "Australia/Melbourne",
        "Pacific/Auckland",
        "Africa/Lagos",
        "Africa/Johannesburg",
        "Africa/Cairo",
        "Africa/Nairobi",
        "Africa/Casablanca",
        "UTC",
    ]
    .iter()
    .map(|v| v.to_string())
    .collect();

    let languages = [
        "en-US", "en-GB", "de-DE", "fr-FR", "es-ES", "es-MX", "pt-BR", "pt-PT", "it-IT", "nl-NL",
        "pl-PL", "cs-CZ", "sv-SE", "da-DK", "fi-FI", "no-NO", "tr-TR", "ru-RU", "uk-UA", "ar-SA",
        "he-IL", "hi-IN", "id-ID", "th-TH", "vi-VN", "ja-JP", "ko-KR", "zh-CN", "zh-TW",
    ]
    .iter()
    .map(|v| v.to_string())
    .collect::<Vec<_>>();

    let oses = ["windows", "macos", "linux"];
    let mut user_agents = Vec::new();
    for os in oses {
        user_agents.push(UaPreset {
            label: format!("{} · Chrome (latest)", os),
            os: os.to_string(),
            value: build_user_agent(os, None),
        });
    }

    Presets {
        user_agents,
        timezones,
        locales: languages.clone(),
        languages,
        screens: vec![
            ScreenPreset { label: "1920 × 1080 (Full HD)".into(), width: 1920, height: 1080, dpr: 1.0 },
            ScreenPreset { label: "2560 × 1440 (QHD)".into(), width: 2560, height: 1440, dpr: 1.0 },
            ScreenPreset { label: "1366 × 768 (Laptop)".into(), width: 1366, height: 768, dpr: 1.0 },
            ScreenPreset { label: "1536 × 864 (125%)".into(), width: 1536, height: 864, dpr: 1.25 },
            ScreenPreset { label: "1440 × 900 (MacBook Air)".into(), width: 1440, height: 900, dpr: 2.0 },
            ScreenPreset { label: "1512 × 982 (MacBook Pro 14)".into(), width: 1512, height: 982, dpr: 2.0 },
            ScreenPreset { label: "1280 × 800 (Small)".into(), width: 1280, height: 800, dpr: 1.0 },
        ],
        color_schemes: vec!["dark".into(), "light".into(), "system".into()],
        webrtc_modes: vec!["default".into(), "proxy".into(), "disabled".into()],
        browser_version: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accept_language_header_is_well_formed() {
        let mut fp = Fingerprint::default();
        fp.languages = vec!["de-DE".into(), "de".into(), "en".into()];
        assert_eq!(fp.accept_language(), "de-DE,de;q=0.9,en;q=0.8");
    }

    #[test]
    fn profile_defaults_are_launchable() {
        let profile = Profile::new("Demo");
        profile.validate().unwrap();
        assert_eq!(profile.status, ProfileStatus::Active);
        assert!(profile.fingerprint.seed > 0);
    }

    #[test]
    fn rejects_impossible_screen_geometry() {
        let mut profile = Profile::new("Demo");
        profile.fingerprint.screen.width = 2560;
        profile.fingerprint.screen.screen_width = 1920;
        assert!(profile.validate().is_err());
    }

    #[test]
    fn proxy_protocol_parsing_is_forgiving() {
        assert_eq!(ProxyProtocol::parse("SOCKS").unwrap(), ProxyProtocol::Socks5);
        assert_eq!(ProxyProtocol::parse("http").unwrap(), ProxyProtocol::Http);
        assert!(ProxyProtocol::parse("ftp").is_err());
    }
}
