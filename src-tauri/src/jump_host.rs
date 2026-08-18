//! First-class SSH jump-host configuration.
//!
//! Jump hosts are persisted separately from server profiles so credentials and
//! route policy stay explicit. The runtime cache is read by every SSH-derived
//! transport. Bastions are intentionally key-auth only: this keeps nested SSH
//! authentication out of argv and avoids competing `sshpass` environments.

use std::collections::HashMap;
use std::sync::RwLock;

use anyhow::{anyhow, Context, Result};
use once_cell::sync::Lazy;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::host_identity;

static ROUTES: Lazy<RwLock<HashMap<String, JumpHostConfig>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JumpHostConfig {
    pub server_id: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub private_key_path: String,
}

fn ensure_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS jump_hosts (
            server_id        TEXT PRIMARY KEY,
            host             TEXT NOT NULL,
            port             INTEGER NOT NULL,
            username         TEXT NOT NULL,
            private_key_path TEXT NOT NULL,
            updated_at       TEXT NOT NULL,
            FOREIGN KEY(server_id) REFERENCES servers(id) ON DELETE CASCADE
        );
        "#,
    )
    .context("failed to initialize jump-host storage")?;
    Ok(())
}

pub fn validate(config: &JumpHostConfig) -> Result<()> {
    if config.server_id.trim().is_empty() {
        return Err(anyhow!("server_id is required"));
    }
    host_identity::validate_target(&config.host, config.port)?;
    let username_ok = !config.username.is_empty()
        && config.username == config.username.trim()
        && config.username.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        });
    if !username_ok {
        return Err(anyhow!(
            "jump-host username may contain only letters, numbers, dot, underscore, and hyphen"
        ));
    }
    if config.private_key_path.trim().is_empty()
        || config.private_key_path.contains('\0')
        || config.private_key_path.contains('\n')
        || config.private_key_path.contains('\r')
    {
        return Err(anyhow!("a valid jump-host private key path is required"));
    }
    Ok(())
}

pub fn hydrate(conn: &Connection) -> Result<()> {
    ensure_schema(conn)?;
    let mut stmt = conn.prepare(
        "SELECT server_id, host, port, username, private_key_path FROM jump_hosts ORDER BY server_id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(JumpHostConfig {
            server_id: row.get(0)?,
            host: row.get(1)?,
            port: row.get(2)?,
            username: row.get(3)?,
            private_key_path: row.get(4)?,
        })
    })?;
    let values = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    let mut guard = ROUTES
        .write()
        .map_err(|_| anyhow!("jump-host route cache lock poisoned"))?;
    guard.clear();
    for value in values {
        validate(&value)?;
        guard.insert(value.server_id.clone(), value);
    }
    Ok(())
}

pub fn get(conn: &Connection, server_id: &str) -> Result<Option<JumpHostConfig>> {
    ensure_schema(conn)?;
    let result = conn.query_row(
        "SELECT server_id, host, port, username, private_key_path FROM jump_hosts WHERE server_id=?1",
        params![server_id],
        |row| {
            Ok(JumpHostConfig {
                server_id: row.get(0)?,
                host: row.get(1)?,
                port: row.get(2)?,
                username: row.get(3)?,
                private_key_path: row.get(4)?,
            })
        },
    );
    match result {
        Ok(value) => Ok(Some(value)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub fn get_cached(server_id: &str) -> Option<JumpHostConfig> {
    ROUTES
        .read()
        .ok()
        .and_then(|guard| guard.get(server_id).cloned())
}

pub fn save(conn: &Connection, config: &JumpHostConfig) -> Result<()> {
    validate(config)?;
    ensure_schema(conn)?;
    conn.execute(
        "INSERT INTO jump_hosts (server_id,host,port,username,private_key_path,updated_at)
         VALUES (?1,?2,?3,?4,?5,?6)
         ON CONFLICT(server_id) DO UPDATE SET
             host=excluded.host,
             port=excluded.port,
             username=excluded.username,
             private_key_path=excluded.private_key_path,
             updated_at=excluded.updated_at",
        params![
            config.server_id,
            config.host,
            config.port,
            config.username,
            config.private_key_path,
            chrono::Utc::now().to_rfc3339(),
        ],
    )?;
    ROUTES
        .write()
        .map_err(|_| anyhow!("jump-host route cache lock poisoned"))?
        .insert(config.server_id.clone(), config.clone());
    Ok(())
}

pub fn delete(conn: &Connection, server_id: &str) -> Result<()> {
    ensure_schema(conn)?;
    conn.execute(
        "DELETE FROM jump_hosts WHERE server_id=?1",
        params![server_id],
    )?;
    forget(server_id);
    Ok(())
}

pub fn forget(server_id: &str) {
    if let Ok(mut guard) = ROUTES.write() {
        guard.remove(server_id);
    }
}

#[cfg(feature = "integration-fixture")]
pub fn set_fixture_route(config: Option<JumpHostConfig>) {
    let mut guard = ROUTES.write().expect("fixture route cache lock");
    guard.clear();
    if let Some(config) = config {
        guard.insert(config.server_id.clone(), config);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_rejects_shell_metacharacters_and_passwordless_key_gaps() {
        let mut config = JumpHostConfig {
            server_id: "server-1".into(),
            host: "bastion.internal".into(),
            port: 22,
            username: "ops-user".into(),
            private_key_path: "/home/ops/.ssh/id_ed25519".into(),
        };
        assert!(validate(&config).is_ok());
        config.username = "ops;whoami".into();
        assert!(validate(&config).is_err());
        config.username = "ops".into();
        config.private_key_path.clear();
        assert!(validate(&config).is_err());
    }
}
