# RemoteOpsX

**A unified Linux remote-operations workspace — not just another terminal.**

RemoteOpsX is *MobaXterm + Remmina + a Netdata-lite + a server runbook engine*,
built for Linux operators. It combines remote access (SSH / SFTP / FTP / RDP / VNC),
**agentless** live server-health monitoring, systemd diagnostics, log tooling,
SSH tunnels and **executable runbooks** into one keyboard-friendly desktop app.

> Working name: **RemoteOpsX**. Linux-first (Arch, Ubuntu, Debian, Fedora).

---

## Why it's different from a normal terminal

A terminal gives you a shell. RemoteOpsX gives you an **operations cockpit**:

| Plain terminal | RemoteOpsX |
| --- | --- |
| One SSH shell | SSH + SFTP + RDP + VNC + tunnels, tabbed |
| You type `top`, `df`, `free`… | **Live agentless health panel** auto-collects CPU/RAM/disk/net/load/uptime, top processes, ports and failed services — no agent installed on the server |
| You remember the diagnosis steps | **Runbooks**: versioned, step-by-step, confirmation-gated, with captured output and history |
| Secrets in `~/.ssh/config` or your head | Secrets in the **OS keyring**, never in the database |
| You `grep` logs by hand | Logs panel + one-click **diagnostic bundle** |

The health collector reads `/proc`, `/sys`, `df`, `ss` and `systemctl` over a
**separate SSH exec channel** (never your interactive shell), so the metrics
never interfere with what you're typing.

---

## Features (MVP)

- **Server Manager** — profiles with host/port/user, protocol flags, auth type,
  key path, tags, group/folder, environment (prod/staging/dev) and notes.
  Persisted in SQLite; searchable, grouped sidebar.
- **SSH Terminal** — xterm.js terminals backed by server-side PTYs running the
  system `ssh` client. Multiple tabs, reconnect, resize, copy/paste, non-blocking.
- **SFTP / FTP File Browser** — list / upload / download / delete / rename remote files.
  SFTP is preferred; legacy FTP is supported through curl and is explicitly
  marked as plaintext in the UI. FTP profiles use password authentication.
- **RDP** — launches `xfreerdp` with the profile (fullscreen / resolution).
- **VNC** — launches an installed VNC viewer (tigervnc, remmina, …).
- **Live Health Panel** — agentless metrics every 2–5s (configurable): CPU, RAM,
  swap, disks, load, uptime, network rate, top CPU/MEM processes, listening
  ports and failed services. Threshold warnings.
- **Services Panel** — list failed systemd units, inspect status/logs,
  start/stop/restart with **confirmation + exact-command preview**.
- **Logs Panel** — tail remote files, read `journalctl`, filter, save locally,
  and build a one-shot **diagnostic bundle**.
- **Runbooks** — YAML-defined, executed step-by-step over SSH with per-step
  output, confirmation gates and persisted run history. Six built-ins ship
  by default (Linux Health Check, Diagnose High Disk Usage, Diagnose Failed
  Service, Restart Service Safely, VoIP Server Check, SMPP Gateway Check).
- **SSH Tunnels** — local (`-L`), remote (`-R`) and dynamic SOCKS (`-D`) forwards,
  tracked and stoppable, profiles persisted.

---

## Architecture

```
src/                         React + TypeScript frontend
  api.ts                     typed wrappers over Tauri commands
  store.ts                   Zustand global UI state
  types.ts                   shared types (mirror Rust models)
  components/
    ServerSidebar / ServerForm
    TabBar / TabContent
    TerminalTab               (xterm.js)
    HealthPanel / ServicesPanel
    RunbookRunner / RunbookLauncher
    SftpPanel / RemoteDesktopTab / LogsPanel
    TunnelManager / RightPanel / BottomPanel / NotesSnippetsPanel

src-tauri/src/               Rust backend (Tauri v2 commands)
  lib.rs                     command surface + AppState wiring
  database.rs                SQLite schema + queries
  vault.rs                   OS keyring (Secret Service) — secrets only here
  ssh_manager.rs             ssh argv builder + one-shot remote exec
  pty_manager.rs             interactive PTY terminals (system ssh)
  health_collector.rs        agentless metric probe + parsing + rate deltas
  runbook_runner.rs          YAML runbook engine + built-ins
  sftp_manager.rs            list/upload/download/delete/rename (ssh/scp)
  ftp_manager.rs             legacy plaintext FTP operations (curl)
  rdp_adapter.rs             xfreerdp launcher (swappable for embedded later)
  vnc_adapter.rs             VNC viewer launcher
  tunnel_manager.rs          ssh -L/-R/-D process registry
  models.rs                  serde models
```

SSH uses its configured profile port. FTP, RDP and VNC have independent
per-profile ports with protocol-standard defaults (21, 3389 and 5900).

The SSH/SFTP/FTP/RDP/VNC/tunnel layers are intentionally thin abstractions over the
system OpenSSH/curl/FreeRDP binaries so the MVP is robust today, while leaving clean
seams to swap in native transports later.

---

## Installation requirements

RemoteOpsX is a Tauri app. To **run it**, the host needs the system tools it
drives:

| Tool | Used for | Required? |
| --- | --- | --- |
| `ssh`, `scp` (OpenSSH client) | SSH, SFTP, health, runbooks, tunnels | **Yes** |
| `curl` | Legacy FTP browser | Only if you use FTP |
| `sshpass` | password-auth (non-interactive) | Only if you use password auth |
| `xfreerdp` / `xfreerdp3` | RDP | Only for RDP |
| a VNC viewer (`tigervnc`, `remmina`, …) | VNC | Only for VNC |
| OS Secret Service (GNOME Keyring / KWallet) | secret storage | **Yes** |

SQLite is **bundled** into the binary — no system SQLite needed.

Install the runtime tools:

```bash
# Arch
sudo pacman -S openssh sshpass freerdp tigervnc gnome-keyring

# Debian / Ubuntu
sudo apt install openssh-client sshpass freerdp2-x11 tigervnc-viewer gnome-keyring

# Fedora
sudo dnf install openssh-clients sshpass freerdp tigervnc gnome-keyring
```

---

## Build / run from source (development)

Prerequisites: **Rust** (stable, via rustup), **Node 18+**, and the Tauri Linux
system deps (`webkit2gtk-4.1`, `libappindicator`, etc).

```bash
# Tauri system deps (Arch example)
sudo pacman -S webkit2gtk-4.1 base-devel curl wget file openssl appmenu-gtk-module libappindicator-gtk3 librsvg

# install JS deps
npm install

# run the app in dev mode (Vite + Tauri)
npm run app:dev
```

Useful scripts:

```bash
npm run dev          # Vite dev server only (web UI, no Tauri shell)
npm run build        # type-check + build the frontend
npm run app:dev      # full Tauri desktop app, hot-reload
npm run app:build    # produce AppImage / .deb / .rpm bundles
npm run app:build:arch # Arch workaround for current linuxdeploy/gdk-pixbuf incompatibilities
```

Backend-only compile check:

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

---

## Packaging

`npm run app:build` produces, on Linux: **AppImage**, **.deb** and **.rpm**
(configured in `src-tauri/tauri.conf.json`). A pacman package can be added later.
On current Arch systems, use `npm run app:build:arch`; it disables linuxdeploy's
incompatible legacy strip step and supplies the empty loader directory expected
by its GTK plugin. Regular Ubuntu/Debian and CI builds should use `app:build`.

---

## Security model (and MVP limitations)

**What we do well today**
- Passwords live in the **OS keyring (Secret Service)**, keyed
  per server. SQLite stores only a `secret_ref`, never the secret.
- Encrypted private keys use the SSH agent or the interactive SSH prompt; the
  application does not persist key passphrases.
- Passwords are fed to `ssh`/`scp` via `sshpass -e` (environment), never on the
  process command line, and never logged.
- The production WebView uses a restrictive Content Security Policy.
- Private key **paths** are stored; key **contents** are not.
- Destructive actions (service restart/stop and confirmation-gated runbook
  steps) require explicit confirmation and show the exact command first.

**MVP limitations (be aware)**
- `StrictHostKeyChecking=accept-new`: first-seen host keys are trusted
  automatically (changes are still detected). A known-hosts management UI is TODO.
- `FreeRDP` receives the password via `/p:` on its own command line — a known
  FreeRDP limitation, not under our control.
- No app-level master-password lock yet (keyring is the trust anchor).
- RDP/VNC are launched as **external** windows; not embedded.
- FTP credentials and data are plaintext on the network by protocol design.
- Secrets masking in interactive terminal output is best-effort.

See [TODO.md](TODO.md) for the roadmap that hardens these.

---

## License

MIT (placeholder — adjust as needed).
