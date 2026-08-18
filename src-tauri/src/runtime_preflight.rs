//! Runtime dependency and keyring readiness checks.
//!
//! These checks are side-effect free. They inspect PATH for the external
//! clients RemoteOpsX delegates to and probe the keyring without creating a
//! credential. Feature-specific tools remain optional globally and are made
//! required by the Diagnostics UI only when the selected profile uses them.

use std::env;
use std::path::PathBuf;

use serde::Serialize;

use crate::vault;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DependencyStatus {
    pub id: String,
    pub label: String,
    pub required: bool,
    pub available: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RuntimePreflightReport {
    pub ready: bool,
    pub dependencies: Vec<DependencyStatus>,
}

pub fn collect() -> RuntimePreflightReport {
    let mut dependencies = vec![
        binary("ssh", "OpenSSH client", true, &["ssh"]),
        binary("scp", "OpenSSH SCP", true, &["scp"]),
        binary(
            "ssh-keyscan",
            "OpenSSH host-key scanner",
            true,
            &["ssh-keyscan"],
        ),
        binary(
            "ssh-keygen",
            "OpenSSH fingerprint tool",
            true,
            &["ssh-keygen"],
        ),
        binary("sshpass", "SSH password helper", false, &["sshpass"]),
        binary("curl", "curl (legacy FTP)", false, &["curl"]),
        binary(
            "freerdp",
            "FreeRDP",
            false,
            &["xfreerdp3", "xfreerdp"],
        ),
        vnc_status(),
    ];

    let keyring = match vault::probe() {
        Ok(()) => DependencyStatus {
            id: "keyring".into(),
            label: "OS keyring / Secret Service".into(),
            required: false,
            available: true,
            detail: "Keyring backend is reachable.".into(),
        },
        Err(error) => DependencyStatus {
            id: "keyring".into(),
            label: "OS keyring / Secret Service".into(),
            required: false,
            available: false,
            detail: format!("Keyring is unavailable: {error}"),
        },
    };
    dependencies.push(keyring);

    let ready = dependencies
        .iter()
        .filter(|dependency| dependency.required)
        .all(|dependency| dependency.available);
    RuntimePreflightReport {
        ready,
        dependencies,
    }
}

#[cfg(target_os = "macos")]
fn vnc_status() -> DependencyStatus {
    binary(
        "vnc",
        "macOS Screen Sharing",
        false,
        &["open", "vncviewer", "remmina"],
    )
}

#[cfg(not(target_os = "macos"))]
fn vnc_status() -> DependencyStatus {
    binary(
        "vnc",
        "VNC viewer",
        false,
        &[
            "vncviewer",
            "vinagre",
            "remmina",
            "gvncviewer",
            "xtigervncviewer",
        ],
    )
}

fn binary(id: &str, label: &str, required: bool, alternatives: &[&str]) -> DependencyStatus {
    if let Some((name, path)) = alternatives
        .iter()
        .find_map(|name| find_in_path(name).map(|path| ((*name).to_string(), path)))
    {
        DependencyStatus {
            id: id.into(),
            label: label.into(),
            required,
            available: true,
            detail: format!("{}: {}", name, path.to_string_lossy()),
        }
    } else {
        DependencyStatus {
            id: id.into(),
            label: label.into(),
            required,
            available: false,
            detail: format!(
                "Not found in PATH. Expected one of: {}",
                alternatives.join(", ")
            ),
        }
    }
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_binary_report_is_actionable() {
        let status = binary(
            "definitely-missing",
            "Missing test binary",
            false,
            &["remoteopsx-binary-that-does-not-exist-7f11"],
        );
        assert!(!status.available);
        assert!(status.detail.contains("Not found in PATH"));
        assert!(status
            .detail
            .contains("remoteopsx-binary-that-does-not-exist-7f11"));
    }

    #[test]
    fn optional_feature_does_not_fail_global_core_readiness() {
        let status = binary(
            "optional-missing",
            "Optional missing binary",
            false,
            &["remoteopsx-optional-binary-that-does-not-exist"],
        );
        assert!(!status.required);
        assert!(!status.available);
    }
}
