# RemoteOpsX MVP Finish Handoff Spec

## Goal

Finalize the current RemoteOpsX MVP as a buildable, documented Linux desktop application with typed settings persistence, stable IPC errors, regression coverage, refreshed roadmap docs, and a current Graphify map.

## Completed scope

- Preserve the existing React/Tauri architecture and system-tool adapters.
- Add a stable backend `DomainError` contract with safe internal-error redaction and validation context.
- Add persisted application settings backed by a singleton SQLite row.
- Add frontend settings types, client-side validation, normalized remote errors, and rollback-safe Zustand state.
- Add a Settings modal reachable from the top bar and command palette.
- Apply dark/light/system theme from settings and use the persisted health refresh interval in the health panel.
- Keep secrets in the OS keyring; SQLite stores only metadata and `secret_ref` references.
- Keep the hardening roadmap explicit for native SSH, host-key UI, app lock, embedded desktop protocols, alerting, CI, and integration testing.

## Settings contract

The settings schema version is `1`. Defaults are: system theme, SSH `22`, FTP `21`, RDP `3389`, VNC `5900`, health refresh `3000 ms`, history retention `90 days`, app-lock timeout `15 minutes`, transfer conflict policy `ask`, and enabled desktop clipboard/audio/notifications.

Rust validation rejects unsupported schema versions, zero ports, refresh outside `1000..=60000 ms`, retention outside `1..=3650 days`, and app-lock timeout outside `1..=1440 minutes`. Frontend validation mirrors these constraints before IPC.

## Acceptance

Required checks for this handoff:

- `npm test`
- `npm run build`
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
- `cargo test --manifest-path src-tauri/Cargo.toml`
- `git diff --check`
- `graphify update .`
- `graphify cluster-only . --no-label`

Manual desktop smoke to run on a workstation with Tauri runtime dependencies:

1. Start `npm run app:dev`.
2. Open Settings from the top bar and command palette.
3. Change theme, default ports, and health refresh; save and restart.
4. Confirm persisted values reload from `remoteopsx.db`.
5. Submit an invalid numeric setting and confirm the UI keeps the previous persisted state.

## Non-goals

- Native `russh` transport.
- Known-hosts management UI.
- Real app-lock encryption/unlock flow.
- Embedded RDP/VNC rendering.
- Live SSH integration fixtures.
- Signed release automation.
