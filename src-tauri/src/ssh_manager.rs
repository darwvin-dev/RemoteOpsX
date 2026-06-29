//! SSH command construction + one-shot remote execution.
//!
//! For the MVP we drive the system OpenSSH client (`ssh`) rather than a native
//! Rust SSH stack. This keeps auth (agent, keys, known_hosts, GSSAPI, jump
//! hosts) behaving exactly like the user's shell. A clean abstraction here
//! means a native transport can replace it later without touching callers.
//!
//! Two execution modes share the same argument builder:
//!   * interactive PTY (see `pty_manager`) — the terminal tab
//!   * one-shot exec (`run_remote`) — health, runbooks, services, sftp

use std::process::Command;

use anyhow::{anyhow, Result};

use crate::models::{CommandOutput, Server};
use crate::vault;

/// Common ssh options applied to every connection.
/// `accept-new` trusts first-seen host keys but still detects key changes.
fn base_opts() -> Vec<String> {
    vec![
        "-o".into(),
        "StrictHostKeyChecking=accept-new".into(),
        "-o".into(),
        "ConnectTimeout=12".into(),
        "-o".into(),
        "ServerAliveInterval=15".into(),
    ]
}

/// True if a stored password should be injected via `sshpass`.
fn wants_password(server: &Server) -> bool {
    server.auth_type == "password"
}

/// Resolve the secret for a server from the keyring (if any).
fn lookup_secret(server: &Server) -> Option<String> {
    vault::get_secret(&vault::secret_ref(&server.id))
        .ok()
        .flatten()
}

/// Append `-i <key>` plus `IdentitiesOnly=yes` for key-based servers.
///
/// `IdentitiesOnly=yes` is essential: without it, ssh first offers every key in
/// the user's agent. On a host with many agent keys the server can hit
/// `MaxAuthTries` and reject the connection ("Too many authentication
/// failures") before our configured key is ever tried.
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
        // Without this, ssh still offers every ssh-agent key (and default
        // identity files) before falling back to password auth. On a host
        // with several agent keys loaded, the server's MaxAuthTries can be
        // exhausted by those pubkey attempts alone, and sshd disconnects with
        // "Too many authentication failures" before the password is tried.
        args.push("-o".into());
        args.push("PubkeyAuthentication=no".into());
    }
}

/// Build the full argv for an *interactive* ssh session (terminal tab).
/// Returns (program, args). When a password is configured and `sshpass`
/// exists, the program becomes `sshpass` wrapping `ssh`.
pub fn interactive_argv(server: &Server) -> Result<(String, Vec<String>)> {
    let mut args: Vec<String> = base_opts();

    // Force a PTY so remote shells render correctly.
    args.push("-tt".into());
    args.push("-p".into());
    args.push(server.port.to_string());

    push_key_args(server, &mut args);

    args.push(format!("{}@{}", server.username, server.host));

    wrap_with_password(server, "ssh", args)
}

/// Build argv for a one-shot remote command.
fn exec_argv(server: &Server, remote_command: &str) -> Result<(String, Vec<String>)> {
    let mut args: Vec<String> = base_opts();
    args.push("-o".into());
    // Non-interactive: never hang waiting on a prompt unless sshpass feeds it.
    args.push(if wants_password(server) {
        "BatchMode=no".into()
    } else {
        "BatchMode=yes".into()
    });
    args.push("-p".into());
    args.push(server.port.to_string());

    push_key_args(server, &mut args);

    args.push(format!("{}@{}", server.username, server.host));
    args.push(remote_command.to_string());

    wrap_with_password(server, "ssh", args)
}

/// If the server uses password auth and `sshpass` is installed, wrap the call
/// so the password is fed on stdin (never the process table / logs). Otherwise
/// return ssh directly (key / agent auth).
fn wrap_with_password(
    server: &Server,
    program: &str,
    args: Vec<String>,
) -> Result<(String, Vec<String>)> {
    if wants_password(server) {
        match lookup_secret(server) {
            Some(_) if sshpass_available() => {
                // `-p` would leak via argv; `-e` reads SSHPASS from the env we
                // set on the Command just before spawning. Marker arg here.
                let mut wrapped = vec!["-e".to_string(), program.to_string()];
                wrapped.extend(args);
                Ok(("sshpass".to_string(), wrapped))
            }
            Some(_) => Err(anyhow!(
                "This server uses password auth but `sshpass` is not installed. \
                 Install sshpass, or switch the profile to key-based auth."
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

/// Inject SSHPASS into a Command's environment if this server uses password
/// auth. Call right before spawning. Keeps the secret out of argv/logs.
pub fn apply_password_env(cmd: &mut Command, server: &Server) {
    if wants_password(server) {
        if let Some(pw) = lookup_secret(server) {
            cmd.env("SSHPASS", pw);
        }
    }
}

fn redact_text(text: String, secret: Option<&str>) -> String {
    match secret {
        Some(secret) if secret.len() >= 4 && text.contains(secret) => {
            text.replace(secret, "••••••")
        }
        _ => text,
    }
}

fn redact_output(server: &Server, output: CommandOutput) -> CommandOutput {
    let secret = lookup_secret(server);
    CommandOutput {
        stdout: redact_text(output.stdout, secret.as_deref()),
        stderr: redact_text(output.stderr, secret.as_deref()),
        ..output
    }
}

/// Execute a remote command and capture stdout/stderr/exit code.
/// This is the workhorse for health, runbooks and services.
pub fn run_remote(server: &Server, remote_command: &str) -> Result<CommandOutput> {
    let (program, args) = exec_argv(server, remote_command)?;
    let mut cmd = Command::new(&program);
    cmd.args(&args);
    apply_password_env(&mut cmd, server);

    let output = cmd
        .output()
        .map_err(|e| anyhow!("failed to spawn ssh: {e}"))?;
    let exit_code = output.status.code().unwrap_or(-1);
    Ok(redact_output(
        server,
        CommandOutput {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code,
            success: output.status.success(),
        },
    ))
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
            private_key_path: key_path.map(|s| s.to_string()),
            tags: vec![],
            group_name: None,
            environment: "dev".into(),
            notes: None,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn has_opt(args: &[String], value: &str) -> bool {
        args.windows(2).any(|w| w[0] == "-o" && w[1] == value)
    }

    #[test]
    fn password_auth_disables_pubkey_so_agent_keys_cant_exhaust_maxauthtries() {
        let server = test_server("password", None);
        let mut args = base_opts();
        push_key_args(&server, &mut args);

        assert!(
            has_opt(&args, "PubkeyAuthentication=no"),
            "password auth must disable pubkey auth, otherwise ssh offers every \
             ssh-agent key first and a busy agent exhausts the remote's \
             MaxAuthTries (\"Too many authentication failures\") before the \
             password is ever tried: {args:?}"
        );
    }

    #[test]
    fn key_auth_still_uses_identities_only() {
        let server = test_server("key", Some("/home/user/.ssh/id_ed25519"));
        let mut args = base_opts();
        push_key_args(&server, &mut args);

        assert!(has_opt(&args, "IdentitiesOnly=yes"));
        assert!(args.iter().any(|a| a == "/home/user/.ssh/id_ed25519"));
        assert!(!has_opt(&args, "PubkeyAuthentication=no"));
    }

    #[test]
    fn redacts_stored_secret_from_captured_output() {
        let server = test_server("password", None);
        let output = redact_output(
            &server,
            CommandOutput {
                stdout: "prefix password123 suffix".into(),
                stderr: "password123".into(),
                exit_code: 0,
                success: true,
            },
        );

        assert_eq!(output.stdout, "prefix password123 suffix");
        assert_eq!(output.stderr, "password123");

        let output = CommandOutput {
            stdout: redact_text("token abcdef token".into(), Some("abcdef")),
            stderr: redact_text("short abc token".into(), Some("abc")),
            exit_code: 0,
            success: true,
        };
        assert_eq!(output.stdout, "token •••••• token");
        assert_eq!(output.stderr, "short abc token");
    }
}
