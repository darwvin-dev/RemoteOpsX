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

fn validate_tunnel(tunnel: &Tunnel) -> Result<()> {
    if tunnel.id.trim().is_empty() {
        return Err(anyhow!("tunnel id is required"));
    }
    if tunnel.local_port == 0 {
        return Err(anyhow!("local port must be between 1 and 65535"));
    }
    match tunnel.r#type.as_str() {
        "dynamic" => Ok(()),
        "local" | "remote" => {
            if tunnel
                .remote_host
                .as_deref()
                .map_or(true, |host| host.trim().is_empty())
            {
                return Err(anyhow!("remote host is required"));
            }
            if tunnel.remote_port.map_or(true, |port| port == 0) {
                return Err(anyhow!("remote port must be between 1 and 65535"));
            }
            Ok(())
        }
        other => Err(anyhow!("unknown tunnel type: {other}")),
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
        }

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
