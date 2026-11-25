// Mirrors crates/prism-core/src/model.rs (serde camelCase).

export type ProxyProtocol = "http" | "https" | "socks5";
export type ProfileStatus = "active" | "archived";
export type StatusIndicator = "running" | "archived" | "idle";

export interface ProxyTest {
  ok: boolean;
  latencyMs: number | null;
  exitIp: string | null;
  country: string | null;
  error: string | null;
  testedAt: string;
}

export interface Proxy {
  id: string;
  label: string;
  protocol: ProxyProtocol;
  host: string;
  port: number;
  username: string;
  password: string;
  createdAt: string;
  updatedAt: string;
  lastTest: ProxyTest | null;
}

export interface ProxyInput {
  id?: string | null;
  label: string;
  protocol: ProxyProtocol;
  host: string;
  port: number;
  username: string;
  password: string;
}

export interface Screen {
  width: number;
  height: number;
  screenWidth: number;
  screenHeight: number;
  deviceScaleFactor: number;
  mobile: boolean;
}

export interface Fingerprint {
  os: string;
  userAgent: string;
  platform: string;
  languages: string[];
  timezone: string;
  locale: string;
  screen: Screen;
  hardwareConcurrency: number;
  deviceMemoryGb: number;
  colorScheme: string;
  webrtc: string;
  canvasNoise: boolean;
  webglNoise: boolean;
  doNotTrack: boolean;
  seed: number;
  headless: boolean;
}

export interface Profile {
  id: string;
  name: string;
  notes: string;
  tags: string[];
  folder: string;
  status: ProfileStatus;
  color: string;
  proxyId: string | null;
  startUrls: string[];
  fingerprint: Fingerprint;
  createdAt: string;
  updatedAt: string;
  lastLaunchedAt: string | null;
  launchCount: number;
}

export interface ProfileView extends Profile {
  proxyLabel: string | null;
  proxyProtocol: string | null;
  statusIndicator: StatusIndicator;
  storageBytes: number;
}

export interface ProfileQuery {
  search: string;
  status: string;
  tag: string;
  proxyId: string;
  folder: string;
  sort: string;
}

export interface Settings {
  browserPath: string;
  maxConcurrent: number;
  confirmOnDelete: boolean;
  defaultStartUrls: string[];
  reduceAnimations: boolean;
  cdpOverrides: boolean;
  hardwareAcceleration: boolean;
  closeBrowsersOnExit: boolean;
  proxyTestUrl: string;
  logLevel: string;
}

export interface BrowserInfo {
  path: string | null;
  name: string;
  version: string | null;
  detected: boolean;
  candidates: string[];
}

export interface AppInfo {
  version: string;
  dataDir: string;
  dbPath: string;
  logsDir: string;
  os: string;
  browser: BrowserInfo;
  profileCount: number;
  proxyCount: number;
  storageBytes: number;
}

export interface RunningInstance {
  profileId: string;
  profileName: string;
  pid: number;
  processCount: number;
  cdpPort: number;
  relayPort: number | null;
  startedAt: string;
  uptimeSecs: number;
  memoryBytes: number;
  cpuPercent: number;
  browserDir: string;
  proxyLabel: string | null;
}

export interface LaunchOutcome {
  profileId: string;
  pid: number;
  cdpPort: number;
  relayPort: number | null;
  startedAt: string;
  warnings: string[];
}

export interface HistoryEntry {
  id: number;
  profileId: string;
  profileName: string;
  startedAt: string;
  endedAt: string | null;
  exitCode: number | null;
  error: string | null;
}

export interface UaPreset {
  label: string;
  os: string;
  value: string;
}

export interface ScreenPreset {
  label: string;
  width: number;
  height: number;
  dpr: number;
}

export interface Presets {
  userAgents: UaPreset[];
  timezones: string[];
  languages: string[];
  locales: string[];
  screens: ScreenPreset[];
  colorSchemes: string[];
  webrtcModes: string[];
  browserVersion: string | null;
}

export interface ImportReport {
  imported: number;
  skipped: number;
  errors: string[];
}

export interface BackupReport {
  path: string;
  profiles: number;
  proxies: number;
  bytes: number;
  encrypted: boolean;
}

export type EngineEvent =
  | { type: "profileStarted"; profileId: string; name: string; pid: number }
  | { type: "profileStopped"; profileId: string; name: string; exitCode: number | null; error: string | null }
  | { type: "notice"; level: string; message: string };
