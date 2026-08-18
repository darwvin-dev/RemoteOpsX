# RemoteOpsX

**A unified Linux and macOS remote-operations workspace — not just another terminal.**

RemoteOpsX combines SSH/SFTP/FTP/RDP/VNC access, agentless server-health monitoring, systemd diagnostics, log tooling, SSH tunnels, executable runbooks, session history, snippets, and focused-host connection diagnostics in one Tauri desktop application.

> Current release line: **0.2.0-alpha**. Linux targets include Arch, Ubuntu, Debian, Fedora, and openSUSE; macOS is also supported.

## Project status

RemoteOpsX is a validated MVP undergoing production hardening. The core operator workflows are implemented; the remaining release blockers are explicit SSH host-identity trust, runtime dependency preflight, restart reconciliation, centralized secret redaction, live integration fixtures, packaged-app E2E coverage, and signed distribution.

Current automated checks:

```bash
npm ci
npm run version:check
npm audit --audit-level=high
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --locked --manifest-path src-tauri/Cargo.toml
cargo build --locked --manifest-path src-tauri/Cargo.toml
```

CI runs the relevant gates on Linux and macOS. Tagged releases repeat the release preflight, verify that the tag matches the application version, build platform bundles, publish SHA-256 manifests, and generate GitHub release notes.

See [TODO.md](TODO.md) for the production roadmap, [CHANGELOG.md](CHANGELOG.md) for product changes, and [CONTRIBUTING.md](CONTRIBUTING.md) for the release discipline.

## Why it is different from a normal terminal

| Plain terminal | RemoteOpsX |
| --- | --- |
| One SSH shell | SSH + SFTP + RDP + VNC + tunnels, tabbed |
| Manual `top`, `df`, `free` checks | Agentless CPU/RAM/disk/net/load/uptime/process/service health |
| Diagnosis steps live in memory | Versioned, confirmation-gated executable runbooks |
| Credentials scattered across tools | Password secrets in the OS keyring; SQLite stores references only |
| Manual log collection | Logs panel + diagnostic bundle workflow |
| Trial-and-error connection debugging | Read-only saved-profile SSH diagnostics with actionable failure classes |

Health collection runs over a separate SSH exec path and does not interfere with the interactive terminal PTY.

## Implemented features

- **Server Manager** — profile CRUD, independent protocol ports, tags, groups, environment classification, notes, search, and SQLite persistence.
- **SSH Terminal** — xterm.js terminal tabs backed by PTYs driving the system OpenSSH client.
- **SSH diagnostics** — read-only focused-host probe through the same auth/keyring/host-key path used by live operations.
- **SFTP-style browser** — list/upload/download/delete/rename over SSH/SCP.
- **Legacy FTP** — curl-backed file operations with plaintext-protocol warning.
- **RDP** — external FreeRDP launcher using certificate TOFU and stdin credential delivery for stored passwords.
- **VNC** — external installed-viewer launcher.
- **Live Health** — agentless CPU, RAM, swap, disks, load, uptime, network rate, processes, ports, and failed services.
- **Services** — inspect/status/logs and confirmation-gated start/stop/restart.
- **Logs / diagnostic bundles** — remote log and journal collection with local export.
- **Runbooks** — YAML steps, variables, confirmation boundaries, output capture, and persisted history.
- **SSH tunnels** — local, remote, and dynamic SOCKS forwards with persisted records and live-process reconciliation.
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
  error.rs                   stable IPC error payload
  vault.rs                   OS keyring access
  ssh_manager.rs             OpenSSH argv + one-shot exec
  pty_manager.rs             interactive SSH PTYs
  health_collector.rs        agentless health probes
  runbook_runner.rs          runbook parser/executor
  sftp_manager.rs            SSH/SCP file operations
  ftp_manager.rs             curl FTP operations
  rdp_adapter.rs             FreeRDP launcher
  vnc_adapter.rs             VNC launcher
  tunnel_manager.rs          SSH forward process registry
```

The transport layer intentionally uses mature system clients so native transports can be introduced later without coupling UI workflows to a specific SSH/RDP implementation.

## Local data and secrets

Operational metadata is stored in `remoteopsx.db` under Tauri's per-user application data directory. Passwords are not stored in SQLite: the database contains a `secret_ref`, while the secret itself is stored through the OS keyring/Secret Service.

SQLite is bundled into the Rust binary; no system SQLite package is required.

## Runtime requirements

| Tool | Used for | Required? |
| --- | --- | --- |
| `ssh`, `scp` | SSH, SFTP-style operations, health, runbooks, tunnels | **Yes** |
| OS Secret Service / keyring backend | password secret storage | **Yes for stored passwords** |
| `sshpass` | non-interactive SSH password auth | Only for password-auth profiles |
| `curl` | legacy FTP | Only for FTP |
| `xfreerdp3` / `xfreerdp` | RDP | Only for RDP |
| VNC viewer (`vncviewer`, TigerVNC, Remmina, etc.) | VNC | Only for VNC |

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

PRs use Conventional Commit-style titles because squash merge titles feed the generated release history.

## Packaging

`npm run app:build` currently produces Linux **AppImage**, **.deb**, and **.rpm** bundles. Tagged releases upload those artifacts plus SHA-256 manifests. Native Arch/pacman repository packaging and signed APT/pacman repositories are later distribution milestones after the production/security gates pass.

## Security model

Current protections:

- Passwords are stored in the OS keyring and are not persisted as plaintext in SQLite.
- SSH/SCP password auth uses `sshpass -e`, keeping the password out of argv.
- FreeRDP stored passwords are delivered over stdin rather than `/p:<password>` argv.
- FreeRDP uses certificate TOFU instead of unconditional certificate ignore.
- Private-key paths, not key contents, are persisted.
- Destructive service/runbook actions retain explicit confirmation boundaries.
- The production WebView uses a restrictive CSP.

Current release blockers:

- SSH first contact still uses OpenSSH `StrictHostKeyChecking=accept-new`; explicit app-managed fingerprint trust is P0.
- Runtime dependency/keyring readiness is not yet surfaced centrally at startup.
- Central secret redaction is not yet guaranteed across every output/persistence/export path.
- Stale sessions need deterministic startup reconciliation; tunnel reconciliation currently happens when tunnels are listed.
- RDP/VNC remain external windows.
- FTP is plaintext by protocol design.
- Release artifacts have checksums but are not yet signed/notarized.

Do not treat an alpha build as production-ready until the P0 gates in [TODO.md](TODO.md) are complete.

## License

MIT (placeholder — adjust as needed).
