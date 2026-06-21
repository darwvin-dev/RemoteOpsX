//! RemoteOpsX backend entry point.
//!
//! Wires the module managers into Tauri's managed `AppState` and exposes the
//! command surface consumed by the React frontend.

// Modules are `pub` so integration tests (in `tests/`) can drive the remote-ops
// layer (ssh exec, health collection, runbook execution) against a real host.
pub mod database;
pub mod error;
pub mod ftp_manager;
pub mod health_collector;
pub mod models;
pub mod pty_manager;
pub mod rdp_adapter;
pub mod runbook_runner;
pub mod settings;
pub mod sftp_manager;
pub mod ssh_manager;
pub mod tunnel_manager;
pub mod vault;
pub mod vnc_adapter;

use std::sync::Mutex;

use rusqlite::Connection;
use tauri::{AppHandle, Manager, State};

use error::{CommandResult, DomainError};
use health_collector::HealthSnapshot;
use models::*;
use pty_manager::PtyManager;
use tunnel_manager::TunnelManager;

/// Shared application state, managed by Tauri and injected into commands.
pub struct AppState {
    db: Mutex<Connection>,
    pty: PtyManager,
    health: health_collector::HealthState,
    tunnels: TunnelManager,
}

/// Convert an operational error into the safe transport contract.
fn e<T, E: std::fmt::Display>(r: Result<T, E>) -> CommandResult<T> {
    r.map_err(DomainError::internal)
}

/// Load a server profile by id.
fn load_server(state: &State<AppState>, id: &str) -> CommandResult<Server> {
    let conn = state.db.lock().unwrap();
    e(database::get_server(&conn, id))
}

// =================== Server Manager ===================

#[tauri::command]
fn servers_list(state: State<AppState>) -> CommandResult<Vec<Server>> {
    let conn = state.db.lock().unwrap();
    e(database::list_servers(&conn))
}

#[tauri::command]
fn server_get(state: State<AppState>, id: String) -> CommandResult<Server> {
    load_server(&state, &id)
}

/// Create or update a profile. The transient `secret` is written to the OS
/// keyring (never SQLite); only a reference is recorded.
#[tauri::command]
fn server_save(state: State<AppState>, mut input: ServerInput) -> CommandResult<String> {
    database::validate_server_input(&input)
        .map_err(|err| DomainError::validation("server", err.to_string()))?;
    if input.id.is_none() {
        input.id = Some(uuid::Uuid::new_v4().to_string());
    }
    let id = input.id.clone().expect("id assigned above");
    let sref = vault::secret_ref(&id);

    if input.auth_type == "key" {
        let conn = state.db.lock().unwrap();
        let saved = e(database::save_server_profile(&conn, &input, None, true))?;
        drop(conn);
        let _ = vault::delete_secret(&sref);
        return Ok(saved);
    }

    let supplied = input.secret.as_deref().filter(|secret| !secret.is_empty());
    let previous = e(vault::get_secret(&sref))?;
    if supplied.is_none() && previous.is_none() {
        return Err(DomainError::validation(
            "secret",
            "a password is required for password authentication",
        ));
    }
    if let Some(secret) = supplied {
        e(vault::set_secret(&sref, secret))?;
    }

    let saved = {
        let conn = state.db.lock().unwrap();
        database::save_server_profile(&conn, &input, Some(&sref), false)
    };
    match saved {
        Ok(saved) => Ok(saved),
        Err(err) => {
            if supplied.is_some() {
                match previous {
                    Some(previous) => {
                        let _ = vault::set_secret(&sref, &previous);
                    }
                    None => {
                        let _ = vault::delete_secret(&sref);
                    }
                }
            }
            Err(DomainError::internal(err))
        }
    }
}

#[tauri::command]
fn server_delete(state: State<AppState>, id: String) -> CommandResult<()> {
    // Best-effort secret cleanup; ignore missing keyring entries.
    let _ = vault::delete_secret(&vault::secret_ref(&id));
    state.health.forget(&id);
    let conn = state.db.lock().unwrap();
    e(database::delete_server(&conn, &id))
}

// =================== SSH Terminal (PTY) ===================

#[tauri::command]
fn pty_spawn(
    app: AppHandle,
    state: State<AppState>,
    session_id: String,
    server_id: String,
    cols: u16,
    rows: u16,
) -> CommandResult<()> {
    let server = load_server(&state, &server_id)?;
    e(state
        .pty
        .spawn(app, session_id.clone(), &server, cols, rows))?;
    // Record the session in SQLite for the sessions history.
    let conn = state.db.lock().unwrap();
    let _ = database::open_session(&conn, &session_id, &server_id, "ssh");
    Ok(())
}

#[tauri::command]
fn pty_write(state: State<AppState>, session_id: String, data: Vec<u8>) -> CommandResult<()> {
    e(state.pty.write(&session_id, &data))
}

#[tauri::command]
fn pty_resize(
    state: State<AppState>,
    session_id: String,
    cols: u16,
    rows: u16,
) -> CommandResult<()> {
    e(state.pty.resize(&session_id, cols, rows))
}

#[tauri::command]
fn pty_close(state: State<AppState>, session_id: String) -> CommandResult<()> {
    e(state.pty.close(&session_id))?;
    let conn = state.db.lock().unwrap();
    let _ = database::close_session(&conn, &session_id);
    Ok(())
}

// =================== Live Health ===================

#[tauri::command]
fn health_collect(state: State<AppState>, server_id: String) -> CommandResult<HealthSnapshot> {
    let server = load_server(&state, &server_id)?;
    e(state.health.collect(&server))
}

// =================== Generic remote exec (logs panel, etc.) ===================

#[tauri::command]
fn run_remote(
    state: State<AppState>,
    server_id: String,
    command: String,
) -> CommandResult<CommandOutput> {
    let server = load_server(&state, &server_id)?;
    e(ssh_manager::run_remote(&server, &command))
}

// =================== Runbooks ===================

#[tauri::command]
fn runbooks_list(state: State<AppState>) -> CommandResult<Vec<Runbook>> {
    let conn = state.db.lock().unwrap();
    e(database::list_runbooks(&conn))
}

#[tauri::command]
fn runbook_get(state: State<AppState>, id: String) -> CommandResult<Runbook> {
    let conn = state.db.lock().unwrap();
    e(database::get_runbook(&conn, &id))
}

/// Parse a runbook's YAML into its executable spec (for the pre-run preview).
#[tauri::command]
fn runbook_spec(state: State<AppState>, id: String) -> CommandResult<RunbookSpec> {
    let rb = {
        let conn = state.db.lock().unwrap();
        e(database::get_runbook(&conn, &id))?
    };
    e(runbook_runner::parse(&rb.content_yaml))
}

#[tauri::command]
fn runbook_save(
    state: State<AppState>,
    id: Option<String>,
    name: String,
    description: String,
    content_yaml: String,
) -> CommandResult<String> {
    // Validate YAML before saving.
    runbook_runner::parse(&content_yaml)
        .map_err(|err| DomainError::validation("content_yaml", err.to_string()))?;
    let conn = state.db.lock().unwrap();
    e(database::save_runbook(
        &conn,
        &name,
        &description,
        &content_yaml,
        id.as_deref(),
    ))
}

/// Run a single runbook step over SSH. The frontend drives the loop so it can
/// pause for confirmation between destructive steps.
#[tauri::command]
fn runbook_run_step(
    state: State<AppState>,
    server_id: String,
    step: RunbookStep,
) -> CommandResult<StepResult> {
    let server = load_server(&state, &server_id)?;
    Ok(runbook_runner::run_step(&server, &step))
}

/// Persist a completed runbook execution.
#[tauri::command]
fn runbook_record_run(
    state: State<AppState>,
    runbook_id: String,
    server_id: String,
    started_at: String,
    status: String,
    results: Vec<StepResult>,
) -> CommandResult<String> {
    let run = RunbookRun {
        id: uuid::Uuid::new_v4().to_string(),
        runbook_id,
        server_id,
        started_at,
        ended_at: Some(chrono::Utc::now().to_rfc3339()),
        status,
        results,
    };
    {
        let conn = state.db.lock().unwrap();
        e(database::insert_runbook_run(&conn, &run))?;
    }
    Ok(run.id)
}

#[tauri::command]
fn runbook_runs_list(state: State<AppState>, limit: Option<i64>) -> CommandResult<Vec<RunbookRun>> {
    let conn = state.db.lock().unwrap();
    e(database::list_runbook_runs(&conn, limit.unwrap_or(50)))
}

// =================== Services (systemd) ===================

#[tauri::command]
fn service_action(
    state: State<AppState>,
    server_id: String,
    action: String,
    unit: String,
) -> CommandResult<CommandOutput> {
    let server = load_server(&state, &server_id)?;
    let unit_q = shell_quote(&unit);
    let cmd = match action.as_str() {
        "status" => format!("systemctl status {unit_q} --no-pager"),
        "logs" => format!("journalctl -u {unit_q} -n 200 --no-pager"),
        "start" => format!("sudo systemctl start {unit_q}"),
        "stop" => format!("sudo systemctl stop {unit_q}"),
        "restart" => format!("sudo systemctl restart {unit_q}"),
        "list-failed" => "systemctl --failed --no-pager --plain --no-legend".to_string(),
        other => {
            return Err(DomainError::validation(
                "action",
                format!("unknown service action: {other}"),
            ))
        }
    };
    e(ssh_manager::run_remote(&server, &cmd))
}

// =================== SFTP ===================

#[tauri::command]
fn sftp_list(
    state: State<AppState>,
    server_id: String,
    path: String,
) -> CommandResult<Vec<RemoteFile>> {
    let server = load_server(&state, &server_id)?;
    e(sftp_manager::list_dir(&server, &path))
}

#[tauri::command]
fn sftp_upload(
    state: State<AppState>,
    server_id: String,
    local_path: String,
    remote_dir: String,
) -> CommandResult<()> {
    let server = load_server(&state, &server_id)?;
    e(sftp_manager::upload(&server, &local_path, &remote_dir))
}

#[tauri::command]
fn sftp_download(
    state: State<AppState>,
    server_id: String,
    remote_path: String,
    local_path: String,
) -> CommandResult<()> {
    let server = load_server(&state, &server_id)?;
    e(sftp_manager::download(&server, &remote_path, &local_path))
}

#[tauri::command]
fn sftp_delete(
    state: State<AppState>,
    server_id: String,
    remote_path: String,
) -> CommandResult<()> {
    let server = load_server(&state, &server_id)?;
    e(sftp_manager::delete(&server, &remote_path))
}

#[tauri::command]
fn sftp_rename(
    state: State<AppState>,
    server_id: String,
    from: String,
    to: String,
) -> CommandResult<()> {
    let server = load_server(&state, &server_id)?;
    e(sftp_manager::rename(&server, &from, &to))
}

// =================== FTP ===================

#[tauri::command]
fn ftp_list(
    state: State<AppState>,
    server_id: String,
    path: String,
) -> CommandResult<Vec<RemoteFile>> {
    let server = load_server(&state, &server_id)?;
    e(ftp_manager::list_dir(&server, &path))
}

#[tauri::command]
fn ftp_upload(
    state: State<AppState>,
    server_id: String,
    local_path: String,
    remote_dir: String,
) -> CommandResult<()> {
    let server = load_server(&state, &server_id)?;
    e(ftp_manager::upload(&server, &local_path, &remote_dir))
}

#[tauri::command]
fn ftp_download(
    state: State<AppState>,
    server_id: String,
    remote_path: String,
    local_path: String,
) -> CommandResult<()> {
    let server = load_server(&state, &server_id)?;
    e(ftp_manager::download(&server, &remote_path, &local_path))
}

#[tauri::command]
fn ftp_delete(state: State<AppState>, server_id: String, remote_path: String) -> CommandResult<()> {
    let server = load_server(&state, &server_id)?;
    e(ftp_manager::delete(&server, &remote_path))
}

#[tauri::command]
fn ftp_rename(
    state: State<AppState>,
    server_id: String,
    from: String,
    to: String,
) -> CommandResult<()> {
    let server = load_server(&state, &server_id)?;
    e(ftp_manager::rename(&server, &from, &to))
}

// =================== Remote desktop ===================

#[tauri::command]
fn rdp_launch(
    state: State<AppState>,
    server_id: String,
    options: rdp_adapter::RdpOptions,
) -> CommandResult<()> {
    let server = load_server(&state, &server_id)?;
    e(rdp_adapter::launch(&server, &options))
}

#[tauri::command]
fn vnc_launch(
    state: State<AppState>,
    server_id: String,
    options: vnc_adapter::VncOptions,
) -> CommandResult<()> {
    let server = load_server(&state, &server_id)?;
    e(vnc_adapter::launch(&server, &options))
}

// =================== Tunnels ===================

fn validate_tunnel_start_input(tunnel: &Tunnel) -> CommandResult<()> {
    tunnel_manager::validate_tunnel(tunnel)
        .map_err(|err| DomainError::validation(err.field, err.to_string()))
}

fn map_tunnel_start_result<E>(result: Result<(), E>) -> CommandResult<()>
where
    E: std::fmt::Display,
{
    e(result)
}

#[tauri::command]
fn tunnel_start(state: State<AppState>, tunnel: Tunnel) -> CommandResult<Tunnel> {
    let mut t = tunnel;
    if t.id.is_empty() {
        t.id = uuid::Uuid::new_v4().to_string();
    }
    validate_tunnel_start_input(&t)?;
    let server = load_server(&state, &t.server_id)?;
    map_tunnel_start_result(state.tunnels.start(&server, &t))?;
    t.status = "active".into();
    {
        let conn = state.db.lock().unwrap();
        e(database::insert_tunnel(&conn, &t))?;
    }
    Ok(t)
}

#[tauri::command]
fn tunnel_stop(state: State<AppState>, id: String) -> CommandResult<()> {
    e(state.tunnels.stop(&id))?;
    let conn = state.db.lock().unwrap();
    e(database::set_tunnel_status(&conn, &id, "stopped"))
}

#[tauri::command]
fn tunnels_list(state: State<AppState>) -> CommandResult<Vec<Tunnel>> {
    let active = state.tunnels.active_ids();
    let conn = state.db.lock().unwrap();
    let mut tunnels = e(database::list_tunnels(&conn))?;
    // Reconcile DB status with live process state.
    for t in tunnels.iter_mut() {
        if t.status == "active" && !active.contains(&t.id) {
            t.status = "stopped".into();
            let _ = database::set_tunnel_status(&conn, &t.id, "stopped");
        }
    }
    Ok(tunnels)
}

/// Write text to a local file (used by the logs panel "save" / diagnostic
/// bundle features). Path is user-chosen via the save dialog.
#[tauri::command]
fn save_text_file(path: String, content: String) -> CommandResult<()> {
    e(std::fs::write(&path, content))
}

/// Minimal single-quote shell escaping for interpolated identifiers.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Open the database under the app's data dir and seed built-ins.
            let data_dir = app.path().app_data_dir().expect("no app data dir");
            let db_path = data_dir.join("remoteopsx.db");
            let conn = database::open(&db_path).expect("failed to open database");
            for (name, desc, yaml) in runbook_runner::builtins() {
                let _ = database::seed_builtin_runbook(&conn, name, desc, yaml);
            }

            app.manage(AppState {
                db: Mutex::new(conn),
                pty: PtyManager::new(),
                health: health_collector::HealthState::new(),
                tunnels: TunnelManager::new(),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            servers_list,
            server_get,
            server_save,
            server_delete,
            pty_spawn,
            pty_write,
            pty_resize,
            pty_close,
            health_collect,
            run_remote,
            runbooks_list,
            runbook_get,
            runbook_spec,
            runbook_save,
            runbook_run_step,
            runbook_record_run,
            runbook_runs_list,
            service_action,
            sftp_list,
            sftp_upload,
            sftp_download,
            sftp_delete,
            sftp_rename,
            ftp_list,
            ftp_upload,
            ftp_download,
            ftp_delete,
            ftp_rename,
            rdp_launch,
            vnc_launch,
            tunnel_start,
            tunnel_stop,
            tunnels_list,
            save_text_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running RemoteOpsX");
}

#[cfg(test)]
mod tunnel_error_tests {
    use super::*;

    fn tunnel() -> Tunnel {
        Tunnel {
            id: "tunnel-1".into(),
            server_id: "server-1".into(),
            r#type: "local".into(),
            local_host: Some("127.0.0.1".into()),
            local_port: 8080,
            remote_host: Some("example.com".into()),
            remote_port: Some(80),
            status: "pending".into(),
            created_at: String::new(),
        }
    }

    #[test]
    fn invalid_tunnel_shapes_are_validation_errors_with_precise_fields() {
        let mut cases = Vec::new();

        let mut missing_server = tunnel();
        missing_server.server_id.clear();
        cases.push((missing_server, "server_id"));

        let mut zero_local_port = tunnel();
        zero_local_port.local_port = 0;
        cases.push((zero_local_port, "local_port"));

        let mut missing_remote_host = tunnel();
        missing_remote_host.remote_host = None;
        cases.push((missing_remote_host, "remote_host"));

        let mut missing_remote_port = tunnel();
        missing_remote_port.remote_port = None;
        cases.push((missing_remote_port, "remote_port"));

        let mut unknown_type = tunnel();
        unknown_type.r#type = "unknown".into();
        cases.push((unknown_type, "type"));

        for (value, field) in cases {
            let error = validate_tunnel_start_input(&value).expect_err("shape should be invalid");
            assert_eq!(error.code, "validation.invalid_value");
            assert_eq!(error.context.get("field").map(String::as_str), Some(field));
        }
    }

    #[test]
    fn operational_tunnel_start_failures_are_internal_errors() {
        let error = map_tunnel_start_result(Err(anyhow::anyhow!("ssh executable unavailable")))
            .expect_err("spawn failure should be returned");

        assert_eq!(error.code, "internal.unexpected");
        assert!(error.context.is_empty());
    }
}
