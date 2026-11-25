# Architecture

Prism is split into a Rust engine, a thin Tauri shell and a React interface. The engine holds
all behaviour; the shell only forwards commands; the interface never talks to the OS directly
except for window chrome and file dialogs.

```
┌──────────────────────────── ui/ (React + TypeScript) ────────────────────────────┐
│  pages/  components/  state/useAppData.ts  lib/api.ts (typed IPC surface)        │
└───────────────────────────────────┬──────────────────────────────────────────────┘
                                    │ invoke("<command>", args)
┌───────────────────────────────────▼──────────────────────────────────────────────┐
│ src-tauri/src/lib.rs — command layer: decode args, call Service, map errors       │
│ emits engine://event (profile started/stopped) to the webview                    │
└───────────────────────────────────┬──────────────────────────────────────────────┘
                                    │ Arc<Service>
┌───────────────────────────────────▼──────────────────────────────────────────────┐
│ crates/prism-core                                                                │
│  service.rs   orchestration, running registry, reaper tasks, events              │
│  browser/     detection · launch args · relay · CDP supervisor · extension       │
│  db.rs        atomic JSON document store                                         │
│  crypto.rs    DPAPI-wrapped master key + AES-256-GCM envelope                    │
│  backup.rs    config export, streaming encrypted ZIP backups                     │
│  proxy.rs     connectivity tests, bulk parsing/export                            │
│  model.rs     serde types shared with the UI                                     │
└──────────────────────────────────────────────────────────────────────────────────┘
```

## Why a JSON document store instead of SQLite

The workspace is a single user's data: hundreds of profiles at most, one writer, one process.
A single JSON document written with `temp file → fsync → rename` gives crash-atomic writes,
human-inspectable backups, zero native dependencies and a build that does not need a C
toolchain. Filtering and sorting happen in memory over a cloned vector, which is faster than a
SQL round-trip at this size. History is capped at 2000 rows so the document cannot grow without
bound. A document that fails to parse is copied aside as `prism.corrupt-<timestamp>.json`
before an empty workspace is started, so nothing is silently discarded.

## Why a loopback proxy relay

Chromium's `--proxy-server` switch accepts a scheme, host and port — not credentials — and its
SOCKS5 client implements no authentication at all. Real multi-account browsers therefore put a
local hop in front of the upstream proxy. Prism does the same:

```
Chrome ──SOCKS5 (no auth)──► 127.0.0.1:<ephemeral> ──authenticated handshake──► provider
```

The relay is started per launch, bound to `127.0.0.1` on an ephemeral port, and dropped when
the profile stops. It supports three upstream kinds:

| Upstream | Handshake performed by the relay |
|---|---|
| HTTP | `CONNECT host:port` with `Proxy-Authorization: Basic …`, then tunnel |
| HTTPS | TLS to the proxy (platform trust store), then the same `CONNECT` |
| SOCKS5 | Greeting, optional RFC 1929 username/password, then `CONNECT` |

Benefits: credentials never enter the browser process or its profile directory, each profile
gets an independent network path, and failures surface as precise errors ("the SOCKS5 proxy
rejected the supplied username and password") instead of a blank page.

## Fingerprint surface: what is real and where it comes from

| Signal | Mechanism |
|---|---|
| Language, `Accept-Language` | `--lang`, `--accept-lang` |
| User agent (initial requests) | `--user-agent` |
| Timezone, locale | CDP `Emulation.setTimezoneOverride`, `setLocaleOverride` |
| Screen, window, DPR, mobile | `--window-size`, `--force-device-scale-factor`, CDP `setDeviceMetricsOverride` |
| Hardware concurrency | CDP `setHardwareConcurrencyOverride` + extension fallback |
| User agent (per page), platform | CDP `setUserAgentOverride` |
| Colour scheme | CDP `setEmulatedMedia` |
| Canvas / WebGL readback | Generated MV3 content script in the page's MAIN world, seeded PRNG |
| `navigator.deviceMemory` | Generated MV3 content script |
| WebRTC | `chrome.privacy.network.webRTCIPHandlingPolicy` (documented Chrome API) |
| Cookies, storage, history | Chromium `--user-data-dir` isolation |

The supervisor attaches with `Target.setAutoAttach` (`flatten`, `waitForDebuggerOnStart`) so a
tab opened *after* launch is paused, configured, and only then resumed. The first tab is
covered by enumerating existing targets with `Target.getTargets` + `Target.attachToTarget`.

## Process lifecycle

1. `Service::launch` validates the profile, resolves the browser, starts the relay (if any),
   writes the per-profile extension, and spawns Chromium with `CREATE_NO_WINDOW |
   CREATE_NEW_PROCESS_GROUP`.
2. It polls `http://127.0.0.1:<port>/json/version` while watching `child.try_wait()`. A browser
   that exits immediately produces an actionable error rather than a silent no-op.
3. Once the debug endpoint answers, a supervisor task starts and a reaper task owns the child.
4. On exit the reaper removes the registry entry, closes the history row and emits
   `ProfileStopped`. Dropping the entry drops the relay and cancels the supervisor.
5. `Service::stop` first asks Chromium to close through CDP (`Browser.close`), waits up to 8 s
   for a clean exit, and only then falls back to `taskkill /T /F` — so sessions are flushed.

## Security model

* Master key: 32 random bytes, wrapped with DPAPI (`CryptProtectData`) on Windows, stored in
  `<data>/vault.key`. Another Windows user on the same machine cannot unwrap it.
* Secrets: AES-256-GCM, random 96-bit nonce per value, stored as `v1:<base64(nonce||ct||tag)>`.
  Empty secrets are stored as empty strings.
* Backups: optional passphrase, Argon2id (64 MiB, t=3) over a 16-byte salt, then chunked
  AES-256-GCM with a per-file nonce prefix and a 32-bit chunk counter.
* Restores use `enclosed_name()` to reject archive entries that would escape the target
  directory (zip-slip).
* Deletions remove the profile directory from disk, not just the record.

## Failure handling

| Failure | Behaviour |
|---|---|
| Browser missing | Launch fails with instructions; Settings lists detected candidates |
| Browser exits during start-up | Error includes the exit code and the tail of `browser.log` |
| Debug endpoint unavailable | Profile still launches; a warning explains which overrides were skipped |
| Proxy unreachable | Relay fails, warning recorded, launch continues on a direct connection |
| App killed while profiles run | Next start closes dangling history rows with an explicit reason |
| Data document corrupted | Copied aside as `prism.corrupt-*.json`; empty workspace starts |
| Backup passphrase wrong | Clear error from the AEAD tag check; no partial restore |

## Extension points

* Add a browser: extend `browser::candidates()`.
* Add a fingerprint signal: add a field to `Fingerprint`, apply it in
  `supervisor::Session::overrides_for` or `extension::guard_script`, and expose it in the UI.
* Add a storage backend: implement the method surface of `db::Database`.
