# RemoteOpsX MVP Finish Handoff Plan

**Goal:** Finish the local MVP branch with settings persistence, updated documentation, validation, and refreshed Graphify artifacts.

## Steps

- [x] Inspect repository, roadmap, specs, package scripts, existing graph output, and implementation coverage.
- [x] Run baseline validation and identify the Vitest discovery leak from `.worktrees/`.
- [x] Add typed backend errors and settings modules.
- [x] Persist validated settings in SQLite via the `app_settings` singleton table.
- [x] Add frontend settings contracts, normalized errors, rollback-safe settings store, and regression tests.
- [x] Add Settings UI access from the top bar and command palette.
- [x] Apply theme settings and move health refresh interval ownership to persisted settings.
- [x] Constrain Vitest discovery to root `src/**/*.test.ts(x)` files.
- [x] Update README and TODO with settings, validation ranges, DB location, test commands, specs/plans, and graphify handoff.
- [x] Regenerate Graphify outputs from the final tree.
- [x] Run final validation: frontend tests/build, Rust tests/fmt, diff whitespace check.

## Follow-up manual smoke

- [ ] Run `npm run app:dev` on a Linux desktop with Tauri dependencies.
- [ ] Verify Settings open/close/focus behavior from top bar and command palette.
- [ ] Save theme/ports/refresh settings, restart, and verify persistence.
- [ ] Verify invalid values surface `validation.invalid_value` and roll back safely.
