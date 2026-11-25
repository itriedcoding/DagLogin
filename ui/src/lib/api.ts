import { invoke } from "@tauri-apps/api/core";
import type {
  AppInfo,
  BackupReport,
  BrowserInfo,
  HistoryEntry,
  ImportReport,
  LaunchOutcome,
  Presets,
  Profile,
  ProfileQuery,
  ProfileView,
  Proxy,
  ProxyInput,
  ProxyTest,
  RunningInstance,
  Settings,
} from "./types";

/** Errors from Rust arrive as strings; normalise them for the UI. */
export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  if (error && typeof error === "object" && "message" in error) {
    return String((error as { message: unknown }).message);
  }
  return "Something went wrong. Check the log for details.";
}

export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),
  settings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<Settings>("save_settings", { settings }),
  presets: () => invoke<Presets>("get_presets"),
  browserInfo: () => invoke<BrowserInfo>("browser_info"),
  detectBrowser: (path?: string) => invoke<BrowserInfo>("detect_browser", { path: path ?? null }),

  listProfiles: (query: ProfileQuery) => invoke<ProfileView[]>("list_profiles", { query }),
  getProfile: (id: string) => invoke<Profile>("get_profile", { id }),
  createProfile: (name?: string) => invoke<Profile>("create_profile", { name: name ?? null }),
  updateProfile: (profile: Profile) => invoke<Profile>("update_profile", { profile }),
  duplicateProfile: (id: string, withData: boolean) =>
    invoke<Profile>("duplicate_profile", { id, withData }),
  deleteProfile: (id: string) => invoke<void>("delete_profile", { id }),
  setArchived: (id: string, archived: boolean) =>
    invoke<Profile>("set_archived", { id, archived }),
  bulkAction: (ids: string[], action: string) =>
    invoke<number>("bulk_action", { ids, action }),

  launch: (id: string) => invoke<LaunchOutcome>("launch_profile", { id }),
  stop: (id: string) => invoke<void>("stop_profile", { id }),
  stopAll: () => invoke<number>("stop_all_profiles"),
  running: () => invoke<RunningInstance[]>("list_running"),
  profileDataDir: (id: string) => invoke<string>("profile_data_dir", { id }),

  proxies: () => invoke<Proxy[]>("list_proxies"),
  saveProxy: (input: ProxyInput) => invoke<Proxy>("save_proxy", { input }),
  deleteProxy: (id: string) => invoke<void>("delete_proxy", { id }),
  testProxy: (id: string) => invoke<ProxyTest>("test_proxy", { id }),
  testProxyDraft: (input: ProxyInput) => invoke<ProxyTest>("test_proxy_draft", { input }),
  testAllProxies: () => invoke<Proxy[]>("test_all_proxies"),
  importProxies: (text: string) => invoke<number>("import_proxies", { text }),
  exportProxies: () => invoke<string>("export_proxies"),

  exportConfig: (dest: string, ids: string[], passphrase: string | null) =>
    invoke<BackupReport>("export_config", { dest, ids, passphrase }),
  importConfig: (source: string, passphrase: string | null) =>
    invoke<ImportReport>("import_config", { source, passphrase }),
  createBackup: (dest: string, passphrase: string | null) =>
    invoke<BackupReport>("create_backup", { dest, passphrase }),
  restoreBackup: (source: string, passphrase: string | null) =>
    invoke<ImportReport>("restore_backup", { source, passphrase }),

  history: (profileId: string | null, limit = 100) =>
    invoke<HistoryEntry[]>("list_history", { profileId, limit }),
  clearHistory: () => invoke<void>("clear_history"),
  exportsDir: () => invoke<string>("exports_dir"),
  dataDir: () => invoke<string>("data_dir"),

  readTextFile: (path: string) => invoke<string>("read_text_file", { path }),
  saveTextFile: (path: string, contents: string) =>
    invoke<void>("save_text_file", { path, contents }),

  openPath: (path: string, reveal: boolean) =>
    invoke<void>("open_path", { path, reveal }),
};
