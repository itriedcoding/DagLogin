# Building Prism

## Prerequisites

| Tool | Version | Notes |
|---|---|---|
| Rust | stable, `x86_64-pc-windows-msvc` | <https://rustup.rs> |
| Visual Studio Build Tools | 2019/2022, "Desktop development with C++" | Supplies `link.exe` and the Windows SDK |
| Node.js | 20 or newer | Builds the interface |
| WebView2 runtime | Evergreen | Preinstalled on Windows 11; the installer bootstraps it on Windows 10 |

> The repository itself has **no C dependencies** — the engine uses a pure-Rust store and
> SChannel TLS — so `cargo check`/`cargo test` work with the GNU toolchain too. Only the final
> `.exe` link and installer packaging want the MSVC toolchain and the Windows SDK.

## Quick start

```bash
npm --prefix ui install          # frontend dependencies
cargo test -p prism-core         # engine tests
npm --prefix ui run typecheck    # strict TypeScript
npm --prefix ui run build        # produces ui/dist

# Development (two terminals)
npm --prefix ui run dev          # Vite on 127.0.0.1:1420
cargo run -p prism               # Tauri window; beforeDevCommand is already wired

# Installer
npm install -g @tauri-apps/cli@^2
tauri build --bundles nsis
```

Output: `src-tauri/target/release/bundle/nsis/Prism_1.0.0_x64-setup.exe`.

## Release pipeline

`.github/workflows/release.yml` runs on a `v*` tag (or manually) on `windows-latest`:

1. checkout, Node 20, Rust stable (MSVC) with the `x86_64-pc-windows-msvc` target;
2. `npm ci` in `ui/` (falls back to `npm install` when no lockfile is present);
3. `npm run build` in `ui/`;
4. `cargo test -p prism-core`;
5. `tauri build --bundles nsis`;
6. upload the installer to the GitHub Release, with generated release notes.

To cut a release:

```bash
# keep the three version fields in sync
#   Cargo.toml (workspace.package.version)
#   src-tauri/Cargo.toml / src-tauri/tauri.conf.json
#   ui/package.json
git tag v1.0.0
git push origin v1.0.0
```

Artifacts are attached automatically. The workflow also accepts `workflow_dispatch` for a
dry-run build without publishing.

## Configuration reference

`src-tauri/tauri.conf.json`

| Key | Value | Why |
|---|---|---|
| `app.windows[0].decorations` | `false` | Custom title bar in the UI |
| `app.windows[0].minWidth/minHeight` | 1040 × 660 | Below this the layout collapses to icons |
| `bundle.targets` | `["nsis"]` | Single `.exe` installer, per-user, no admin prompt |
| `bundle.windows.webviewInstallMode` | `downloadBootstrapper` | Installs WebView2 when missing |
| `app.security.csp` | explicit `default-src 'self'` | No remote code, no inline scripts beyond IPC |

Runtime data lives in `%APPDATA%\Prism` (override with `PRISM_DATA_DIR`):

```
prism.json          profiles, proxies, settings, history (atomic writes)
vault.key           DPAPI-wrapped master key
profiles/<id>/browser/     Chromium user-data-dir (cookies, storage, cache)
profiles/<id>/extension/   generated MV3 privacy extension
profiles/<id>/browser.log  Chromium stderr for the last launch
logs/prism.log      rolling daily application log
exports/            default destination for exports
```

## Troubleshooting

**"No Chromium-based browser was found"** — install Chrome or Edge, or pick the executable in
Settings. The list under *Settings → Browser engine* shows every candidate Prism found.

**A profile exits immediately** — open the profile's *Advanced* tab → *Open folder* and read
`browser.log`. A stale `SingletonLock` left by a killed browser is recovered automatically by
Chromium; a read-only folder or an antivirus lock is not.

**Proxy test says 407** — the proxy wants credentials; fill in username and password.

**Proxy test says "connection refused"** — wrong port, or the provider requires IP allow-listing.

**Installer build fails with `link.exe not found`** — install Visual Studio Build Tools with the
C++ workload, then restart the shell so `PATH` picks up the MSVC environment.

**A backup restore is refused** — every profile must be closed first. The restore writes whole
browser directories and cannot merge into a live one.

## Verifying a build

```bash
cargo test -p prism-core
npm --prefix ui run typecheck
npm --prefix ui run build
```

The test suite is the contract for the engine: storage round-trips, encryption at rest,
corruption recovery, proxy parsing, the authenticated SOCKS5 relay, backup/restore and the
exact CDP payloads. If those pass, the remaining risk is UI wiring and Windows packaging.
