//! Local runtime prerequisite checks.
//!
//! RemoteOpsX deliberately uses several well-understood system clients for its
//! transport surface. This module makes those requirements visible before an
//! operator discovers them through a failed production action.

use std::process::Command;

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RuntimeDependency {
    pub id: String,
    pub name: String,
    pub required_for: Vec<String>,
    pub required: bool,
    pub available: bool,
    pub detail: String,
}

fn command_available(program: &str, args: &[&str]) -> bool {
    Command::new(program).args(args).output().is_ok()
}

fn dependency(
    id: &str,
    name: &str,
    required_for: &[&str],
    required: bool,
    available: bool,
    install_hint: &str,
) -> RuntimeDependency {
    RuntimeDependency {
        id: id.into(),
        name: name.into(),
        required_for: required_for.iter().map(|v| (*v).to_string()).collect(),
        required,
        available,
        detail: if available {
            "available".into()
        } else {
            install_hint.into()
        },
    }
}

pub fn collect() -> Vec<RuntimeDependency> {
    let ssh = command_available("ssh", &["-V"]);
    let scp = command_available("scp", &["-V"]);
    let curl = command_available("curl", &["--version"]);
    let sshpass = command_available("sshpass", &["-h"]);
    let freerdp = ["xfreerdp3", "xfreerdp"]
        .into_iter()
        .any(|bin| command_available(bin, &["--version"]));

    #[cfg(target_os = "macos")]
    let vnc = command_available("open", &["--help"]);

    #[cfg(not(target_os = "macos"))]
    let vnc = [
        "vncviewer",
        "vinagre",
        "remmina",
        "gvncviewer",
        "xtigervncviewer",
    ]
    .into_iter()
    .any(|bin| command_available(bin, &["--help"]));

    vec![
        dependency(
            "openssh",
            "OpenSSH client",
            &["SSH terminal", "health", "services", "runbooks", "tunnels"],
            true,
            ssh,
            "Install an OpenSSH client and ensure `ssh` is on PATH.",
        ),
        dependency(
            "scp",
            "SCP client",
            &["SFTP-style file transfers"],
            true,
            scp,
            "Install the OpenSSH client package that provides `scp`.",
        ),
        dependency(
            "curl",
            "curl",
            &["legacy FTP"],
            false,
            curl,
            "Install curl if legacy FTP profiles are required.",
        ),
        dependency(
            "sshpass",
            "sshpass",
            &["password SSH/SCP"],
            false,
            sshpass,
            "Install sshpass when using password-auth SSH/SFTP profiles; key auth does not require it.",
        ),
        dependency(
            "freerdp",
            "FreeRDP viewer",
            &["RDP"],
            false,
            freerdp,
            "Install FreeRDP (`xfreerdp3` or `xfreerdp`) to launch RDP sessions.",
        ),
        dependency(
            "vnc-viewer",
            "VNC viewer",
            &["VNC"],
            false,
            vnc,
            if cfg!(target_os = "macos") {
                "macOS Screen Sharing (`open`) is unavailable."
            } else {
                "Install TigerVNC, Remmina, Vinagre or another supported VNC viewer."
            },
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dependency_metadata_is_stable_and_unique() {
        let deps = collect();
        let mut ids: Vec<&str> = deps.iter().map(|d| d.id.as_str()).collect();
        let original_len = ids.len();
        ids.sort_unstable();
        ids.dedup();

        assert_eq!(ids.len(), original_len);
        assert!(deps.iter().any(|d| d.id == "openssh" && d.required));
        assert!(deps.iter().any(|d| d.id == "scp" && d.required));
        assert!(deps.iter().any(|d| d.id == "sshpass" && !d.required));
        assert!(deps.iter().all(|d| !d.required_for.is_empty()));
    }
}
