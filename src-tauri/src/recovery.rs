//! Startup crash/restart reconciliation.
//!
//! PTYs and tunnel child handles are process-local. If the app exits before a
//! normal close path updates SQLite, records must not remain "active" forever
//! on the next launch.

use anyhow::{Context, Result};
use rusqlite::{params, Connection};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoverySummary {
    pub sessions_interrupted: usize,
    pub tunnels_stopped: usize,
    pub runbooks_interrupted: usize,
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
    Ok(RecoverySummary {
        sessions_interrupted,
        tunnels_stopped,
        runbooks_interrupted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database;

    #[test]
    fn startup_reconciliation_closes_stale_sessions_and_tunnels() {
        let path = std::env::temp_dir().join(format!("remoteopsx-recovery-{}.db", uuid::Uuid::new_v4()));
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

        let session: (String, Option<String>) = conn
            .query_row("SELECT status, ended_at FROM sessions WHERE id='s1'", [], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap();
        assert_eq!(session.0, "interrupted");
        assert!(session.1.is_some());
        let tunnel: String = conn.query_row("SELECT status FROM tunnels WHERE id='t1'", [], |row| row.get(0)).unwrap();
        assert_eq!(tunnel, "stopped");

        drop(conn);
        let _ = std::fs::remove_file(path);
    }
}
