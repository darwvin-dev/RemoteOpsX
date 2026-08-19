# Operator Data Plane

The operator data plane is the durable local state behind RemoteOpsX health, alerts, transfers, multi-host operations, tunnel resilience, and workspace backup.

## Health and alerts

Health is sampled over the existing strict SSH transport and persisted at a bounded 30-second cadence. Per-server history is retained for seven days. Alert rules support consecutive-sample gates and cooldowns; fired events and acknowledgements persist in SQLite.

## Transfers

SFTP workflows use OpenSSH/`scp` with the same app-managed host-key policy as terminals and tunnels. ControlMaster sessions may be reused for a server route. Jobs are cancellable and recursive transfers are supported; single-file jobs expose byte progress when the local file size is available.

## Multi-host safety

Broadcast commands are limited to 50 targets and concurrency 1–8. Production targets require explicit production confirmation, and destructive command families require a separate confirmation. Every completed host result is persisted.

## Tunnel resilience

Tunnel policy is desired state. Autostart and auto-reconnect reconcile persisted tunnel descriptors with live SSH child processes; explicit stop remains authoritative.

## Workspace backup

Backups are versioned and encrypted with OpenSSL AES-256-CBC plus PBKDF2. The passphrase is supplied through environment state rather than process argv. Keyring credentials are never exported. Restore validates first, applies SQLite data transactionally, restores tunnels inert, and clears stale local credentials for every restored server ID so password profiles require explicit credential re-entry.
