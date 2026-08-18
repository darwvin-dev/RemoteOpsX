# Changelog

All notable changes to RemoteOpsX are documented here.

The project follows Semantic Versioning. Pre-release progression for a release line is `alpha` → `beta` → `rc` → stable. Pull requests use Conventional Commit-style titles so GitHub-generated release notes remain useful after squash merges.

## [Unreleased]

### Security
- Explicit SSH host identity management, runtime dependency preflight, crash recovery, and centralized secret redaction are the remaining P0 hardening tracks for the `0.2.0` release line.

### Changed
- Release tooling now treats `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json` as one versioned product.
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
