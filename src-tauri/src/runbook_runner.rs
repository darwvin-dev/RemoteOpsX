//! Runbook engine.
//!
//! A runbook is a YAML doc (name/description/steps). Steps are executed in
//! order over SSH; each step's output and pass/fail is captured. Success/
//! failure can be asserted with regex-free substring patterns (kept simple and
//! dependency-light for the MVP). The full run is persisted to SQLite.

use anyhow::Result;

use crate::models::{RunbookSpec, Server, StepResult};
use crate::ssh_manager;

/// Parse a runbook YAML body into its spec.
pub fn parse(yaml: &str) -> Result<RunbookSpec> {
    let spec: RunbookSpec = serde_yaml::from_str(yaml)?;
    Ok(spec)
}

/// Execute a single step over SSH and classify the result.
pub fn run_step(server: &Server, step: &crate::models::RunbookStep) -> StepResult {
    let out = match ssh_manager::run_remote(server, &step.command) {
        Ok(o) => o,
        Err(e) => {
            return StepResult {
                name: step.name.clone(),
                command: step.command.clone(),
                stdout: String::new(),
                stderr: format!("execution error: {e}"),
                exit_code: -1,
                status: "failure".into(),
            }
        }
    };

    let combined = format!("{}\n{}", out.stdout, out.stderr);
    let mut status = if out.success { "success" } else { "failure" };

    // Explicit patterns override the exit code if provided.
    if let Some(fp) = &step.failure_pattern {
        if !fp.is_empty() && combined.contains(fp.as_str()) {
            status = "failure";
        }
    }
    if let Some(sp) = &step.success_pattern {
        if !sp.is_empty() {
            status = if combined.contains(sp.as_str()) { "success" } else { "failure" };
        }
    }

    StepResult {
        name: step.name.clone(),
        command: step.command.clone(),
        stdout: out.stdout,
        stderr: out.stderr,
        exit_code: out.exit_code,
        status: status.to_string(),
    }
}

/// The built-in runbooks seeded on first launch. Returned as
/// (name, description, yaml) tuples.
pub fn builtins() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "Linux Health Check",
            "Quick overview of host, load, memory, disk, failed services, top processes and ports.",
            LINUX_HEALTH_CHECK,
        ),
        (
            "Diagnose High Disk Usage",
            "Find what is filling the disk: usage by filesystem, largest directories and inode pressure.",
            DIAGNOSE_DISK,
        ),
        (
            "Diagnose Failed Service",
            "Inspect failed systemd units and their recent logs.",
            DIAGNOSE_FAILED_SERVICE,
        ),
        (
            "Restart Service Safely",
            "Show status, restart a unit (confirmation required) and verify it came back.",
            RESTART_SERVICE,
        ),
        (
            "Docker Container Diagnosis",
            "List containers, resource usage and recent logs for troubleshooting.",
            DOCKER_DIAGNOSIS,
        ),
        ("VoIP Server Check", "Check OpenSIPS/rtpengine state, SIP ports and recent logs.", VOIP_CHECK),
        ("SMPP Gateway Check", "Check SMPP listener ports, failed units, containers and logs.", SMPP_CHECK),
    ]
}

const LINUX_HEALTH_CHECK: &str = r#"name: Linux Health Check
description: Quick overview of a Linux host.
target_os: linux
variables: {}
steps:
  - name: Host info
    command: hostnamectl
  - name: Uptime & load
    command: uptime
  - name: Memory
    command: free -m
  - name: Disk usage
    command: df -h
  - name: Failed services
    command: systemctl --failed --no-pager
  - name: Top processes
    command: ps -eo pid,comm,%cpu,%mem --sort=-%cpu | head -15
  - name: Listening ports
    command: ss -tulpen | head -50
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_builtins_parse() {
        for (name, _desc, yaml) in builtins() {
            let spec = parse(yaml).unwrap_or_else(|e| panic!("{name} failed to parse: {e}"));
            assert!(!spec.steps.is_empty(), "{name} has no steps");
            for step in &spec.steps {
                assert!(!step.command.trim().is_empty(), "{name} has an empty command");
            }
        }
    }

    #[test]
    fn linux_health_check_has_expected_steps() {
        let spec = parse(LINUX_HEALTH_CHECK).unwrap();
        assert_eq!(spec.name, "Linux Health Check");
        assert_eq!(spec.steps.len(), 7);
        assert_eq!(spec.steps[0].command, "hostnamectl");
    }

    #[test]
    fn restart_service_step_requires_confirmation() {
        let spec = parse(RESTART_SERVICE).unwrap();
        let restart = spec.steps.iter().find(|s| s.name == "Restart unit").unwrap();
        assert!(restart.requires_confirmation);
        // success_pattern is carried through
        let verify = spec.steps.iter().find(|s| s.name == "Verify active").unwrap();
        assert_eq!(verify.success_pattern.as_deref(), Some("active"));
    }

    #[test]
    fn variables_are_parsed() {
        let spec = parse(RESTART_SERVICE).unwrap();
        assert_eq!(spec.variables.get("service").map(String::as_str), Some("nginx"));
    }
}

const DIAGNOSE_DISK: &str = r#"name: Diagnose High Disk Usage
description: Locate what is filling the disk.
target_os: linux
variables:
  path: /
steps:
  - name: Filesystem usage
    command: df -h
  - name: Inode usage
    command: df -i
  - name: Largest top-level dirs
    command: du -xhd1 / 2>/dev/null | sort -rh | head -20
  - name: Largest files under /var
    command: find /var -xdev -type f -printf '%s %p\n' 2>/dev/null | sort -rn | head -20
"#;

const DIAGNOSE_FAILED_SERVICE: &str = r#"name: Diagnose Failed Service
description: Inspect failed systemd units.
target_os: linux
variables:
  service: ""
steps:
  - name: Failed units
    command: systemctl --failed --no-pager
  - name: System log tail
    command: journalctl -p err -n 100 --no-pager
"#;

const RESTART_SERVICE: &str = r#"name: Restart Service Safely
description: Restart a unit with confirmation and verification.
target_os: linux
variables:
  service: nginx
steps:
  - name: Current status
    command: systemctl status {{service}} --no-pager || true
  - name: Restart unit
    command: sudo systemctl restart {{service}}
    requires_confirmation: true
  - name: Verify active
    command: systemctl is-active {{service}}
    success_pattern: active
"#;

const DOCKER_DIAGNOSIS: &str = r#"name: Docker Container Diagnosis
description: Inspect Docker containers and resource usage.
target_os: linux
variables: {}
steps:
  - name: Containers
    command: docker ps -a
  - name: Resource usage
    command: docker stats --no-stream
  - name: Compose status
    command: docker compose ps 2>/dev/null || true
"#;

const VOIP_CHECK: &str = r#"name: VoIP Server Check
description: OpenSIPS / rtpengine health.
target_os: linux
variables: {}
steps:
  - name: OpenSIPS status
    command: systemctl status opensips --no-pager || true
  - name: rtpengine status
    command: systemctl status rtpengine --no-pager || true
  - name: SIP ports
    command: "ss -lunpt | grep -E ':5060|:5061' || true"
  - name: OpenSIPS logs
    command: journalctl -u opensips -n 100 --no-pager || true
  - name: Docker containers
    command: docker ps || true
"#;

const SMPP_CHECK: &str = r#"name: SMPP Gateway Check
description: SMPP gateway listeners and health.
target_os: linux
variables: {}
steps:
  - name: SMPP ports
    command: "ss -tunlp | grep -E ':2775|:2776|:3550' || true"
  - name: Failed services
    command: systemctl --failed --no-pager
  - name: Docker containers
    command: docker ps || true
  - name: Recent logs
    command: journalctl -n 150 --no-pager
  - name: Disk usage
    command: df -h
  - name: Memory
    command: free -m
"#;
