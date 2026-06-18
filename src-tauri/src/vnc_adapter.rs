//! VNC adapter.
//!
//! MVP strategy: launch an installed VNC viewer as an external window. We probe
//! a few common binaries. Like the RDP adapter, this is intentionally a thin
//! launch surface so an embedded VNC client can be slotted in later.

use std::process::Command;

use anyhow::{anyhow, Result};

use crate::models::Server;

#[derive(Debug, serde::Deserialize)]
pub struct VncOptions {
    #[serde(default)]
    pub fullscreen: bool,
}

/// Candidate VNC viewer binaries, in preference order.
fn vnc_bin() -> Option<&'static str> {
    for bin in ["vncviewer", "vinagre", "remmina", "gvncviewer", "xtigervncviewer"] {
        // `command -v` style probe: spawning with no args is unreliable, so we
        // check existence via `which`-equivalent (try to spawn --help).
        if Command::new(bin).arg("--help").output().is_ok() {
            return Some(bin);
        }
    }
    None
}

/// Launch an external VNC viewer for the given server.
pub fn launch(server: &Server, opts: &VncOptions) -> Result<()> {
    let bin = vnc_bin().ok_or_else(|| {
        anyhow!("No VNC viewer found. Install one (e.g. `pacman -S tigervnc` / `apt install tigervnc-viewer`).")
    })?;

    let port = if server.port == 22 { 5900 } else { server.port };
    let target = format!("{}:{}", server.host, port);

    let mut cmd = Command::new(bin);
    match bin {
        "remmina" => {
            cmd.arg(format!("vnc://{target}"));
        }
        "vinagre" => {
            cmd.arg(format!("vnc://{target}"));
        }
        _ => {
            // tigervnc / tightvnc style: `vncviewer host:port`
            if opts.fullscreen {
                cmd.arg("-FullScreen");
            }
            cmd.arg(&target);
        }
    }

    cmd.spawn().map_err(|e| anyhow!("failed to launch {bin}: {e}"))?;
    Ok(())
}
