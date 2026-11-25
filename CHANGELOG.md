# Changelog

All notable changes to Prism are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] — 2026-11-25

First public release.

### Added

- **Profiles** — unlimited, with per-profile Chromium `--user-data-dir` isolation; create,
  edit, duplicate (optionally with browser data), archive, delete, search, filter, sort and
  bulk actions. Tags, notes, folders and accent colours.
- **Launching** — many profiles at once, each in its own window, with graceful CDP shutdown and
  a process-tree kill fallback. Launch history with exit codes and crash recovery.
- **Proxy manager** — HTTP, HTTPS and SOCKS5 with optional credentials, per-profile assignment,
  real connectivity tests (latency, exit IP, country, precise errors), bulk paste/file import
  and export.
- **Authenticating loopback relay** — solves Chromium's inability to send proxy credentials or
  authenticate to SOCKS5 upstreams; one relay per running profile, credentials never reach the
  browser.
- **Fingerprint controls** — timezone, locale, languages, user agent, platform, screen and
  window geometry, pixel ratio, hardware concurrency, device memory, colour scheme, Do Not
  Track, WebRTC policy, and deterministic canvas/WebGL readback noise.
- **CDP supervisor** — applies per-profile overrides to existing tabs and to every tab opened
  later, using official Chrome DevTools Protocol domains.
- **Backup and migration** — JSON configuration export/import, full ZIP backups including
  browser state, optional passphrase encryption (Argon2id + streaming AES-256-GCM).
- **Security** — AES-256-GCM secret storage with a DPAPI-wrapped master key; atomic data
  writes; corruption detection with automatic side-copy.
- **Interface** — custom title bar, sidebar navigation, profile cards and table views, drawer
  editor, context menus, toasts, skeleton loading and responsive layout.
- **Packaging** — NSIS per-user installer, GitHub Actions release pipeline for Windows x64.

### Known limitations

- Requires an installed Chromium-family browser (Chrome, Edge, Brave or Chromium); Prism does
  not bundle one.
- Fingerprint controls produce a consistent, privacy-hardened Chromium identity; they are not a
  patched browser engine and do not claim to defeat platform anti-abuse systems.
- Restoring a full backup requires every profile to be closed.

[1.0.0]: https://github.com/itriedcoding/DagLogin/releases/tag/v1.0.0
