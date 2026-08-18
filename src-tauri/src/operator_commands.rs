use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{CommandResult, DomainError};
use crate::multi_host::MultiHostRequest;
use crate::operator_data::{
    AlertRule, AlertRuleInput, HealthPoint, MultiHostRun, OperatorAlert, TunnelPolicy,
    TunnelPolicyInput,
};
use crate::transfer_manager::{TransferJob, TransferRequest};
use crate::workspace_backup::BackupRestoreReport;
use crate::{
    database, jump_host, multi_host, operator_data, redaction, transfer_manager, tunnel_resilience,
    workspace_backup, AppState,
};

static BOOTSTRAPPED: AtomicBool = AtomicBool::new(false);

fn internal<T, E: std::fmt::Display>(result: Result<T, E>) -> CommandResult<T> {
    result.map_err(DomainError::internal)
}

fn remote<T, E: std::fmt::Display>(result: Result<T, E>) -> CommandResult<T> {
    result.map_err(|error| DomainError::remote(error.to_string()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorBootstrapReport {
    pub tunnel_reconcile: tunnel_resilience::TunnelReconcileReport,
}

#[tauri::command]
pub fn operator_bootstrap(state: State<AppState>) -> CommandResult<OperatorBootstrapReport> {
    let startup = !BOOTSTRAPPED.swap(true, Ordering::SeqCst);
    let conn = state.db.lock().unwrap();
    internal(operator_data::ensure_schema(&conn))?;
    let tunnel_reconcile = internal(tunnel_resilience::reconcile(&conn, &state.tunnels, startup))?;
    Ok(OperatorBootstrapReport { tunnel_reconcile })
}

#[tauri::command]
pub fn operator_health_collect(
    state: State<AppState>,
    server_id: String,
) -> CommandResult<crate::health_collector::HealthSnapshot> {
    let server = {
        let conn = state.db.lock().unwrap();
        internal(database::get_server(&conn, &server_id))?
    };
    let snapshot = remote(state.health.collect(&server))?;
    {
        let conn = state.db.lock().unwrap();
        internal(operator_data::record_health(&conn, &server_id, &snapshot))?;
    }
    Ok(snapshot)
}

#[tauri::command]
pub fn health_history_list(
    state: State<AppState>,
    server_id: String,
    limit: Option<i64>,
) -> CommandResult<Vec<HealthPoint>> {
    let conn = state.db.lock().unwrap();
    internal(operator_data::health_history(
        &conn,
        &server_id,
        limit.unwrap_or(720),
    ))
}

#[tauri::command]
pub fn alert_rules_list(state: State<AppState>) -> CommandResult<Vec<AlertRule>> {
    let conn = state.db.lock().unwrap();
    internal(operator_data::alert_rules(&conn))
}

#[tauri::command]
pub fn alert_rule_save(state: State<AppState>, input: AlertRuleInput) -> CommandResult<AlertRule> {
    let conn = state.db.lock().unwrap();
    internal(operator_data::save_alert_rule(&conn, &input))
}

#[tauri::command]
pub fn alert_rule_delete(state: State<AppState>, id: String) -> CommandResult<()> {
    let conn = state.db.lock().unwrap();
    internal(operator_data::delete_alert_rule(&conn, &id))
}

#[tauri::command]
pub fn operator_alerts_list(
    state: State<AppState>,
    limit: Option<i64>,
) -> CommandResult<Vec<OperatorAlert>> {
    let conn = state.db.lock().unwrap();
    internal(operator_data::alerts(&conn, limit.unwrap_or(200)))
}

#[tauri::command]
pub fn operator_alert_acknowledge(state: State<AppState>, id: String) -> CommandResult<()> {
    let conn = state.db.lock().unwrap();
    internal(operator_data::acknowledge_alert(&conn, &id))
}

#[tauri::command]
pub fn transfer_start(
    state: State<AppState>,
    request: TransferRequest,
) -> CommandResult<TransferJob> {
    let server = {
        let conn = state.db.lock().unwrap();
        internal(database::get_server(&conn, &request.server_id))?
    };
    remote(state.transfers.start(&server, request))
}

#[tauri::command]
pub fn transfer_cancel(state: State<AppState>, id: String) -> CommandResult<()> {
    internal(state.transfers.cancel(&id))
}

#[tauri::command]
pub fn transfer_jobs_list(state: State<AppState>) -> CommandResult<Vec<TransferJob>> {
    Ok(state.transfers.jobs())
}

#[tauri::command]
pub fn transfer_chmod(
    state: State<AppState>,
    server_id: String,
    remote_path: String,
    mode: String,
) -> CommandResult<()> {
    let server = {
        let conn = state.db.lock().unwrap();
        internal(database::get_server(&conn, &server_id))?
    };
    remote(state.transfers.chmod(&server, &remote_path, &mode))
}

#[tauri::command]
pub fn tunnel_policies_list(state: State<AppState>) -> CommandResult<Vec<TunnelPolicy>> {
    let conn = state.db.lock().unwrap();
    internal(operator_data::tunnel_policies(&conn))
}

#[tauri::command]
pub fn tunnel_policy_save(
    state: State<AppState>,
    input: TunnelPolicyInput,
) -> CommandResult<TunnelPolicy> {
    let conn = state.db.lock().unwrap();
    internal(operator_data::save_tunnel_policy(&conn, &input))
}

#[tauri::command]
pub fn tunnels_reconcile(
    state: State<AppState>,
) -> CommandResult<tunnel_resilience::TunnelReconcileReport> {
    let conn = state.db.lock().unwrap();
    internal(tunnel_resilience::reconcile(&conn, &state.tunnels, false))
}

#[tauri::command]
pub fn multi_host_run(
    state: State<AppState>,
    request: MultiHostRequest,
) -> CommandResult<MultiHostRun> {
    if redaction::contains_known_secret(&request.command) {
        return Err(DomainError::validation(
            "command",
            "multi-host command contains a stored credential",
        ));
    }
    let unique = request.server_ids.iter().collect::<HashSet<_>>();
    if unique.len() != request.server_ids.len() {
        return Err(DomainError::validation(
            "server_ids",
            "duplicate server targets are not allowed",
        ));
    }
    let servers = {
        let conn = state.db.lock().unwrap();
        let mut servers = Vec::with_capacity(request.server_ids.len());
        for id in &request.server_ids {
            servers.push(internal(database::get_server(&conn, id))?);
        }
        servers
    };
    let run = remote(multi_host::execute(&request, servers))?;
    {
        let conn = state.db.lock().unwrap();
        internal(operator_data::save_multi_host_run(&conn, &run))?;
    }
    Ok(run)
}

#[tauri::command]
pub fn multi_host_runs_list(
    state: State<AppState>,
    limit: Option<i64>,
) -> CommandResult<Vec<MultiHostRun>> {
    let conn = state.db.lock().unwrap();
    internal(operator_data::multi_host_runs(&conn, limit.unwrap_or(50)))
}

#[tauri::command]
pub fn workspace_backup_export(
    state: State<AppState>,
    path: String,
    passphrase: String,
) -> CommandResult<()> {
    let conn = state.db.lock().unwrap();
    internal(workspace_backup::export_encrypted(
        &conn,
        &path,
        &passphrase,
    ))
}

#[tauri::command]
pub fn workspace_backup_import(
    state: State<AppState>,
    path: String,
    passphrase: String,
) -> CommandResult<BackupRestoreReport> {
    let conn = state.db.lock().unwrap();
    let report = internal(workspace_backup::import_encrypted(
        &conn,
        &path,
        &passphrase,
    ))?;
    internal(jump_host::hydrate(&conn))?;
    Ok(report)
}
