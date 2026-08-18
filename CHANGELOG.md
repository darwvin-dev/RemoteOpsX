# Changelog

All notable changes to RemoteOpsX are documented here.

The project follows Semantic Versioning. Pre-release progression for a release line is `alpha` → `beta` → `rc` → stable. Pull requests use Conventional Commit-style titles so GitHub-generated release notes remain useful after merges.

## [Unreleased]

### Security
- SSH first contact now uses an app-managed `known_hosts` store with SHA-256 fingerprint preview and explicit Trust / Replace / Remove actions; terminal, one-shot exec, SCP, and tunnels all require strict host-key verification.
- SSH trust is isolated from system-wide known-host files, configured known-host helpers, DNS SSHFP auto-trust, automatic host-key updates, and localhost host-auth bypasses so the app review flow is the authoritative trust source.
- Saved host syntax is restricted to safe DNS/IP forms before it can be represented in `known_hosts`, preventing wildcard/list/marker injection through profile values.
- One-shot remote commands are delivered through SSH stdin so command text is not exposed in the local process argv.
- Runtime preflight checks core OpenSSH tooling plus profile-specific password, FTP, RDP, VNC, and keyring readiness.
- Known keyring secrets are registered in a central redaction layer and masked from buffered SSH output, IPC errors, every textual runbook result field, backend diagnostics, and exported text; profile metadata, user runbooks, snippets, and persisted tunnel endpoint text reject known stored credentials before SQLite persistence.
- Existing saved credentials are preloaded from keyring references during startup on a best-effort basis, so redaction and persistence guards are active before the first connection attempt; historical user-controlled SQLite text is also scrubbed from logical database values when it contains a known secret.
- CI and tagged-release preflight contain source-level regression gates that reject weakened SSH/RDP trust and password-in-argv patterns.

### Reliability
- Startup recovery marks stale sessions and runbook executions interrupted and stale persisted tunnel state stopped after an unclean restart.
- SSH terminal exit closes the persisted session immediately; PTY and tunnel child processes are killed and reaped deterministically instead of lingering until process exit.

### Changed
- Release tooling treats `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json` as one versioned product.
- Node and Rust toolchains are pinned for CI and tagged release builds so toolchain drift is explicit and reviewable.

## [0.2.0-alpha.1] - 2026-08-18

### Security
- FreeRDP uses certificate trust-on-first-use instead of unconditional certificate bypass.
- Stored RDP passwords are sent through stdin instead of process argv.
- Unused frontend shell capability was removed from the main Tauri window.
- Dependency audit and locked Rust builds are enforced in CI.

### Added
- Focused-host SSH connection diagnostics with actionable DNS, network, authentication, credential, dependency, and host-key failure classification.
- Linux and macOS release checksum manifests.
- Dependabot coverage for npm, Cargo, and GitHub Actions.

[Unreleased]: https://github.com/darwvin-dev/RemoteOpsX/compare/v0.2.0-alpha.1...HEAD
[0.2.0-alpha.1]: https://github.com/darwvin-dev/RemoteOpsX/releases/tag/v0.2.0-alpha.1
