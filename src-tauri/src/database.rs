//! SQLite persistence layer.
//!
//! Holds server profiles, credential references (never secrets), sessions,
//! runbooks, runbook runs and tunnels. The connection is wrapped in a Mutex
//! inside `AppState`; all access goes through these helpers.

use anyhow::{Context, Result};
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
        conn.execute(
            "UPDATE servers SET name=?2, host=?3, port=?4, username=?5, protocols_json=?6,
             auth_type=?7, private_key_path=?8, tags_json=?9, group_name=?10,
             environment=?11, notes=?12, updated_at=?13 WHERE id=?1",
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
            ],
        )?;
        Ok(id.clone())
    } else {
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO servers (id,name,host,port,username,protocols_json,auth_type,
             private_key_path,tags_json,group_name,environment,notes,created_at,updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?13)",
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
            ],
        )?;
        Ok(id)
    }
}

pub fn delete_server(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM credentials WHERE server_id = ?1", params![id])?;
    conn.execute("DELETE FROM servers WHERE id = ?1", params![id])?;
    Ok(())
}

/// Record that a credential reference exists for this server (the secret
/// itself lives in the keyring).
pub fn record_credential(conn: &Connection, server_id: &str, secret_ref: &str, auth_type: &str) -> Result<()> {
    conn.execute("DELETE FROM credentials WHERE server_id = ?1", params![server_id])?;
    conn.execute(
        "INSERT INTO credentials (id, server_id, secret_ref, auth_type, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![uuid::Uuid::new_v4().to_string(), server_id, secret_ref, auth_type, now()],
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
    let mut stmt = conn.prepare("SELECT * FROM runbooks ORDER BY builtin DESC, name COLLATE NOCASE")?;
    let rows = stmt.query_map([], row_to_runbook)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn get_runbook(conn: &Connection, id: &str) -> Result<Runbook> {
    let mut stmt = conn.prepare("SELECT * FROM runbooks WHERE id = ?1")?;
    Ok(stmt.query_row(params![id], row_to_runbook)?)
}

/// Insert a built-in runbook if a runbook with the same name does not already
/// exist. Used to seed defaults on startup.
pub fn seed_builtin_runbook(conn: &Connection, name: &str, description: &str, yaml: &str) -> Result<()> {
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM runbooks WHERE name = ?1 AND builtin = 1",
        params![name],
        |r| r.get(0),
    )?;
    if exists == 0 {
        conn.execute(
            "INSERT INTO runbooks (id,name,description,content_yaml,builtin,created_at,updated_at)
             VALUES (?1,?2,?3,?4,1,?5,?5)",
            params![uuid::Uuid::new_v4().to_string(), name, description, yaml, now()],
        )?;
    }
    Ok(())
}

pub fn save_runbook(conn: &Connection, name: &str, description: &str, yaml: &str, id: Option<&str>) -> Result<String> {
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
    conn.execute("UPDATE tunnels SET status=?2 WHERE id=?1", params![id, status])?;
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
