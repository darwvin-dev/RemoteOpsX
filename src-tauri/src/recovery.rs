//! Startup crash/restart reconciliation.
//!
//! PTYs and tunnel child handles are process-local. If the app exits before a
//! normal close path updates SQLite, records must not remain "active" forever
//! on the next launch. Existing keyring credentials are also loaded into the
//! central redaction registry during startup so persistence/output guards are
//! active before the first connection attempt.

use anyhow::{Context, Result};
use rusqlite::{params, Connection};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoverySummary {
    pub sessions_interrupted: usize,
    pub tunnels_stopped: usize,
    pub runbooks_interrupted: usize,
    pub secrets_preloaded: usize,
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

    Ok(RecoverySummary {
        sessions_interrupted,
        tunnels_stopped,
        runbooks_interrupted,
        secrets_preloaded,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database;

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
}
