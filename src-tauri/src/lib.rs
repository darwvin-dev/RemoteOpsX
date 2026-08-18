//! RemoteOpsX backend entry point.
//!
//! Wires the module managers into Tauri's managed `AppState` and exposes the
//! command surface consumed by the React frontend.

pub mod database;
pub mod error;
pub mod ftp_manager;
pub mod health_collector;
pub mod host_identity;
pub mod models;
pub mod pty_manager;
pub mod rdp_adapter;
pub mod recovery;
pub mod redaction;
pub mod runbook_runner;
pub mod runtime_preflight;
pub mod settings;
pub mod sftp_manager;
pub mod ssh_keys;
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
use ssh_keys::SshKeyInfo;
use tunnel_manager::TunnelManager;

pub struct AppState {
    db: Mutex<Connection>,
    pty: PtyManager,
    health: health_collector::HealthState,
    tunnels: TunnelManager,
}

fn e<T, E: std::fmt::Display>(result: Result<T, E>) -> CommandResult<T> {
    result.map_err(DomainError::internal)
}

fn re<T, E: std::fmt::Display>(result: Result<T, E>) -> CommandResult<T> {
    result.map_err(|error| DomainError::remote(error.to_string()))
}

fn load_server(state: &State<AppState>, id: &str) -> CommandResult<Server> {
    let conn = state.db.lock().unwrap();
    e(database::get_server(&conn, id))
}

fn settings_get_from_db(conn: &Connection) -> CommandResult<settings::AppSettings> {
    e(database::load_settings(conn))
}

fn settings_save_to_db(
    conn: &Connection,
    settings: settings::AppSettings,
) -> CommandResult<settings::AppSettings> {
    settings.validate()?;
    e(database::save_settings(conn, &settings))?;
    Ok(settings)
}

fn persistent_server_text(input: &ServerInput) -> String {
    [
        input.name.as_str(),
        input.host.as_str(),
        input.username.as_str(),
        input.private_key_path.as_deref().unwrap_or_default(),
        input.group_name.as_deref().unwrap_or_default(),
        input.notes.as_deref().unwrap_or_default(),
    ]
    .into_iter()
    .chain(input.tags.iter().map(String::as_str))
    .collect::<Vec<_>>()
    .join("\n")
}

fn reject_known_secret(field: &'static str, value: &str) -> CommandResult<()> {
    if redaction::contains_known_secret(value) {
        return Err(DomainError::validation(
            field,
            "content contains a stored credential; use a runtime variable or secret reference instead",
        ));
    }
    Ok(())
}

// =================== Settings ===================

#[tauri::command]
fn settings_get(state: State<AppState>) -> CommandResult<settings::AppSettings> {
    let conn = state
        .db
        .lock()
        .map_err(|_| DomainError::internal("database lock poisoned"))?;
    settings_get_from_db(&conn)
}

#[tauri::command]
fn settings_save(
    state: State<AppState>,
    settings: settings::AppSettings,
) -> CommandResult<settings::AppSettings> {
    let conn = state
        .db
        .lock()
        .map_err(|_| DomainError::internal("database lock poisoned"))?;
    settings_save_to_db(&conn, settings)
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

#[tauri::command]
fn server_save(state: State<AppState>, mut input: ServerInput) -> CommandResult<String> {
    database::validate_server_input(&input)
        .map_err(|error| DomainError::validation("server", error.to_string()))?;
    host_identity::validate_target(&input.host, input.port)
        .map_err(|error| DomainError::validation("host", error.to_string()))?;
    if input.id.is_none() {
        input.id = Some(uuid::Uuid::new_v4().to_string());
    }
    let id = input.id.clone().expect("id assigned above");
    let secret_ref = vault::secret_ref(&id);
    let previous = e(vault::get_secret(&secret_ref))?;
    let supplied = input.secret.as_deref().filter(|secret| !secret.is_empty());
    let persistent_text = persistent_server_text(&input);

    if let Some(secret) = supplied {
        if secret.len() >= 4 && persistent_text.contains(secret) {
            return Err(DomainError::validation(
                "server",
                "a password must not be copied into profile metadata",
            ));
        }
    }
    reject_known_secret("server", &persistent_text)?;

    if input.auth_type == "key" {
        let conn = state.db.lock().unwrap();
        let saved = e(database::save_server_profile(&conn, &input, None, true))?;
        drop(conn);
        let _ = vault::delete_secret(&secret_ref);
        return Ok(saved);
    }

    if supplied.is_none() && previous.is_none() {
        return Err(DomainError::validation(
            "secret",
            "a password is required for password authentication",
        ));
    }
    if let Some(secret) = supplied {
        e(vault::set_secret(&secret_ref, secret))?;
    }

    let saved = {
        let conn = state.db.lock().unwrap();
        database::save_server_profile(&conn, &input, Some(&secret_ref), false)
    };
    match saved {
        Ok(saved) => Ok(saved),
        Err(error) => {
            if supplied.is_some() {
                match previous {
                    Some(previous) => {
                        let _ = vault::set_secret(&secret_ref, &previous);
                    }
                    None => {
                        let _ = vault::delete_secret(&secret_ref);
                    }
                }
            }
            Err(DomainError::internal(error))
        }
    }
}

#[tauri::command]
fn server_delete(state: State<AppState>, id: String) -> CommandResult<()> {
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
    re(state
        .pty
        .spawn(app, session_id.clone(), &server, cols, rows))?;
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

// =================== SSH keys ===================

#[tauri::command]
fn ssh_keys_list() -> CommandResult<Vec<SshKeyInfo>> {
    re(ssh_keys::discover_local_keys())
}

#[tauri::command]
fn ssh_key_install(
    state: State<AppState>,
    server_id: String,
    private_key_path: String,
) -> CommandResult<CommandOutput> {
    let server = load_server(&state, &server_id)?;
    let public_key = re(ssh_keys::public_key_for_private_key(private_key_path))?;
    let command = ssh_keys::authorized_keys_install_command(&public_key);
    re(ssh_manager::run_remote(&server, &command))
}

// =================== Runtime preflight / SSH trust ===================

#[tauri::command]
fn runtime_preflight() -> CommandResult<runtime_preflight::RuntimePreflightReport> {
    Ok(runtime_preflight::collect())
}

#[tauri::command]
fn ssh_host_identity_inspect(
    state: State<AppState>,
    server_id: String,
) -> CommandResult<host_identity::HostIdentityReport> {
    let server = load_server(&state, &server_id)?;
    re(host_identity::inspect(&server.host, server.port))
}

#[tauri::command]
fn ssh_host_identity_trust(
    state: State<AppState>,
    server_id: String,
    expected_fingerprint: String,
    replace: bool,
) -> CommandResult<host_identity::HostIdentityReport> {
    let server = load_server(&state, &server_id)?;
    re(host_identity::trust(
        &server.host,
        server.port,
        &expected_fingerprint,
        replace,
    ))
}

#[tauri::command]
fn ssh_host_identity_remove(state: State<AppState>, server_id: String) -> CommandResult<()> {
    let server = load_server(&state, &server_id)?;
    re(host_identity::remove(&server.host, server.port))
}

// =================== Live Health ===================

#[tauri::command]
fn health_collect(state: State<AppState>, server_id: String) -> CommandResult<HealthSnapshot> {
    let server = load_server(&state, &server_id)?;
    re(state.health.collect(&server))
}

// =================== Generic remote exec ===================

#[tauri::command]
fn run_remote(
    state: State<AppState>,
    server_id: String,
    command: String,
) -> CommandResult<CommandOutput> {
    let server = load_server(&state, &server_id)?;
    re(ssh_manager::run_remote(&server, &command))
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

#[tauri::command]
fn runbook_spec(state: State<AppState>, id: String) -> CommandResult<RunbookSpec> {
    let runbook = {
        let conn = state.db.lock().unwrap();
        e(database::get_runbook(&conn, &id))?
    };
    e(runbook_runner::parse(&runbook.content_yaml))
}

#[tauri::command]
fn runbook_save(
    state: State<AppState>,
    id: Option<String>,
    name: String,
    description: String,
    content_yaml: String,
) -> CommandResult<String> {
    reject_known_secret("runbook.name", &name)?;
    reject_known_secret("runbook.description", &description)?;
    reject_known_secret("runbook.content_yaml", &content_yaml)?;
    runbook_runner::parse(&content_yaml)
        .map_err(|error| DomainError::validation("content_yaml", error.to_string()))?;
    let conn = state.db.lock().unwrap();
    e(database::save_runbook(
        &conn,
        &name,
        &description,
        &content_yaml,
        id.as_deref(),
    ))
}

#[tauri::command]
fn runbook_run_step(
    state: State<AppState>,
    server_id: String,
    step: RunbookStep,
) -> CommandResult<StepResult> {
    let server = load_server(&state, &server_id)?;
    Ok(redaction::redact_step_result(runbook_runner::run_step(
        &server, &step,
    )))
}

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
        results: results
            .into_iter()
            .map(redaction::redact_step_result)
            .collect(),
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

// =================== Sessions history ===================

#[tauri::command]
fn sessions_list(state: State<AppState>, limit: Option<i64>) -> CommandResult<Vec<SessionRecord>> {
    let conn = state.db.lock().unwrap();
    e(database::list_sessions(&conn, limit.unwrap_or(100)))
}

// =================== Command snippets ===================

#[tauri::command]
fn command_snippets_list(state: State<AppState>) -> CommandResult<Vec<CommandSnippet>> {
    let conn = state.db.lock().unwrap();
    e(database::list_command_snippets(&conn))
}

#[tauri::command]
fn command_snippet_save(
    state: State<AppState>,
    input: CommandSnippetInput,
) -> CommandResult<CommandSnippet> {
    database::validate_snippet_input(&input)
        .map_err(|error| DomainError::validation("snippet", error.to_string()))?;
    let snippet_text = std::iter::once(input.label.as_str())
        .chain(std::iter::once(input.command.as_str()))
        .chain(input.tags.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join("\n");
    reject_known_secret("snippet", &snippet_text)?;
    let conn = state.db.lock().unwrap();
    e(database::save_command_snippet(&conn, &input))
}

#[tauri::command]
fn command_snippet_delete(state: State<AppState>, id: String) -> CommandResult<()> {
    let conn = state.db.lock().unwrap();
    e(database::delete_command_snippet(&conn, &id))
}

// =================== Services ===================

#[tauri::command]
fn service_action(
    state: State<AppState>,
    server_id: String,
    action: String,
    unit: String,
) -> CommandResult<CommandOutput> {
    let server = load_server(&state, &server_id)?;
    let unit_q = shell_quote(&unit);
    let command = match action.as_str() {
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
    re(ssh_manager::run_remote(&server, &command))
}

// =================== SFTP ===================

#[tauri::command]
fn sftp_list(
    state: State<AppState>,
    server_id: String,
    path: String,
) -> CommandResult<Vec<RemoteFile>> {
    let server = load_server(&state, &server_id)?;
    re(sftp_manager::list_dir(&server, &path))
}

#[tauri::command]
fn sftp_upload(
    state: State<AppState>,
    server_id: String,
    local_path: String,
    remote_dir: String,
) -> CommandResult<()> {
    let server = load_server(&state, &server_id)?;
    re(sftp_manager::upload(&server, &local_path, &remote_dir))
}

#[tauri::command]
fn sftp_download(
    state: State<AppState>,
    server_id: String,
    remote_path: String,
    local_path: String,
) -> CommandResult<()> {
    let server = load_server(&state, &server_id)?;
    re(sftp_manager::download(&server, &remote_path, &local_path))
}

#[tauri::command]
fn sftp_delete(
    state: State<AppState>,
    server_id: String,
    remote_path: String,
) -> CommandResult<()> {
    let server = load_server(&state, &server_id)?;
    re(sftp_manager::delete(&server, &remote_path))
}

#[tauri::command]
fn sftp_rename(
    state: State<AppState>,
    server_id: String,
    from: String,
    to: String,
) -> CommandResult<()> {
    let server = load_server(&state, &server_id)?;
    re(sftp_manager::rename(&server, &from, &to))
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
        .map_err(|error| DomainError::validation(error.field, error.to_string()))
}

fn map_tunnel_start_result<E>(result: Result<(), E>) -> CommandResult<()>
where
    E: std::fmt::Display,
{
    re(result)
}

#[tauri::command]
fn tunnel_start(state: State<AppState>, tunnel: Tunnel) -> CommandResult<Tunnel> {
    let mut tunnel = tunnel;
    if tunnel.id.is_empty() {
        tunnel.id = uuid::Uuid::new_v4().to_string();
    }
    let tunnel_text = [
        tunnel.r#type.as_str(),
        tunnel.local_host.as_deref().unwrap_or_default(),
        tunnel.remote_host.as_deref().unwrap_or_default(),
    ]
    .join("\n");
    reject_known_secret("tunnel", &tunnel_text)?;
    validate_tunnel_start_input(&tunnel)?;
    let server = load_server(&state, &tunnel.server_id)?;
    map_tunnel_start_result(state.tunnels.start(&server, &tunnel))?;
    tunnel.status = "active".into();
    {
        let conn = state.db.lock().unwrap();
        e(database::insert_tunnel(&conn, &tunnel))?;
    }
    Ok(tunnel)
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
    for tunnel in tunnels.iter_mut() {
        if tunnel.status == "active" && !active.contains(&tunnel.id) {
            tunnel.status = "stopped".into();
            let _ = database::set_tunnel_status(&conn, &tunnel.id, "stopped");
        }
    }
    Ok(tunnels)
}

#[tauri::command]
fn save_text_file(path: String, content: String) -> CommandResult<()> {
    e(std::fs::write(&path, redaction::redact(content)))
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir().expect("no app data dir");
            host_identity::init(data_dir.join("known_hosts"))
                .expect("failed to initialize SSH host identity store");

            let preflight = runtime_preflight::collect();
            for dependency in preflight
                .dependencies
                .iter()
                .filter(|dependency| dependency.required && !dependency.available)
            {
                eprintln!(
                    "runtime preflight: missing required dependency {}: {}",
                    dependency.label, dependency.detail
                );
            }

            let db_path = data_dir.join("remoteopsx.db");
            let conn = database::open(&db_path).expect("failed to open database");
            let recovered = recovery::reconcile_startup(&conn)
                .expect("failed to reconcile stale runtime state");
            if recovered.sessions_interrupted > 0
                || recovered.tunnels_stopped > 0
                || recovered.runbooks_interrupted > 0
            {
                eprintln!(
                    "startup recovery: {} session(s) interrupted, {} tunnel(s) stopped, {} runbook(s) interrupted",
                    recovered.sessions_interrupted,
                    recovered.tunnels_stopped,
                    recovered.runbooks_interrupted
                );
            }

            for (name, description, yaml) in runbook_runner::builtins() {
                let _ = database::seed_builtin_runbook(&conn, name, description, yaml);
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
            settings_get,
            settings_save,
            servers_list,
            server_get,
            server_save,
            server_delete,
            pty_spawn,
            pty_write,
            pty_resize,
            pty_close,
            ssh_keys_list,
            ssh_key_install,
            runtime_preflight,
            ssh_host_identity_inspect,
            ssh_host_identity_trust,
            ssh_host_identity_remove,
            health_collect,
            run_remote,
            runbooks_list,
            runbook_get,
            runbook_spec,
            runbook_save,
            runbook_run_step,
            runbook_record_run,
            runbook_runs_list,
            sessions_list,
            command_snippets_list,
            command_snippet_save,
            command_snippet_delete,
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
        missing_remote_port = Tunnel {
            remote_port: None,
            ..missing_remote_port
        };
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
    fn operational_tunnel_start_failures_preserve_the_diagnostic_message() {
        let error = map_tunnel_start_result(Err(anyhow::anyhow!("ssh executable unavailable")))
            .expect_err("spawn failure should be returned");

        assert_eq!(error.code, "remote.operation_failed");
        assert_eq!(error.message, "ssh executable unavailable");
        assert!(error.retryable);
        assert!(error.context.is_empty());
    }
}

#[cfg(test)]
mod settings_command_tests {
    use super::*;

    fn database() -> (std::path::PathBuf, Connection) {
        let path =
            std::env::temp_dir().join(format!("remoteopsx-settings-{}.db", uuid::Uuid::new_v4()));
        let conn = database::open(&path).expect("test database should open");
        (path, conn)
    }

    #[test]
    fn get_returns_defaults_and_save_returns_persisted_value() {
        let (path, conn) = database();
        let defaults = settings_get_from_db(&conn).expect("defaults should load");
        assert_eq!(defaults, settings::AppSettings::default());

        let mut changed = defaults;
        changed.theme = settings::Theme::Dark;
        changed.default_ports.ssh = 2222;
        let saved = settings_save_to_db(&conn, changed.clone()).expect("settings should save");

        assert_eq!(saved, changed);
        assert_eq!(settings_get_from_db(&conn).unwrap(), changed);
        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn save_validates_before_replacing_persisted_settings() {
        let (path, conn) = database();
        let original = settings::AppSettings::default();
        settings_save_to_db(&conn, original.clone()).unwrap();

        let mut invalid = original.clone();
        invalid.default_ports.ssh = 0;
        let error = settings_save_to_db(&conn, invalid).expect_err("invalid settings should fail");

        assert_eq!(error.code, "validation.invalid_value");
        assert_eq!(settings_get_from_db(&conn).unwrap(), original);
        drop(conn);
        let _ = std::fs::remove_file(path);
    }
}
