# Screenshots

Images in this folder are captured from a running build of Prism, never mocked up. To refresh
them after a UI change:

1. Build and start the app: `npm --prefix ui run build && cargo run -p prism`.
2. Capture each screen at 1380 × 880 (the default window size):
   - `profiles.png` — the profile dashboard with the grid view and at least three profiles;
   - `editor-identity.png` — the profile editor on the *Identity* tab;
   - `proxies.png` — the proxy pool with one successful and one failed test;
   - `running.png` — two running profiles with live memory and CPU;
   - `backup.png` — the backup and import page;
   - `settings.png` — the settings page.
3. Save them here as PNG and reference them from the README.

The folder is intentionally shipped without pre-rendered images: fabricating screenshots of
features that were never rendered is worse than not having them.
