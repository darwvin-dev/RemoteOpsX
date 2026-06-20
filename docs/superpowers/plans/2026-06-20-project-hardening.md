# RemoteOpsX Completion and Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the current RemoteOpsX tree compile, complete FTP, repair runbook and terminal races, harden persistence and CSP, and add regression coverage.

**Architecture:** Preserve the React/Tauri and system-tool adapters. Extract deterministic runbook bookkeeping into a frontend state-machine module, keep transport port resolution in Rust model helpers, and make migrations additive so existing databases remain usable.

**Tech Stack:** React 18, TypeScript, Zustand, Vitest, Tauri 2, Rust, rusqlite, system OpenSSH/curl.

---

## File map

- `src/runbookMachine.ts`: pure runbook execution state transitions.
- `src/runbookMachine.test.ts`: regression tests for confirmation, skip, accumulation, and restart.
- `src/components/RunbookRunner.tsx`: async orchestration and rendering around the state machine.
- `src-tauri/src/models.rs`: protocol-specific port fields and resolution helpers.
- `src-tauri/src/database.rs`: additive port migration and failure-safe profile persistence helpers.
- `src-tauri/src/ftp_manager.rs`: compile-safe curl argument construction and exact-path downloads.
- `src-tauri/src/sftp_manager.rs`: exact-path downloads.
- `src-tauri/src/pty_manager.rs`, `src/components/TerminalTab.tsx`: ready handshake that prevents initial event loss.
- `src/components/ServerForm.tsx`, `src/components/CommandPalette.tsx`, `src/components/SftpPanel.tsx`, `src/api.ts`, `src/types.ts`: completed FTP/port/download UI contracts.
- `src-tauri/tauri.conf.json`: production CSP.
- `package.json`, `vite.config.ts`: frontend test runner and chunk splitting.

### Task 1: Repair and test the FTP adapter

**Files:**
- Modify: `src-tauri/src/ftp_manager.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add failing unit tests**

Add `#[cfg(test)]` tests for directory normalization, percent encoding, LIST parsing, and quote argument ownership. The quote test calls a new pure `quote_args(&[String]) -> Vec<String>` and expects alternating `--quote` values.

- [ ] **Step 2: Verify the current failure**

Run: `cargo test --manifest-path src-tauri/Cargo.toml ftp_manager::tests`

Expected: compilation fails at the existing borrowed `owned_args` implementation.

- [ ] **Step 3: Implement owned curl arguments**

Change `run_curl` to accept `&[String]`; build every argument, including URLs and quote commands, as owned `String` values. Implement:

```rust
fn quote_args(quotes: &[String]) -> Vec<String> {
    let mut args = vec!["--fail", "--silent", "--show-error", "--path-as-is"]
        .into_iter().map(str::to_owned).collect::<Vec<_>>();
    for quote in quotes {
        args.push("--quote".into());
        args.push(quote.clone());
    }
    args
}
```

- [ ] **Step 4: Run focused tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml ftp_manager::tests`

Expected: all FTP unit tests pass.

### Task 2: Add protocol-specific ports with compatible migration

**Files:**
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/database.rs`
- Modify: `src/types.ts`
- Modify: `src/components/ServerForm.tsx`
- Modify: `src-tauri/src/ftp_manager.rs`
- Modify: `src-tauri/src/rdp_adapter.rs`
- Modify: `src-tauri/src/vnc_adapter.rs`

- [ ] **Step 1: Add failing Rust tests for port resolution**

Tests must assert `ftp_port() == 21`, `rdp_port() == 3389`, and `vnc_port() == 5900` when optional ports are absent, and return explicit configured values when present.

- [ ] **Step 2: Add optional model fields and helpers**

Add `ftp_port`, `rdp_port`, and `vnc_port` as `Option<u16>` to `Server` and `ServerInput`; add methods that apply standard defaults. Mirror optional fields in TypeScript.

- [ ] **Step 3: Add an idempotent SQLite migration**

Use `PRAGMA table_info(servers)` to conditionally run:

```sql
ALTER TABLE servers ADD COLUMN ftp_port INTEGER;
ALTER TABLE servers ADD COLUMN rdp_port INTEGER;
ALTER TABLE servers ADD COLUMN vnc_port INTEGER;
```

Update row mapping and insert/update statements.

- [ ] **Step 4: Wire adapters and form fields**

FTP, RDP, and VNC adapters use the model helpers. The form exposes the relevant port only when that protocol is enabled and submits `null` for disabled protocols.

- [ ] **Step 5: Run model and database tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml models database`

Expected: port defaults and migration tests pass.

### Task 3: Complete FTP and exact destination downloads

**Files:**
- Modify: `src/components/ServerForm.tsx`
- Modify: `src/components/CommandPalette.tsx`
- Modify: `src/components/SftpPanel.tsx`
- Modify: `src/api.ts`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/sftp_manager.rs`
- Modify: `src-tauri/src/ftp_manager.rs`

- [ ] **Step 1: Complete FTP discovery**

Add `ftp` to `ALL_PROTOCOLS` and `SERVER_ACTIONS`, requiring the FTP protocol flag. Label FTP as insecure/plaintext in the form and file panel.

- [ ] **Step 2: Change download contracts**

Rename frontend/backend `localDir` parameters to `localPath`. SFTP passes the exact path as the final `scp` argument; FTP passes it to curl `--output`.

- [ ] **Step 3: Honor the save-dialog destination**

Pass `dest` directly from `SftpPanel` instead of deriving its parent directory.

- [ ] **Step 4: Verify frontend types and Rust compile**

Run: `npm run build && cargo check --manifest-path src-tauri/Cargo.toml`

Expected: both exit successfully.

### Task 4: Make credential persistence failure-safe

**Files:**
- Modify: `src-tauri/src/database.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/components/ServerForm.tsx`

- [ ] **Step 1: Add database transaction tests**

Create an in-memory database test that verifies server plus credential metadata commit together and that auth-mode changes remove obsolete credential metadata.

- [ ] **Step 2: Separate validation from mutation**

Validate non-empty name/host/username, port ranges, auth type, protocol names, and protocol ports before any write.

- [ ] **Step 3: Add compensating keyring flow**

For a supplied password, write the keyring first, then commit server and credential metadata in one SQLite transaction. If the transaction fails, delete the newly written keyring entry. For key auth, remove credential metadata and best-effort delete the prior password. Do not offer a key-passphrase field.

- [ ] **Step 4: Run persistence tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml database`

Expected: transaction and auth-change tests pass.

### Task 5: Replace RunbookRunner bookkeeping with a tested state machine

**Files:**
- Create: `src/runbookMachine.ts`
- Create: `src/runbookMachine.test.ts`
- Modify: `src/components/RunbookRunner.tsx`
- Modify: `src/types.ts`
- Modify: `package.json`

- [ ] **Step 1: Configure Vitest and write failing tests**

Add `test: "vitest run"`. Tests cover: first confirmation pauses before execution; confirm resumes the same run; skip records a skipped result and advances; results before a gate remain accumulated; rerun resets timestamp/results; current variables are substituted at run start.

- [ ] **Step 2: Run tests to verify failure**

Run: `npm test`

Expected: failure because `runbookMachine.ts` does not exist.

- [ ] **Step 3: Implement pure transitions**

Define `RunState` with `startedAt`, `steps`, `cursor`, `results`, `overall`, `pendingConfirmation`, and `phase`. Export `createRun`, `nextAction`, `confirmStep`, `skipStep`, and `recordResult` without React dependencies.

- [ ] **Step 4: Refactor RunbookRunner**

Drive one async loop from state refs so confirmation does not create a new timestamp or result array. Persist all results once phase reaches `complete`. Render skipped steps explicitly.

- [ ] **Step 5: Run frontend tests and build**

Run: `npm test && npm run build`

Expected: all state-machine tests pass and TypeScript builds.

### Task 6: Remove the terminal event race

**Files:**
- Modify: `src/components/TerminalTab.tsx`
- Modify: `src-tauri/src/pty_manager.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Define a listener-first handshake**

Register output and exit listeners before invoking `pty_spawn`. Keep a `spawned` flag so cleanup only closes sessions that were requested, and always unregister listeners when disposed.

- [ ] **Step 2: Surface write/resize failures**

Deduplicate user-visible terminal I/O errors instead of silently swallowing every rejection.

- [ ] **Step 3: Verify frontend and backend builds**

Run: `npm run build && cargo check --manifest-path src-tauri/Cargo.toml`

Expected: both pass.

### Task 7: Harden CSP, tunnels, and bundle output

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/src/tunnel_manager.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `vite.config.ts`

- [ ] **Step 1: Add tunnel validation tests**

Extract pure validation and test empty IDs, invalid type, zero ports, missing remote ports, and accepted dynamic/local/remote tunnels.

- [ ] **Step 2: Validate before spawn and confirm startup**

Reject invalid tunnels before `Command::spawn`; briefly poll `try_wait` so immediate authentication/forwarding failures are returned instead of stored as active.

- [ ] **Step 3: Enable production CSP**

Set default/script/font/img/style/connect directives for bundled assets and Tauri IPC. Allow inline styles because the existing React code uses style props; do not enable remote scripts or shell execution.

- [ ] **Step 4: Split stable vendor chunks**

Configure Rollup manual chunks for React, Tauri, Zustand, and xterm. Keep the output deterministic and verify the 500 KB warning is removed.

- [ ] **Step 5: Build frontend and inspect output**

Run: `npm run build`

Expected: successful build with no chunk larger than 500 KB.

### Task 8: Full verification and documentation alignment

**Files:**
- Modify: `README.md`
- Modify: `TODO.md`

- [ ] **Step 1: Align documentation**

Document FTP/curl, protocol-specific ports, plaintext FTP warning, password-only keyring behavior, CSP, frontend tests, and the remaining native-transport limitations. Update checklist items only when verification proves them.

- [ ] **Step 2: Run formatting available in the repository**

Run: `npx prettier --check` only if Prettier is already installed. Run `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings` only if those Rust components are available; otherwise report them as not run.

- [ ] **Step 3: Run the full verification suite**

```bash
npm test
npm run build
npm audit
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
git diff --check
```

Expected: all commands exit 0. The ignored Docker SSH integration test remains explicitly reported unless Docker is available and it is run.

- [ ] **Step 4: Rendered smoke test**

Use the configured browser runner to verify app load, command palette, new-server FTP fields, and modal layout. If no runner is installed, report the check as not run without installing a browser implicitly.
