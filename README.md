<p align="center">
  <img src="src-tauri/icons/128x128.png" width="96" height="96" alt="Prism" />
</p>

<h1 align="center">Prism</h1>

<p align="center">
  A lightweight, professional multi-profile browser for Windows 10 and 11.<br />
  Unlimited profiles, genuinely isolated browser state, per-profile proxies and real privacy controls.
</p>

<p align="center">
  <img alt="Platform" src="https://img.shields.io/badge/platform-Windows%2010%20%7C%2011-0b0e14?style=flat-square" />
  <img alt="Rust" src="https://img.shields.io/badge/core-Rust-0b0e14?style=flat-square" />
  <img alt="Tauri" src="https://img.shields.io/badge/shell-Tauri%202-0b0e14?style=flat-square" />
  <img alt="License" src="https://img.shields.io/badge/license-MIT-0b0e14?style=flat-square" />
</p>

---

## What Prism is

Prism is a desktop application that manages **many independent browser identities** on one
machine. Each profile is a real Chromium browser instance launched with its own
`--user-data-dir`, so cookies, local storage, IndexedDB, cache, history and sessions never
leak between profiles — that isolation is enforced by Chromium itself, not by simulation.

On top of isolation, Prism gives each profile its own proxy, timezone, locale, language,
screen geometry, user agent and privacy policy, and manages the whole fleet from one window.

**Prism is not a patched Chromium distribution.** It drives the browser you already have
(Chrome, Edge, Brave or Chromium) and applies settings through documented mechanisms:
command-line switches, a small generated extension, and the Chrome DevTools Protocol. See
[How isolation works](#how-isolation-works) and [Scope and limitations](#scope-and-limitations).

## Features

### Profiles
- **Unlimited profiles** — no artificial cap, no licence tiers.
- **True browser isolation** — one user-data directory per profile; cookies, cache, storage,
  sessions, history and downloads stay inside it.
- **Independent sessions** — log into several accounts on the same platform at the same time.
- Create, edit, duplicate (with or without browser data), archive, delete, search, filter,
  sort and bulk-manage profiles.
- Tags, free-text notes, folders, accent colours and a grid or table view.
- Every profile keeps its data across application restarts, upgrades and crashes.

### Proxies
- HTTP, HTTPS and SOCKS5, with optional username/password.
- Per-profile assignment and one-click reassignment.
- **Real connection tests**: latency, exit IP and country, or a precise error message
  (407, DNS failure, refused connection, TLS failure, timeout).
- Paste or file import in the formats providers actually use, and export of the whole pool.
- **Credentials never reach the browser.** Chromium's `--proxy-server` cannot carry a
  password and its SOCKS5 client cannot authenticate, so Prism starts a loopback relay per
  running profile that performs the authenticated handshake upstream. The browser only ever
  talks to `127.0.0.1`.

### Privacy controls (per profile)
- Timezone, locale and language overrides applied through CDP in every page and every new tab.
- User agent and `navigator.platform`, kept consistent with the selected platform.
- Screen size, window size, pixel ratio and hardware concurrency.
- WebRTC policy: browser default, proxy-only ICE (no non-proxied UDP), or blocked entirely.
- Canvas and WebGL readback noise with a **deterministic per-profile seed**, so a profile
  always reports the same values instead of flickering between page loads.
- Do Not Track header, colour scheme, headless toggle.

### Operations
- Launch several profiles at once, each in its own window.
- Live process table: per-profile memory, CPU, process count and uptime.
- Graceful shutdown through CDP so cookies and storage are flushed before the process exits,
  with a hard `taskkill` fallback when a browser hangs.
- Concurrency limit (unlimited by default) and hardware-acceleration toggle.
- Launch history with exit codes; rows left open by a crash are closed on next start.
- Configuration export/import (JSON), and full backups including browser data, optionally
  sealed with a passphrase (Argon2id + AES-256-GCM, streamed in 64 KiB chunks).

### Interface
- Custom title bar, sidebar navigation, dark surfaces with a single blue accent.
- Animated drawers, dialogs, context menus, toasts and skeleton loading states.
- Responsive down to narrow windows (the sidebar collapses to icons).
- Full keyboard access: `Ctrl+N` for a new profile, `Esc` to dismiss overlays.

> Interface screenshots live in [`docs/screenshots`](docs/screenshots). They are captured from
> a running build so they always reflect the shipped UI; the repository does not ship mockups.

## Requirements

| | |
|---|---|
| OS | Windows 10 or 11, 64-bit |
| Browser | Google Chrome, Microsoft Edge, Brave or Chromium installed |
| Build | Rust stable (MSVC toolchain), Node.js 20+, WebView2 (installed by the installer) |

Prism detects the browser automatically from the standard installation paths, the `PATH`, and
the per-user install locations. A different executable can be selected in **Settings → Browser
engine**.

## Install

Download the latest `Prism_<version>_x64-setup.exe` from
[Releases](https://github.com/itriedcoding/DagLogin/releases), run it, and launch Prism from the
Start menu. The installer is per-user and does not require administrator rights.

## Build from source

```bash
git clone https://github.com/itriedcoding/DagLogin.git
cd DagLogin

# Frontend dependencies
npm --prefix ui install

# Development: Tauri dev server with hot reload
npm --prefix ui run dev          # terminal 1
cargo run -p prism               # terminal 2

# Production build (installer)
npm install -g @tauri-apps/cli@^2
tauri build
```

The installer is written to `src-tauri/target/release/bundle/nsis/`.

Detailed instructions, Architecture Decision Records and troubleshooting: [`docs/BUILD.md`](docs/BUILD.md).

## Using Prism

1. **Create a profile.** It starts with sensible Windows defaults: 1920×1080, `en-US`,
   `America/New_York`, canvas/WebGL noise on, WebRTC limited to the proxy.
2. **Assign a proxy** (optional but recommended when accounts must not share an IP). Add it
   under *Proxies*, press **Test connection**, then select it in the profile editor.
3. **Adjust identity and privacy** in the profile's *Identity* and *Privacy* tabs.
4. **Launch.** The profile opens in its own window. The first launch creates its browser
   directory; subsequent launches reuse it, so logins persist.
5. **Manage the fleet** from *Running* — memory, CPU and uptime per profile, plus *Stop* and
   *Stop all*.
6. **Back up** from *Backup & import*: configuration only, or a full archive with browser data.

## How isolation works

```
profile id ──► %APPDATA%\Prism\profiles\<id>\browser\   ← Chromium --user-data-dir
                        ├─ Cookies, Local Storage, IndexedDB, History, Sessions
                        └─ Cache, Service Worker, Preferences
```

* **State isolation** — each browser process is started with a distinct `--user-data-dir`.
  Chromium refuses to share state between directories; this is the same mechanism Chrome
  profiles use, minus the shared process.
* **Network isolation** — a profile with a proxy gets its own loopback relay process; two
  profiles using two proxies never share a socket path.
* **Identity isolation** — a generated MV3 extension (per profile, per launch) plus a CDP
  session apply language, timezone, locale, UA, screen metrics and privacy policy.
* **Storage isolation** — secrets are encrypted with AES-256-GCM under a master key that is
  wrapped with Windows DPAPI for the current user.

## Scope and limitations

Prism implements browser configuration and separation. It deliberately does **not** include:

* patched browser binaries or modified rendering engines;
* automation-detection evasion, `--disable-blink-features=AutomationControlled`, or similar;
* any mechanism intended to defeat platform security, authentication or anti-abuse systems.

Consequences worth knowing:

* A profile's fingerprint is a *consistent, privacy-hardened Chromium identity*, not a
  bit-for-bit spoof of another machine. Some sites compare dozens of signals; treat profiles as
  clean, separated environments rather than guaranteed anonymity.
* Websites that require a specific patched browser build are out of scope.
* Chromium must be installed separately; Prism does not bundle it.
* Use profiles only for accounts you are entitled to operate, and follow each platform's terms.

## Repository layout

```
crates/prism-core/     Rust engine: storage, crypto, proxies, relay, CDP, launching, backups
src-tauri/             Tauri shell (thin command layer), config, icons, capabilities
ui/                    React + TypeScript interface (Vite)
docs/                  Architecture, build guide, screenshots
.github/workflows/     CI and Windows release pipeline
scripts/gen-icons.mjs  Deterministic icon generator (no external deps)
```

## Testing

```bash
cargo test -p prism-core          # engine: storage, crypto, relay, parsing, backups, CDP commands
npm --prefix ui run typecheck     # TypeScript, strict mode
npm --prefix ui run build         # production frontend bundle
```

The engine's unit tests cover profile CRUD and filtering, secret encryption at rest, atomic
persistence and corruption recovery, launch history, proxy list parsing, the SOCKS5 relay
(including authenticated upstream handshakes), backup round-trips and the exact CDP command
payloads.

## Contributing

Issues and pull requests are welcome. Please run the three commands above before opening a PR.
See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for the module map and the reasoning behind
the relay and supervisor design.

## License

MIT — see [LICENSE](LICENSE).
