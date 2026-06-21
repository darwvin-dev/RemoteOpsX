//! SQLite persistence layer.
//!
//! Holds server profiles, credential references (never secrets), sessions,
//! runbooks, runbook runs and tunnels. The connection is wrapped in a Mutex
//! inside `AppState`; all access goes through these helpers.

use anyhow::{anyhow, Context, Result};
use rusqlite::{params, Connection};

use crate::models::*;

/// Open (creating if needed) the SQLite database and run migrations.
pub fn open(path: &std::path::Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let conn = Connection::open(path).context("failed to open sqlite database")?;
    conn.pragma_update(None, "journal_mode", "WAL").ok();
    conn.pragma_update(None, "foreign_keys", "ON").ok();
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS servers (
            id              TEXT PRIMARY KEY,
            name            TEXT NOT NULL,
            host            TEXT NOT NULL,
            port            INTEGER NOT NULL DEFAULT 22,
            ftp_port        INTEGER,
            rdp_port        INTEGER,
            vnc_port        INTEGER,
            username        TEXT NOT NULL,
            protocols_json  TEXT NOT NULL DEFAULT '["ssh"]',
            auth_type       TEXT NOT NULL DEFAULT 'key',
            private_key_path TEXT,
            tags_json       TEXT NOT NULL DEFAULT '[]',
            group_name      TEXT,
            environment     TEXT NOT NULL DEFAULT 'dev',
            notes           TEXT,
            created_at      TEXT NOT NULL,
            updated_at      TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS credentials (
            id          TEXT PRIMARY KEY,
            server_id   TEXT NOT NULL,
            secret_ref  TEXT NOT NULL,
            auth_type   TEXT NOT NULL,
            created_at  TEXT NOT NULL,
            FOREIGN KEY(server_id) REFERENCES servers(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS sessions (
            id          TEXT PRIMARY KEY,
            server_id   TEXT NOT NULL,
            protocol    TEXT NOT NULL,
            started_at  TEXT NOT NULL,
            ended_at    TEXT,
            status      TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS runbooks (
            id           TEXT PRIMARY KEY,
            name         TEXT NOT NULL,
            description  TEXT NOT NULL DEFAULT '',
            content_yaml TEXT NOT NULL,
            builtin      INTEGER NOT NULL DEFAULT 0,
            created_at   TEXT NOT NULL,
            updated_at   TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS runbook_runs (
            id          TEXT PRIMARY KEY,
            runbook_id  TEXT NOT NULL,
            server_id   TEXT NOT NULL,
            started_at  TEXT NOT NULL,
            ended_at    TEXT,
            status      TEXT NOT NULL,
            output_json TEXT NOT NULL DEFAULT '[]'
        );

        CREATE TABLE IF NOT EXISTS tunnels (
            id          TEXT PRIMARY KEY,
            server_id   TEXT NOT NULL,
            type        TEXT NOT NULL,
            local_host  TEXT,
            local_port  INTEGER NOT NULL,
            remote_host TEXT,
            remote_port INTEGER,
            status      TEXT NOT NULL,
            created_at  TEXT NOT NULL
        );
        "#,
    )
    .context("failed to run migrations")?;
    add_column_if_missing(conn, "servers", "ftp_port", "INTEGER")?;
    add_column_if_missing(conn, "servers", "rdp_port", "INTEGER")?;
    add_column_if_missing(conn, "servers", "vnc_port", "INTEGER")?;
    Ok(())
}

fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    sql_type: &str,
) -> Result<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let names = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for name in names {
        if name? == column {
            return Ok(());
        }
    }
    conn.execute(
        &format!("ALTER TABLE {table} ADD COLUMN {column} {sql_type}"),
        [],
    )?;
    Ok(())
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn row_to_server(row: &rusqlite::Row) -> rusqlite::Result<Server> {
    let protocols_json: String = row.get("protocols_json")?;
    let tags_json: String = row.get("tags_json")?;
    Ok(Server {
        id: row.get("id")?,
        name: row.get("name")?,
        host: row.get("host")?,
        port: row.get("port")?,
        ftp_port: row.get("ftp_port")?,
        rdp_port: row.get("rdp_port")?,
        vnc_port: row.get("vnc_port")?,
        username: row.get("username")?,
        protocols: serde_json::from_str(&protocols_json).unwrap_or_default(),
        auth_type: row.get("auth_type")?,
        private_key_path: row.get("private_key_path")?,
        tags: serde_json::from_str(&tags_json).unwrap_or_default(),
        group_name: row.get("group_name")?,
        environment: row.get("environment")?,
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

pub fn list_servers(conn: &Connection) -> Result<Vec<Server>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM servers ORDER BY group_name IS NULL, group_name, name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], row_to_server)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn get_server(conn: &Connection, id: &str) -> Result<Server> {
    let mut stmt = conn.prepare("SELECT * FROM servers WHERE id = ?1")?;
    let server = stmt.query_row(params![id], row_to_server)?;
    Ok(server)
}

/// Insert or update a server. Returns the stored row id. Does NOT touch
/// secrets — the caller handles the keyring.
pub fn upsert_server(conn: &Connection, input: &ServerInput) -> Result<String> {
    let protocols = serde_json::to_string(&input.protocols)?;
    let tags = serde_json::to_string(&input.tags)?;
    let ts = now();

    if let Some(id) = &input.id {
        let updated = conn.execute(
            "UPDATE servers SET name=?2, host=?3, port=?4, username=?5, protocols_json=?6,
             auth_type=?7, private_key_path=?8, tags_json=?9, group_name=?10,
             environment=?11, notes=?12, updated_at=?13, ftp_port=?14, rdp_port=?15,
             vnc_port=?16 WHERE id=?1",
            params![
                id,
                input.name,
                input.host,
                input.port,
                input.username,
                protocols,
                input.auth_type,
                input.private_key_path,
                tags,
                input.group_name,
                input.environment,
                input.notes,
                ts,
                input.ftp_port,
                input.rdp_port,
                input.vnc_port,
            ],
        )?;
        if updated > 0 {
            return Ok(id.clone());
        }
        conn.execute(
            "INSERT INTO servers (id,name,host,port,username,protocols_json,auth_type,
             private_key_path,tags_json,group_name,environment,notes,created_at,updated_at,
             ftp_port,rdp_port,vnc_port)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?13,?14,?15,?16)",
            params![
                id,
                input.name,
                input.host,
                input.port,
                input.username,
                protocols,
                input.auth_type,
                input.private_key_path,
                tags,
                input.group_name,
                input.environment,
                input.notes,
                ts,
                input.ftp_port,
                input.rdp_port,
                input.vnc_port,
            ],
        )?;
        Ok(id.clone())
    } else {
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO servers (id,name,host,port,username,protocols_json,auth_type,
             private_key_path,tags_json,group_name,environment,notes,created_at,updated_at,
             ftp_port,rdp_port,vnc_port)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?13,?14,?15,?16)",
            params![
                id,
                input.name,
                input.host,
                input.port,
                input.username,
                protocols,
                input.auth_type,
                input.private_key_path,
                tags,
                input.group_name,
                input.environment,
                input.notes,
                ts,
                input.ftp_port,
                input.rdp_port,
                input.vnc_port,
            ],
        )?;
        Ok(id)
    }
}

pub fn validate_server_input(input: &ServerInput) -> Result<()> {
    if input.name.trim().is_empty()
        || input.host.trim().is_empty()
        || input.username.trim().is_empty()
    {
        return Err(anyhow!("name, host and username are required"));
    }
    if input.port == 0 {
        return Err(anyhow!("SSH port must be between 1 and 65535"));
    }
    for (label, port) in [
        ("FTP", input.ftp_port),
        ("RDP", input.rdp_port),
        ("VNC", input.vnc_port),
    ] {
        if port == Some(0) {
            return Err(anyhow!("{label} port must be between 1 and 65535"));
        }
    }
    if !matches!(input.auth_type.as_str(), "password" | "key") {
        return Err(anyhow!("unsupported authentication type"));
    }
    if input.protocols.is_empty() {
        return Err(anyhow!("at least one protocol is required"));
    }
    for protocol in &input.protocols {
        if !matches!(protocol.as_str(), "ssh" | "sftp" | "ftp" | "rdp" | "vnc") {
            return Err(anyhow!("unsupported protocol: {protocol}"));
        }
    }
    if input.protocols.iter().any(|protocol| protocol == "ftp") && input.auth_type != "password" {
        return Err(anyhow!("FTP profiles require password authentication"));
    }
    Ok(())
}

/// Persist the profile and credential metadata as one SQLite transaction.
/// Keyring mutation is coordinated by the caller because it is outside SQLite.
pub fn save_server_profile(
    conn: &Connection,
    input: &ServerInput,
    secret_ref: Option<&str>,
    clear_credential: bool,
) -> Result<String> {
    validate_server_input(input)?;
    let tx = conn.unchecked_transaction()?;
    let id = upsert_server(&tx, input)?;
    if clear_credential {
        tx.execute("DELETE FROM credentials WHERE server_id = ?1", params![id])?;
    } else if let Some(secret_ref) = secret_ref {
        record_credential(&tx, &id, secret_ref, &input.auth_type)?;
    }
    tx.commit()?;
    Ok(id)
}

pub fn delete_server(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM credentials WHERE server_id = ?1", params![id])?;
    conn.execute("DELETE FROM servers WHERE id = ?1", params![id])?;
    Ok(())
}

/// Record that a credential reference exists for this server (the secret
/// itself lives in the keyring).
pub fn record_credential(
    conn: &Connection,
    server_id: &str,
    secret_ref: &str,
    auth_type: &str,
) -> Result<()> {
    conn.execute(
        "DELETE FROM credentials WHERE server_id = ?1",
        params![server_id],
    )?;
    conn.execute(
        "INSERT INTO credentials (id, server_id, secret_ref, auth_type, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            uuid::Uuid::new_v4().to_string(),
            server_id,
            secret_ref,
            auth_type,
            now()
        ],
    )?;
    Ok(())
}

// ---------- runbooks ----------

fn row_to_runbook(row: &rusqlite::Row) -> rusqlite::Result<Runbook> {
    Ok(Runbook {
        id: row.get("id")?,
        name: row.get("name")?,
        description: row.get("description")?,
        content_yaml: row.get("content_yaml")?,
        builtin: row.get::<_, i64>("builtin")? != 0,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

pub fn list_runbooks(conn: &Connection) -> Result<Vec<Runbook>> {
    let mut stmt =
        conn.prepare("SELECT * FROM runbooks ORDER BY builtin DESC, name COLLATE NOCASE")?;
    let rows = stmt.query_map([], row_to_runbook)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn get_runbook(conn: &Connection, id: &str) -> Result<Runbook> {
    let mut stmt = conn.prepare("SELECT * FROM runbooks WHERE id = ?1")?;
    Ok(stmt.query_row(params![id], row_to_runbook)?)
}

/// Insert a built-in runbook if a runbook with the same name does not already
/// exist. Used to seed defaults on startup.
pub fn seed_builtin_runbook(
    conn: &Connection,
    name: &str,
    description: &str,
    yaml: &str,
) -> Result<()> {
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM runbooks WHERE name = ?1 AND builtin = 1",
        params![name],
        |r| r.get(0),
    )?;
    if exists == 0 {
        conn.execute(
            "INSERT INTO runbooks (id,name,description,content_yaml,builtin,created_at,updated_at)
             VALUES (?1,?2,?3,?4,1,?5,?5)",
            params![
                uuid::Uuid::new_v4().to_string(),
                name,
                description,
                yaml,
                now()
            ],
        )?;
    }
    Ok(())
}

pub fn save_runbook(
    conn: &Connection,
    name: &str,
    description: &str,
    yaml: &str,
    id: Option<&str>,
) -> Result<String> {
    let ts = now();
    match id {
        Some(id) => {
            conn.execute(
                "UPDATE runbooks SET name=?2, description=?3, content_yaml=?4, updated_at=?5 WHERE id=?1",
                params![id, name, description, yaml, ts],
            )?;
            Ok(id.to_string())
        }
        None => {
            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO runbooks (id,name,description,content_yaml,builtin,created_at,updated_at)
                 VALUES (?1,?2,?3,?4,0,?5,?5)",
                params![id, name, description, yaml, ts],
            )?;
            Ok(id)
        }
    }
}

pub fn insert_runbook_run(conn: &Connection, run: &RunbookRun) -> Result<()> {
    conn.execute(
        "INSERT INTO runbook_runs (id,runbook_id,server_id,started_at,ended_at,status,output_json)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![
            run.id,
            run.runbook_id,
            run.server_id,
            run.started_at,
            run.ended_at,
            run.status,
            serde_json::to_string(&run.results)?,
        ],
    )?;
    Ok(())
}

pub fn list_runbook_runs(conn: &Connection, limit: i64) -> Result<Vec<RunbookRun>> {
    let mut stmt = conn.prepare(
        "SELECT id,runbook_id,server_id,started_at,ended_at,status,output_json
         FROM runbook_runs ORDER BY started_at DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit], |row| {
        let output_json: String = row.get("output_json")?;
        Ok(RunbookRun {
            id: row.get("id")?,
            runbook_id: row.get("runbook_id")?,
            server_id: row.get("server_id")?,
            started_at: row.get("started_at")?,
            ended_at: row.get("ended_at")?,
            status: row.get("status")?,
            results: serde_json::from_str(&output_json).unwrap_or_default(),
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

// ---------- sessions ----------

/// Record an opened session. The id is supplied by the caller (e.g. the PTY
/// session id) so it can be closed later without a separate mapping.
pub fn open_session(conn: &Connection, id: &str, server_id: &str, protocol: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO sessions (id,server_id,protocol,started_at,status) VALUES (?1,?2,?3,?4,'open')",
        params![id, server_id, protocol, now()],
    )?;
    Ok(())
}

pub fn close_session(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "UPDATE sessions SET ended_at=?2, status='closed' WHERE id=?1",
        params![id, now()],
    )?;
    Ok(())
}

// ---------- tunnels ----------

pub fn insert_tunnel(conn: &Connection, t: &Tunnel) -> Result<()> {
    conn.execute(
        "INSERT INTO tunnels (id,server_id,type,local_host,local_port,remote_host,remote_port,status,created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![
            t.id, t.server_id, t.r#type, t.local_host, t.local_port,
            t.remote_host, t.remote_port, t.status, now()
        ],
    )?;
    Ok(())
}

pub fn set_tunnel_status(conn: &Connection, id: &str, status: &str) -> Result<()> {
    conn.execute(
        "UPDATE tunnels SET status=?2 WHERE id=?1",
        params![id, status],
    )?;
    Ok(())
}

pub fn list_tunnels(conn: &Connection) -> Result<Vec<Tunnel>> {
    let mut stmt = conn.prepare("SELECT * FROM tunnels ORDER BY created_at DESC")?;
    let rows = stmt.query_map([], |row| {
        Ok(Tunnel {
            id: row.get("id")?,
            server_id: row.get("server_id")?,
            r#type: row.get("type")?,
            local_host: row.get("local_host")?,
            local_port: row.get("local_port")?,
            remote_host: row.get("remote_host")?,
            remote_port: row.get("remote_port")?,
            status: row.get("status")?,
            created_at: row.get("created_at")?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(auth_type: &str) -> ServerInput {
        ServerInput {
            id: None,
            name: "server".into(),
            host: "example.test".into(),
            port: 22,
            ftp_port: Some(21),
            rdp_port: Some(3389),
            vnc_port: Some(5900),
            username: "ops".into(),
            protocols: if auth_type == "password" {
                vec!["ssh".into(), "ftp".into()]
            } else {
                vec!["ssh".into()]
            },
            auth_type: auth_type.into(),
            private_key_path: None,
            tags: vec![],
            group_name: None,
            environment: "dev".into(),
            notes: None,
            secret: None,
        }
    }

    #[test]
    fn migrates_legacy_server_table_with_protocol_ports() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE servers (
                id TEXT PRIMARY KEY, name TEXT NOT NULL, host TEXT NOT NULL,
                port INTEGER NOT NULL, username TEXT NOT NULL,
                protocols_json TEXT NOT NULL, auth_type TEXT NOT NULL,
                private_key_path TEXT, tags_json TEXT NOT NULL, group_name TEXT,
                environment TEXT NOT NULL, notes TEXT, created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );",
        )
        .unwrap();
        migrate(&conn).unwrap();
        migrate(&conn).unwrap();
        let mut stmt = conn.prepare("PRAGMA table_info(servers)").unwrap();
        let columns = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert!(columns.contains(&"ftp_port".to_string()));
        assert!(columns.contains(&"rdp_port".to_string()));
        assert!(columns.contains(&"vnc_port".to_string()));
    }

    #[test]
    fn validates_profile_before_persistence() {
        let mut invalid = input("key");
        invalid.port = 0;
        assert!(validate_server_input(&invalid)
            .unwrap_err()
            .to_string()
            .contains("SSH port"));
        invalid.port = 22;
        invalid.protocols.push("telnet".into());
        assert!(validate_server_input(&invalid).is_err());
        invalid.protocols = vec!["ftp".into()];
        assert!(validate_server_input(&invalid)
            .unwrap_err()
            .to_string()
            .contains("password authentication"));
    }

    #[test]
    fn switching_to_key_auth_clears_credential_metadata_atomically() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        let password = input("password");
        let id = save_server_profile(&conn, &password, Some("server::test"), false).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM credentials", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);

        let mut key = input("key");
        key.id = Some(id);
        save_server_profile(&conn, &key, None, true).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM credentials", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn explicit_new_id_is_inserted_when_no_row_exists() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        let mut value = input("key");
        value.id = Some("preallocated-id".into());
        let id = save_server_profile(&conn, &value, None, true).unwrap();
        assert_eq!(id, "preallocated-id");
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM servers WHERE id='preallocated-id'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }
}
