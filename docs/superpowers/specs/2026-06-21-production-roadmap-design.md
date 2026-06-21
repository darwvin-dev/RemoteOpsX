# RemoteOpsX Production Roadmap Design

## Goal

Complete every unfinished item in `TODO.md` as production-grade Linux software. Replace process-based remote transports with a native `russh` stack, embed RDP and VNC, add durable operations features, and ship reproducible signed bundles. Existing local data does not require backward compatibility, so schema and command contracts may be replaced rather than migrated from the current development database.

## Delivery model

Work proceeds foundation-first on `codex/project-hardening-packaging`. Every milestone has its own tests, documentation update, commit, and push. A roadmap checkbox changes to complete only after its acceptance tests pass. Commits never contain partially implemented adjacent milestones.

The implementation milestones are:

1. Platform foundation and persistent settings
2. Native secure SSH transport, known-hosts management, and app lock
3. Persistent SFTP and resilient tunnels
4. Health history, alerting, runbook authoring/scheduling, session history, and snippets
5. Embedded RDP and VNC
6. Packaging, signing, CI matrix, and supply-chain outputs
7. Integration, parser, recovery, and secret-masking quality gates

## Architecture

### Frontend feature modules

React is split by product capability: connections, files, desktop, health, automation, history, snippets, and settings. Feature modules call a typed API client and consume versioned events. They never import transport-specific details or hold native resource pointers.

The global store retains navigation and lightweight cached view state. Long-running operation state belongs to feature-specific stores keyed by opaque job or session IDs. This prevents a single Zustand store from becoming the lifecycle owner for every backend resource.

### Typed IPC boundary

Every command accepts a versioned request and returns a typed response or `DomainError`. Long-running work returns a `JobId` immediately and publishes progress, completion, cancellation, and recovery events. Event payloads carry a schema version and request correlation ID.

High-bandwidth desktop frames and audio do not cross JSON IPC. A bounded native buffer feeds a rendering bridge; JSON IPC controls lifecycle, input, display metadata, and error reporting.

### Rust application services

The Tauri command surface delegates to focused services:

- `SessionService`: SSH connection lifecycle, PTY, exec channels, jump hosts, pooling, and host-key decisions.
- `TransferService`: persistent SFTP channels, recursive transfer jobs, progress, cancellation, chmod, and conflict policy.
- `TunnelService`: local, remote, and dynamic forwards with probes, reconnect policy, and autostart.
- `DesktopService`: embedded FreeRDP and VNC workers, frame/audio/input bridges, resize, clipboard, and teardown.
- `HealthService`: collection, retention, thresholds, aggregation, and alert dispatch.
- `AutomationService`: runbook validation, variable resolution, dry-run, scheduling, and resumable execution.
- `HistoryService`: session and audit queries with retention controls.
- `SettingsService`: typed settings, defaults, validation, and change events.
- `VaultService`: app-lock key derivation, encrypted secret envelopes, keyring integration, unlock state, and zeroization.

Each active connection or desktop instance runs as an actor with a bounded mailbox and cancellation token. Managers keep registries of opaque handles. Shutdown drains jobs within a deadline and then force-closes remaining native resources.

## Native SSH and SFTP

`russh` is the primary SSH implementation. The application owns TCP connection setup, negotiated algorithms, host-key verification, password/public-key/agent authentication, encrypted-key passphrase prompts, jump-host chains, keepalives, exec channels, PTYs, SFTP channels, and forwarding channels. Production code does not invoke `ssh`, `scp`, or `sshpass`.

Known hosts are stored in SQLite with host, port, algorithm, fingerprint, first-seen time, last-seen time, trust state, and replacement history. Unknown and changed keys block connection establishment until the user explicitly accepts or rejects them. Rotation preserves the old fingerprint in audit history.

Connections are pooled per effective endpoint and authentication identity. Pool entries have idle expiry, health checks, bounded channel counts, and deterministic invalidation after auth or host-key changes.

SFTP keeps a subsystem channel open per active file session. Transfers are durable jobs with byte progress, speed, ETA, cancellation, retry classification, conflict policy, and temporary-file atomic completion. Recursive upload/download, drag-and-drop, and chmod use the same job model.

## App lock and secrets

App lock is optional. When enabled, Argon2id derives a wrapping key from the master password using per-install salt and calibrated memory/time parameters. The wrapping key decrypts a random vault key; the vault key encrypts secret envelopes with an authenticated cipher. The operating-system keyring may store only the wrapped vault material and installation identity, never an unwrapped master or vault key.

Unlock state exists only in locked memory where the platform permits it and is zeroized on lock, timeout, suspend, and process exit. Failed unlocks use exponential delay. Password changes rewrap the vault key without re-encrypting every secret. Recovery is an explicit destructive reset because no recoverable copy of the master password exists.

## Embedded desktop protocols

FreeRDP and the selected VNC client library are pinned and built as bundled native dependencies. Builds record exact source revisions and licenses. Rust FFI wrappers expose owned session objects and translate callbacks into bounded frame, audio, clipboard, and status channels.

Frames use a bounded latest-frame queue so a slow WebView cannot exhaust memory. The renderer negotiates dimensions and pixel format and drops stale frames under pressure. Input is rate-limited and validated. Clipboard directions are independently configurable. Credentials are delivered through in-memory native APIs and never command-line arguments.

Native crashes and protocol failures terminate only the affected desktop session, release buffers, and produce a redacted `DomainError`. Sanitizer-enabled native integration jobs exercise connect, resize, input, reconnect, and teardown.

## Operations features

### Health history and alerts

Samples are stored per server with configurable retention and downsampling. Threshold definitions are typed, scoped globally or per server/tag, and versioned. Alert state uses hysteresis and deduplication to avoid flapping. Desktop notifications and signed webhook deliveries share an outbox with retry limits and audit status.

### Runbooks

The editor provides structured YAML-backed forms, schema validation, variable declarations, confirmation policy, and exact command preview. Dry-run resolves variables and renders steps without executing them. Imports are validated before persistence; exports are deterministic.

Schedules persist with timezone and misfire policy. The scheduler leases due runs transactionally so only one execution starts. A failed run may resume from a selected step only when prerequisite and confirmation rules pass. Execution remains append-only and auditable.

### Sessions and snippets

Session history exposes protocol, server, timestamps, outcome, and redacted failure metadata with filters and retention controls. It never stores terminal contents by default.

Snippets are user-editable, tagged, searchable, and optionally scoped to server tags. Broadcast creates one tracked execution per target, requires an exact target/command confirmation, limits concurrency, and presents per-target results. Secrets are masked before persistence or display.

### Tunnel resilience

Tunnel profiles define autostart, reconnect bounds, and health probes. Dynamic SOCKS tunnels perform an end-to-end proxy probe rather than checking only the listening socket. Reconnect uses capped exponential backoff with jitter and stops on non-retryable authentication or host-key errors.

## Settings and persistence

SQLite is the source of truth for settings, known hosts, history, thresholds, schedules, snippets, jobs, and audit events. The schema is rebuilt for the production model because compatibility with development data is not required. Foreign keys, uniqueness constraints, and bounded text/blob sizes enforce invariants.

Settings cover theme, default protocol ports, refresh intervals, retention, app-lock timeout, transfer behavior, desktop clipboard/audio policy, and notification routing. Validation happens in Rust. Frontend optimistic changes roll back when persistence fails.

## Error handling and observability

`DomainError` contains a stable code, safe user message, retryability, correlation ID, and redacted context. Internal causes stay in structured logs. Secret-bearing types cannot implement unrestricted debug formatting.

Logs are structured and bounded by retention. Metrics cover active sessions, reconnects, transfer throughput, job failures, queue pressure, alert delivery, and scheduler lag. Diagnostic bundles apply the same masking engine as terminal and log views.

## Packaging and supply chain

Arch, AppImage, and Flatpak outputs bundle pinned transport and desktop native dependencies. AppImage and release metadata are signed in CI. Builds generate checksums, SBOMs, license inventories, and provenance attestations. Release jobs fail on unapproved licenses, vulnerable locked dependencies above the configured severity gate, missing signatures, or non-reproducible bundle inputs.

CI runs supported build/test jobs for Arch, Ubuntu, Debian, and Fedora. Container images are pinned by digest. Native dependencies are cached by content hash, while final artifacts are always rebuilt and verified.

## Testing strategy

- Rust unit tests cover parsers, validation, state machines, retry classification, migrations, masking, and cryptographic envelope behavior.
- Property and fuzz tests feed health parsers, protocol parsers, YAML validation, and masking with malformed and adversarial input.
- SSH integration tests start an isolated server fixture and exercise host-key unknown/change/rotation, auth methods, jump hosts, PTY, exec, SFTP, forwarding, reconnect, and cancellation.
- Desktop integration tests use controlled RDP and VNC fixtures and verify frame delivery, input, clipboard, resize, reconnect, and cleanup.
- Frontend tests cover editors, progress/cancellation, lock transitions, history filters, alert configuration, and partial runbook reruns.
- Packaging smoke tests install, launch, and remove every artifact in clean distribution containers.
- Recovery tests kill the app during transfers, scheduled runs, and active sessions, then verify deterministic cleanup or resumption.
- Secret canary tests inject recognizable values and fail if they appear in logs, events, database fields, diagnostics, process arguments, or UI snapshots.

## Roadmap coverage

| TODO item | Owning milestone |
| --- | --- |
| Native SSH transport | 2 |
| Known-hosts UI | 2 |
| App lock | 2 |
| Persistent SFTP, progress, drag/drop, recursive, chmod | 3 |
| Embedded RDP and VNC | 5 |
| Health retention, thresholds, notifications/webhooks | 4 |
| Runbook editor, variables, dry-run, import/export, scheduling, partial rerun | 4 |
| Tunnel reconnect, autostart, SOCKS health | 3 |
| Sessions history | 4 |
| Snippets and broadcast | 4 |
| pacman, signed AppImage, Flatpak | 6 |
| CI distribution matrix | 6 |
| Settings persistence | 1 |
| Health parser tests | 7 |
| Live SSH integration tests | 7 |
| Secret masking | 7 |

## Acceptance rule

A milestone is complete only when its focused tests, the full frontend suite, the full Rust suite, production build, formatting/static checks, documentation, and relevant packaging or fixture smoke tests pass from a clean checkout. The commit is then pushed and its TODO entries are marked complete in the same commit.
