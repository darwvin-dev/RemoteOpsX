# Packaged Desktop E2E

RemoteOpsX has a Linux CI gate that drives the actual Tauri desktop binary rather than only the Vite renderer.

## Test boundary

The E2E build uses the Cargo feature `e2e` and the configuration overlay `src-tauri/tauri.e2e.conf.json`. The overlay gives the test build a separate application identifier and replaces the capability set with the normal main-window permissions plus `wdio-webdriver:default`.

The production `tauri.conf.json` continues to set `withGlobalTauri` to `false`. The richer WDIO execute/mock bridge is intentionally not installed or enabled. `tauri-plugin-wdio-webdriver` is an optional Rust dependency and is registered only under `#[cfg(feature = "e2e")]`, so ordinary debug/release builds do not start a WebDriver listener.

The JavaScript test side deliberately has no WebdriverIO dependency. `e2e/run.mjs` uses Node's built-in `fetch` to speak the W3C WebDriver HTTP protocol directly to the feature-gated embedded server on loopback. This keeps the E2E dependency graph small and keeps `npm audit --audit-level=high` applicable to the complete JavaScript dependency tree without exceptions.

The initial WebdriverIO client experiment was removed after the locked dependency graph produced high-severity audit findings. The reduced graph was regenerated from `package.json` and verified with locked install, strict high-severity audit, the production security source gates, and the normal frontend build before the temporary lock-refresh workflow removed itself.

## CI flow

The `packaged-e2e` job:

1. installs the Linux libraries required to build/run Tauri plus Xvfb and a DBus session;
2. uses isolated XDG config/data/cache directories inside the runner;
3. builds an unbundled debug binary with the `e2e` feature and the E2E config overlay;
4. verifies the production config and optional dependency boundary;
5. starts the real desktop binary under Xvfb/DBus;
6. waits for the loopback W3C WebDriver status endpoint, opens a session, and runs `e2e/run.mjs` against the real window.

## Smoke coverage

The suite proves that the packaged app window boots into the persisted Operations Dashboard, the universal command palette is interactive, Runbook Studio can validate/dry-run through the Rust backend, and settings can be saved and read back through real Tauri IPC and SQLite.

The suite does not store credentials, contact real servers, or execute SSH commands.
