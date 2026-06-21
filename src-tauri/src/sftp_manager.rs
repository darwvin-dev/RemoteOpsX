//! SFTP / remote file operations.
//!
//! MVP implements list/upload/download/delete/rename. Listing uses an `ssh`
//! exec of `ls -la` (parsed), which avoids a persistent SFTP subsystem session
//! while still being reliable. Transfers use the system `scp` binary. The
//! surface is small and self-contained so a native SFTP client can replace it.

use std::process::Command;

use anyhow::{anyhow, Result};

use crate::models::{RemoteFile, Server};
use crate::ssh_manager;

/// List a remote directory. Returns entries sorted dirs-first.
pub fn list_dir(server: &Server, path: &str) -> Result<Vec<RemoteFile>> {
    // -A: include dotfiles (not . / ..); --time-style for stable columns.
    let safe = shell_quote(path);
    let cmd = format!("ls -lA --time-style=+%s {safe} 2>/dev/null");
    let out = ssh_manager::run_remote(server, &cmd)?;
    if !out.success && out.stdout.trim().is_empty() {
        return Err(anyhow!("cannot list {path}: {}", out.stderr.trim()));
    }

    let mut files = Vec::new();
    for line in out.stdout.lines() {
        // Skip the "total N" header and blank lines.
        if line.starts_with("total ") || line.trim().is_empty() {
            continue;
        }
        // perms links owner group size epoch name...
        let cols: Vec<&str> = line
            .splitn(7, char::is_whitespace)
            .filter(|s| !s.is_empty())
            .collect();
        if cols.len() < 7 {
            continue;
        }
        let perms = cols[0];
        let size: u64 = cols[4].parse().unwrap_or(0);
        let name = cols[6].to_string();
        // Strip symlink "-> target" decoration.
        let name = name.split(" -> ").next().unwrap_or(&name).to_string();
        files.push(RemoteFile {
            is_dir: perms.starts_with('d'),
            permissions: perms.to_string(),
            size,
            name,
        });
    }
    files.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(files)
}

/// Build the scp remote spec, honoring port/key/password.
fn scp_base(server: &Server) -> (String, Vec<String>) {
    let mut args = vec![
        "-o".to_string(),
        "StrictHostKeyChecking=accept-new".to_string(),
        "-P".to_string(),
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
    if server.auth_type == "password" {
        let mut wrapped = vec!["-e".to_string(), "scp".to_string()];
        wrapped.extend(args);
        ("sshpass".to_string(), wrapped)
    } else {
        ("scp".to_string(), args)
    }
}

/// Upload a local file to a remote directory.
pub fn upload(server: &Server, local_path: &str, remote_dir: &str) -> Result<()> {
    let (program, mut args) = scp_base(server);
    args.push(local_path.to_string());
    args.push(format!(
        "{}@{}:{}",
        server.username, server.host, remote_dir
    ));
    run_transfer(server, &program, &args)
}

/// Download a remote file to a local directory.
pub fn download(server: &Server, remote_path: &str, local_path: &str) -> Result<()> {
    let (program, mut args) = scp_base(server);
    args.push(format!(
        "{}@{}:{}",
        server.username, server.host, remote_path
    ));
    args.push(local_path.to_string());
    run_transfer(server, &program, &args)
}

pub fn delete(server: &Server, remote_path: &str) -> Result<()> {
    let out = ssh_manager::run_remote(server, &format!("rm -rf {}", shell_quote(remote_path)))?;
    if out.success {
        Ok(())
    } else {
        Err(anyhow!(out.stderr))
    }
}

pub fn rename(server: &Server, from: &str, to: &str) -> Result<()> {
    let out = ssh_manager::run_remote(
        server,
        &format!("mv {} {}", shell_quote(from), shell_quote(to)),
    )?;
    if out.success {
        Ok(())
    } else {
        Err(anyhow!(out.stderr))
    }
}

fn run_transfer(server: &Server, program: &str, args: &[String]) -> Result<()> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    ssh_manager::apply_password_env(&mut cmd, server);
    let out = cmd
        .output()
        .map_err(|e| anyhow!("failed to run {program}: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(anyhow!(String::from_utf8_lossy(&out.stderr).to_string()))
    }
}

/// Minimal single-quote shell escaping for paths.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
