use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SshKeyInfo {
    pub name: String,
    pub path: String,
    pub public_key_path: Option<String>,
    pub public_key_preview: Option<String>,
}

pub fn discover_local_keys() -> Result<Vec<SshKeyInfo>> {
    let home = std::env::var_os("HOME").ok_or_else(|| anyhow!("HOME is not set"))?;
    discover_keys_in_dir(Path::new(&home).join(".ssh"))
}

pub fn discover_keys_in_dir(dir: impl AsRef<Path>) -> Result<Vec<SshKeyInfo>> {
    let dir = dir.as_ref();
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut keys = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() || !is_private_key_candidate(&path) || !looks_like_private_key(&path) {
            continue;
        }
        let public_key_path = adjacent_public_key_path(&path);
        let public_key_preview = public_key_path
            .as_ref()
            .and_then(|path| fs::read_to_string(path).ok())
            .and_then(|value| first_public_key_line(&value));
        keys.push(SshKeyInfo {
            name: path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| path.display().to_string()),
            path: path.to_string_lossy().to_string(),
            public_key_path: public_key_path.map(|path| path.to_string_lossy().to_string()),
            public_key_preview,
        });
    }
    keys.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(keys)
}

pub fn public_key_for_private_key(private_key_path: impl AsRef<Path>) -> Result<String> {
    let private_key_path = expand_home(private_key_path.as_ref());
    if let Some(public_key_path) = adjacent_public_key_path(&private_key_path) {
        let value = fs::read_to_string(&public_key_path)
            .with_context(|| format!("failed to read {}", public_key_path.display()))?;
        if let Some(line) = first_public_key_line(&value) {
            return Ok(line);
        }
    }

    let output = Command::new("ssh-keygen")
        .arg("-y")
        .arg("-f")
        .arg(&private_key_path)
        .output()
        .with_context(|| "failed to run ssh-keygen")?;
    if !output.status.success() {
        return Err(anyhow!(
            "ssh-keygen could not derive a public key for {}: {}",
            private_key_path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    first_public_key_line(&stdout).ok_or_else(|| {
        anyhow!(
            "ssh-keygen did not return a public key for {}",
            private_key_path.display()
        )
    })
}

pub fn authorized_keys_install_command(public_key: &str) -> String {
    let public_key = public_key.trim();
    let key = shell_quote(public_key);
    format!(
        "mkdir -p ~/.ssh && chmod 700 ~/.ssh && touch ~/.ssh/authorized_keys && chmod 600 ~/.ssh/authorized_keys && grep -qxF {key} ~/.ssh/authorized_keys || cat >> ~/.ssh/authorized_keys <<'REMOTEOPSX_PUBLIC_KEY'\n{public_key}\nREMOTEOPSX_PUBLIC_KEY"
    )
}

pub fn is_private_key_candidate(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if name.starts_with('.') || name.ends_with(".pub") {
        return false;
    }
    !matches!(
        name,
        "config" | "known_hosts" | "known_hosts.old" | "authorized_keys" | "allowed_signers"
    )
}

fn looks_like_private_key(path: &Path) -> bool {
    let Ok(value) = fs::read_to_string(path) else {
        return false;
    };
    value.contains("-----BEGIN OPENSSH PRIVATE KEY-----")
        || value.contains("-----BEGIN RSA PRIVATE KEY-----")
        || value.contains("-----BEGIN DSA PRIVATE KEY-----")
        || value.contains("-----BEGIN EC PRIVATE KEY-----")
        || value.contains("-----BEGIN PRIVATE KEY-----")
}

fn adjacent_public_key_path(private_key_path: &Path) -> Option<PathBuf> {
    let public_key_path = PathBuf::from(format!("{}.pub", private_key_path.to_string_lossy()));
    public_key_path.exists().then_some(public_key_path)
}

fn first_public_key_line(value: &str) -> Option<String> {
    value
        .lines()
        .map(str::trim)
        .find(|line| {
            line.starts_with("ssh-") || line.starts_with("ecdsa-") || line.starts_with("sk-")
        })
        .map(str::to_string)
}

fn expand_home(path: &Path) -> PathBuf {
    let value = path.to_string_lossy();
    if value == "~" {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home);
        }
    }
    if let Some(rest) = value.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    path.to_path_buf()
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("remoteopsx-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn key_candidate_filter_skips_public_and_config_files() {
        assert!(is_private_key_candidate(Path::new("id_ed25519")));
        assert!(is_private_key_candidate(Path::new("customer.pem")));
        assert!(!is_private_key_candidate(Path::new("id_ed25519.pub")));
        assert!(!is_private_key_candidate(Path::new("known_hosts")));
        assert!(!is_private_key_candidate(Path::new("config")));
        assert!(!is_private_key_candidate(Path::new("authorized_keys")));
    }

    #[test]
    fn discovers_private_keys_with_adjacent_public_key_previews() {
        let dir = temp_dir("discover-keys");
        fs::write(
            dir.join("id_ed25519"),
            "-----BEGIN OPENSSH PRIVATE KEY-----\nfake\n-----END OPENSSH PRIVATE KEY-----\n",
        )
        .unwrap();
        fs::write(
            dir.join("id_ed25519.pub"),
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIFakeKeyExample user@example\n",
        )
        .unwrap();
        fs::write(dir.join("known_hosts"), "example ssh-ed25519 AAAA").unwrap();
        fs::write(dir.join("notes.txt"), "not a key").unwrap();

        let keys = discover_keys_in_dir(&dir).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].name, "id_ed25519");
        assert_eq!(keys[0].path, dir.join("id_ed25519").to_string_lossy());
        assert_eq!(
            keys[0].public_key_path.as_deref(),
            Some(dir.join("id_ed25519.pub").to_string_lossy().as_ref())
        );
        assert_eq!(
            keys[0].public_key_preview.as_deref(),
            Some("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIFakeKeyExample user@example")
        );
    }

    #[test]
    fn authorized_keys_install_command_is_idempotent_and_quotes_values() {
        let command = authorized_keys_install_command("ssh-ed25519 AAAAC3NzaC1 test@example");

        assert!(command.contains("mkdir -p ~/.ssh"));
        assert!(command.contains("chmod 700 ~/.ssh"));
        assert!(command.contains("touch ~/.ssh/authorized_keys"));
        assert!(command.contains("grep -qxF"));
        assert!(command.contains("cat >> ~/.ssh/authorized_keys"));
        assert!(command.contains("'ssh-ed25519 AAAAC3NzaC1 test@example'"));
    }
}
