//! Prism core engine.
//!
//! Layering:
//!   * `paths`     – where data lives on disk
//!   * `model`     – serialisable domain types shared with the UI
//!   * `db`        – SQLite persistence
//!   * `crypto`    – master key (DPAPI on Windows) + AES-256-GCM secrets
//!   * `browser`   – detection, launch arguments, proxy relay, CDP supervisor
//!   * `proxy`     – proxy testing and bulk import/export
//!   * `backup`    – exports and encrypted backups
//!   * `service`   – the orchestrator that the desktop shell calls

pub mod backup;
pub mod browser;
pub mod crypto;
pub mod db;
pub mod error;
pub mod model;
pub mod paths;
pub mod proxy;
pub mod service;

pub use error::{AppError, Result};

/// Keeps the non-blocking log writer alive for the process lifetime.
struct LogGuard(#[allow(dead_code)] tracing_appender::non_blocking::WorkerGuard);

static LOG_GUARD: std::sync::OnceLock<LogGuard> = std::sync::OnceLock::new();

pub fn init_logging(logs_dir: &std::path::Path, level: &str) {
    let _ = std::fs::create_dir_all(logs_dir);
    let appender = tracing_appender::rolling::daily(logs_dir, "prism.log");
    let (writer, guard) = tracing_appender::non_blocking(appender);
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| {
        format!("prism_core={level},prism={level},tauri=warn,info")
    });
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
        .with_writer(writer)
        .with_ansi(false)
        .with_target(true)
        .try_init();
    // The guard must outlive this call or the writer thread stops.
    let _ = LOG_GUARD.set(LogGuard(guard));
}

#[cfg(test)]
mod hygiene_tests {
    /// The product must never ship stealth tooling: no code may disable the
    /// automation-detection features of the browser.
    #[test]
    fn no_blink_automation_flags_are_used() {
        let source = include_str!("browser/mod.rs");
        assert!(!source.contains("AutomationControlled"));
        assert!(!source.contains("--disable-blink-features"));
    }
}
