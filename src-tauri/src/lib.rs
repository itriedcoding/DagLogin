//! Tauri shell.
//!
//! This crate is deliberately thin: every command forwards to
//! [`prism_core::service::Service`]. That keeps the engine testable without a
//! webview and keeps the IPC surface small and explicit.

use prism_core::model::{
    AppInfo, BackupReport, BrowserInfo, HistoryEntry, ImportReport, LaunchOutcome, Presets, Profile,
    ProfileQuery, ProfileView, Proxy, ProxyInput, ProxyTest, RunningInstance, Settings,
};
use prism_core::service::{EngineEvent, Service};
use std::sync::Arc;
use tauri::{Emitter, Manager, State};
use tracing::warn;

struct AppState {
    service: Arc<Service>,
}

type CommandResult<T> = Result<T, String>;

fn to_msg(error: prism_core::AppError) -> String {
    error.to_string()
}

/* ------------------------------------------------------------------ app info */

#[tauri::command]
fn app_info(state: State<'_, AppState>) -> CommandResult<AppInfo> {
    state.service.app_info().map_err(to_msg)
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> CommandResult<Settings> {
    state.service.settings().map_err(to_msg)
}

#[tauri::command]
fn save_settings(state: State<'_, AppState>, settings: Settings) -> CommandResult<Settings> {
    state.service.save_settings(settings).map_err(to_msg)
}

#[tauri::command]
fn get_presets(state: State<'_, AppState>) -> CommandResult<Presets> {
    state.service.presets().map_err(to_msg)
}

#[tauri::command]
fn browser_info(state: State<'_, AppState>) -> CommandResult<BrowserInfo> {
    state.service.browser_info().map_err(to_msg)
}

#[tauri::command]
fn detect_browser(state: State<'_, AppState>, path: Option<String>) -> CommandResult<BrowserInfo> {
    state.service.detect_browser(path).map_err(to_msg)
}

/* ------------------------------------------------------------------ profiles */

#[tauri::command]
fn list_profiles(state: State<'_, AppState>, query: ProfileQuery) -> CommandResult<Vec<ProfileView>> {
    state.service.list_profiles(query).map_err(to_msg)
}

#[tauri::command]
fn get_profile(state: State<'_, AppState>, id: String) -> CommandResult<Profile> {
    state.service.get_profile(&id).map_err(to_msg)
}

#[tauri::command]
fn create_profile(state: State<'_, AppState>, name: Option<String>) -> CommandResult<Profile> {
    state.service.create_profile(name).map_err(to_msg)
}

#[tauri::command]
fn update_profile(state: State<'_, AppState>, profile: Profile) -> CommandResult<Profile> {
    state.service.update_profile(profile).map_err(to_msg)
}

#[tauri::command]
fn duplicate_profile(
    state: State<'_, AppState>,
    id: String,
    with_data: bool,
) -> CommandResult<Profile> {
    state.service.duplicate_profile(&id, with_data).map_err(to_msg)
}

#[tauri::command]
async fn delete_profile(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    state.service.delete_profile(&id).await.map_err(to_msg)
}

#[tauri::command]
fn set_archived(state: State<'_, AppState>, id: String, archived: bool) -> CommandResult<Profile> {
    state.service.set_archived(&id, archived).map_err(to_msg)
}

#[tauri::command]
async fn bulk_action(
    state: State<'_, AppState>,
    ids: Vec<String>,
    action: String,
) -> CommandResult<usize> {
    state.service.bulk_action(ids, action).await.map_err(to_msg)
}

#[tauri::command]
fn profile_data_dir(state: State<'_, AppState>, id: String) -> CommandResult<String> {
    state.service.profile_data_dir(&id).map_err(to_msg)
}

/* ------------------------------------------------------------------- runtime */

#[tauri::command]
async fn launch_profile(state: State<'_, AppState>, id: String) -> CommandResult<LaunchOutcome> {
    state.service.launch(&id).await.map_err(to_msg)
}

#[tauri::command]
async fn stop_profile(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    state.service.stop(&id).await.map_err(to_msg)
}

#[tauri::command]
async fn stop_all_profiles(state: State<'_, AppState>) -> CommandResult<usize> {
    state.service.stop_all().await.map_err(to_msg)
}

#[tauri::command]
fn list_running(state: State<'_, AppState>) -> CommandResult<Vec<RunningInstance>> {
    state.service.running().map_err(to_msg)
}

#[tauri::command]
fn list_history(
    state: State<'_, AppState>,
    profile_id: Option<String>,
    limit: Option<u32>,
) -> CommandResult<Vec<HistoryEntry>> {
    state.service.history(profile_id, limit).map_err(to_msg)
}

#[tauri::command]
fn clear_history(state: State<'_, AppState>) -> CommandResult<()> {
    state.service.clear_history().map_err(to_msg)
}

/* ------------------------------------------------------------------- proxies */

#[tauri::command]
fn list_proxies(state: State<'_, AppState>) -> CommandResult<Vec<Proxy>> {
    state.service.proxies().map_err(to_msg)
}

#[tauri::command]
fn save_proxy(state: State<'_, AppState>, input: ProxyInput) -> CommandResult<Proxy> {
    state.service.save_proxy(input).map_err(to_msg)
}

#[tauri::command]
fn delete_proxy(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    state.service.delete_proxy(&id).map_err(to_msg)
}

#[tauri::command]
async fn test_proxy(state: State<'_, AppState>, id: String) -> CommandResult<ProxyTest> {
    state.service.test_proxy(&id).await.map_err(to_msg)
}

#[tauri::command]
async fn test_proxy_draft(state: State<'_, AppState>, input: ProxyInput) -> CommandResult<ProxyTest> {
    state.service.test_proxy_draft(input).await.map_err(to_msg)
}

#[tauri::command]
async fn test_all_proxies(state: State<'_, AppState>) -> CommandResult<Vec<Proxy>> {
    state.service.test_all_proxies().await.map_err(to_msg)
}

#[tauri::command]
fn import_proxies(state: State<'_, AppState>, text: String) -> CommandResult<usize> {
    state.service.import_proxies(&text).map_err(to_msg)
}

#[tauri::command]
fn export_proxies(state: State<'_, AppState>) -> CommandResult<String> {
    state.service.export_proxies().map_err(to_msg)
}

/* ---------------------------------------------------------------- data files */

#[tauri::command]
fn export_config(
    state: State<'_, AppState>,
    dest: String,
    ids: Vec<String>,
    passphrase: Option<String>,
) -> CommandResult<BackupReport> {
    state
        .service
        .export_config(dest, ids, passphrase)
        .map_err(to_msg)
}

#[tauri::command]
fn import_config(
    state: State<'_, AppState>,
    source: String,
    passphrase: Option<String>,
) -> CommandResult<ImportReport> {
    state.service.import_config(source, passphrase).map_err(to_msg)
}

#[tauri::command]
fn create_backup(
    state: State<'_, AppState>,
    dest: String,
    passphrase: Option<String>,
) -> CommandResult<BackupReport> {
    state.service.create_backup(dest, passphrase).map_err(to_msg)
}

#[tauri::command]
fn restore_backup(
    state: State<'_, AppState>,
    source: String,
    passphrase: Option<String>,
) -> CommandResult<ImportReport> {
    state.service.restore_backup(source, passphrase).map_err(to_msg)
}

#[tauri::command]
fn exports_dir(state: State<'_, AppState>) -> CommandResult<String> {
    Ok(state.service.exports_dir())
}

#[tauri::command]
fn data_dir(state: State<'_, AppState>) -> CommandResult<String> {
    Ok(state.service.data_dir())
}

#[tauri::command]
fn read_text_file(path: String) -> CommandResult<String> {
    std::fs::read_to_string(&path).map_err(|e| format!("could not read {path}: {e}"))
}

#[tauri::command]
fn save_text_file(path: String, contents: String) -> CommandResult<()> {
    if let Some(parent) = std::path::Path::new(&path).parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, contents).map_err(|e| format!("could not write {path}: {e}"))
}

#[tauri::command]
fn open_path(path: String, reveal: bool) -> CommandResult<()> {
    let result = if reveal {
        tauri_plugin_opener::reveal_item_in_dir(path.clone())
    } else {
        tauri_plugin_opener::open_path(path.clone(), None::<&str>)
    };
    result.map_err(|e| format!("could not open {path}: {e}"))
}

/* --------------------------------------------------------------------- setup */

pub fn run() {
    let service = match Service::bootstrap() {
        Ok(service) => Arc::new(service),
        Err(error) => {
            let message = format!("Prism could not start: {error}");
            let log = std::env::temp_dir().join("prism-startup-error.log");
            let _ = std::fs::write(&log, &message);
            eprintln!("{message}");
            panic!("{message}");
        }
    };

    let state = AppState {
        service: Arc::clone(&service),
    };

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .setup({
            let service = Arc::clone(&service);
            move |app| {
                let handle = app.handle().clone();
                let mut events = service.subscribe();
                tauri::async_runtime::spawn(async move {
                    loop {
                        match events.recv().await {
                            Ok(event) => {
                                if let Err(error) = handle.emit("engine://event", &event) {
                                    warn!(%error, "could not forward an engine event");
                                }
                                if matches!(event, EngineEvent::ProfileStopped { .. }) {
                                    // The engine already closed the history row and
                                    // freed the registry slot; nothing else to do.
                                    tracing::debug!("profile stopped");
                                }
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(missed)) => {
                                warn!(missed, "dropped engine events");
                            }
                            Err(_) => break,
                        }
                    }
                });
                Ok(())
            }
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                let app = window.app_handle();
                if let Some(state) = app.try_state::<AppState>() {
                    let close = state
                        .service
                        .settings()
                        .map(|settings| settings.close_browsers_on_exit)
                        .unwrap_or(true);
                    if close {
                        state.service.shutdown();
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            get_settings,
            save_settings,
            get_presets,
            browser_info,
            detect_browser,
            list_profiles,
            get_profile,
            create_profile,
            update_profile,
            duplicate_profile,
            delete_profile,
            set_archived,
            bulk_action,
            profile_data_dir,
            launch_profile,
            stop_profile,
            stop_all_profiles,
            list_running,
            list_history,
            clear_history,
            list_proxies,
            save_proxy,
            delete_proxy,
            test_proxy,
            test_proxy_draft,
            test_all_proxies,
            import_proxies,
            export_proxies,
            export_config,
            import_config,
            create_backup,
            restore_backup,
            exports_dir,
            data_dir,
            read_text_file,
            save_text_file,
            open_path
        ]);

    if let Err(error) = builder.run(tauri::generate_context!()) {
        eprintln!("Prism failed to run: {error}");
        std::process::exit(1);
    }
}
