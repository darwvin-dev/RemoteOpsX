# RemoteOpsX Completion and Hardening Design

## Goal

Make the current RemoteOpsX working tree buildable and internally consistent, repair the confirmed runtime defects, complete the partially added FTP feature, strengthen the Tauri boundary, and add regression coverage for the corrected behavior.

## Scope

The work covers the current React/Tauri architecture. It does not replace system OpenSSH, curl, FreeRDP, or VNC viewers with native transports.

## Backend changes

- Rebuild FTP curl argument construction with owned strings so Rust references remain valid, then add unit coverage for FTP URL/path/list parsing.
- Change download commands to accept an exact local destination path. SFTP and FTP must both honor the filename selected in the save dialog.
- Extend server profiles with optional protocol-specific ports for FTP, RDP, and VNC. Existing databases receive nullable columns; absent values retain the existing defaults (FTP 21, RDP 3389, VNC 5900). SSH continues to use `port`.
- Make server and credential updates failure-safe. A failed keyring or credential-record operation must not leave a newly created or partially updated server reported as failed while persisted. Authentication-mode changes remove obsolete credential metadata and secrets where appropriate.
- Treat key authentication as SSH-agent or interactive-prompt authentication. The UI must not claim that an unused key passphrase is stored.
- Register terminal event listeners before spawning the PTY so initial output and early exit events cannot be missed.
- Validate tunnel input ports and preserve accurate process state reporting.

## Runbook execution

Move execution bookkeeping into a small state-machine module with one run identity, start timestamp, accumulated results, overall status, and explicit pending-confirmation state. Confirmation resumes the same run; Skip records a skipped step and continues. Starting again creates a clean run using the current variables. Persisted history contains every executed or skipped step exactly once.

Variable substitution remains visible in the exact-command preview. Confirmation-gated commands cannot execute before confirmation.

## Frontend completion

- Expose FTP in the server form, sidebar/start dashboard, and command palette whenever enabled.
- Pass exact download destination paths through the typed API.
- Add protocol-port inputs without breaking existing profiles.
- Surface actionable errors instead of silently swallowing terminal and history failures where user action is possible.
- Split heavy xterm loading from the initial bundle if it materially removes the current Vite chunk warning without complicating terminal lifetime behavior.

## Security

Enable a restrictive production CSP for bundled assets and Tauri IPC while allowing only the inline styles currently required by the application. Keep shell execution blocked; the existing default shell permission permits URL opening only. Plain FTP remains explicitly labeled insecure because credentials and data are not encrypted by the protocol.

## Testing and verification

- Rust unit tests cover FTP construction/parsing, protocol-port resolution, and validation.
- Frontend tests cover the runbook state machine, especially confirm, skip, retry, accumulated history, and current-variable substitution.
- Existing Rust and SSH integration tests remain intact.
- Required verification: frontend type-check/build, frontend tests, Rust formatting check, Clippy when available, Rust tests, Rust compile check, dependency audit, and `git diff --check`.
- Rendered UI smoke testing is performed when a browser runner is available; inability to run it is reported rather than treated as a pass.

## Compatibility and migration

Database migrations are additive. Existing server records keep working without user action. Existing profiles with only the shared SSH port use standard protocol defaults for non-SSH transports. No secrets are migrated into SQLite or frontend state.

## Non-goals

- Native SSH/SFTP transport
- Embedded RDP or VNC
- Full runbook editor or scheduler
- Host-key management UI
- Cross-platform packaging beyond the currently configured Linux targets
