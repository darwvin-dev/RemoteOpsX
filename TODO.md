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
- ✅ **Connection diagnostics** — focused-host read-only SSH test uses the same transport/auth/keyring/host-key path as live operations, with latency plus actionable DNS/network/auth/host-key/dependency/credential failure classification.
- ⬜ **Live SSH integration fixture** — CI test against an ephemeral SSH server covering key auth, password auth, exec, PTY, SCP/SFTP operations and host-key mismatch behavior.
- ✅ **Crash/restart reconciliation** — startup marks stale sessions/runbooks interrupted and persisted active/starting tunnels stopped; graceful tunnel-manager teardown kills/reaps managed SSH tunnel children.
- ✅ **Secret-masking pass (known vault secrets)** — vault reads/writes register secrets centrally; buffered SSH output, remote errors, runbook results, diagnostic/local text export and backend logging are redacted before IPC/storage/export. Profile metadata, runbooks and snippets reject known stored credentials before SQLite persistence. PTY output retains its streaming password redactor for split chunks. Live integration tests remain the next verification layer.
- ⬜ **Signed releases** — code-sign/notarize macOS artifacts and sign Linux release artifacts. Checksums are published, but signatures are still required for a strong distribution trust chain.

## P1 — operator depth
- ⬜ **Persistent SFTP subsystem** — replace per-operation `ssh`/`scp`; add progress, cancellation, recursive transfers, chmod and drag/drop.
- ⬜ **Runbook editor** — validated form/YAML editor, variables, dry-run, import/export and partial retry from a failed step.
- ⬜ **Tunnel resilience** — health checks, auto-reconnect, autostart-on-connect and explicit failure state.
- ⬜ **Health history** — bounded per-server time-series retention, custom thresholds and desktop/webhook alert routing.
- ⬜ **App lock** — optional local lock/master credential with keyring-aware unlock behavior.
- ⬜ **Import/export** — encrypted, versioned backup/restore of profiles, settings, runbooks and snippets without exporting keyring secrets by default.
- ⬜ **Jump hosts / ProxyJump UI** — first-class bastion configuration with validation and connection test support.

## P2 — transport and desktop expansion
- ⬜ Native SSH transport (for example `russh`/`libssh2`) if it materially improves host-key UX, multiplexing, passphrase prompts and portability over the hardened system-OpenSSH backend.
- ⬜ Embedded RDP and VNC sessions instead of external viewers.
- ⬜ Flatpak and additional distro packaging targets after the signed release path is stable.
- ⬜ Broadcast-to-many commands with strong environment/risk safeguards and per-host result tracking.

## Quality gates
- ✅ Frontend regression tests
- ✅ Rust unit tests
- ✅ CI runs frontend tests, TypeScript/Vite build, Rust formatting, Rust tests and locked Rust build on Linux and macOS
- ✅ CI includes source-level security invariants that reject weakened SSH/RDP trust and credential-argv patterns
- ✅ Release workflows repeat preflight checks and publish SHA-256 checksum manifests
- ✅ Automated npm/Cargo/GitHub Actions dependency update checks
- ⬜ Live SSH integration fixture
- ⬜ End-to-end desktop smoke suite for the packaged application
- 🚧 Security-focused tests — unit/source gates now cover host identity and known-secret exposure; live transport, filesystem path and packaged-app destructive-operation coverage remains for the integration/E2E phases.
