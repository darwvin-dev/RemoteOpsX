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
        // `ls` pads columns with a variable number of spaces. Using `splitn`
        // with `char::is_whitespace` counts those empty separators toward the
        // limit and silently drops otherwise valid entries. Collecting the
        // whitespace-delimited fields also preserves filenames containing
        // spaces by joining everything after the six metadata columns.
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
                args.push("-o".into());
                args.push("IdentitiesOnly=yes".into());
            }
        }
    } else if server.auth_type == "password" {
        // Without this, scp still offers every ssh-agent key before falling
        // back to password auth, which can exhaust the remote's
        // MaxAuthTries ("Too many authentication failures") first.
        args.push("-o".into());
        args.push("PubkeyAuthentication=no".into());
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

#[cfg(test)]
mod tests {
    use super::*;

    fn server(auth_type: &str, key_path: Option<&str>) -> Server {
        Server {
            id: "s1".into(),
            name: "test".into(),
            host: "example.com".into(),
            port: 22,
            ftp_port: None,
            rdp_port: None,
            vnc_port: None,
            username: "root".into(),
            protocols: vec!["sftp".into()],
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
    fn password_auth_scp_disables_pubkey_so_agent_keys_cant_exhaust_maxauthtries() {
        let (_program, args) = scp_base(&server("password", None));

        assert!(
            has_opt(&args, "PubkeyAuthentication=no"),
            "password-auth scp transfers must disable pubkey auth, otherwise \
             ssh offers every ssh-agent key first and a busy agent exhausts \
             the remote's MaxAuthTries before the password is ever tried: {args:?}"
        );
    }

    #[test]
    fn key_auth_scp_still_uses_identities_only() {
        let (program, args) = scp_base(&server("key", Some("/home/user/.ssh/id_ed25519")));

        assert_eq!(program, "scp");
        assert!(args.iter().any(|a| a == "/home/user/.ssh/id_ed25519"));
        assert!(has_opt(&args, "IdentitiesOnly=yes"));
        assert!(!has_opt(&args, "PubkeyAuthentication=no"));
    }

    #[test]
    fn parses_padded_ls_rows_and_filenames_with_spaces() {
        let line = "-rw-r--r--  1 root root 42 1710000000 release notes.txt";
        let file = parse_ls_line(line).unwrap();
        assert_eq!(file.size, 42);
        assert_eq!(file.name, "release notes.txt");
        assert!(!file.is_dir);
    }

    #[test]
    fn parses_directories_and_strips_symlink_targets() {
        let directory = parse_ls_line("drwxr-xr-x  2 root root 4096 1710000000 releases").unwrap();
        let symlink = parse_ls_line("lrwxrwxrwx 1 root root 12 1710000000 current -> releases/v2").unwrap();
        assert!(directory.is_dir);
        assert_eq!(symlink.name, "current");
    }
}
