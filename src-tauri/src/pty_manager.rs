//! Interactive terminal PTY manager.
//!
//! Each SSH terminal tab maps to one PTY running the system `ssh` client.
//! A dedicated reader thread pumps PTY output to the frontend via Tauri events
//! (`pty://output/<id>`), so the UI never blocks on I/O. The frontend sends
//! keystrokes back through the `pty_write` command and size changes through
//! `pty_resize`.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Mutex;

use anyhow::{anyhow, Result};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use tauri::{AppHandle, Emitter};

use crate::models::Server;
use crate::ssh_manager;

/// One live PTY-backed SSH session.
struct PtySession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
}

/// Registry of all live terminal sessions, keyed by a UI-supplied id.
#[derive(Default)]
pub struct PtyManager {
    sessions: Mutex<HashMap<String, PtySession>>,
}

impl PtyManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Spawn an interactive ssh session inside a PTY. `id` is chosen by the
    /// frontend (one per terminal tab). Output is streamed via events.
    pub fn spawn(&self, app: AppHandle, id: String, server: &Server, cols: u16, rows: u16) -> Result<()> {
        let (program, args) = ssh_manager::interactive_argv(server)?;

        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows: rows.max(1),
            cols: cols.max(1),
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut cmd = CommandBuilder::new(&program);
        for a in &args {
            cmd.arg(a);
        }
        // A sane TERM so curses apps (htop, vim) render.
        cmd.env("TERM", "xterm-256color");
        // Feed the password to sshpass -e via the environment, never argv.
        if program == "sshpass" {
            if let Some(pw) = crate::vault::get_secret(&crate::vault::secret_ref(&server.id)).ok().flatten() {
                cmd.env("SSHPASS", pw);
            }
        }

        let child = pair.slave.spawn_command(cmd)?;
        // Drop the slave handle so EOF propagates when the child exits.
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;

        // Reader thread: pump bytes to the UI until EOF.
        let ev = format!("pty://output/{id}");
        let exit_ev = format!("pty://exit/{id}");
        let app_for_thread = app.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        // Send raw bytes; the frontend feeds them to xterm,
                        // which handles partial UTF-8 sequences correctly.
                        let _ = app_for_thread.emit(&ev, buf[..n].to_vec());
                    }
                    Err(_) => break,
                }
            }
            let _ = app_for_thread.emit(&exit_ev, ());
        });

        self.sessions.lock().unwrap().insert(
            id,
            PtySession {
                master: pair.master,
                writer,
                child,
            },
        );
        Ok(())
    }

    /// Write user keystrokes to the PTY.
    pub fn write(&self, id: &str, data: &[u8]) -> Result<()> {
        let mut guard = self.sessions.lock().unwrap();
        let session = guard.get_mut(id).ok_or_else(|| anyhow!("no such pty session"))?;
        session.writer.write_all(data)?;
        session.writer.flush()?;
        Ok(())
    }

    /// Resize the PTY to match the xterm viewport.
    pub fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<()> {
        let guard = self.sessions.lock().unwrap();
        let session = guard.get(id).ok_or_else(|| anyhow!("no such pty session"))?;
        session.master.resize(PtySize {
            rows: rows.max(1),
            cols: cols.max(1),
            pixel_width: 0,
            pixel_height: 0,
        })?;
        Ok(())
    }

    /// Kill and remove a session (tab closed or reconnect requested).
    pub fn close(&self, id: &str) -> Result<()> {
        if let Some(mut session) = self.sessions.lock().unwrap().remove(id) {
            let _ = session.child.kill();
        }
        Ok(())
    }
}
