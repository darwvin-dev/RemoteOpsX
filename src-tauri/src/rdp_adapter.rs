//! RDP adapter.
//!
//! MVP strategy: launch the system FreeRDP client (`xfreerdp` / `xfreerdp3`) as
//! an external window, while the session record stays inside RemoteOpsX. The
//! adapter is deliberately a thin trait-like surface so an embedded RDP canvas
//! can replace `launch` later without changing callers.

use std::process::Command;

use anyhow::{anyhow, Result};

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

/// Launch an external FreeRDP window for the given server.
pub fn launch(server: &Server, opts: &RdpOptions) -> Result<()> {
    let bin = freerdp_bin().ok_or_else(|| {
        anyhow!("xfreerdp not found. Install FreeRDP (e.g. `pacman -S freerdp` / `apt install freerdp2-x11`).")
    })?;

    let mut args: Vec<String> = vec![
        format!("/v:{}:{}", server.host, server.rdp_port()),
        format!("/u:{}", server.username),
        "/cert:ignore".into(),
        "+clipboard".into(),
    ];

    if let Some(pw) = vault::get_secret(&vault::secret_ref(&server.id))
        .ok()
        .flatten()
    {
        // FreeRDP reads /p:<pw>; argv exposure is a known FreeRDP limitation.
        args.push(format!("/p:{pw}"));
    }
    if opts.fullscreen {
        args.push("/f".into());
    }
    if let Some(res) = &opts.resolution {
        if let Some((w, h)) = res.split_once('x') {
            args.push(format!("/w:{}", w.trim()));
            args.push(format!("/h:{}", h.trim()));
        }
    }

    Command::new(bin)
        .args(&args)
        .spawn()
        .map_err(|e| anyhow!("failed to launch {bin}: {e}"))?;
    Ok(())
}
