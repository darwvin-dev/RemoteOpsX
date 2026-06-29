//! SSH tunnel manager.
//!
//! MVP uses the system ssh client with `-L` (local forward), `-R` (remote
//! forward) and `-D` (dynamic SOCKS) in `-N` mode. Each tunnel is a tracked
//! child process; the registry lets the UI list and stop them. Profiles are
//! persisted to SQLite by the caller.

use std::collections::HashMap;
use std::process::{Child, Command};
use std::sync::Mutex;

use anyhow::{anyhow, Result};

use crate::models::{Server, Tunnel};
use crate::ssh_manager;

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub(crate) struct TunnelValidationError {
    pub field: &'static str,
    message: String,
}

impl TunnelValidationError {
    fn new(field: &'static str, message: impl Into<String>) -> Self {
        Self {
            field,
            message: message.into(),
        }
    }
}

/// Push auth-related ssh options for `server` onto `args`. Mirrors
/// `ssh_manager`'s handling so tunnels get the same MaxAuthTries protection.
fn push_auth_args(server: &Server, args: &mut Vec<String>) {
    if server.auth_type == "key" {
        if let Some(key) = &server.private_key_path {
            if !key.trim().is_empty() {
                args.push("-i".into());
                args.push(key.clone());
                // Only use this key (avoid agent-key MaxAuthTries rejection).
                args.push("-o".into());
                args.push("IdentitiesOnly=yes".into());
            }
        }
    } else if server.auth_type == "password" {
        // Without this, ssh still offers every ssh-agent key before falling
        // back to password auth, which can exhaust the remote's
        // MaxAuthTries ("Too many authentication failures") first.
        args.push("-o".into());
        args.push("PubkeyAuthentication=no".into());
    }
}

pub(crate) fn validate_tunnel(tunnel: &Tunnel) -> Result<(), TunnelValidationError> {
    if tunnel.id.trim().is_empty() {
        return Err(TunnelValidationError::new("id", "tunnel id is required"));
    }
    if tunnel.server_id.trim().is_empty() {
        return Err(TunnelValidationError::new(
            "server_id",
            "server id is required",
        ));
    }
    if tunnel.local_port == 0 {
        return Err(TunnelValidationError::new(
            "local_port",
            "local port must be between 1 and 65535",
        ));
    }
    match tunnel.r#type.as_str() {
        "dynamic" => Ok(()),
        "local" | "remote" => {
            if tunnel
                .remote_host
                .as_deref()
                .map_or(true, |host| host.trim().is_empty())
            {
                return Err(TunnelValidationError::new(
                    "remote_host",
                    "remote host is required",
                ));
            }
            if tunnel.remote_port.map_or(true, |port| port == 0) {
                return Err(TunnelValidationError::new(
                    "remote_port",
                    "remote port must be between 1 and 65535",
                ));
            }
            Ok(())
        }
        other => Err(TunnelValidationError::new(
            "type",
            format!("unknown tunnel type: {other}"),
        )),
    }
}

#[derive(Default)]
pub struct TunnelManager {
    procs: Mutex<HashMap<String, Child>>,
}

impl TunnelManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a tunnel described by `tunnel` against `server`. The tunnel id is
    /// used as the registry key.
    pub fn start(&self, server: &Server, tunnel: &Tunnel) -> Result<()> {
        validate_tunnel(tunnel)?;
        let mut args: Vec<String> = vec![
            "-N".into(),
            "-o".into(),
            "StrictHostKeyChecking=accept-new".into(),
            "-o".into(),
            "ExitOnForwardFailure=yes".into(),
            "-p".into(),
            server.port.to_string(),
        ];

        push_auth_args(server, &mut args);

        let local_host = tunnel
            .local_host
            .clone()
            .unwrap_or_else(|| "127.0.0.1".into());
        match tunnel.r#type.as_str() {
            "local" => {
                let rh = tunnel
                    .remote_host
                    .clone()
                    .unwrap_or_else(|| "127.0.0.1".into());
                let rp = tunnel
                    .remote_port
                    .ok_or_else(|| anyhow!("remote_port required for local forward"))?;
                args.push("-L".into());
                args.push(format!(
                    "{}:{}:{}:{}",
                    local_host, tunnel.local_port, rh, rp
                ));
            }
            "remote" => {
                let rh = tunnel
                    .remote_host
                    .clone()
                    .unwrap_or_else(|| "127.0.0.1".into());
                let rp = tunnel
                    .remote_port
                    .ok_or_else(|| anyhow!("remote_port required for remote forward"))?;
                args.push("-R".into());
                args.push(format!(
                    "{}:{}:{}:{}",
                    local_host, tunnel.local_port, rh, rp
                ));
            }
            "dynamic" => {
                args.push("-D".into());
                args.push(format!("{}:{}", local_host, tunnel.local_port));
            }
            other => return Err(anyhow!("unknown tunnel type: {other}")),
        }

        args.push(format!("{}@{}", server.username, server.host));

        let (program, full_args) = if server.auth_type == "password" {
            // Reuse the password-wrapping logic for consistency.
            let mut wrapped = vec!["-e".to_string(), "ssh".to_string()];
            wrapped.extend(args);
            ("sshpass".to_string(), wrapped)
        } else {
            ("ssh".to_string(), args)
        };

        let mut cmd = Command::new(&program);
        cmd.args(&full_args);
        ssh_manager::apply_password_env(&mut cmd, server);

        let mut child = cmd
            .spawn()
            .map_err(|e| anyhow!("failed to start tunnel: {e}"))?;
        for _ in 0..4 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            if let Some(status) = child.try_wait()? {
                return Err(anyhow!("SSH tunnel exited during startup with {status}"));
            }
        }
        self.procs.lock().unwrap().insert(tunnel.id.clone(), child);
        Ok(())
    }

    /// Stop a running tunnel.
    pub fn stop(&self, id: &str) -> Result<()> {
        if let Some(mut child) = self.procs.lock().unwrap().remove(id) {
            let _ = child.kill();
        }
        Ok(())
    }

    /// Return the set of tunnel ids currently alive (reaps exited ones).
    pub fn active_ids(&self) -> Vec<String> {
        let mut guard = self.procs.lock().unwrap();
        let mut alive = Vec::new();
        let mut dead = Vec::new();
        for (id, child) in guard.iter_mut() {
            match child.try_wait() {
                Ok(Some(_)) => dead.push(id.clone()),
                _ => alive.push(id.clone()),
            }
        }
        for id in dead {
            guard.remove(&id);
        }
        alive
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tunnel(kind: &str) -> Tunnel {
        Tunnel {
            id: "tunnel-1".into(),
            server_id: "server-1".into(),
            r#type: kind.into(),
            local_host: Some("127.0.0.1".into()),
            local_port: 8080,
            remote_host: Some("127.0.0.1".into()),
            remote_port: Some(80),
            status: "pending".into(),
            created_at: String::new(),
        }
    }

    fn server(auth_type: &str, key_path: Option<&str>) -> Server {
        Server {
            id: "server-1".into(),
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
    fn password_auth_tunnel_disables_pubkey_so_agent_keys_cant_exhaust_maxauthtries() {
        let srv = server("password", None);
        let mut args = Vec::new();
        push_auth_args(&srv, &mut args);

        assert!(
            has_opt(&args, "PubkeyAuthentication=no"),
            "password-auth tunnels must disable pubkey auth, otherwise ssh \
             offers every ssh-agent key first and a busy agent exhausts the \
             remote's MaxAuthTries before the tunnel password is ever tried: {args:?}"
        );
    }

    #[test]
    fn key_auth_tunnel_still_uses_identities_only() {
        let srv = server("key", Some("/home/user/.ssh/id_ed25519"));
        let mut args = Vec::new();
        push_auth_args(&srv, &mut args);

        assert!(has_opt(&args, "IdentitiesOnly=yes"));
        assert!(args.iter().any(|a| a == "/home/user/.ssh/id_ed25519"));
        assert!(!has_opt(&args, "PubkeyAuthentication=no"));
    }

    #[test]
    fn rejects_invalid_tunnel_parameters() {
        let mut value = tunnel("local");
        value.local_port = 0;
        assert!(validate_tunnel(&value).is_err());
        value.local_port = 8080;
        value.remote_port = None;
        assert!(validate_tunnel(&value).is_err());
        value.remote_port = Some(80);
        value.r#type = "invalid".into();
        assert!(validate_tunnel(&value).is_err());
    }

    #[test]
    fn accepts_supported_tunnel_shapes() {
        assert!(validate_tunnel(&tunnel("local")).is_ok());
        assert!(validate_tunnel(&tunnel("remote")).is_ok());
        let mut dynamic = tunnel("dynamic");
        dynamic.remote_host = None;
        dynamic.remote_port = None;
        assert!(validate_tunnel(&dynamic).is_ok());
    }
}
