# Contributing to RemoteOpsX

RemoteOpsX is an operations tool. Changes that affect authentication, host identity, command execution, file transfer, tunnels, secrets, packaging, or persisted state are treated as production-sensitive.

## Pull requests

Use a short Conventional Commit-style PR title. Supported prefixes are:

- `feat:` new user-facing capability
- `fix:` bug fix
- `security:` security hardening
- `perf:` performance improvement
- `refactor:` behavior-preserving restructuring
- `test:` test-only work
- `docs:` documentation-only work
- `build:` build or dependency work
- `ci:` CI/release automation
- `chore:` maintenance
- `revert:` revert of an earlier change

The repository uses squash merges. The PR title therefore becomes the durable release-history subject and must describe the complete change.

Before review, run:

```bash
npm ci
npm run version:check
npm audit --audit-level=high
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --locked --manifest-path src-tauri/Cargo.toml
cargo build --locked --manifest-path src-tauri/Cargo.toml
```

Do not merge a production-sensitive change while any required CI job is failing.

## Versioning

`package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json` must always contain the same version. `npm run version:check` enforces this contract and release CI additionally requires the tag to equal `v<version>`.

RemoteOpsX follows Semantic Versioning. For `0.2.0`, the promotion path is:

```text
0.2.0-alpha.N -> 0.2.0-beta.N -> 0.2.0-rc.N -> 0.2.0
```

Promotion criteria:

- `alpha`: feature/security work may still be incomplete; no compatibility promise.
- `beta`: P0 feature/security scope is complete; integration and packaging validation may still find defects.
- `rc`: release scope is frozen except for release-blocking defects; full test and packaging gates are expected to pass.
- stable: all release gates pass from the exact tagged commit and published artifacts are the tested artifacts.

## Changelog and release notes

Keep `CHANGELOG.md` focused on user-visible, operational, security, compatibility, and migration changes. GitHub release notes are generated automatically from merged PRs; the changelog is the curated product-level record.

## Security invariants

- Never persist plaintext passwords in SQLite.
- Never put credentials in process argv.
- Never bypass a changed SSH host key to make a connection succeed.
- Production hosts must not silently trust a first-seen SSH identity.
- Destructive operations must preserve explicit confirmation boundaries.
- Any output or diagnostic export that can contain a known secret must pass through the central redaction layer before persistence or UI delivery.
