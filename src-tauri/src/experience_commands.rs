//! Product-experience commands for the operations dashboard and Runbook Studio.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{CommandResult, DomainError};
use crate::models::{RunbookRun, RunbookSpec, Tunnel};
use crate::operator_data::{MultiHostRun, OperatorAlert};
use crate::{database, multi_host, operator_data, redaction, runbook_runner, AppState};

const MAX_RUNBOOK_IMPORT_BYTES: u64 = 256 * 1024;

fn internal<T, E: std::fmt::Display>(result: Result<T, E>) -> CommandResult<T> {
    result.map_err(DomainError::internal)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunbookPreviewStep {
    pub index: usize,
    pub name: String,
    pub command: String,
    pub requires_confirmation: bool,
    pub destructive: bool,
    pub unresolved_variables: Vec<String>,
    pub success_pattern: Option<String>,
    pub failure_pattern: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunbookPreview {
    pub spec: RunbookSpec,
    pub steps: Vec<RunbookPreviewStep>,
    pub unresolved_variables: Vec<String>,
    pub valid: bool,
}

fn valid_variable_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
}

/// Render every `{{ variable }}` span in one pass. This deliberately parses the
/// exact source spans rather than replacing a couple of whitespace variants, so
/// preview and execution cannot disagree on `{{name}}`, `{{ name }}` or wider
/// spacing. Malformed template syntax is rejected instead of being treated as a
/// valid dry-run.
fn substitute(
    command: &str,
    variables: &HashMap<String, String>,
) -> CommandResult<(String, Vec<String>)> {
    let mut rendered = String::with_capacity(command.len());
    let mut unresolved = BTreeSet::new();
    let mut remaining = command;

    while let Some(start) = remaining.find("{{") {
        rendered.push_str(&remaining[..start]);
        let after_open = &remaining[start + 2..];
        let Some(relative_end) = after_open.find("}}") else {
            return Err(DomainError::validation(
                "command",
                "runbook variable placeholder is missing a closing }}",
            ));
        };
        let raw = &after_open[..relative_end];
        let name = raw.trim();
        if !valid_variable_name(name) {
            return Err(DomainError::validation(
                "command",
                "runbook variables must use letters, numbers, or underscore inside {{...}}",
            ));
        }
        match variables.get(name) {
            Some(value) if !value.is_empty() => rendered.push_str(value),
            _ => {
                unresolved.insert(name.to_string());
                rendered.push_str(&remaining[start..start + 2 + relative_end + 2]);
            }
        }
        remaining = &after_open[relative_end + 2..];
    }
    rendered.push_str(remaining);

    Ok((rendered, unresolved.into_iter().collect()))
}

fn preview_yaml(
    content_yaml: &str,
    variables: Option<HashMap<String, String>>,
) -> CommandResult<RunbookPreview> {
    if content_yaml.len() > MAX_RUNBOOK_IMPORT_BYTES as usize {
        return Err(DomainError::validation(
            "content_yaml",
            "runbook YAML is limited to 256 KiB",
        ));
    }
    if redaction::contains_known_secret(content_yaml) {
        return Err(DomainError::validation(
            "content_yaml",
            "runbook YAML contains a stored credential",
        ));
    }
    let spec = runbook_runner::parse(content_yaml)
        .map_err(|error| DomainError::validation("content_yaml", error.to_string()))?;
    if spec.name.trim().is_empty() {
        return Err(DomainError::validation("name", "runbook name is required"));
    }
    if spec.steps.is_empty() {
        return Err(DomainError::validation(
            "steps",
            "runbook must contain at least one step",
        ));
    }

    let mut resolved_variables = spec.variables.clone();
    if let Some(overrides) = variables {
        resolved_variables.extend(overrides);
    }
    let mut all_unresolved = BTreeSet::new();
    let steps = spec
        .steps
        .iter()
        .enumerate()
        .map(|(index, step)| {
            if step.name.trim().is_empty() || step.command.trim().is_empty() {
                return Err(DomainError::validation(
                    "steps",
                    format!("step {} requires a name and command", index + 1),
                ));
            }
            let (command, unresolved_variables) = substitute(&step.command, &resolved_variables)?;
            all_unresolved.extend(unresolved_variables.iter().cloned());
            let destructive = multi_host::looks_destructive(&command);
            Ok(RunbookPreviewStep {
                index,
                name: step.name.clone(),
                command,
                // A rendered variable can turn a benign template into a
                // destructive operation. Confirmation is therefore derived
                // after rendering and cannot be disabled by the YAML author.
                requires_confirmation: step.requires_confirmation || destructive,
                destructive,
                unresolved_variables,
                success_pattern: step.success_pattern.clone(),
                failure_pattern: step.failure_pattern.clone(),
            })
        })
        .collect::<CommandResult<Vec<_>>>()?;
    let unresolved_variables = all_unresolved.into_iter().collect::<Vec<_>>();
    Ok(RunbookPreview {
        spec,
        steps,
        valid: unresolved_variables.is_empty(),
        unresolved_variables,
    })
}

#[tauri::command]
pub fn runbook_preview_yaml(
    content_yaml: String,
    variables: Option<HashMap<String, String>>,
) -> CommandResult<RunbookPreview> {
    preview_yaml(&content_yaml, variables)
}

/// Prepare a saved runbook immediately before execution. The frontend executes
/// only these server-rendered commands, keeping variable resolution and
/// destructive confirmation policy identical to Studio's dry-run preview.
#[tauri::command]
pub fn runbook_preview_saved(
    state: State<AppState>,
    runbook_id: String,
    variables: Option<HashMap<String, String>>,
) -> CommandResult<RunbookPreview> {
    let runbook = {
        let conn = state
            .db
            .lock()
            .map_err(|_| DomainError::internal("database lock poisoned"))?;
        internal(database::get_runbook(&conn, &runbook_id))?
    };
    preview_yaml(&runbook.content_yaml, variables)
}

#[tauri::command]
pub fn runbook_import_yaml(path: String) -> CommandResult<String> {
    let path_ref = Path::new(&path);
    let extension = path_ref
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !matches!(extension.as_str(), "yaml" | "yml") {
        return Err(DomainError::validation(
            "path",
            "Runbook Studio imports only .yaml or .yml files",
        ));
    }
    let metadata = internal(std::fs::metadata(path_ref))?;
    if !metadata.is_file() || metadata.len() > MAX_RUNBOOK_IMPORT_BYTES {
        return Err(DomainError::validation(
            "path",
            "runbook import must be a file no larger than 256 KiB",
        ));
    }
    let content = internal(std::fs::read_to_string(path_ref))?;
    let _ = preview_yaml(&content, None)?;
    Ok(content)
}

#[tauri::command]
pub fn runbook_export_yaml(path: String, content_yaml: String) -> CommandResult<()> {
    let _ = preview_yaml(&content_yaml, None)?;
    let extension = Path::new(&path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !matches!(extension.as_str(), "yaml" | "yml") {
        return Err(DomainError::validation(
            "path",
            "runbook export path must end in .yaml or .yml",
        ));
    }
    internal(std::fs::write(path, content_yaml))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardServer {
    pub server_id: String,
    pub name: String,
    pub environment: String,
    pub status: String,
    pub sampled_at: Option<String>,
    pub cpu_percent: Option<f64>,
    pub mem_percent: Option<f64>,
    pub max_disk_percent: Option<f64>,
    pub failed_services: Option<u32>,
    pub unacknowledged_alerts: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardSummary {
    pub servers_total: usize,
    pub healthy: usize,
    pub warning: usize,
    pub critical: usize,
    pub unknown: usize,
    pub active_tunnels: usize,
    pub failed_tunnels: usize,
    pub unacknowledged_alerts: usize,
    pub servers: Vec<DashboardServer>,
    pub recent_alerts: Vec<OperatorAlert>,
    pub recent_runbooks: Vec<RunbookRun>,
    pub recent_multi_host: Vec<MultiHostRun>,
}

fn quick_status(cpu: f64, memory: f64, disk: f64, failed_services: u32) -> &'static str {
    if failed_services > 0 || cpu >= 95.0 || memory >= 95.0 || disk >= 95.0 {
        "critical"
    } else if cpu >= 80.0 || memory >= 85.0 || disk >= 85.0 {
        "warning"
    } else {
        "healthy"
    }
}

#[tauri::command]
pub fn operator_dashboard_summary(state: State<AppState>) -> CommandResult<DashboardSummary> {
    let conn = state
        .db
        .lock()
        .map_err(|_| DomainError::internal("database lock poisoned"))?;
    internal(operator_data::ensure_schema(&conn))?;
    let servers = internal(database::list_servers(&conn))?;
    let alerts = internal(operator_data::alerts(&conn, 200))?;
    let recent_runbooks = internal(database::list_runbook_runs(&conn, 12))?;
    let recent_multi_host = internal(operator_data::multi_host_runs(&conn, 12))?;
    let tunnels: Vec<Tunnel> = internal(database::list_tunnels(&conn))?;

    let unacknowledged_alerts = alerts
        .iter()
        .filter(|alert| alert.acknowledged_at.is_none())
        .count();
    let mut rollups = Vec::with_capacity(servers.len());
    for server in servers {
        let point = internal(operator_data::health_history(&conn, &server.id, 1))?
            .into_iter()
            .next();
        let server_alerts = alerts
            .iter()
            .filter(|alert| alert.server_id == server.id && alert.acknowledged_at.is_none())
            .count();
        let status = point
            .as_ref()
            .map(|point| {
                quick_status(
                    point.cpu_percent,
                    point.mem_percent,
                    point.max_disk_percent,
                    point.failed_services,
                )
                .to_string()
            })
            .unwrap_or_else(|| "unknown".into());
        rollups.push(DashboardServer {
            server_id: server.id,
            name: server.name,
            environment: server.environment,
            status,
            sampled_at: point.as_ref().map(|point| point.sampled_at.clone()),
            cpu_percent: point.as_ref().map(|point| point.cpu_percent),
            mem_percent: point.as_ref().map(|point| point.mem_percent),
            max_disk_percent: point.as_ref().map(|point| point.max_disk_percent),
            failed_services: point.as_ref().map(|point| point.failed_services),
            unacknowledged_alerts: server_alerts,
        });
    }
    rollups.sort_by_key(|server| match server.status.as_str() {
        "critical" => 0,
        "warning" => 1,
        "unknown" => 2,
        _ => 3,
    });
    Ok(DashboardSummary {
        servers_total: rollups.len(),
        healthy: rollups
            .iter()
            .filter(|server| server.status == "healthy")
            .count(),
        warning: rollups
            .iter()
            .filter(|server| server.status == "warning")
            .count(),
        critical: rollups
            .iter()
            .filter(|server| server.status == "critical")
            .count(),
        unknown: rollups
            .iter()
            .filter(|server| server.status == "unknown")
            .count(),
        active_tunnels: tunnels
            .iter()
            .filter(|tunnel| tunnel.status == "active")
            .count(),
        failed_tunnels: tunnels
            .iter()
            .filter(|tunnel| tunnel.status == "failed")
            .count(),
        unacknowledged_alerts,
        servers: rollups,
        recent_alerts: alerts.into_iter().take(20).collect(),
        recent_runbooks,
        recent_multi_host,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_renders_variable_whitespace_and_marks_destructive_steps() {
        let yaml = r#"name: Deploy
description: Preview
target_os: linux
variables:
  service: nginx
steps:
  - name: Status
    command: systemctl status {{  service   }}
  - name: Restart
    command: sudo systemctl {{action}} {{service}}
"#;
        let mut overrides = HashMap::new();
        overrides.insert("action".into(), "restart".into());
        let preview = preview_yaml(yaml, Some(overrides)).unwrap();
        assert!(preview.valid);
        assert_eq!(preview.steps[0].command, "systemctl status nginx");
        assert_eq!(preview.steps[1].command, "sudo systemctl restart nginx");
        assert!(preview.steps[1].destructive);
        assert!(preview.steps[1].requires_confirmation);
    }

    #[test]
    fn preview_reports_unresolved_variables() {
        let yaml = r#"name: Check
description: Missing variable
variables: {}
steps:
  - name: Check
    command: echo {{missing}}
"#;
        let preview = preview_yaml(yaml, None).unwrap();
        assert!(!preview.valid);
        assert_eq!(preview.unresolved_variables, vec!["missing"]);
    }

    #[test]
    fn malformed_variable_placeholders_are_rejected() {
        let yaml = r#"name: Check
description: Bad variable
variables: {}
steps:
  - name: Check
    command: echo {{bad-name}}
"#;
        assert!(preview_yaml(yaml, None).is_err());
    }
}
