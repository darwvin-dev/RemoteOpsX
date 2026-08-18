//! Versioned encrypted workspace backup/restore.
//!
//! Keyring secrets are never exported. The passphrase is supplied to OpenSSL
//! through an environment variable (never argv) and registered with the central
//! redactor. Password-auth profiles restore without credential metadata and
//! explicitly require password re-entry.

use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::jump_host::{self, JumpHostConfig};
use crate::models::{CommandSnippet, CommandSnippetInput, Runbook, Server, ServerInput, Tunnel};
use crate::operator_data::{self, AlertRule, AlertRuleInput, TunnelPolicy, TunnelPolicyInput};
use crate::{database, host_identity, redaction, runbook_runner, settings};

const MAGIC: &str = "REMOTEOPSX-BACKUP-1\n";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkspaceBackup {
    version: u32,
    created_at: String,
    servers: Vec<Server>,
    settings: settings::AppSettings,
    runbooks: Vec<Runbook>,
    snippets: Vec<CommandSnippet>,
    jump_hosts: Vec<JumpHostConfig>,
    alert_rules: Vec<AlertRule>,
    tunnels: Vec<Tunnel>,
    tunnel_policies: Vec<TunnelPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupRestoreReport {
    pub servers: usize,
    pub runbooks: usize,
    pub snippets: usize,
    pub jump_hosts: usize,
    pub alert_rules: usize,
    pub tunnels: usize,
    pub password_reentry_server_ids: Vec<String>,
}

fn openssl(input: &[u8], password: &str, decrypt: bool) -> Result<Vec<u8>> {
    if password.len() < 10 {
        return Err(anyhow!(
            "backup passphrase must contain at least 10 characters"
        ));
    }
    redaction::register_secret(password);
    let mut command = Command::new("openssl");
    command.args([
        "enc",
        "-aes-256-cbc",
        "-pbkdf2",
        "-iter",
        "250000",
        "-md",
        "sha256",
        "-salt",
        "-a",
        "-A",
    ]);
    if decrypt {
        command.arg("-d");
    }
    command
        .args(["-pass", "env:REMOTEOPSX_BACKUP_PASSWORD"])
        .env("REMOTEOPSX_BACKUP_PASSWORD", password)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| anyhow!("failed to start openssl: {error}. Install OpenSSL."))?;
    child
        .stdin
        .as_mut()
        .context("failed to open openssl stdin")?
        .write_all(input)?;
    drop(child.stdin.take());
    let output = child.wait_with_output()?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(anyhow!(redaction::redact(format!(
            "OpenSSL backup operation failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))))
    }
}

fn collect(conn: &Connection) -> Result<WorkspaceBackup> {
    operator_data::ensure_schema(conn)?;
    let servers = database::list_servers(conn)?;
    let mut jump_hosts = Vec::new();
    for server in &servers {
        if let Some(jump) = jump_host::get(conn, &server.id)? {
            jump_hosts.push(jump);
        }
    }
    let mut tunnels = database::list_tunnels(conn)?;
    for tunnel in &mut tunnels {
        tunnel.status = "stopped".into();
    }
    Ok(WorkspaceBackup {
        version: 1,
        created_at: Utc::now().to_rfc3339(),
        servers,
        settings: database::load_settings(conn)?,
        runbooks: database::list_runbooks(conn)?
            .into_iter()
            .filter(|runbook| !runbook.builtin)
            .collect(),
        snippets: database::list_command_snippets(conn)?,
        jump_hosts,
        alert_rules: operator_data::alert_rules(conn)?,
        tunnels,
        tunnel_policies: operator_data::tunnel_policies(conn)?,
    })
}

pub fn export_encrypted(conn: &Connection, path: &str, password: &str) -> Result<()> {
    let backup = collect(conn)?;
    let json = serde_json::to_vec(&backup)?;
    let encrypted = openssl(&json, password, false)?;
    let mut output = MAGIC.as_bytes().to_vec();
    output.extend_from_slice(&encrypted);
    std::fs::write(path, output)?;
    Ok(())
}

fn validate_backup(backup: &WorkspaceBackup) -> Result<()> {
    if backup.version != 1 {
        return Err(anyhow!(
            "unsupported RemoteOpsX backup version {}",
            backup.version
        ));
    }
    backup
        .settings
        .validate()
        .map_err(|error| anyhow!(error.to_string()))?;
    for server in &backup.servers {
        let input = server_input(server);
        database::validate_server_input(&input)?;
        host_identity::validate_target(&server.host, server.port)?;
    }
    for runbook in &backup.runbooks {
        runbook_runner::parse(&runbook.content_yaml)?;
    }
    for snippet in &backup.snippets {
        database::validate_snippet_input(&CommandSnippetInput {
            id: Some(snippet.id.clone()),
            label: snippet.label.clone(),
            command: snippet.command.clone(),
            tags: snippet.tags.clone(),
        })?;
    }
    for jump in &backup.jump_hosts {
        jump_host::validate(jump)?;
    }
    Ok(())
}

fn server_input(server: &Server) -> ServerInput {
    ServerInput {
        id: Some(server.id.clone()),
        name: server.name.clone(),
        host: server.host.clone(),
        port: server.port,
        ftp_port: server.ftp_port,
        rdp_port: server.rdp_port,
        vnc_port: server.vnc_port,
        username: server.username.clone(),
        protocols: server.protocols.clone(),
        auth_type: server.auth_type.clone(),
        private_key_path: server.private_key_path.clone(),
        tags: server.tags.clone(),
        group_name: server.group_name.clone(),
        environment: server.environment.clone(),
        notes: server.notes.clone(),
        secret: None,
    }
}

pub fn import_encrypted(
    conn: &Connection,
    path: &str,
    password: &str,
) -> Result<BackupRestoreReport> {
    let bytes = std::fs::read(path)?;
    if !bytes.starts_with(MAGIC.as_bytes()) {
        return Err(anyhow!("not a supported RemoteOpsX encrypted backup"));
    }
    let plaintext = openssl(&bytes[MAGIC.len()..], password, true)?;
    let backup: WorkspaceBackup = serde_json::from_slice(&plaintext)
        .map_err(|_| anyhow!("backup passphrase is incorrect or the backup is corrupted"))?;
    validate_backup(&backup)?;
    operator_data::ensure_schema(conn)?;

    let mut password_reentry_server_ids = Vec::new();
    for server in &backup.servers {
        database::save_server_profile(conn, &server_input(server), None, true)?;
        if server.auth_type == "password" {
            password_reentry_server_ids.push(server.id.clone());
        }
    }
    database::save_settings(conn, &backup.settings)?;
    for runbook in &backup.runbooks {
        database::save_runbook(
            conn,
            &runbook.name,
            &runbook.description,
            &runbook.content_yaml,
            Some(&runbook.id),
        )?;
    }
    for snippet in &backup.snippets {
        database::save_command_snippet(
            conn,
            &CommandSnippetInput {
                id: Some(snippet.id.clone()),
                label: snippet.label.clone(),
                command: snippet.command.clone(),
                tags: snippet.tags.clone(),
            },
        )?;
    }
    for jump in &backup.jump_hosts {
        jump_host::save(conn, jump)?;
    }
    for rule in &backup.alert_rules {
        operator_data::save_alert_rule(
            conn,
            &AlertRuleInput {
                id: Some(rule.id.clone()),
                server_id: rule.server_id.clone(),
                metric: rule.metric.clone(),
                comparison: rule.comparison.clone(),
                threshold: rule.threshold,
                consecutive_samples: rule.consecutive_samples,
                cooldown_seconds: rule.cooldown_seconds,
                enabled: rule.enabled,
            },
        )?;
    }
    for tunnel in &backup.tunnels {
        database::insert_tunnel(conn, tunnel)?;
    }
    for policy in &backup.tunnel_policies {
        // Restores are intentionally inert: users explicitly re-enable autostart.
        operator_data::save_tunnel_policy(
            conn,
            &TunnelPolicyInput {
                tunnel_id: policy.tunnel_id.clone(),
                autostart: false,
                auto_reconnect: policy.auto_reconnect,
                health_interval_secs: policy.health_interval_secs,
            },
        )?;
    }

    Ok(BackupRestoreReport {
        servers: backup.servers.len(),
        runbooks: backup.runbooks.len(),
        snippets: backup.snippets.len(),
        jump_hosts: backup.jump_hosts.len(),
        alert_rules: backup.alert_rules.len(),
        tunnels: backup.tunnels.len(),
        password_reentry_server_ids,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magic_is_versioned_and_passphrases_are_bounded() {
        assert!(MAGIC.starts_with("REMOTEOPSX-BACKUP-1"));
        assert!(openssl(b"test", "short", false).is_err());
    }
}
