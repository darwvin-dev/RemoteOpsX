//! Plain FTP file operations.
//!
//! This adapter intentionally uses the system `curl` binary so the MVP can
//! support legacy FTP servers without adding a native transport dependency.
//! Credentials are passed through curl's stdin config (`--config -`) instead of
//! argv, keeping passwords out of the process list.

use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Result};

use crate::models::{RemoteFile, Server};
use crate::vault;

pub fn list_dir(server: &Server, path: &str) -> Result<Vec<RemoteFile>> {
    let url = ftp_url(server, path, true);
    let out = run_curl(server, &base_args_with_url(url))?;
    if !out.status.success() {
        return Err(anyhow!(String::from_utf8_lossy(&out.stderr).to_string()));
    }

    let mut files = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if line.trim().is_empty() {
            continue;
        }
        files.push(parse_list_line(line));
    }
    files.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(files)
}

pub fn upload(server: &Server, local_path: &str, remote_dir: &str) -> Result<()> {
    let url = ftp_url(server, remote_dir, true);
    let mut args = base_args();
    args.extend([
        "--ftp-create-dirs".into(),
        "--upload-file".into(),
        local_path.into(),
        url,
    ]);
    let out = run_curl(server, &args)?;
    status_result(out, "upload")
}

pub fn download(server: &Server, remote_path: &str, local_path: &str) -> Result<()> {
    let url = ftp_url(server, remote_path, false);
    let mut args = base_args();
    args.extend(["--output".into(), local_path.into(), url]);
    let out = run_curl(server, &args)?;
    status_result(out, "download")
}

pub fn delete(server: &Server, remote_path: &str) -> Result<()> {
    let delete_file = run_quote(server, &[format!("DELE {}", ftp_command_path(remote_path))]);
    if delete_file.is_ok() {
        return Ok(());
    }
    run_quote(server, &[format!("RMD {}", ftp_command_path(remote_path))])
}

pub fn rename(server: &Server, from: &str, to: &str) -> Result<()> {
    run_quote(
        server,
        &[
            format!("RNFR {}", ftp_command_path(from)),
            format!("RNTO {}", ftp_command_path(to)),
        ],
    )
}

fn run_quote(server: &Server, quotes: &[String]) -> Result<()> {
    let mut args = quote_args(quotes);
    let url = ftp_url(server, "/", true);
    args.push(url);
    let out = run_curl(server, &args)?;
    status_result(out, "ftp command")
}

fn base_args() -> Vec<String> {
    ["--fail", "--silent", "--show-error", "--path-as-is"]
        .into_iter()
        .map(str::to_owned)
        .collect()
}

fn base_args_with_url(url: String) -> Vec<String> {
    let mut args = base_args();
    args.push(url);
    args
}

fn quote_args(quotes: &[String]) -> Vec<String> {
    let mut args = base_args();
    for quote in quotes {
        args.push("--quote".into());
        args.push(quote.clone());
    }
    args
}

fn run_curl(server: &Server, args: &[String]) -> Result<std::process::Output> {
    let mut child = Command::new("curl")
        .arg("--config")
        .arg("-")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            anyhow!("failed to run curl for FTP: {e}. Install curl to use FTP profiles.")
        })?;

    let password = vault::get_secret(&vault::secret_ref(&server.id))
        .ok()
        .flatten()
        .unwrap_or_default();
    let config = format!(
        "user = \"{}\"\n",
        curl_cfg_value(&format!("{}:{password}", server.username))
    );
    child
        .stdin
        .as_mut()
        .ok_or_else(|| anyhow!("failed to open curl stdin"))?
        .write_all(config.as_bytes())?;

    child
        .wait_with_output()
        .map_err(|e| anyhow!("failed to read curl output: {e}"))
}

fn status_result(out: std::process::Output, action: &str) -> Result<()> {
    if out.status.success() {
        Ok(())
    } else {
        Err(anyhow!(
            "{action} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ))
    }
}

fn parse_list_line(line: &str) -> RemoteFile {
    let columns: Vec<&str> = line.split_whitespace().collect();
    if columns.len() >= 9 && columns[0].len() >= 10 {
        let permissions = columns[0].to_string();
        let size = columns[4].parse().unwrap_or(0);
        return RemoteFile {
            name: columns[8..].join(" "),
            is_dir: permissions.starts_with('d'),
            size,
            permissions,
        };
    }

    RemoteFile {
        name: line.trim().to_string(),
        is_dir: false,
        size: 0,
        permissions: "----------".to_string(),
    }
}

fn ftp_url(server: &Server, path: &str, directory: bool) -> String {
    let normalized = normalize_path(path, directory);
    format!(
        "ftp://{}:{}{}",
        server.host,
        server.ftp_port(),
        percent_encode_path(&normalized)
    )
}

fn normalize_path(path: &str, directory: bool) -> String {
    let mut normalized = if path.trim().is_empty() {
        "/".to_string()
    } else if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    if directory && !normalized.ends_with('/') {
        normalized.push('/');
    }
    normalized
}

fn percent_encode_path(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

fn ftp_command_path(path: &str) -> String {
    if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}

fn curl_cfg_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_owned_quote_arguments_in_order() {
        let quotes = vec!["RNFR /old name".to_string(), "RNTO /new name".to_string()];
        assert_eq!(
            quote_args(&quotes),
            vec![
                "--fail",
                "--silent",
                "--show-error",
                "--path-as-is",
                "--quote",
                "RNFR /old name",
                "--quote",
                "RNTO /new name",
            ]
        );
    }

    #[test]
    fn normalizes_and_encodes_remote_paths() {
        assert_eq!(normalize_path("folder name", true), "/folder name/");
        assert_eq!(
            percent_encode_path("/folder name/file#1"),
            "/folder%20name/file%231"
        );
    }

    #[test]
    fn parses_unix_list_entries_with_spaces() {
        let file = parse_list_line("-rw-r--r-- 1 user group 42 Jan 01 12:00 report final.txt");
        assert_eq!(file.name, "report final.txt");
        assert_eq!(file.size, 42);
        assert!(!file.is_dir);
    }
}
