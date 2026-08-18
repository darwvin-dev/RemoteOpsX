//! Startup crash/restart reconciliation.
//!
//! PTYs and tunnel child handles are process-local. If the app exits before a
//! normal close path updates SQLite, records must not remain "active" forever
//! on the next launch. Existing keyring credentials are loaded into the central
//! redaction registry during startup, then persisted user-controlled text is
//! scrubbed so historical data follows the same secret boundary as new writes.

use anyhow::{Context, Result};
use rusqlite::{params, Connection};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoverySummary {
    pub sessions_interrupted: usize,
    pub tunnels_stopped: usize,
    pub runbooks_interrupted: usize,
    pub secrets_preloaded: usize,
    /// Number of persisted text fields whose logical SQLite value was changed
    /// because it contained a known keyring secret.
    pub persisted_fields_scrubbed: usize,
}

pub fn reconcile_startup(conn: &Connection) -> Result<RecoverySummary> {
    let now = chrono::Utc::now().to_rfc3339();
    let transaction = conn
        .unchecked_transaction()
        .context("failed to begin startup reconciliation")?;

    let sessions_interrupted = transaction.execute(
        "UPDATE sessions
         SET status = 'interrupted', ended_at = COALESCE(ended_at, ?1)
         WHERE ended_at IS NULL OR status IN ('active', 'open', 'running')",
        params![now],
    )?;

    // Tunnel processes belong to the previous app process and are not present
    // in the new in-memory TunnelManager registry. Never claim they are active.
    let tunnels_stopped = transaction.execute(
        "UPDATE tunnels SET status = 'stopped' WHERE status IN ('active', 'starting')",
        [],
    )?;

    let runbooks_interrupted = transaction.execute(
        "UPDATE runbook_runs
         SET status = 'interrupted', ended_at = COALESCE(ended_at, ?1)
         WHERE ended_at IS NULL AND status IN ('active', 'open', 'running')",
        params![now],
    )?;

    transaction.commit()?;

    // Keyring availability is an operational/runtime concern reported by
    // Runtime Preflight. A locked or unavailable keyring must not make the app
    // fail to start; preload every credential we can read and leave unavailable
    // entries for the preflight/connection diagnostics to report.
    let secrets_preloaded = preload_known_secrets(conn).unwrap_or(0);
    let persisted_fields_scrubbed = scrub_persisted_known_secrets(conn)?;

    Ok(RecoverySummary {
        sessions_interrupted,
        tunnels_stopped,
        runbooks_interrupted,
        secrets_preloaded,
        persisted_fields_scrubbed,
    })
}

fn preload_known_secrets(conn: &Connection) -> Result<usize> {
    let mut statement = conn.prepare("SELECT DISTINCT secret_ref FROM credentials")?;
    let secret_refs = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut loaded = 0;
    for secret_ref in secret_refs {
        if matches!(crate::vault::get_secret(&secret_ref), Ok(Some(_))) {
            // vault::get_secret registers the value with the central redactor.
            loaded += 1;
        }
    }
    Ok(loaded)
}

/// Redact known vault secrets from all user-controlled text that can already
/// exist in SQLite from an older build. This protects the logical database
/// contents; forensic erasure of previously freed SQLite pages is outside this
/// routine's contract and should not be confused with normal application reads.
fn scrub_persisted_known_secrets(conn: &Connection) -> Result<usize> {
    const COLUMNS: &[(&str, &str)] = &[
        ("servers", "name"),
        ("servers", "host"),
        ("servers", "username"),
        ("servers", "private_key_path"),
        ("servers", "tags_json"),
        ("servers", "group_name"),
        ("servers", "notes"),
        ("command_snippets", "label"),
        ("command_snippets", "command"),
        ("command_snippets", "tags_json"),
        ("runbooks", "name"),
        ("runbooks", "description"),
        ("runbooks", "content_yaml"),
        ("runbook_runs", "status"),
        ("runbook_runs", "output_json"),
        ("tunnels", "type"),
        ("tunnels", "local_host"),
        ("tunnels", "remote_host"),
    ];

    let transaction = conn
        .unchecked_transaction()
        .context("failed to begin persisted-secret scrub")?;
    let mut changed = 0;
    for (table, column) in COLUMNS {
        changed += scrub_column(&transaction, table, column)?;
    }
    transaction.commit()?;
    Ok(changed)
}

fn scrub_column(conn: &Connection, table: &str, column: &str) -> Result<usize> {
    // table/column are compile-time constants from COLUMNS above; they are not
    // derived from user input.
    let select_sql = format!("SELECT rowid, {column} FROM {table} WHERE {column} IS NOT NULL");
    let mut statement = conn.prepare(&select_sql)?;
    let values = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(statement);

    let update_sql = format!("UPDATE {table} SET {column} = ?1 WHERE rowid = ?2");
    let mut changed = 0;
    for (rowid, value) in values {
        let redacted = crate::redaction::redact(&value);
        if redacted != value {
            conn.execute(&update_sql, params![redacted, rowid])?;
            changed += 1;
        }
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{database, redaction};

    #[test]
    fn startup_reconciliation_closes_stale_sessions_and_tunnels() {
        let path =
            std::env::temp_dir().join(format!("remoteopsx-recovery-{}.db", uuid::Uuid::new_v4()));
        let conn = database::open(&path).unwrap();
        conn.execute(
            "INSERT INTO sessions (id, server_id, protocol, started_at, ended_at, status)
             VALUES ('s1', 'server', 'ssh', 'start', NULL, 'active')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tunnels (id, server_id, type, local_host, local_port, remote_host, remote_port, status, created_at)
             VALUES ('t1', 'server', 'dynamic', '127.0.0.1', 1080, NULL, NULL, 'active', 'start')",
            [],
        )
        .unwrap();

        let summary = reconcile_startup(&conn).unwrap();
        assert_eq!(summary.sessions_interrupted, 1);
        assert_eq!(summary.tunnels_stopped, 1);
        assert_eq!(summary.secrets_preloaded, 0);
        assert_eq!(summary.persisted_fields_scrubbed, 0);

        let session: (String, Option<String>) = conn
            .query_row(
                "SELECT status, ended_at FROM sessions WHERE id='s1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(session.0, "interrupted");
        assert!(session.1.is_some());
        let tunnel: String = conn
            .query_row("SELECT status FROM tunnels WHERE id='t1'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(tunnel, "stopped");

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn preload_is_safe_when_no_credentials_exist() {
        let path = std::env::temp_dir().join(format!(
            "remoteopsx-recovery-empty-{}.db",
            uuid::Uuid::new_v4()
        ));
        let conn = database::open(&path).unwrap();
        assert_eq!(preload_known_secrets(&conn).unwrap(), 0);
        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn historical_user_text_is_scrubbed_from_logical_database_values() {
        const SECRET: &str = "recovery-test-historical-secret-canary";
        redaction::register_secret(SECRET);
        let path = std::env::temp_dir().join(format!(
            "remoteopsx-recovery-scrub-{}.db",
            uuid::Uuid::new_v4()
        ));
        let conn = database::open(&path).unwrap();

        conn.execute(
            "INSERT INTO command_snippets (id, label, command, tags_json, created_at, updated_at)
             VALUES ('snippet', ?1, ?2, '[]', 'start', 'start')",
            params![format!("label {SECRET}"), format!("echo {SECRET}")],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO runbooks (id, name, description, content_yaml, builtin, created_at, updated_at)
             VALUES ('runbook', 'legacy', '', ?1, 0, 'start', 'start')",
            params![format!("name: legacy\nsteps:\n  - command: echo {SECRET}\n")],
        )
        .unwrap();

        let changed = scrub_persisted_known_secrets(&conn).unwrap();
        assert_eq!(changed, 3);

        let (label, command): (String, String) = conn
            .query_row(
                "SELECT label, command FROM command_snippets WHERE id='snippet'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let yaml: String = conn
            .query_row(
                "SELECT content_yaml FROM runbooks WHERE id='runbook'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!label.contains(SECRET));
        assert!(!command.contains(SECRET));
        assert!(!yaml.contains(SECRET));
        assert!(label.contains("••••••"));
        assert!(command.contains("••••••"));
        assert!(yaml.contains("••••••"));

        drop(conn);
        let _ = std::fs::remove_file(path);
    }
}
