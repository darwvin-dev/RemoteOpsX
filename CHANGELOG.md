# Changelog

All notable changes to RemoteOpsX are documented here.

The project follows Semantic Versioning. Pre-release progression for a release line is `alpha` → `beta` → `rc` → stable. Pull requests use Conventional Commit-style titles so GitHub-generated release notes remain useful after merges.

## [Unreleased]

### Added
- Appearance presets now include Obsidian, Paper, Nord, Dracula, Tokyo Night, Solarized Dark, and Solarized Light, plus system-following mode.
- Interface font presets include System UI, Inter, IBM Plex Sans, Noto Sans, Ubuntu, and Roboto with safe local fallbacks.
- Interface density presets now include Compact, Comfortable, and Spacious layouts.
- Terminal font presets include JetBrains Mono, Fira Code, Cascadia Code, IBM Plex Mono, Source Code Pro, DejaVu Sans Mono, and system monospace.
- Terminal appearance controls now include font size, line height, block/underline/bar cursor style, and bounded background opacity.
- Terminal palette, font, cursor, sizing, spacing, and opacity changes apply to existing SSH tabs without reconnecting the session.
- First-class key-auth SSH jump hosts route terminal, command execution, health/runbooks, SFTP/SCP, and tunnels through one strict transport policy.
- Linux CI now exercises real ephemeral target and bastion `sshd` instances for key/password auth, PTY, SCP, local forwarding, bastion routing, host-key mismatch/replacement, and secret redaction.
- Operator health history persists bounded 30-second samples for up to seven days per server, with configurable persistent alert rules, cooldowns, consecutive-sample gates, and acknowledgement.
- File transfers can reuse persistent OpenSSH ControlMaster sessions, run recursively, report single-file byte progress, be cancelled, and apply remote chmod changes.
- Multi-host commands support bounded concurrency, a 50-host cap, production confirmation, destructive-command confirmation, and persisted per-host execution results.
- Tunnel policies add desired-state autostart/auto-reconnect reconciliation.
- Versioned encrypted workspace backup/restore uses OpenSSL AES-256-CBC with PBKDF2, excludes keyring secrets, clears stale local credentials for restored server IDs, and restores transactionally with tunnel autostart disabled.
- Operator Center provides one place to manage persistent alerts, transfers, multi-host operations, tunnel policies, and backup/restore.

### Security
- SSH first contact now uses an app-managed `known_hosts` store with SHA-256 fingerprint preview and explicit Trust / Replace / Remove actions; terminal, one-shot exec, SCP, and tunnels all require strict host-key verification.
- SSH trust is isolated from system-wide known-host files, configured known-host helpers, DNS SSHFP auto-trust, automatic host-key updates, and localhost host-auth bypasses so the app review flow is the authoritative trust source.
- Saved host syntax is restricted to safe DNS/IP forms before it can be represented in `known_hosts`, preventing wildcard/list/marker injection through profile values.
- Jump-host routes preserve the same explicit host-key trust boundary for both the bastion and destination; bastions are key-auth only and user/system SSH configuration is bypassed for deterministic routing.
- One-shot remote commands are delivered through SSH stdin so command text is not exposed in the local process argv.
- Runtime preflight checks core OpenSSH tooling plus profile-specific password, FTP, RDP, VNC, and keyring readiness.
- Known keyring secrets are registered in a central redaction layer and masked from buffered SSH output, IPC errors, every textual runbook result field, backend diagnostics, and exported text; profile metadata, user runbooks, snippets, and persisted tunnel endpoint text reject known stored credentials before SQLite persistence.
- Existing saved credentials are preloaded from keyring references during startup on a best-effort basis, so redaction and persistence guards are active before the first connection attempt; historical user-controlled SQLite text is also scrubbed from logical database values when it contains a known secret.
- Workspace restore never revives an unrelated stale keyring credential sharing a restored server ID; all restored IDs have local credentials explicitly cleared after the database transaction commits.
- CI and tagged-release preflight contain source-level regression gates that reject weakened SSH/RDP trust and password-in-argv patterns.

### Reliability
- Startup recovery marks stale sessions and runbook executions interrupted and stale persisted tunnel state stopped after an unclean restart.
- SSH terminal exit closes the persisted session immediately; PTY, tunnel, transfer, and persistent SSH master child processes are killed and reaped deterministically instead of lingering until process exit.
- Workspace restore applies database changes atomically and re-hydrates jump-host runtime state after rollback or commit.

### Changed
- Appearance settings remain schema-1 compatible: older saved settings load the new interface/terminal font, density, cursor, sizing, spacing, and opacity defaults automatically.
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
