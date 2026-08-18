//! Tunnel desired-state reconciliation.

use anyhow::Result;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::operator_data;
use crate::{database, tunnel_manager::TunnelManager};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TunnelReconcileReport {
    pub active: usize,
    pub restarted: usize,
    pub failed: usize,
    pub stopped: usize,
}

pub fn reconcile(
    conn: &Connection,
    manager: &TunnelManager,
    startup: bool,
) -> Result<TunnelReconcileReport> {
    operator_data::ensure_schema(conn)?;
    let policies = operator_data::tunnel_policies(conn)?;
    let tunnels = database::list_tunnels(conn)?;
    let active_ids = manager.active_ids();
    let mut report = TunnelReconcileReport::default();

    for tunnel in tunnels {
        if active_ids.contains(&tunnel.id) {
            if tunnel.status != "active" {
                database::set_tunnel_status(conn, &tunnel.id, "active")?;
            }
            report.active += 1;
            continue;
        }

        let policy = policies.iter().find(|policy| policy.tunnel_id == tunnel.id);
        let should_start = policy.is_some_and(|policy| {
            if startup {
                policy.autostart
            } else {
                policy.auto_reconnect && matches!(tunnel.status.as_str(), "active" | "failed")
            }
        });

        if should_start {
            let server = database::get_server(conn, &tunnel.server_id)?;
            match manager.start(&server, &tunnel) {
                Ok(()) => {
                    database::set_tunnel_status(conn, &tunnel.id, "active")?;
                    report.active += 1;
                    report.restarted += 1;
                }
                Err(_) => {
                    database::set_tunnel_status(conn, &tunnel.id, "failed")?;
                    report.failed += 1;
                }
            }
        } else if tunnel.status == "active" {
            database::set_tunnel_status(conn, &tunnel.id, "failed")?;
            report.failed += 1;
        } else if tunnel.status == "failed" {
            report.failed += 1;
        } else {
            report.stopped += 1;
        }
    }
    Ok(report)
}
