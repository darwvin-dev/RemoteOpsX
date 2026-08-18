//! RDP adapter.
//!
//! RemoteOpsX launches the system FreeRDP client (`xfreerdp` / `xfreerdp3`) as
//! an external window. Credentials are kept out of argv and server certificates
//! use trust-on-first-use instead of being accepted unconditionally.

use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Context, Result};

use crate::models::Server;
use crate::vault;

/// Options coming from the RDP tab UI.
#[derive(Debug, serde::Deserialize)]
pub struct RdpOptions {
    #[serde(default)]
    pub fullscreen: bool,
    /// e.g. "1920x1080"; empty -> client default.
    #[serde(default)]
    pub resolution: Option<String>,
}

fn freerdp_bin() -> Option<&'static str> {
    ["xfreerdp3", "xfreerdp"]
        .into_iter()
        .find(|bin| Command::new(bin).arg("--version").output().is_ok())
}

fn build_args(server: &Server, opts: &RdpOptions, password_from_stdin: bool) -> Vec<String> {
    let mut args: Vec<String> = vec![
        format!("/v:{}:{}", server.host, server.rdp_port()),
        format!("/u:{}", server.username),
        // Trust the first certificate we see for this endpoint, then reject a
        // changed certificate on later connections. `/cert:ignore` would make
        // active interception invisible to the user.
        "/cert:tofu".into(),
        "+clipboard".into(),
    ];

    if password_from_stdin {
        // FreeRDP explicitly supports credential input through stdin. This
        // avoids `/p:<password>`, which exposes the password in process argv.
        args.push("/from-stdin:force".into());
    }
    if opts.fullscreen {
        args.push("/f".into());
    }
    if let Some(res) = &opts.resolution {
        if let Some((w, h)) = res.split_once('x') {
            let w = w.trim();
            let h = h.trim();
            if !w.is_empty() && !h.is_empty() {
                args.push(format!("/w:{w}"));
                args.push(format!("/h:{h}"));
            }
        }
    }

    args
}

/// Launch an external FreeRDP window for the given server.
pub fn launch(server: &Server, opts: &RdpOptions) -> Result<()> {
    let bin = freerdp_bin().ok_or_else(|| {
        anyhow!("xfreerdp not found. Install FreeRDP (e.g. `pacman -S freerdp` / `apt install freerdp2-x11`).")
    })?;

    let password = vault::get_secret(&vault::secret_ref(&server.id))
        .ok()
        .flatten();
    let args = build_args(server, opts, password.is_some());

    let mut command = Command::new(bin);
    command.args(&args);
    if password.is_some() {
        command.stdin(Stdio::piped());
    } else {
        command.stdin(Stdio::null());
    }

    let mut child = command
        .spawn()
        .map_err(|e| anyhow!("failed to launch {bin}: {e}"))?;

    if let Some(password) = password {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("failed to open FreeRDP credential input"))?;
        stdin
            .write_all(password.as_bytes())
            .context("failed to send credentials to FreeRDP")?;
        stdin
            .write_all(b"\n")
            .context("failed to finish FreeRDP credential input")?;
        // Closing stdin lets FreeRDP continue immediately after consuming the
        // password and avoids keeping a credential-bearing pipe open.
        drop(stdin);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server() -> Server {
        Server {
            id: "server-1".into(),
            name: "rdp-test".into(),
            host: "rdp.example.com".into(),
            port: 22,
            ftp_port: None,
            rdp_port: Some(3390),
            vnc_port: None,
            username: "operator".into(),
            protocols: vec!["rdp".into()],
            auth_type: "password".into(),
            private_key_path: None,
            tags: vec![],
            group_name: None,
            environment: "production".into(),
            notes: None,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn rdp_uses_tofu_instead_of_disabling_certificate_validation() {
        let args = build_args(
            &server(),
            &RdpOptions {
                fullscreen: false,
                resolution: None,
            },
            false,
        );

        assert!(args.iter().any(|arg| arg == "/cert:tofu"));
        assert!(!args.iter().any(|arg| arg == "/cert:ignore"));
    }

    #[test]
    fn stored_password_is_requested_from_stdin_and_never_placed_in_argv() {
        let args = build_args(
            &server(),
            &RdpOptions {
                fullscreen: true,
                resolution: Some("1920x1080".into()),
            },
            true,
        );

        assert!(args.iter().any(|arg| arg == "/from-stdin:force"));
        assert!(!args.iter().any(|arg| arg.starts_with("/p:")));
        assert!(args.iter().any(|arg| arg == "/f"));
        assert!(args.iter().any(|arg| arg == "/w:1920"));
        assert!(args.iter().any(|arg| arg == "/h:1080"));
    }
}
