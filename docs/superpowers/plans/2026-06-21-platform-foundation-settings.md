# Platform Foundation and Settings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add typed error/settings contracts, persist validated settings in SQLite, and provide a settings UI that applies theme and refresh preferences immediately.

**Architecture:** Rust owns validation and persistence. Tauri commands return a serializable `DomainError`; TypeScript normalizes IPC failures into `RemoteOpsError`. A focused Zustand store owns the frontend settings lifecycle.

**Tech Stack:** Rust 2021, Tauri 2, serde, rusqlite, React 18, TypeScript, Zustand, Vitest.

---

## File map

- Create `src-tauri/src/error.rs` and `src-tauri/src/settings.rs`.
- Modify `src-tauri/src/database.rs` and `src-tauri/src/lib.rs`.
- Create `src/errors.ts`, `src/settings.ts`, `src/settingsStore.ts`, and `src/settings.test.ts`.
- Create `src/components/SettingsModal.tsx`.
- Modify `src/api.ts`, `src/App.tsx`, `src/store.ts`, `src/components/CommandPalette.tsx`, and `src/styles.css`.
- Update `README.md` and `TODO.md` only after acceptance passes.

### Task 1: Stable backend error contract

**Files:** Create `src-tauri/src/error.rs`; modify `src-tauri/src/lib.rs`; test in `src-tauri/src/error.rs`.

- [x] Write failing tests proving validation errors serialize `code`, `retryable`, `correlation_id`, and `context.field`, while an internal error created from `"secret-canary-value"` never serializes that value.

- [x] Run `cargo test --manifest-path src-tauri/Cargo.toml error::tests` and verify compilation fails because `DomainError` is absent.

- [x] Implement this contract:

```rust
use std::collections::BTreeMap;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct DomainError {
    pub code: &'static str,
    pub message: String,
    pub retryable: bool,
    pub correlation_id: String,
    pub context: BTreeMap<String, String>,
}

pub type CommandResult<T> = Result<T, DomainError>;

impl DomainError {
    pub fn validation(field: &str, message: &str) -> Self {
        Self { code: "validation.invalid_value", message: message.into(), retryable: false,
            correlation_id: uuid::Uuid::new_v4().to_string(),
            context: BTreeMap::from([("field".into(), field.into())]) }
    }
    pub fn internal(error: impl std::fmt::Display) -> Self {
        eprintln!("remoteopsx internal error: {error}");
        Self { code: "internal.unexpected", message: "An internal operation failed".into(),
            retryable: false, correlation_id: uuid::Uuid::new_v4().to_string(), context: BTreeMap::new() }
    }
}
```

- [x] Declare `pub mod error`, change `e()` to map through `DomainError::internal`, and replace command return types `Result<T, String>` with `CommandResult<T>`. Direct user-input failures use `DomainError::validation`; operational failures use `e`.

- [x] Run the focused tests and `cargo test --manifest-path src-tauri/Cargo.toml`; both must exit 0.

- [ ] Commit and push:

```bash
git add src-tauri/src/error.rs src-tauri/src/lib.rs
git commit -m "feat: add typed backend error contract"
git push
```

### Task 2: Typed settings and SQLite persistence

**Files:** Create `src-tauri/src/settings.rs`; modify `src-tauri/src/database.rs` and `src-tauri/src/lib.rs`; test both Rust modules.

- [x] Write failing tests asserting system theme, ports 22/21/3389/5900, 3000 ms refresh, and rejection of refresh below 1000 ms and port zero.

- [x] Run `cargo test --manifest-path src-tauri/Cargo.toml settings::tests`; verify RED because `AppSettings` is absent.

- [x] Implement serde snake-case enums `Theme { System, Dark, Light }` and `TransferConflictPolicy { Ask, Overwrite, Rename, Skip }`, plus:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefaultPorts { pub ssh: u16, pub ftp: u16, pub rdp: u16, pub vnc: u16 }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSettings {
    pub schema_version: u32,
    pub theme: Theme,
    pub default_ports: DefaultPorts,
    pub health_refresh_interval_ms: u64,
    pub history_retention_days: u32,
    pub app_lock_timeout_minutes: u32,
    pub transfer_conflict_policy: TransferConflictPolicy,
    pub desktop_clipboard_enabled: bool,
    pub desktop_audio_enabled: bool,
    pub desktop_notifications_enabled: bool,
}
```

- [x] Implement defaults `1`, `system`, `22/21/3389/5900`, `3000`, `90`, `15`, `ask`, and three enabled booleans. `validate()` accepts refresh `1000..=60000`, retention `1..=3650`, timeout `1..=1440`, and nonzero ports; failures return `DomainError::validation` with the exact field path.

- [x] Write a failing database test: empty DB returns defaults; save light theme and 5000 ms; reload equals saved value; `app_settings` contains exactly one row.

- [x] Add this migration:

```sql
CREATE TABLE IF NOT EXISTS app_settings (
  singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
  schema_version INTEGER NOT NULL,
  value_json TEXT NOT NULL CHECK (length(value_json) <= 65536),
  updated_at TEXT NOT NULL
);
```

- [x] Implement `load_settings`: select `value_json`, return defaults on `QueryReturnedNoRows`, otherwise deserialize and validate. Implement `save_settings`: validate, serialize, then atomically upsert singleton row 1 inside `unchecked_transaction()`.

- [ ] Run settings/database suites; commit and push:

```bash
cargo test --manifest-path src-tauri/Cargo.toml settings::tests
cargo test --manifest-path src-tauri/Cargo.toml database::tests
git add src-tauri/src/settings.rs src-tauri/src/database.rs src-tauri/src/lib.rs
git commit -m "feat: persist typed application settings"
git push
```

### Task 3: Settings IPC and frontend contracts

**Files:** Modify `src-tauri/src/lib.rs` and `src/api.ts`; create `src/errors.ts`, `src/settings.ts`, and `src/settings.test.ts`.

- [x] Write failing Vitest cases: nested port patches do not mutate defaults; a structured backend rejection becomes `RemoteOpsError` retaining code and correlation ID; unknown objects become `client.unknown`.

- [x] Run `npm test -- src/settings.test.ts`; verify RED because the new modules are absent.

- [x] Mirror Rust settings types in `settings.ts`, export immutable `DEFAULT_SETTINGS`, and implement `patchSettings(current, patch)` with a nested `default_ports` merge.

- [x] Implement the frontend error type:

```ts
export class RemoteOpsError extends Error {
  constructor(message: string, readonly code: string, readonly retryable: boolean,
    readonly correlationId: string | null, readonly context: Record<string, string>) {
    super(message); this.name = "RemoteOpsError";
  }
}

export function normalizeRemoteError(value: unknown): RemoteOpsError {
  if (value instanceof RemoteOpsError) return value;
  if (value instanceof Error) return new RemoteOpsError(value.message, "client.error", false, null, {});
  if (typeof value === "object" && value !== null) {
    const p = value as Record<string, unknown>;
    if (typeof p.code === "string" && typeof p.message === "string")
      return new RemoteOpsError(p.message, p.code, p.retryable === true,
        typeof p.correlation_id === "string" ? p.correlation_id : null,
        (p.context as Record<string, string>) ?? {});
  }
  return new RemoteOpsError("An unknown operation failed", "client.unknown", false, null, {});
}
```

- [x] Add `settings_get` and `settings_save` commands. Both lock the DB safely; save validates before persistence and returns the saved value. Register both in `generate_handler!`.

- [x] Replace direct Tauri invoke in `api.ts` with one local generic wrapper that catches and throws `normalizeRemoteError`. Add `settingsGet()` and `settingsSave(settings)`.

- [ ] Verify, commit, and push:

```bash
npm test -- src/settings.test.ts
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
git add src-tauri/src/lib.rs src/api.ts src/errors.ts src/settings.ts src/settings.test.ts
git commit -m "feat: expose typed settings contracts"
git push
```

### Task 4: Settings state and UI

**Files:** Create `src/settingsStore.ts` and `src/components/SettingsModal.tsx`; modify `src/App.tsx`, `src/store.ts`, `src/components/CommandPalette.tsx`, and `src/styles.css`; extend `src/settings.test.ts`.

- [x] Write a failing state test using injected API functions: load dark settings, patch light, make save throw `disk full`, then assert state rolls back to dark, becomes clean, and retains a normalized error.

- [x] Implement a focused Zustand store with `settings`, `persisted`, `loading`, `saving`, `dirty`, `error`, `load`, `patch`, `reset`, and `save`. Export the injected state-machine factory used by the test. Save snapshots persisted state and restores it before rethrowing on failure.

- [x] Implement `SettingsModal` controlled fields for theme, four ports, refresh seconds, retention days, lock timeout, conflict policy, clipboard, audio, and notifications. Disable Save while clean/loading/saving. Display code and correlation ID. Close only after successful save or confirmed discard.

- [x] In `App.tsx`, load once and apply `data-theme`; system mode follows `matchMedia("(prefers-color-scheme: dark)")`. Add top-bar and palette Settings actions.

- [x] Remove `healthIntervalMs` ownership from `store.ts`; health polling reads `health_refresh_interval_ms` from the settings store. Add complete light-theme variables and responsive settings styles.

- [ ] Verify, commit, and push:

```bash
npm test
npm run build
git add src/settingsStore.ts src/components/SettingsModal.tsx src/App.tsx src/store.ts src/components/CommandPalette.tsx src/styles.css src/settings.test.ts
git commit -m "feat: add persistent application settings UI"
git push
```

### Task 5: Acceptance and roadmap update

**Files:** Modify `README.md`, `TODO.md`, and this plan.

- [x] Run full acceptance:

```bash
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml
git diff --check
```

- [ ] Run `npm run app:dev`; change theme, ports, and refresh interval; restart and verify persistence. Submit an invalid refresh and verify `validation.invalid_value` appears without changing persisted state.

- [x] Document fields, defaults, ranges, and DB location in `README.md`. Change only the Settings line in `TODO.md` to `✅`. Mark completed plan checkboxes `[x]`.

- [ ] Commit, push, and compare remote SHA:

```bash
git add README.md TODO.md docs/superpowers/plans/2026-06-21-platform-foundation-settings.md
git commit -m "docs: complete platform settings milestone"
git push
git status -sb
git ls-remote origin "refs/heads/$(git branch --show-current)"
```

Expected: clean worktree and remote SHA equals local HEAD.
