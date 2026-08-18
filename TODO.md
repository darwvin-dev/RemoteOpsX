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
- 🚧 **Host identity trust** — current OpenSSH transport uses `StrictHostKeyChecking=accept-new`, so changed keys are rejected but first contact is implicit. Add a known-hosts UI with fingerprint preview, explicit trust/replace/remove actions and clear changed-key recovery.
- ✅ **RDP credential/certificate baseline** — use certificate TOFU instead of unconditional certificate ignore, and send stored passwords through stdin rather than process argv.
- ⬜ **Runtime preflight** — detect required/optional binaries and keyring availability at startup; show actionable status for OpenSSH, SCP, curl, sshpass, FreeRDP and VNC before a user hits a failure.
- ⬜ **Connection test** — profile-level "Test connection" with DNS/TCP/auth/host-key diagnostics and a safe latency/result summary before saving or operating on production hosts.
- ⬜ **Live SSH integration fixture** — CI test against an ephemeral SSH server covering key auth, password auth, exec, PTY, SCP/SFTP operations and host-key mismatch behavior.
- ⬜ **Crash/restart reconciliation** — close stale session rows on startup and reconcile orphaned tunnel/session state deterministically.
- ⬜ **Secret-masking pass** — central redaction for command/log/runbook output, diagnostic bundles and error payloads; add regression tests for stored credentials and user-defined sensitive values.
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
- ✅ Release workflows repeat preflight checks and publish SHA-256 checksum manifests
- ✅ Automated npm/Cargo/GitHub Actions dependency update checks
- ⬜ End-to-end desktop smoke suite for the packaged application
- ⬜ Security-focused tests for host identity, credential exposure, path handling and destructive operations
