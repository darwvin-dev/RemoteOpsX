//! Guarded broadcast execution with bounded concurrency and per-host results.

use std::thread;

use anyhow::{anyhow, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::models::{CommandOutput, Server};
use crate::operator_data::{MultiHostResult, MultiHostRun};
use crate::{redaction, ssh_manager};

const MAX_TARGETS: usize = 50;
const MAX_CONCURRENCY: usize = 8;
const MAX_COMMAND_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiHostRequest {
    pub server_ids: Vec<String>,
    pub command: String,
    pub concurrency: usize,
    pub production_confirmed: bool,
    pub destructive_confirmed: bool,
}

pub fn looks_destructive(command: &str) -> bool {
    let normalized = command.to_ascii_lowercase();
    [
        "rm -rf",
        "mkfs",
        "shutdown",
        "poweroff",
        "reboot",
        "halt",
        "dd if=",
        "systemctl stop",
        "systemctl restart",
        "docker system prune",
        "kubectl delete",
        "DROP DATABASE",
    ]
    .iter()
    .any(|needle| normalized.contains(&needle.to_ascii_lowercase()))
}

pub fn validate(request: &MultiHostRequest, servers: &[Server]) -> Result<()> {
    if request.server_ids.is_empty() || servers.is_empty() {
        return Err(anyhow!("select at least one server"));
    }
    if request.server_ids.len() > MAX_TARGETS || servers.len() > MAX_TARGETS {
        return Err(anyhow!(
            "multi-host operations are limited to {MAX_TARGETS} targets"
        ));
    }
    if request.command.trim().is_empty() || request.command.len() > MAX_COMMAND_BYTES {
        return Err(anyhow!(
            "command must contain 1 to {MAX_COMMAND_BYTES} bytes"
        ));
    }
    if !(1..=MAX_CONCURRENCY).contains(&request.concurrency) {
        return Err(anyhow!(
            "concurrency must be between 1 and {MAX_CONCURRENCY}"
        ));
    }
    if servers
        .iter()
        .any(|server| server.environment == "production")
        && !request.production_confirmed
    {
        return Err(anyhow!(
            "production targets require explicit production confirmation"
        ));
    }
    if looks_destructive(&request.command) && !request.destructive_confirmed {
        return Err(anyhow!(
            "destructive commands require an additional explicit confirmation"
        ));
    }
    Ok(())
}

fn failed_output(message: impl ToString) -> CommandOutput {
    CommandOutput {
        stdout: String::new(),
        stderr: redaction::redact(message.to_string()),
        exit_code: -1,
        success: false,
    }
}

pub fn execute(request: &MultiHostRequest, servers: Vec<Server>) -> Result<MultiHostRun> {
    validate(request, &servers)?;
    let started_at = Utc::now().to_rfc3339();
    let mut results = Vec::with_capacity(servers.len());

    for batch in servers.chunks(request.concurrency) {
        let command = request.command.as_str();
        let batch_results = thread::scope(|scope| {
            let handles = batch
                .iter()
                .cloned()
                .map(|server| {
                    scope.spawn(move || MultiHostResult {
                        server_id: server.id.clone(),
                        server_name: server.name.clone(),
                        environment: server.environment.clone(),
                        output: ssh_manager::run_remote(&server, command)
                            .unwrap_or_else(failed_output),
                    })
                })
                .collect::<Vec<_>>();
            handles
                .into_iter()
                .map(|handle| {
                    handle.join().unwrap_or_else(|_| MultiHostResult {
                        server_id: "unknown".into(),
                        server_name: "worker panic".into(),
                        environment: "unknown".into(),
                        output: failed_output("multi-host worker panicked"),
                    })
                })
                .collect::<Vec<_>>()
        });
        results.extend(batch_results);
    }

    let successes = results
        .iter()
        .filter(|result| result.output.success)
        .count();
    let status = if successes == results.len() {
        "success"
    } else if successes == 0 {
        "failed"
    } else {
        "partial"
    };
    Ok(MultiHostRun {
        id: uuid::Uuid::new_v4().to_string(),
        command: request.command.clone(),
        status: status.into(),
        started_at,
        ended_at: Utc::now().to_rfc3339(),
        results,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(environment: &str) -> Server {
        Server {
            id: environment.into(),
            name: environment.into(),
            host: "example.test".into(),
            port: 22,
            ftp_port: None,
            rdp_port: None,
            vnc_port: None,
            username: "ops".into(),
            protocols: vec!["ssh".into()],
            auth_type: "key".into(),
            private_key_path: None,
            tags: vec![],
            group_name: None,
            environment: environment.into(),
            notes: None,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn requires_separate_production_and_destructive_confirmations() {
        let production = server("production");
        let mut request = MultiHostRequest {
            server_ids: vec![production.id.clone()],
            command: "sudo systemctl restart nginx".into(),
            concurrency: 2,
            production_confirmed: false,
            destructive_confirmed: false,
        };
        assert!(validate(&request, std::slice::from_ref(&production)).is_err());
        request.production_confirmed = true;
        assert!(validate(&request, std::slice::from_ref(&production)).is_err());
        request.destructive_confirmed = true;
        assert!(validate(&request, &[production]).is_ok());
    }

    #[test]
    fn detects_high_risk_command_families() {
        assert!(looks_destructive("rm -rf /tmp/build"));
        assert!(looks_destructive("sudo reboot"));
        assert!(looks_destructive("kubectl delete pod api"));
        assert!(!looks_destructive("systemctl status nginx"));
    }
}
