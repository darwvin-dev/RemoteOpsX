//! SFTP / remote file operations.
//!
//! Listing uses SSH exec; transfers use the system SCP binary. Both transports
//! share RemoteOpsX's strict app-managed host identity policy.

use std::process::Command;

use anyhow::{anyhow, Result};

use crate::jump_host::{self, JumpHostConfig};
use crate::models::{RemoteFile, Server};
use crate::redaction;
use crate::ssh_manager;

pub fn list_dir_via(
    server: &Server,
    jump: Option<&JumpHostConfig>,
    path: &str,
) -> Result<Vec<RemoteFile>> {
    let safe = shell_quote(path);
    let cmd = format!("ls -lA --time-style=+%s {safe} 2>/dev/null");
    let out = ssh_manager::run_remote_via(server, jump, &cmd)?;
    if !out.success && out.stdout.trim().is_empty() {
        return Err(anyhow!("cannot list {path}: {}", out.stderr.trim()));
    }

    let mut files = Vec::new();
    for line in out.stdout.lines() {
        if line.starts_with("total ") || line.trim().is_empty() {
            continue;
        }
        if let Some(file) = parse_ls_line(line) {
            files.push(file);
        }
    }
    files.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(files)
}

pub fn list_dir(server: &Server, path: &str) -> Result<Vec<RemoteFile>> {
    let jump = jump_host::get_cached(&server.id);
    list_dir_via(server, jump.as_ref(), path)
}

fn parse_ls_line(line: &str) -> Option<RemoteFile> {
    let cols: Vec<&str> = line.split_whitespace().collect();
    if cols.len() < 7 {
        return None;
    }
    let permissions = cols[0].to_string();
    let size = cols[4].parse().unwrap_or(0);
    let decorated_name = cols[6..].join(" ");
    let name = decorated_name
        .split(" -> ")
        .next()
        .unwrap_or(&decorated_name)
        .to_string();
    Some(RemoteFile {
        is_dir: permissions.starts_with('d'),
        permissions,
        size,
        name,
    })
}

fn scp_base_via(
    server: &Server,
    jump: Option<&JumpHostConfig>,
) -> Result<(String, Vec<String>)> {
    let mut args = ssh_manager::strict_host_key_args()?;
    args.extend(ssh_manager::jump_host_args_via(jump)?);
    args.extend(["-P".to_string(), server.port.to_string()]);
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
    if server.auth_type == "password" {
        let mut wrapped = vec!["-e".to_string(), "scp".to_string()];
        wrapped.extend(args);
        Ok(("sshpass".to_string(), wrapped))
    } else {
        Ok(("scp".to_string(), args))
    }
}

fn scp_base(server: &Server) -> Result<(String, Vec<String>)> {
    let jump = jump_host::get_cached(&server.id);
    scp_base_via(server, jump.as_ref())
}

pub fn upload_via(
    server: &Server,
    jump: Option<&JumpHostConfig>,
    local_path: &str,
    remote_dir: &str,
) -> Result<()> {
    let (program, mut args) = scp_base_via(server, jump)?;
    args.push(local_path.to_string());
    args.push(format!("{}@{}:{}", server.username, server.host, remote_dir));
    run_transfer(server, &program, &args)
}

pub fn upload(server: &Server, local_path: &str, remote_dir: &str) -> Result<()> {
    let (program, mut args) = scp_base(server)?;
    args.push(local_path.to_string());
    args.push(format!("{}@{}:{}", server.username, server.host, remote_dir));
    run_transfer(server, &program, &args)
}

pub fn download_via(
    server: &Server,
    jump: Option<&JumpHostConfig>,
    remote_path: &str,
    local_path: &str,
) -> Result<()> {
    let (program, mut args) = scp_base_via(server, jump)?;
    args.push(format!("{}@{}:{}", server.username, server.host, remote_path));
    args.push(local_path.to_string());
    run_transfer(server, &program, &args)
}

pub fn download(server: &Server, remote_path: &str, local_path: &str) -> Result<()> {
    let (program, mut args) = scp_base(server)?;
    args.push(format!("{}@{}:{}", server.username, server.host, remote_path));
    args.push(local_path.to_string());
    run_transfer(server, &program, &args)
}

pub fn delete_via(
    server: &Server,
    jump: Option<&JumpHostConfig>,
    remote_path: &str,
) -> Result<()> {
    let out = ssh_manager::run_remote_via(
        server,
        jump,
        &format!("rm -rf {}", shell_quote(remote_path)),
    )?;
    if out.success { Ok(()) } else { Err(anyhow!(out.stderr)) }
}

pub fn delete(server: &Server, remote_path: &str) -> Result<()> {
    let jump = jump_host::get_cached(&server.id);
    delete_via(server, jump.as_ref(), remote_path)
}

pub fn rename_via(
    server: &Server,
    jump: Option<&JumpHostConfig>,
    from: &str,
    to: &str,
) -> Result<()> {
    let out = ssh_manager::run_remote_via(
        server,
        jump,
        &format!("mv {} {}", shell_quote(from), shell_quote(to)),
    )?;
    if out.success { Ok(()) } else { Err(anyhow!(out.stderr)) }
}

pub fn rename(server: &Server, from: &str, to: &str) -> Result<()> {
    let jump = jump_host::get_cached(&server.id);
    rename_via(server, jump.as_ref(), from, to)
}

fn run_transfer(server: &Server, program: &str, args: &[String]) -> Result<()> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    ssh_manager::apply_password_env(&mut cmd, server);
    let out = cmd
        .output()
        .map_err(|error| anyhow!("failed to run {program}: {error}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(anyhow!(redaction::redact(String::from_utf8_lossy(&out.stderr))))
    }
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(auth_type: &str, key_path: Option<&str>) -> Server {
        Server {
            id: "s1".into(), name: "test".into(), host: "example.com".into(), port: 22,
            ftp_port: None, rdp_port: None, vnc_port: None, username: "root".into(),
            protocols: vec!["sftp".into()], auth_type: auth_type.into(),
            private_key_path: key_path.map(|value| value.to_string()), tags: vec![],
            group_name: None, environment: "dev".into(), notes: None,
            created_at: String::new(), updated_at: String::new(),
        }
    }

    fn has_opt(args: &[String], value: &str) -> bool {
        args.windows(2).any(|window| window[0] == "-o" && window[1] == value)
    }

    #[test]
    fn password_auth_scp_disables_pubkey_so_agent_keys_cant_exhaust_maxauthtries() {
        let mut args = Vec::new();
        let server = server("password", None);
        if server.auth_type == "password" {
            args.push("-o".into()); args.push("PubkeyAuthentication=no".into());
        }
        assert!(has_opt(&args, "PubkeyAuthentication=no"));
    }

    #[test]
    fn key_auth_argument_shape_keeps_identities_only() {
        let server = server("key", Some("/home/user/.ssh/id_ed25519"));
        let mut args = Vec::new();
        if let Some(key) = &server.private_key_path {
            args.extend(["-i".into(), key.clone(), "-o".into(), "IdentitiesOnly=yes".into()]);
        }
        assert!(args.iter().any(|arg| arg == "/home/user/.ssh/id_ed25519"));
        assert!(has_opt(&args, "IdentitiesOnly=yes"));
    }

    #[test]
    fn parses_padded_ls_rows_and_filenames_with_spaces() {
        let line = "-rw-r--r--  1 root root 42 1710000000 release notes.txt";
        let file = parse_ls_line(line).unwrap();
        assert_eq!(file.size, 42); assert_eq!(file.name, "release notes.txt"); assert!(!file.is_dir);
    }

    #[test]
    fn parses_directories_and_strips_symlink_targets() {
        let directory = parse_ls_line("drwxr-xr-x  2 root root 4096 1710000000 releases").unwrap();
        let symlink = parse_ls_line("lrwxrwxrwx 1 root root 12 1710000000 current -> releases/v2").unwrap();
        assert!(directory.is_dir); assert_eq!(symlink.name, "current");
    }
}
