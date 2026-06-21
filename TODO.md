# RemoteOpsX — Roadmap

Status legend: ✅ done (MVP) · 🚧 partial · ⬜ planned

## MVP vertical slice (delivered)
- ✅ Server Manager (CRUD, groups, tags, environments, search) in SQLite
- ✅ Secrets in OS keyring (Secret Service); no plaintext in SQLite
- ✅ SSH terminal tabs (xterm.js + server-side PTY over system `ssh`), reconnect/resize
- ✅ Live agentless Health panel (CPU/RAM/swap/disk/load/uptime/net, top procs, ports, failed services) with thresholds + sparklines
- ✅ Runbook engine + 6 built-ins, step-by-step run with confirmation + persisted history
- ✅ Services panel (failed units, status/logs, confirmed start/stop/restart)
- ✅ SFTP browser (list/upload/download/delete/rename)
- ✅ Legacy FTP browser via curl, with independent port and plaintext warning
- ✅ RDP launcher (`xfreerdp`), VNC launcher (system viewer)
- ✅ Logs panel (tail / journalctl / filter / save / diagnostic bundle)
- ✅ SSH tunnels (-L / -R / -D), tracked + persisted

## Next: hardening & depth
- ⬜ **Native SSH transport** (e.g. `russh`/`libssh2`) to replace the system-`ssh`
      abstraction — removes `sshpass`, enables in-app host-key management, key
      passphrase prompts, jump hosts and connection multiplexing.
- ⬜ **Known-hosts UI** — review/trust/rotate host keys instead of `accept-new`.
- ⬜ **App lock** — optional master password gating the app + per-launch keyring unlock.
- ⬜ Persistent SFTP subsystem session (replace per-op `ssh`/`scp`), progress bars,
      drag-and-drop, recursive transfers, chmod.
- ⬜ Embedded RDP (FreeRDP via libfreerdp + canvas) and embedded VNC.
- ⬜ Health: historical retention (per-server time-series), pluggable thresholds,
      alert routing (desktop notifications / webhook).
- ⬜ Runbook editor UI (YAML form + validation), variable prompts, dry-run,
      import/export, scheduling, partial re-run from a failed step.
- ⬜ Tunnel auto-reconnect + autostart on connect; SOCKS health checks.
- ⬜ Sessions history view (the `sessions` table is recorded; add a UI).
- ⬜ Command snippets manager (user-editable, per-tag), broadcast-to-many.

## Platform & packaging
- ⬜ pacman package target; signed AppImage; Flatpak.
- ⬜ CI matrix builds (Arch/Ubuntu/Debian/Fedora).
- ⬜ Settings store (theme, default ports, refresh interval persistence).

## Quality
- ⬜ Rust unit tests for health parsers (feed fixture `/proc` output).
- ✅ Frontend regression tests for RunbookRunner state machine and PTY startup ordering.
- ⬜ Live SSH integration test against a reachable Linux test host.
- ⬜ Secret-masking pass over terminal/log output.
