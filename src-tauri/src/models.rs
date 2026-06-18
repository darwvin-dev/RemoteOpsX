//! Shared data models. These mirror the SQLite schema and are the contract
//! between the Rust backend and the TypeScript frontend (serde -> JSON).

use serde::{Deserialize, Serialize};

/// A saved server profile. `protocols` is the set of enabled protocol flags
/// ("ssh", "sftp", "rdp", "vnc"). Secrets are NEVER stored here — only a
/// reference (`secret_ref`) into the OS keyring lives in the `credentials`
/// table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    /// "ssh" | "sftp" | "rdp" | "vnc"
    #[serde(default)]
    pub protocols: Vec<String>,
    /// "password" | "key"
    pub auth_type: String,
    #[serde(default)]
    pub private_key_path: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub group_name: Option<String>,
    /// "production" | "staging" | "dev"
    #[serde(default = "default_env")]
    pub environment: String,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

fn default_env() -> String {
    "dev".to_string()
}

/// Payload used when creating/updating a profile from the UI. A transient
/// `secret` field carries the password / passphrase only in memory; it is
/// written straight to the keyring and never persisted to SQLite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInput {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(default)]
    pub protocols: Vec<String>,
    pub auth_type: String,
    #[serde(default)]
    pub private_key_path: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub group_name: Option<String>,
    #[serde(default = "default_env")]
    pub environment: String,
    #[serde(default)]
    pub notes: Option<String>,
    /// Transient secret (password). Not persisted to SQLite.
    #[serde(default)]
    pub secret: Option<String>,
}

/// Result of a one-shot remote command execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub success: bool,
}

/// A runbook definition (the YAML is the source of truth; this is the parsed
/// form sent to the UI).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Runbook {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub content_yaml: String,
    pub builtin: bool,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

/// Parsed YAML body of a runbook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunbookSpec {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub target_os: Option<String>,
    #[serde(default)]
    pub variables: std::collections::HashMap<String, String>,
    pub steps: Vec<RunbookStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunbookStep {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub requires_confirmation: bool,
    #[serde(default)]
    pub success_pattern: Option<String>,
    #[serde(default)]
    pub failure_pattern: Option<String>,
}

/// The captured outcome of a single executed step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub name: String,
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    /// "success" | "failure"
    pub status: String,
}

/// A persisted runbook execution record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunbookRun {
    pub id: String,
    pub runbook_id: String,
    pub server_id: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub status: String,
    pub results: Vec<StepResult>,
}

/// SSH tunnel descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tunnel {
    pub id: String,
    pub server_id: String,
    /// "local" | "remote" | "dynamic"
    pub r#type: String,
    #[serde(default)]
    pub local_host: Option<String>,
    pub local_port: u16,
    #[serde(default)]
    pub remote_host: Option<String>,
    #[serde(default)]
    pub remote_port: Option<u16>,
    pub status: String,
    #[serde(default)]
    pub created_at: String,
}

/// Remote directory entry for the SFTP browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteFile {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub permissions: String,
}
