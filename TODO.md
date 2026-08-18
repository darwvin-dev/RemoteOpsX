# RemoteOpsX — Production Roadmap

Status legend: ✅ done · 🚧 partial · ⬜ planned

## Usable core (delivered)
- ✅ Server Manager (CRUD, groups, tags, environments, search) in SQLite
- ✅ Secrets in the OS keyring; no plaintext password storage in SQLite
- ✅ SSH terminal tabs (xterm.js + server-side PTY over system `ssh`), reconnect/resize
- ✅ Local SSH key discovery and public-key installation helper
- ✅ Live agentless health panel (CPU/RAM/swap/disk/load/uptime/net, top processes, ports, failed services)
- ✅ Runbook engine + built-ins, confirmation-gated steps, persisted run history
- ✅ Services panel with confirmed start/stop/restart actions
- ✅ SFTP-style browser over SSH/SCP plus legacy FTP support with plaintext warning
- ✅ RDP and VNC launchers
- ✅ Logs panel and diagnostic bundle export
- ✅ SSH tunnels (-L / -R / -D), tracked + persisted
- ✅ Settings store/UI with theme, ports, refresh, retention and desktop flags
- ✅ Sessions history UI
- ✅ User-editable command snippets
- ✅ Linux/macOS dependency bootstrap and Linux/macOS release workflows

## P0 — production trust and reliability
- ✅ **Host identity trust** — RemoteOpsX owns a dedicated `known_hosts` file. Terminal, one-shot SSH, SCP and tunnels use `StrictHostKeyChecking=yes`; first contact requires fingerprint review and explicit Trust, changed keys require explicit Replace, and stored trust can be removed.
- ✅ **RDP credential/certificate baseline** — certificate TOFU replaces unconditional certificate ignore, and stored passwords are sent through stdin rather than process argv.
- ✅ **Runtime preflight** — startup/Diagnostics checks OpenSSH (`ssh`, `scp`, `ssh-keyscan`, `ssh-keygen`), password helper, curl, FreeRDP, VNC viewer and keyring readiness with required/optional status.
- ✅ **Connection diagnostics** — focused-host read-only SSH test uses the same transport/auth/keyring/strict-host-key path as live operations, with latency plus actionable DNS/network/auth/host-key/dependency/credential failure classification.
- ✅ **Live SSH integration fixture** — Linux CI launches ephemeral destination + bastion sshd instances and exercises key auth, password auth, exec, real PTY, SCP/SFTP operations, local forwarding, bastion routing, host-key mismatch blocking/explicit replacement and a secret-redaction canary.
- ✅ **Crash/restart reconciliation** — startup marks stale sessions/runbooks interrupted and persisted active/starting tunnels stopped; graceful tunnel-manager teardown kills/reaps managed SSH tunnel children.
- ✅ **Secret-masking pass (known vault secrets)** — vault reads/writes register secrets centrally and existing credential references are preloaded from the keyring at startup on a best-effort basis. Buffered SSH output, remote errors, runbook results, diagnostic/local text export and backend logging are redacted before IPC/storage/export. New profile metadata, runbook names/descriptions/YAML, snippet labels/commands/tags, jump-host metadata and persisted tunnel endpoint text reject known stored credentials before SQLite persistence. PTY output retains its streaming password redactor for split chunks.
- ⬜ **Signed releases** — code-sign/notarize macOS artifacts and sign Linux release artifacts. Checksums are published, but signatures are still required for a strong distribution trust chain.

## P1 — operator depth
- ✅ **Persistent SFTP transfer subsystem** — strict per-server OpenSSH ControlMaster reuse, cancellable background upload/download jobs, recursive transfers, single-file byte progress, chmod and Tauri-native drag/drop queueing.
- ✅ **Runbook Studio** — bounded YAML import/export, validation, variable rendering, non-executing dry-run preview, destructive/confirmation markers and retry from the first failed step.
- ✅ **Tunnel resilience** — persisted autostart/auto-reconnect desired-state policies, reconciliation, explicit failed state and explicit Stop precedence.
- 🚧 **Health history + alerts** — bounded 30-second/7-day per-server history, custom threshold/consecutive/cooldown rules, persisted acknowledgement and dashboard visualization are implemented. Desktop/webhook delivery remains planned.
- ⬜ **App lock** — optional local lock/master credential with keyring-aware unlock behavior.
- ✅ **Import/export** — encrypted versioned workspace backup/restore for profiles, settings, runbooks, snippets, bastions, alert rules and tunnels; keyring secrets are never exported and password profiles require credential re-entry.
- ✅ **Jump hosts / bastion routing** — first-class key-auth bastion configuration with strict app-owned SSH config, explicit bastion fingerprint trust, destination fingerprint inspection through the trusted bastion, and shared routing across terminal/exec/health/runbooks/SCP/tunnels.

## Product experience
- ✅ **Operations Dashboard** — fleet rollup from persisted health, alerts, tunnel state and recent automation, with direct server/SSH drill-down.
- ✅ **Universal Command Palette** — fuzzy index across application actions, protocol-specific server actions, health/diagnostics, runbooks and snippets; snippet execution requires a focused target and confirmation.

## P2 — transport and desktop expansion
- ⬜ Native SSH transport (for example `russh`/`libssh2`) if it materially improves host-key UX, multiplexing, passphrase prompts and portability over the hardened system-OpenSSH backend.
- ⬜ Embedded RDP and VNC sessions instead of external viewers.
- ⬜ Flatpak and additional distro packaging targets after the signed release path is stable.
- ✅ Broadcast-to-many commands with a 50-host cap, bounded concurrency, independent production/destructive confirmations, known-secret rejection and persisted per-host result tracking.

## Quality gates
- ✅ Frontend regression tests
- ✅ Rust unit tests
- ✅ CI runs frontend tests, TypeScript/Vite build, Rust formatting, Rust tests and locked Rust build on Linux and macOS
- ✅ CI includes source-level security invariants that reject weakened SSH/RDP trust and credential-argv patterns
- ✅ Release workflows repeat preflight checks and publish SHA-256 checksum manifests
- ✅ Automated npm/Cargo/GitHub Actions dependency update checks
- ✅ Live SSH integration fixture
- ⬜ End-to-end desktop smoke suite for the packaged application
- 🚧 Security-focused tests — unit/source gates cover host identity and known-secret exposure, and live transport now covers the real SSH/SCP/PTY/tunnel path; packaged-app destructive-operation coverage remains for the desktop E2E phase.
