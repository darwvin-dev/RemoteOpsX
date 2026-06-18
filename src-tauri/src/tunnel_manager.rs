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
                }
            }
        }

        let local_host = tunnel.local_host.clone().unwrap_or_else(|| "127.0.0.1".into());
        match tunnel.r#type.as_str() {
            "local" => {
                let rh = tunnel.remote_host.clone().unwrap_or_else(|| "127.0.0.1".into());
                let rp = tunnel.remote_port.ok_or_else(|| anyhow!("remote_port required for local forward"))?;
                args.push("-L".into());
                args.push(format!("{}:{}:{}:{}", local_host, tunnel.local_port, rh, rp));
            }
            "remote" => {
                let rh = tunnel.remote_host.clone().unwrap_or_else(|| "127.0.0.1".into());
                let rp = tunnel.remote_port.ok_or_else(|| anyhow!("remote_port required for remote forward"))?;
                args.push("-R".into());
                args.push(format!("{}:{}:{}:{}", local_host, tunnel.local_port, rh, rp));
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

        let child = cmd.spawn().map_err(|e| anyhow!("failed to start tunnel: {e}"))?;
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
