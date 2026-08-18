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

const REDACTION: &[u8] =
    b"\xE2\x80\xA2\xE2\x80\xA2\xE2\x80\xA2\xE2\x80\xA2\xE2\x80\xA2\xE2\x80\xA2";

#[derive(Debug, Clone)]
struct StreamRedactor {
    secret: Option<Vec<u8>>,
    carry: Vec<u8>,
}

impl StreamRedactor {
    fn new(secret: Option<String>) -> Self {
        let secret = secret
            .filter(|secret| secret.len() >= 4)
            .map(String::into_bytes);
        Self {
            secret,
            carry: Vec::new(),
        }
    }

    fn push(&mut self, chunk: &[u8]) -> Vec<u8> {
        let Some(secret) = &self.secret else {
            return chunk.to_vec();
        };
        let keep = secret.len().saturating_sub(1);
        let mut combined = std::mem::take(&mut self.carry);
        combined.extend_from_slice(chunk);
        if combined.len() <= keep {
            self.carry = combined;
            return Vec::new();
        }
        replace_all(&mut combined, secret, REDACTION);
        let emit_len = combined.len() - keep;
        let emit = combined[..emit_len].to_vec();
        self.carry = combined[emit_len..].to_vec();
        emit
    }

    fn finish(&mut self) -> Vec<u8> {
        let Some(secret) = &self.secret else {
            return std::mem::take(&mut self.carry);
        };
        let mut emit = std::mem::take(&mut self.carry);
        replace_all(&mut emit, secret, REDACTION);
        emit
    }
}

fn replace_all(buffer: &mut Vec<u8>, needle: &[u8], replacement: &[u8]) {
    if needle.is_empty() {
        return;
    }
    let mut index = 0;
    let mut output = Vec::with_capacity(buffer.len());
    while let Some(offset) = find_bytes(&buffer[index..], needle) {
        let match_start = index + offset;
        output.extend_from_slice(&buffer[index..match_start]);
        output.extend_from_slice(replacement);
        index = match_start + needle.len();
    }
    output.extend_from_slice(&buffer[index..]);
    *buffer = output;
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

struct PtySession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
}

#[derive(Default)]
pub struct PtyManager {
    sessions: Mutex<HashMap<String, PtySession>>,
}

impl PtyManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn spawn(
        &self,
        app: AppHandle,
        id: String,
        server: &Server,
        cols: u16,
        rows: u16,
    ) -> Result<()> {
        let (program, args) = ssh_manager::interactive_argv(server)?;

        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows: rows.max(1),
            cols: cols.max(1),
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut cmd = CommandBuilder::new(&program);
        for arg in &args {
            cmd.arg(arg);
        }
        cmd.env("TERM", "xterm-256color");

        // Feed the password to sshpass -e through the child environment, never
        // process argv, and retain a copy only for the streaming redactor.
        let stored_secret = if program == "sshpass" {
            crate::vault::get_secret(&crate::vault::secret_ref(&server.id))
                .ok()
                .flatten()
        } else {
            None
        };
        if let Some(password) = stored_secret.as_deref() {
            cmd.env("SSHPASS", password);
        }

        let child = pair.slave.spawn_command(cmd)?;
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;

        let output_event = format!("pty://output/{id}");
        let exit_event = format!("pty://exit/{id}");
        let app_for_thread = app.clone();
        std::thread::spawn(move || {
            let mut buffer = [0u8; 8192];
            let mut redactor = StreamRedactor::new(stored_secret);
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(size) => {
                        let bytes = redactor.push(&buffer[..size]);
                        if !bytes.is_empty() {
                            let _ = app_for_thread.emit(&output_event, bytes);
                        }
                    }
                    Err(_) => break,
                }
            }
            let tail = redactor.finish();
            if !tail.is_empty() {
                let _ = app_for_thread.emit(&output_event, tail);
            }
            let _ = app_for_thread.emit(&exit_event, ());
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

    pub fn write(&self, id: &str, data: &[u8]) -> Result<()> {
        let mut guard = self.sessions.lock().unwrap();
        let session = guard
            .get_mut(id)
            .ok_or_else(|| anyhow!("no such pty session"))?;
        session.writer.write_all(data)?;
        session.writer.flush()?;
        Ok(())
    }

    pub fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<()> {
        let guard = self.sessions.lock().unwrap();
        let session = guard
            .get(id)
            .ok_or_else(|| anyhow!("no such pty session"))?;
        session.master.resize(PtySize {
            rows: rows.max(1),
            cols: cols.max(1),
            pixel_width: 0,
            pixel_height: 0,
        })?;
        Ok(())
    }

    /// Kill, reap, and remove a session. Waiting after kill prevents a closed
    /// SSH child from remaining as a zombie until application shutdown.
    pub fn close(&self, id: &str) -> Result<()> {
        if let Some(mut session) = self.sessions.lock().unwrap().remove(id) {
            let _ = session.child.kill();
            let _ = session.child.wait();
        }
        Ok(())
    }
}

impl Drop for PtyManager {
    fn drop(&mut self) {
        if let Ok(mut sessions) = self.sessions.lock() {
            for (_, session) in sessions.iter_mut() {
                let _ = session.child.kill();
                let _ = session.child.wait();
            }
            sessions.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_redactor_replaces_secret_inside_single_chunk() {
        let mut redactor = StreamRedactor::new(Some("password123".into()));
        let mut output = redactor.push(b"before password123 after");
        output.extend(redactor.finish());

        assert_eq!(String::from_utf8(output).unwrap(), "before •••••• after");
    }

    #[test]
    fn stream_redactor_replaces_secret_split_across_chunks() {
        let mut redactor = StreamRedactor::new(Some("password123".into()));
        let mut output = redactor.push(b"before pass");
        output.extend(redactor.push(b"word123 after"));
        output.extend(redactor.finish());

        assert_eq!(String::from_utf8(output).unwrap(), "before •••••• after");
    }

    #[test]
    fn stream_redactor_ignores_short_values_to_avoid_noise() {
        let mut redactor = StreamRedactor::new(Some("abc".into()));
        let mut output = redactor.push(b"abc abc");
        output.extend(redactor.finish());

        assert_eq!(output, b"abc abc");
    }
}
