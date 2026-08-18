//! Persistent SSH/SCP transfer subsystem.
//!
//! A strict OpenSSH ControlMaster is maintained per server and transfers reuse
//! that authenticated connection through ControlPath. Jobs are cancellable and
//! expose honest byte progress when a single-file size can be measured.

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

use anyhow::{anyhow, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::models::Server;
use crate::{redaction, ssh_manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferDirection {
    Upload,
    Download,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferRequest {
    pub server_id: String,
    pub direction: TransferDirection,
    pub source: String,
    pub destination: String,
    pub recursive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferJob {
    pub id: String,
    pub server_id: String,
    pub direction: TransferDirection,
    pub source: String,
    pub destination: String,
    pub recursive: bool,
    pub status: String,
    pub total_bytes: Option<u64>,
    pub transferred_bytes: Option<u64>,
    pub progress_percent: Option<f64>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub error: Option<String>,
}

struct MasterProcess {
    child: Child,
    control_path: PathBuf,
}

struct TransferProcess {
    child: Child,
    server: Server,
    control_path: PathBuf,
    progress_probe_path: Option<String>,
}

pub struct TransferManager {
    masters: Mutex<HashMap<String, MasterProcess>>,
    processes: Mutex<HashMap<String, TransferProcess>>,
    jobs: Mutex<HashMap<String, TransferJob>>,
    control_dir: PathBuf,
}

impl Default for TransferManager {
    fn default() -> Self {
        let control_dir =
            std::env::temp_dir().join(format!("remoteopsx-ctl-{}", std::process::id()));
        let _ = fs::create_dir_all(&control_dir);
        Self {
            masters: Mutex::new(HashMap::new()),
            processes: Mutex::new(HashMap::new()),
            jobs: Mutex::new(HashMap::new()),
            control_dir,
        }
    }
}

impl TransferManager {
    pub fn new() -> Self {
        Self::default()
    }

    fn control_path(&self, server_id: &str) -> PathBuf {
        let safe: String = server_id
            .chars()
            .filter(|character| character.is_ascii_alphanumeric())
            .take(18)
            .collect();
        self.control_dir.join(format!("{safe}.sock"))
    }

    fn push_auth_args(server: &Server, args: &mut Vec<String>) {
        if server.auth_type == "key" {
            if let Some(key) = &server.private_key_path {
                if !key.trim().is_empty() {
                    args.extend([
                        "-i".into(),
                        key.clone(),
                        "-o".into(),
                        "IdentitiesOnly=yes".into(),
                    ]);
                }
            }
        } else {
            args.extend(["-o".into(), "PubkeyAuthentication=no".into()]);
        }
    }

    fn ensure_master(&self, server: &Server) -> Result<PathBuf> {
        let mut masters = self
            .masters
            .lock()
            .map_err(|_| anyhow!("transfer master lock poisoned"))?;
        if let Some(master) = masters.get_mut(&server.id) {
            if master.child.try_wait()?.is_none() && master.control_path.exists() {
                return Ok(master.control_path.clone());
            }
            let _ = master.child.kill();
            let _ = master.child.wait();
            let _ = fs::remove_file(&master.control_path);
            masters.remove(&server.id);
        }

        let control_path = self.control_path(&server.id);
        let _ = fs::remove_file(&control_path);
        let mut args = ssh_manager::strict_host_key_args()?;
        args.extend(ssh_manager::jump_host_args(server)?);
        args.extend([
            "-M".into(),
            "-N".into(),
            "-o".into(),
            "ControlMaster=yes".into(),
            "-o".into(),
            "ControlPersist=120".into(),
            "-o".into(),
            format!("ControlPath={}", control_path.to_string_lossy()),
            "-o".into(),
            "ExitOnForwardFailure=yes".into(),
            "-p".into(),
            server.port.to_string(),
        ]);
        Self::push_auth_args(server, &mut args);
        args.push(format!("{}@{}", server.username, server.host));

        let (program, full_args) = if server.auth_type == "password" {
            let mut wrapped = vec!["-e".to_string(), "ssh".to_string()];
            wrapped.extend(args);
            ("sshpass".to_string(), wrapped)
        } else {
            ("ssh".to_string(), args)
        };
        let mut command = Command::new(program);
        command
            .args(full_args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        ssh_manager::apply_password_env(&mut command, server);
        let mut child = command.spawn()?;
        for _ in 0..30 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            if control_path.exists() {
                masters.insert(
                    server.id.clone(),
                    MasterProcess {
                        child,
                        control_path: control_path.clone(),
                    },
                );
                return Ok(control_path);
            }
            if let Some(status) = child.try_wait()? {
                return Err(anyhow!(
                    "persistent SSH master exited during startup: {status}"
                ));
            }
        }
        let _ = child.kill();
        let _ = child.wait();
        Err(anyhow!(
            "persistent SSH master did not create its control socket"
        ))
    }

    fn local_file_size(path: &str) -> Option<u64> {
        fs::metadata(path)
            .ok()
            .filter(|meta| meta.is_file())
            .map(|meta| meta.len())
    }

    fn remote_file_size(server: &Server, control_path: &Path, path: &str) -> Option<u64> {
        let quoted = format!("'{}'", path.replace('\'', "'\\''"));
        let mut args = ssh_manager::strict_host_key_args().ok()?;
        args.extend(ssh_manager::jump_host_args(server).ok()?);
        args.extend([
            "-S".into(),
            control_path.to_string_lossy().to_string(),
            "-o".into(),
            "ControlMaster=no".into(),
            "-p".into(),
            server.port.to_string(),
            format!("{}@{}", server.username, server.host),
            format!("stat -c %s -- {quoted}"),
        ]);
        let output = Command::new("ssh").args(args).output().ok()?;
        if !output.status.success() {
            return None;
        }
        String::from_utf8_lossy(&output.stdout).trim().parse().ok()
    }

    fn basename(path: &str) -> Option<&str> {
        path.trim_end_matches('/')
            .rsplit('/')
            .next()
            .filter(|value| !value.is_empty())
    }

    pub fn start(&self, server: &Server, request: TransferRequest) -> Result<TransferJob> {
        if request.server_id != server.id {
            return Err(anyhow!("transfer server_id does not match selected server"));
        }
        if request.source.trim().is_empty() || request.destination.trim().is_empty() {
            return Err(anyhow!("transfer source and destination are required"));
        }
        let control_path = self.ensure_master(server)?;
        let id = uuid::Uuid::new_v4().to_string();
        let mut args = ssh_manager::strict_host_key_args()?;
        args.extend(ssh_manager::jump_host_args(server)?);
        args.push("-q".into());
        args.extend([
            "-o".into(),
            format!("ControlPath={}", control_path.to_string_lossy()),
            "-o".into(),
            "ControlMaster=no".into(),
            "-P".into(),
            server.port.to_string(),
        ]);
        if request.recursive {
            args.push("-r".into());
        }
        Self::push_auth_args(server, &mut args);

        let (total_bytes, probe_path) = match request.direction {
            TransferDirection::Upload => {
                let total = if request.recursive {
                    None
                } else {
                    Self::local_file_size(&request.source)
                };
                let remote_probe = if request.recursive {
                    None
                } else {
                    Self::basename(&request.source).map(|name| {
                        format!("{}/{}", request.destination.trim_end_matches('/'), name)
                    })
                };
                args.push(request.source.clone());
                args.push(format!(
                    "{}@{}:{}",
                    server.username, server.host, request.destination
                ));
                (total, remote_probe)
            }
            TransferDirection::Download => {
                let total = if request.recursive {
                    None
                } else {
                    Self::remote_file_size(server, &control_path, &request.source)
                };
                args.push(format!(
                    "{}@{}:{}",
                    server.username, server.host, request.source
                ));
                args.push(request.destination.clone());
                let local_probe = if request.recursive {
                    None
                } else if Path::new(&request.destination).is_dir() {
                    Self::basename(&request.source).map(|name| {
                        Path::new(&request.destination)
                            .join(name)
                            .to_string_lossy()
                            .to_string()
                    })
                } else {
                    Some(request.destination.clone())
                };
                (total, local_probe)
            }
        };

        let mut command = Command::new("scp");
        command
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        ssh_manager::apply_password_env(&mut command, server);
        let child = command.spawn()?;
        let job = TransferJob {
            id: id.clone(),
            server_id: server.id.clone(),
            direction: request.direction,
            source: request.source,
            destination: request.destination,
            recursive: request.recursive,
            status: "running".into(),
            total_bytes,
            transferred_bytes: Some(0),
            progress_percent: total_bytes.map(|_| 0.0),
            started_at: Utc::now().to_rfc3339(),
            ended_at: None,
            error: None,
        };
        self.jobs.lock().unwrap().insert(id.clone(), job.clone());
        self.processes.lock().unwrap().insert(
            id,
            TransferProcess {
                child,
                server: server.clone(),
                control_path,
                progress_probe_path: probe_path,
            },
        );
        Ok(job)
    }

    pub fn cancel(&self, id: &str) -> Result<()> {
        let mut processes = self.processes.lock().unwrap();
        if let Some(mut process) = processes.remove(id) {
            let _ = process.child.kill();
            let _ = process.child.wait();
        }
        if let Some(job) = self.jobs.lock().unwrap().get_mut(id) {
            if job.status == "running" {
                job.status = "cancelled".into();
                job.ended_at = Some(Utc::now().to_rfc3339());
            }
        }
        Ok(())
    }

    fn refresh(&self) {
        let mut processes = match self.processes.lock() {
            Ok(processes) => processes,
            Err(_) => return,
        };
        let mut jobs = match self.jobs.lock() {
            Ok(jobs) => jobs,
            Err(_) => return,
        };
        let ids = processes.keys().cloned().collect::<Vec<_>>();
        for id in ids {
            let Some(process) = processes.get_mut(&id) else {
                continue;
            };
            if let Some(job) = jobs.get_mut(&id) {
                if let Some(total) = job.total_bytes.filter(|total| *total > 0) {
                    let transferred = match job.direction {
                        TransferDirection::Upload => {
                            process.progress_probe_path.as_deref().and_then(|path| {
                                Self::remote_file_size(&process.server, &process.control_path, path)
                            })
                        }
                        TransferDirection::Download => process
                            .progress_probe_path
                            .as_deref()
                            .and_then(Self::local_file_size),
                    };
                    if let Some(transferred) = transferred {
                        let bounded = transferred.min(total);
                        job.transferred_bytes = Some(bounded);
                        job.progress_percent = Some((bounded as f64 / total as f64) * 100.0);
                    }
                }
                match process.child.try_wait() {
                    Ok(Some(status)) => {
                        job.ended_at = Some(Utc::now().to_rfc3339());
                        if status.success() {
                            job.status = "completed".into();
                            job.transferred_bytes = job.total_bytes;
                            job.progress_percent = job.total_bytes.map(|_| 100.0);
                        } else {
                            job.status = "failed".into();
                            let mut stderr = String::new();
                            if let Some(mut pipe) = process.child.stderr.take() {
                                let _ = pipe.read_to_string(&mut stderr);
                            }
                            job.error = Some(redaction::redact(if stderr.trim().is_empty() {
                                format!("scp exited with status {status}")
                            } else {
                                stderr.trim().to_string()
                            }));
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        job.status = "failed".into();
                        job.ended_at = Some(Utc::now().to_rfc3339());
                        job.error = Some(redaction::redact(error.to_string()));
                    }
                }
            }
        }
        processes.retain(|id, _| jobs.get(id).is_some_and(|job| job.status == "running"));
    }

    pub fn jobs(&self) -> Vec<TransferJob> {
        self.refresh();
        let mut jobs = self
            .jobs
            .lock()
            .map(|jobs| jobs.values().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        jobs.sort_by(|left, right| right.started_at.cmp(&left.started_at));
        jobs
    }

    pub fn chmod(&self, server: &Server, remote_path: &str, mode: &str) -> Result<()> {
        if mode.len() != 3 && mode.len() != 4 || !mode.chars().all(|c| matches!(c, '0'..='7')) {
            return Err(anyhow!("chmod mode must be a 3- or 4-digit octal value"));
        }
        let control_path = self.ensure_master(server)?;
        let quoted = format!("'{}'", remote_path.replace('\'', "'\\''"));
        let mut args = ssh_manager::strict_host_key_args()?;
        args.extend(ssh_manager::jump_host_args(server)?);
        args.extend([
            "-S".into(),
            control_path.to_string_lossy().to_string(),
            "-o".into(),
            "ControlMaster=no".into(),
            "-p".into(),
            server.port.to_string(),
            format!("{}@{}", server.username, server.host),
            format!("chmod {mode} -- {quoted}"),
        ]);
        let output = Command::new("ssh").args(args).output()?;
        if output.status.success() {
            Ok(())
        } else {
            Err(anyhow!(redaction::redact(String::from_utf8_lossy(
                &output.stderr
            ))))
        }
    }
}

impl Drop for TransferManager {
    fn drop(&mut self) {
        if let Ok(processes) = self.processes.get_mut() {
            for (_, process) in processes.iter_mut() {
                let _ = process.child.kill();
                let _ = process.child.wait();
            }
        }
        if let Ok(masters) = self.masters.get_mut() {
            for (_, master) in masters.iter_mut() {
                let _ = master.child.kill();
                let _ = master.child.wait();
                let _ = fs::remove_file(&master.control_path);
            }
        }
        let _ = fs::remove_dir_all(&self.control_dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfer_request_rejects_empty_paths_and_chmod_is_bounded() {
        let manager = TransferManager::new();
        let server = Server {
            id: "server".into(),
            name: "server".into(),
            host: "example.test".into(),
            port: 22,
            ftp_port: None,
            rdp_port: None,
            vnc_port: None,
            username: "ops".into(),
            protocols: vec!["ssh".into()],
            auth_type: "key".into(),
            private_key_path: None,
            tags: vec![],
            group_name: None,
            environment: "dev".into(),
            notes: None,
            created_at: String::new(),
            updated_at: String::new(),
        };
        let request = TransferRequest {
            server_id: server.id.clone(),
            direction: TransferDirection::Upload,
            source: String::new(),
            destination: "/tmp".into(),
            recursive: false,
        };
        assert!(manager.start(&server, request).is_err());
        assert!(manager.chmod(&server, "/tmp/x", "999").is_err());
    }
}
