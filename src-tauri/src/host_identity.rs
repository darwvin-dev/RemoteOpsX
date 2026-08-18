//! App-managed SSH host identity trust.
//!
//! RemoteOpsX owns a dedicated known_hosts file under the application data
//! directory. All SSH/SCP/tunnel transports use that file with strict host
//! verification. First contact therefore requires an explicit fingerprint
//! review + trust action in the UI; changed keys remain blocked until the
//! operator explicitly replaces the trusted identity.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{anyhow, Context, Result};
use once_cell::sync::OnceCell;
use serde::Serialize;

static KNOWN_HOSTS_PATH: OnceCell<PathBuf> = OnceCell::new();

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HostKeyCandidate {
    pub key_type: String,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HostIdentityReport {
    pub host: String,
    pub port: u16,
    /// unseen | trusted | changed
    pub status: String,
    pub candidates: Vec<HostKeyCandidate>,
    pub trusted_fingerprints: Vec<String>,
}

pub fn init(path: PathBuf) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("failed to create known_hosts directory")?;
    }
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .context("failed to create RemoteOpsX known_hosts file")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&path)?.permissions();
        permissions.set_mode(0o600);
        fs::set_permissions(&path, permissions)?;
    }

    KNOWN_HOSTS_PATH
        .set(path)
        .map_err(|_| anyhow!("RemoteOpsX known_hosts path was already initialized"))?;
    Ok(())
}

pub fn known_hosts_path() -> Result<PathBuf> {
    KNOWN_HOSTS_PATH
        .get()
        .cloned()
        .ok_or_else(|| anyhow!("RemoteOpsX known_hosts manager is not initialized"))
}

fn strict_options_for_path(path: &Path) -> Vec<String> {
    vec![
        "-o".into(),
        format!("UserKnownHostsFile={}", path.to_string_lossy()),
        // Do not let system-wide known_hosts or a configured helper silently
        // establish trust outside RemoteOpsX's explicit fingerprint workflow.
        "-o".into(),
        "GlobalKnownHostsFile=/dev/null".into(),
        "-o".into(),
        "KnownHostsCommand=none".into(),
        "-o".into(),
        "StrictHostKeyChecking=yes".into(),
        // Secure SSHFP records and OpenSSH's host-key update extension are
        // useful in other contexts, but both would create trust outside the
        // app-managed known_hosts review boundary.
        "-o".into(),
        "VerifyHostKeyDNS=no".into(),
        "-o".into(),
        "UpdateHostKeys=no".into(),
        "-o".into(),
        "NoHostAuthenticationForLocalhost=no".into(),
    ]
}

pub fn strict_ssh_options() -> Result<Vec<String>> {
    Ok(strict_options_for_path(&known_hosts_path()?))
}

pub fn inspect(host: &str, port: u16) -> Result<HostIdentityReport> {
    validate_target(host, port)?;
    let scanned = scan_lines(host, port)?;
    if scanned.is_empty() {
        return Err(anyhow!(
            "No SSH host key was returned by {}:{}. Check DNS/network/port before trusting.",
            host,
            port
        ));
    }

    let candidates = fingerprints(&scanned)?;
    let trusted_lines = trusted_lines(host, port)?;
    let trusted = fingerprints(&trusted_lines)?;
    let candidate_fingerprints: Vec<&str> = candidates
        .iter()
        .map(|value| value.fingerprint.as_str())
        .collect();
    let status = if trusted.is_empty() {
        "unseen"
    } else if trusted
        .iter()
        .any(|value| candidate_fingerprints.contains(&value.fingerprint.as_str()))
    {
        "trusted"
    } else {
        "changed"
    };

    Ok(HostIdentityReport {
        host: host.to_string(),
        port,
        status: status.to_string(),
        candidates,
        trusted_fingerprints: trusted
            .into_iter()
            .map(|value| value.fingerprint)
            .collect(),
    })
}

pub fn trust(
    host: &str,
    port: u16,
    expected_fingerprint: &str,
    replace: bool,
) -> Result<HostIdentityReport> {
    validate_target(host, port)?;
    let before = inspect(host, port)?;

    match before.status.as_str() {
        "trusted" if !replace => return Ok(before),
        "changed" if !replace => {
            return Err(anyhow!(
                "The stored SSH identity does not match the scanned host. Verify the new fingerprint out-of-band, then use Replace explicitly."
            ));
        }
        _ => {}
    }

    // Re-scan immediately before persistence so a fingerprint that disappeared
    // between preview and Trust/Replace cannot be written accidentally.
    let scanned = scan_lines(host, port)?;
    let scanned_with_fingerprints = scanned
        .iter()
        .map(|line| Ok((line.clone(), fingerprint(line)?)))
        .collect::<Result<Vec<_>>>()?;
    let selected = scanned_with_fingerprints
        .iter()
        .find(|(_, candidate)| candidate.fingerprint == expected_fingerprint)
        .ok_or_else(|| {
            anyhow!(
                "The expected fingerprint is no longer offered by the server. Scan again before trusting."
            )
        })?;

    if replace || before.status == "unseen" {
        remove(host, port)?;
    }

    let path = known_hosts_path()?;
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    writeln!(file, "{}", selected.0)?;
    file.sync_all()?;

    inspect(host, port)
}

pub fn remove(host: &str, port: u16) -> Result<()> {
    validate_target(host, port)?;
    let path = known_hosts_path()?;
    let target = known_hosts_target(host, port);
    let current = fs::read_to_string(&path).unwrap_or_default();
    let retained = current
        .lines()
        .filter(|line| !line_matches_target(line, &target))
        .collect::<Vec<_>>()
        .join("\n");
    let tmp = path.with_extension("known_hosts.tmp");
    fs::write(
        &tmp,
        if retained.is_empty() {
            String::new()
        } else {
            format!("{retained}\n")
        },
    )?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&tmp)?.permissions();
        permissions.set_mode(0o600);
        fs::set_permissions(&tmp, permissions)?;
    }

    fs::rename(tmp, path)?;
    Ok(())
}

fn validate_target(host: &str, port: u16) -> Result<()> {
    if host.trim().is_empty()
        || host.chars().any(char::is_whitespace)
        || host.chars().any(char::is_control)
    {
        return Err(anyhow!("invalid SSH host name"));
    }
    if port == 0 {
        return Err(anyhow!("invalid SSH port"));
    }
    Ok(())
}

fn known_hosts_target(host: &str, port: u16) -> String {
    if port == 22 {
        host.to_string()
    } else {
        format!("[{host}]:{port}")
    }
}

fn line_matches_target(line: &str, target: &str) -> bool {
    line.split_whitespace()
        .next()
        .is_some_and(|hosts| hosts.split(',').any(|value| value == target))
}

fn normalize_scanned_line(line: &str, host: &str, port: u16) -> Option<String> {
    let mut columns = line.split_whitespace();
    let _reported_host = columns.next()?;
    let key_type = columns.next()?;
    let key = columns.next()?;
    Some(format!(
        "{} {} {}",
        known_hosts_target(host, port),
        key_type,
        key
    ))
}

fn scan_lines(host: &str, port: u16) -> Result<Vec<String>> {
    let port_string = port.to_string();
    let output = Command::new("ssh-keyscan")
        .args(["-T", "5", "-p", port_string.as_str(), "--", host])
        .output()
        .map_err(|error| {
            anyhow!("failed to run ssh-keyscan: {error}. Install OpenSSH client tools.")
        })?;
    if !output.status.success() && output.stdout.is_empty() {
        return Err(anyhow!(
            "ssh-keyscan failed for {host}:{port}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .filter_map(|line| normalize_scanned_line(line, host, port))
        .collect())
}

fn trusted_lines(host: &str, port: u16) -> Result<Vec<String>> {
    let target = known_hosts_target(host, port);
    let content = fs::read_to_string(known_hosts_path()?).unwrap_or_default();
    Ok(content
        .lines()
        .filter(|line| line_matches_target(line, &target))
        .map(str::to_string)
        .collect())
}

fn fingerprints(lines: &[String]) -> Result<Vec<HostKeyCandidate>> {
    lines.iter().map(|line| fingerprint(line)).collect()
}

fn fingerprint(line: &str) -> Result<HostKeyCandidate> {
    let key_type = line
        .split_whitespace()
        .nth(1)
        .unwrap_or("unknown")
        .to_string();
    let mut child = Command::new("ssh-keygen")
        .args(["-lf", "-", "-E", "sha256"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            anyhow!("failed to run ssh-keygen: {error}. Install OpenSSH client tools.")
        })?;
    child
        .stdin
        .as_mut()
        .context("failed to open ssh-keygen stdin")?
        .write_all(format!("{line}\n").as_bytes())?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(anyhow!(
            "ssh-keygen could not fingerprint the scanned host key"
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let fingerprint = stdout
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| anyhow!("ssh-keygen returned an unexpected fingerprint format"))?
        .to_string();
    Ok(HostKeyCandidate {
        key_type,
        fingerprint,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_transport_options_require_explicit_app_managed_trust() {
        let args = strict_options_for_path(Path::new("/tmp/remoteopsx-known-hosts"));
        for required in [
            "StrictHostKeyChecking=yes",
            "GlobalKnownHostsFile=/dev/null",
            "KnownHostsCommand=none",
            "VerifyHostKeyDNS=no",
            "UpdateHostKeys=no",
            "NoHostAuthenticationForLocalhost=no",
        ] {
            assert!(args.iter().any(|arg| arg == required), "missing {required}");
        }
        assert!(args
            .iter()
            .any(|arg| arg == "UserKnownHostsFile=/tmp/remoteopsx-known-hosts"));
        assert!(!args.iter().any(|arg| arg.contains("accept-new")));
        assert!(!args.iter().any(|arg| arg == "StrictHostKeyChecking=no"));
    }

    #[test]
    fn normalizes_default_and_nonstandard_port_targets() {
        assert_eq!(known_hosts_target("example.com", 22), "example.com");
        assert_eq!(
            known_hosts_target("example.com", 2222),
            "[example.com]:2222"
        );
    }

    #[test]
    fn target_matching_does_not_remove_neighboring_hosts() {
        assert!(line_matches_target(
            "example.com ssh-ed25519 AAAA",
            "example.com"
        ));
        assert!(!line_matches_target(
            "example.com.evil ssh-ed25519 AAAA",
            "example.com"
        ));
    }

    #[test]
    fn scanned_lines_are_rewritten_to_the_exact_connection_target() {
        assert_eq!(
            normalize_scanned_line("10.0.0.1 ssh-ed25519 AAAA", "db.internal", 2222).as_deref(),
            Some("[db.internal]:2222 ssh-ed25519 AAAA")
        );
    }
}
