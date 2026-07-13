# SSH Key Install Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users pick local SSH private keys from `~/.ssh`, add external key paths manually, and install the matching public key into a remote server's `~/.ssh/authorized_keys` without copying private keys to the server.

**Architecture:** Add a focused Rust `ssh_keys` module for local key discovery, public key resolution, and safe install command construction. Expose Tauri commands through `lib.rs`, add typed frontend wrappers, and extend `ServerForm` with key selection plus an explicit install action.

**Tech Stack:** Rust/Tauri commands, system `ssh-keygen`, existing `ssh_manager::run_remote`, React/TypeScript server profile modal, Vitest and Cargo unit tests.

---

### Task 1: Backend SSH Key Utilities

**Files:**
- Create: `src-tauri/src/ssh_keys.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/ssh_keys.rs`

- [ ] Write tests for filtering private-key candidates, resolving `.pub` files, and building an idempotent `authorized_keys` install command.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml ssh_keys`.
- [ ] Implement `SshKeyInfo`, `discover_local_keys`, `public_key_for_private_key`, and `authorized_keys_install_command`.
- [ ] Expose `ssh_keys_list` and `ssh_key_install` Tauri commands.

### Task 2: Frontend API and Server Form

**Files:**
- Modify: `src/types.ts`
- Modify: `src/api.ts`
- Modify: `src/components/ServerForm.tsx`
- Test: `src/settings.test.ts` or existing frontend tests if contracts are touched.

- [ ] Add `SshKeyInfo` TypeScript type and `sshKeysList` / `sshKeyInstall` API wrappers.
- [ ] Load discovered keys when key auth is selected.
- [ ] Render a key dropdown, keep manual path entry, and add an explicit “Install public key on server” action.
- [ ] Show success/error feedback without exposing private-key content.

### Task 3: Verification and Publishing

**Files:**
- Existing changed files only.

- [ ] Run `npm test`.
- [ ] Run `npm run build`.
- [ ] Run `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml`.
- [ ] Run `git diff --check`.
- [ ] Commit and push to `codex/project-hardening-packaging`.
- [ ] Comment on PR #1 with validation and remaining manual smoke steps.
