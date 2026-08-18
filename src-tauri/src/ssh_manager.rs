//! SSH command construction + one-shot remote execution.
//!
//! RemoteOpsX drives the system OpenSSH client. Every SSH-derived transport
//! uses the app-managed known_hosts file and StrictHostKeyChecking=yes; first
//! contact therefore cannot silently establish trust. One-shot remote commands
//! are delivered to the remote shell over stdin so command text (and any
//! sensitive value it contains) is not exposed in the local process list.

use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Result};

use crate::host_identity;
use crate::models::{CommandOutput, Server};
use crate::redaction;
use crate::vault;

fn base_opts() -> Result<Vec<String>> {
    let mut args = host_identity::strict_ssh_options()?;
    args.extend([
        "-o".into(),
        "ConnectTimeout=12".into(),
        "-o".into(),
        "ServerAliveInterval=15".into(),
    ]);
    Ok(args)
}

pub fn strict_host_key_args() -> Result<Vec<String>> {
    host_identity::strict_ssh_options()
}

fn wants_password(server: &Server) -> bool {
    server.auth_type == "password"
}

fn lookup_secret(server: &Server) -> Option<String> {
    vault::get_secret(&vault::secret_ref(&server.id))
        .ok()
        .flatten()
}

fn push_key_args(server: &Server, args: &mut Vec<String>) {
    if server.auth_type == "key" {
        if let Some(key) = &server.private_key_path {
            if !key.trim().is_empty() {
                args.push("-i".into());
                args.push(key.clone());
                args.push("-o".into());
                args.push("IdentitiesOnly=yes".into());
            }
        }
    } else if server.auth_type == "password" {
        args.push("-o".into());
        args.push("PubkeyAuthentication=no".into());
    }
}

pub fn interactive_argv(server: &Server) -> Result<(String, Vec<String>)> {
    let mut args = base_opts()?;
    args.push("-tt".into());
    args.push("-p".into());
    args.push(server.port.to_string());
    push_key_args(server, &mut args);
    args.push(format!("{}@{}", server.username, server.host));
    wrap_with_password(server, "ssh", args)
}

/// Build a one-shot SSH process without embedding the remote command in argv.
/// With no explicit command OpenSSH starts the user's remote shell; the caller
/// sends the command on stdin and closes it, which executes the command and
/// then cleanly terminates the remote shell at EOF.
fn exec_argv(server: &Server) -> Result<(String, Vec<String>)> {
    let mut args = base_opts()?;
    args.push("-T".into());
    args.push("-o".into());
    args.push(if wants_password(server) {
        "BatchMode=no".into()
    } else {
        "BatchMode=yes".into()
    });
    args.push("-p".into());
    args.push(server.port.to_string());
    push_key_args(server, &mut args);
    args.push(format!("{}@{}", server.username, server.host));
    wrap_with_password(server, "ssh", args)
}

fn wrap_with_password(
    server: &Server,
    program: &str,
    args: Vec<String>,
) -> Result<(String, Vec<String>)> {
    if wants_password(server) {
        match lookup_secret(server) {
            Some(_) if sshpass_available() => {
                let mut wrapped = vec!["-e".to_string(), program.to_string()];
                wrapped.extend(args);
                Ok(("sshpass".to_string(), wrapped))
            }
            Some(_) => Err(anyhow!(
                "This server uses password auth but `sshpass` is not installed. Install sshpass, or switch the profile to key-based auth."
            )),
            None => Err(anyhow!(
                "No stored password for this server. Re-save the profile with a password."
            )),
        }
    } else {
        Ok((program.to_string(), args))
    }
}

fn sshpass_available() -> bool {
    Command::new("sshpass")
        .arg("-h")
        .output()
        .map(|_| true)
        .unwrap_or(false)
}

pub fn apply_password_env(cmd: &mut Command, server: &Server) {
    if wants_password(server) {
        if let Some(password) = lookup_secret(server) {
            cmd.env("SSHPASS", password);
        }
    }
}

pub fn run_remote(server: &Server, remote_command: &str) -> Result<CommandOutput> {
    let (program, args) = exec_argv(server)?;
    let mut cmd = Command::new(&program);
    cmd.args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    apply_password_env(&mut cmd, server);

    let mut child = cmd
        .spawn()
        .map_err(|error| anyhow!("failed to spawn ssh: {error}"))?;
    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| anyhow!("failed to open ssh stdin"))?;
        stdin.write_all(remote_command.as_bytes())?;
        if !remote_command.ends_with('\n') {
            stdin.write_all(b"\n")?;
        }
    }
    drop(child.stdin.take());

    let output = child
        .wait_with_output()
        .map_err(|error| anyhow!("failed to read ssh output: {error}"))?;
    let exit_code = output.status.code().unwrap_or(-1);
    Ok(redaction::redact_command_output(CommandOutput {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code,
        success: output.status.success(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_server(auth_type: &str, key_path: Option<&str>) -> Server {
        Server {
            id: "s1".into(),
            name: "test".into(),
            host: "example.com".into(),
            port: 22,
            ftp_port: None,
            rdp_port: None,
            vnc_port: None,
            username: "root".into(),
            protocols: vec!["ssh".into()],
            auth_type: auth_type.into(),
            private_key_path: key_path.map(|value| value.to_string()),
            tags: vec![],
            group_name: None,
            environment: "dev".into(),
            notes: None,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn has_opt(args: &[String], value: &str) -> bool {
        args.windows(2)
            .any(|window| window[0] == "-o" && window[1] == value)
    }

    #[test]
    fn password_auth_disables_pubkey_so_agent_keys_cant_exhaust_maxauthtries() {
        let server = test_server("password", None);
        let mut args = Vec::new();
        push_key_args(&server, &mut args);
        assert!(has_opt(&args, "PubkeyAuthentication=no"));
    }

    #[test]
    fn key_auth_still_uses_identities_only() {
        let server = test_server("key", Some("/home/user/.ssh/id_ed25519"));
        let mut args = Vec::new();
        push_key_args(&server, &mut args);
        assert!(has_opt(&args, "IdentitiesOnly=yes"));
        assert!(args.iter().any(|arg| arg == "/home/user/.ssh/id_ed25519"));
        assert!(!has_opt(&args, "PubkeyAuthentication=no"));
    }

    #[test]
    fn one_shot_argv_never_needs_remote_command_text() {
        let signature: fn(&Server) -> Result<(String, Vec<String>)> = exec_argv;
        let _ = signature;
    }

    #[test]
    fn captured_output_uses_the_central_redactor() {
        const SECRET: &str = "ssh-manager-test-secret-canary";
        redaction::register_secret(SECRET);
        let output = redaction::redact_command_output(CommandOutput {
            stdout: format!("token {SECRET} token"),
            stderr: SECRET.into(),
            exit_code: 0,
            success: true,
        });
        assert_eq!(output.stdout, "token •••••• token");
        assert_eq!(output.stderr, "••••••");
    }
}
