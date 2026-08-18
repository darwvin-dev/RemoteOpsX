# RemoteOpsX

**A unified Linux and macOS remote-operations workspace — not just another terminal.**

RemoteOpsX combines SSH/SFTP/FTP/RDP/VNC access, agentless server-health monitoring, systemd diagnostics, log tooling, SSH tunnels, executable runbooks, session history, snippets, and focused-host connection diagnostics in one Tauri desktop application.

> Current release line: **0.2.0-alpha**. Linux targets include Arch, Ubuntu, Debian, Fedora, and openSUSE; macOS is also supported.

## Project status

RemoteOpsX is a validated MVP undergoing release verification. The core operator workflows and the P0 implementation for explicit SSH host trust, runtime dependency preflight, restart reconciliation, and known-secret redaction are implemented. The remaining public-release blockers are the live SSH integration fixture, packaged-app E2E/security coverage, signed distribution, and repository-side enforcement of the documented branch-protection policy.

Current automated checks:

```bash
npm ci
npm run version:check
bash scripts/security-gates.sh
npm audit --audit-level=high
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --locked --manifest-path src-tauri/Cargo.toml
cargo build --locked --manifest-path src-tauri/Cargo.toml
```

CI runs the relevant gates on Linux and macOS. Tagged releases repeat the release preflight, verify that the tag matches the application version, build platform bundles, publish SHA-256 manifests, and generate GitHub release notes.

See [TODO.md](TODO.md) for the production roadmap, [CHANGELOG.md](CHANGELOG.md) for product changes, [CONTRIBUTING.md](CONTRIBUTING.md) for the release discipline, and [.github/BRANCH_PROTECTION.md](.github/BRANCH_PROTECTION.md) for the required `main` protection contract.

## Why it is different from a normal terminal

| Plain terminal | RemoteOpsX |
| --- | --- |
| One SSH shell | SSH + SFTP + RDP + VNC + tunnels, tabbed |
| Manual `top`, `df`, `free` checks | Agentless CPU/RAM/disk/net/load/uptime/process/service health |
| Diagnosis steps live in memory | Versioned, confirmation-gated executable runbooks |
| Credentials scattered across tools | Password secrets in the OS keyring; SQLite stores references only |
| Manual log collection | Logs panel + diagnostic bundle workflow |
| Trial-and-error connection debugging | Runtime preflight + explicit SSH fingerprint trust + read-only connection diagnostics |

Health collection runs over a separate SSH exec path and does not interfere with the interactive terminal PTY.

## Implemented features

- **Server Manager** — profile CRUD, independent protocol ports, tags, groups, environment classification, notes, search, and SQLite persistence.
- **SSH Terminal** — xterm.js terminal tabs backed by PTYs driving the system OpenSSH client.
- **SSH host identity** — app-managed `known_hosts`, SHA-256 fingerprint preview, explicit Trust / Replace / Remove, and strict host-key enforcement across terminal, exec, SCP, and tunnels.
- **Runtime preflight** — checks core OpenSSH tooling and profile-specific password, FTP, RDP, VNC, and keyring dependencies before live operations.
- **SSH diagnostics** — read-only focused-host probe through the same auth/keyring/strict-host-key path used by live operations.
- **SFTP-style browser** — list/upload/download/delete/rename over SSH/SCP.
- **Legacy FTP** — curl-backed file operations with plaintext-protocol warning.
- **RDP** — external FreeRDP launcher using certificate TOFU and stdin credential delivery for stored passwords.
- **VNC** — external installed-viewer launcher; macOS can use Screen Sharing via `open`.
- **Live Health** — agentless CPU, RAM, swap, disks, load, uptime, network rate, processes, ports, and failed services.
- **Services** — inspect/status/logs and confirmation-gated start/stop/restart.
- **Logs / diagnostic bundles** — remote log and journal collection with local export.
- **Runbooks** — YAML steps, variables, confirmation boundaries, output capture, and persisted history.
- **SSH tunnels** — local, remote, and dynamic SOCKS forwards with persisted records and live-process reconciliation.
- **Startup recovery** — stale sessions/runbooks are marked interrupted and stale persisted tunnel state is reconciled after an unclean restart.
- **Known-secret redaction** — keyring secrets are centrally registered and masked from buffered remote output, backend errors/logging, runbook results, and exported text; known credentials are rejected from persisted profile metadata, snippets, and runbooks.
- **Session history** and **command snippets**.
- **Settings** — theme, ports, health refresh, history retention, transfer behavior, and desktop options.

## Architecture

```text
src/                         React + TypeScript frontend
  api.ts                     typed Tauri command wrappers
  errors.ts                  normalized frontend error contract
  store.ts                   Zustand UI state
  types.ts                   shared frontend models
  components/                server, terminal, files, health, runbooks, etc.

src-tauri/src/               Rust backend
  lib.rs                     Tauri command surface + AppState
  database.rs                SQLite schema + queries
  error.rs                   stable/redacted IPC error payload
  vault.rs                   OS keyring + secret registration
  host_identity.rs           app-managed SSH known_hosts / fingerprints
  runtime_preflight.rs       local dependency/keyring readiness
  recovery.rs                startup stale-state reconciliation
  redaction.rs               central known-secret masking/rejection
  ssh_manager.rs             strict OpenSSH + stdin one-shot exec
  pty_manager.rs             interactive SSH PTYs + streaming redaction
  health_collector.rs        agentless health probes
  runbook_runner.rs          runbook parser/executor
  sftp_manager.rs            strict SSH/SCP file operations
  ftp_manager.rs             curl FTP operations
  rdp_adapter.rs             FreeRDP launcher
  vnc_adapter.rs             VNC launcher
  tunnel_manager.rs          strict SSH forward process registry
```

The transport layer intentionally uses mature system clients so native transports can be introduced later without coupling UI workflows to a specific SSH/RDP implementation.

## Local data, trust, and secrets

Operational metadata is stored in `remoteopsx.db` under Tauri's per-user application data directory. Passwords are not stored in SQLite: the database contains a `secret_ref`, while the secret itself is stored through the OS keyring/Secret Service.

RemoteOpsX also owns a dedicated `known_hosts` file under its application data directory. SSH, SCP, PTY sessions, and tunnels use that file with `StrictHostKeyChecking=yes`. A first-seen key is not trusted automatically: the Diagnostics panel shows SHA-256 fingerprints and requires an explicit operator Trust action after out-of-band verification. A changed key remains blocked until an explicit Replace action.

SQLite is bundled into the Rust binary; no system SQLite package is required.

## Runtime requirements

| Tool | Used for | Required? |
| --- | --- | --- |
| `ssh`, `scp`, `ssh-keyscan`, `ssh-keygen` | SSH transport, SFTP-style operations, fingerprint trust, health, runbooks, tunnels | **Yes** |
| OS Secret Service / keyring backend | stored password secret storage | **For stored-password profiles** |
| `sshpass` | non-interactive SSH password auth | Only for password-auth profiles |
| `curl` | legacy FTP | Only for FTP |
| `xfreerdp3` / `xfreerdp` | RDP | Only for RDP |
| platform VNC viewer / macOS Screen Sharing | VNC | Only for VNC |

The Diagnostics panel evaluates optional dependencies against the selected profile, so an SSH-key-only user is not blocked by missing FreeRDP, VNC, FTP, or password-auth tooling.

Example runtime packages:

```bash
# Arch
sudo pacman -S openssh sshpass curl freerdp tigervnc gnome-keyring

# Debian / Ubuntu
sudo apt install openssh-client sshpass curl freerdp2-x11 tigervnc-viewer gnome-keyring

# Fedora
sudo dnf install openssh-clients sshpass curl freerdp tigervnc gnome-keyring
```

## Development

The repository pins the release/CI toolchains in `.nvmrc` and `rust-toolchain.toml`. Install the platform build prerequisites with:

```bash
npm run deps:build
```

Then:

```bash
npm ci
npm run version:check
npm run app:dev
```

Useful commands:

```bash
npm test
npm run build
npm run app:build
npm run app:build:arch
cargo check --manifest-path src-tauri/Cargo.toml
```

## Version and release policy

The following three files are one version contract and must match:

```text
package.json
src-tauri/Cargo.toml
src-tauri/tauri.conf.json
```

`npm run version:check` enforces the contract. Release CI also rejects a tag that does not exactly equal `v<project-version>`.

The `0.2.0` promotion path is:

```text
v0.2.0-alpha.N -> v0.2.0-beta.N -> v0.2.0-rc.N -> v0.2.0
```

PRs use Conventional Commit-style titles because merge history feeds generated release notes.

## Packaging

`npm run app:build` currently produces Linux **AppImage**, **.deb**, and **.rpm** bundles. Tagged releases upload those artifacts plus SHA-256 manifests. Native Arch/pacman repository packaging and signed APT/pacman repositories are later distribution milestones after the live integration, packaged E2E, and signing gates pass.

## Security model

Current protections:

- Passwords are stored in the OS keyring and are not persisted as plaintext in SQLite.
- SSH/SCP password auth uses `sshpass -e`, keeping the password out of argv.
- One-shot SSH command text is sent through SSH stdin rather than embedded in the local process argv.
- SSH first contact requires explicit SHA-256 fingerprint Trust; all SSH-derived transports use the app-managed known-hosts file with strict verification.
- Changed SSH identities remain blocked until the operator independently verifies and explicitly replaces the stored identity.
- FreeRDP stored passwords are delivered over stdin rather than password argv.
- FreeRDP uses certificate TOFU instead of unconditional certificate ignore.
- Known keyring secrets are centrally redacted before buffered IPC output, runbook persistence, backend error/log delivery, and local text/diagnostic export.
- Known stored credentials are rejected when saving profile metadata, user runbooks, or command snippets to SQLite.
- Interactive PTY output retains streaming redaction so a known password split across read chunks is masked.
- Startup reconciliation prevents stale sessions/runbooks/tunnel rows from continuing to claim active state after a crash/restart.
- Private-key paths, not key contents, are persisted.
- Destructive service/runbook actions retain explicit confirmation boundaries.
- The production WebView uses a restrictive CSP.
- CI contains source-level regression barriers against weakened SSH/RDP verification and credential-in-argv patterns.

Remaining release blockers:

- Live ephemeral-SSH integration tests must verify key/password auth, PTY/exec/SCP/tunnel behavior, and host-key mismatch rejection against a real SSH server.
- Packaged desktop E2E/security tests must cover fresh install, upgrade, keyring/dependency failures, destructive confirmation boundaries, and diagnostic-export leakage.
- Release artifacts have checksums but are not yet signed/notarized.
- The repository-side `main` branch protection/ruleset must enforce the required CI checks documented in `.github/BRANCH_PROTECTION.md`.
- RDP/VNC remain external windows and FTP remains plaintext by protocol design; these are product limitations rather than blockers for the hardened SSH release path.

Do not treat an alpha build as production-ready until the remaining gates in [TODO.md](TODO.md) are complete.

## License

MIT (placeholder — adjust as needed).
