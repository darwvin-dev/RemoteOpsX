//! Persistent operator data: bounded health history, alert rules/events,
//! tunnel resilience policies and multi-host execution audit records.

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::health_collector::HealthSnapshot;
use crate::models::CommandOutput;

const HEALTH_SAMPLE_INTERVAL_SECONDS: i64 = 30;
const HEALTH_POINTS_PER_SERVER: i64 = 20_160; // 7 days at 30-second cadence.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthPoint {
    pub server_id: String,
    pub sampled_at: String,
    pub cpu_percent: f64,
    pub mem_percent: f64,
    pub swap_percent: f64,
    pub load1: f64,
    pub net_rx_rate: f64,
    pub net_tx_rate: f64,
    pub max_disk_percent: f64,
    pub failed_services: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    pub id: String,
    pub server_id: Option<String>,
    pub metric: String,
    pub comparison: String,
    pub threshold: f64,
    pub consecutive_samples: u32,
    pub cooldown_seconds: u32,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRuleInput {
    pub id: Option<String>,
    pub server_id: Option<String>,
    pub metric: String,
    pub comparison: String,
    pub threshold: f64,
    pub consecutive_samples: u32,
    pub cooldown_seconds: u32,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorAlert {
    pub id: String,
    pub rule_id: String,
    pub server_id: String,
    pub metric: String,
    pub value: f64,
    pub threshold: f64,
    pub message: String,
    pub fired_at: String,
    pub acknowledged_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelPolicy {
    pub tunnel_id: String,
    pub autostart: bool,
    pub auto_reconnect: bool,
    pub health_interval_secs: u32,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelPolicyInput {
    pub tunnel_id: String,
    pub autostart: bool,
    pub auto_reconnect: bool,
    pub health_interval_secs: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiHostResult {
    pub server_id: String,
    pub server_name: String,
    pub environment: String,
    pub output: CommandOutput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiHostRun {
    pub id: String,
    pub command: String,
    pub status: String,
    pub started_at: String,
    pub ended_at: String,
    pub results: Vec<MultiHostResult>,
}

pub fn ensure_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS health_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            server_id TEXT NOT NULL,
            sampled_at TEXT NOT NULL,
            cpu_percent REAL NOT NULL,
            mem_percent REAL NOT NULL,
            swap_percent REAL NOT NULL,
            load1 REAL NOT NULL,
            net_rx_rate REAL NOT NULL,
            net_tx_rate REAL NOT NULL,
            max_disk_percent REAL NOT NULL,
            failed_services INTEGER NOT NULL,
            FOREIGN KEY(server_id) REFERENCES servers(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_health_history_server_time
            ON health_history(server_id, sampled_at DESC);

        CREATE TABLE IF NOT EXISTS alert_rules (
            id TEXT PRIMARY KEY,
            server_id TEXT,
            metric TEXT NOT NULL,
            comparison TEXT NOT NULL,
            threshold REAL NOT NULL,
            consecutive_samples INTEGER NOT NULL,
            cooldown_seconds INTEGER NOT NULL,
            enabled INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY(server_id) REFERENCES servers(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS operator_alerts (
            id TEXT PRIMARY KEY,
            rule_id TEXT NOT NULL,
            server_id TEXT NOT NULL,
            metric TEXT NOT NULL,
            value REAL NOT NULL,
            threshold REAL NOT NULL,
            message TEXT NOT NULL,
            fired_at TEXT NOT NULL,
            acknowledged_at TEXT,
            FOREIGN KEY(rule_id) REFERENCES alert_rules(id) ON DELETE CASCADE,
            FOREIGN KEY(server_id) REFERENCES servers(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_operator_alerts_time
            ON operator_alerts(fired_at DESC);

        CREATE TABLE IF NOT EXISTS tunnel_policies (
            tunnel_id TEXT PRIMARY KEY,
            autostart INTEGER NOT NULL,
            auto_reconnect INTEGER NOT NULL,
            health_interval_secs INTEGER NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY(tunnel_id) REFERENCES tunnels(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS multi_host_runs (
            id TEXT PRIMARY KEY,
            command TEXT NOT NULL,
            status TEXT NOT NULL,
            started_at TEXT NOT NULL,
            ended_at TEXT NOT NULL,
            results_json TEXT NOT NULL
        );
        "#,
    )?;
    Ok(())
}

fn max_disk_percent(snapshot: &HealthSnapshot) -> f64 {
    snapshot
        .disks
        .iter()
        .map(|disk| disk.use_percent)
        .fold(0.0, f64::max)
}

fn health_point(server_id: &str, snapshot: &HealthSnapshot, sampled_at: String) -> HealthPoint {
    HealthPoint {
        server_id: server_id.to_string(),
        sampled_at,
        cpu_percent: snapshot.cpu_percent,
        mem_percent: snapshot.mem_percent,
        swap_percent: snapshot.swap_percent,
        load1: snapshot.load1,
        net_rx_rate: snapshot.net_rx_rate,
        net_tx_rate: snapshot.net_tx_rate,
        max_disk_percent: max_disk_percent(snapshot),
        failed_services: snapshot.failed_services.len() as u32,
    }
}

fn parse_time(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|date| date.with_timezone(&Utc))
}

pub fn record_health(
    conn: &Connection,
    server_id: &str,
    snapshot: &HealthSnapshot,
) -> Result<Vec<OperatorAlert>> {
    ensure_schema(conn)?;
    let now = Utc::now();
    let now_text = now.to_rfc3339();
    let latest: Option<String> = conn
        .query_row(
            "SELECT sampled_at FROM health_history WHERE server_id=?1 ORDER BY sampled_at DESC LIMIT 1",
            params![server_id],
            |row| row.get(0),
        )
        .optional()?;
    let due = latest
        .as_deref()
        .and_then(parse_time)
        .map(|last| (now - last).num_seconds() >= HEALTH_SAMPLE_INTERVAL_SECONDS)
        .unwrap_or(true);

    if due {
        let point = health_point(server_id, snapshot, now_text.clone());
        conn.execute(
            "INSERT INTO health_history (
                server_id,sampled_at,cpu_percent,mem_percent,swap_percent,load1,
                net_rx_rate,net_tx_rate,max_disk_percent,failed_services
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                point.server_id,
                point.sampled_at,
                point.cpu_percent,
                point.mem_percent,
                point.swap_percent,
                point.load1,
                point.net_rx_rate,
                point.net_tx_rate,
                point.max_disk_percent,
                point.failed_services,
            ],
        )?;
        conn.execute(
            "DELETE FROM health_history WHERE id IN (
                SELECT id FROM health_history WHERE server_id=?1
                ORDER BY sampled_at DESC LIMIT -1 OFFSET ?2
             )",
            params![server_id, HEALTH_POINTS_PER_SERVER],
        )?;
        return evaluate_alerts(conn, server_id, snapshot);
    }

    Ok(Vec::new())
}

pub fn health_history(conn: &Connection, server_id: &str, limit: i64) -> Result<Vec<HealthPoint>> {
    ensure_schema(conn)?;
    let limit = limit.clamp(1, HEALTH_POINTS_PER_SERVER);
    let mut statement = conn.prepare(
        "SELECT server_id,sampled_at,cpu_percent,mem_percent,swap_percent,load1,
                net_rx_rate,net_tx_rate,max_disk_percent,failed_services
         FROM health_history WHERE server_id=?1 ORDER BY sampled_at DESC LIMIT ?2",
    )?;
    let rows = statement.query_map(params![server_id, limit], |row| {
        Ok(HealthPoint {
            server_id: row.get(0)?,
            sampled_at: row.get(1)?,
            cpu_percent: row.get(2)?,
            mem_percent: row.get(3)?,
            swap_percent: row.get(4)?,
            load1: row.get(5)?,
            net_rx_rate: row.get(6)?,
            net_tx_rate: row.get(7)?,
            max_disk_percent: row.get(8)?,
            failed_services: row.get(9)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn validate_rule(input: &AlertRuleInput) -> Result<()> {
    if !matches!(
        input.metric.as_str(),
        "cpu_percent"
            | "mem_percent"
            | "swap_percent"
            | "load1"
            | "max_disk_percent"
            | "failed_services"
    ) {
        return Err(anyhow!("unsupported alert metric"));
    }
    if !matches!(input.comparison.as_str(), "gt" | "gte" | "lt" | "lte") {
        return Err(anyhow!("comparison must be gt, gte, lt or lte"));
    }
    if !input.threshold.is_finite() {
        return Err(anyhow!("threshold must be finite"));
    }
    if !(1..=20).contains(&input.consecutive_samples) {
        return Err(anyhow!("consecutive_samples must be between 1 and 20"));
    }
    if !(30..=86_400).contains(&input.cooldown_seconds) {
        return Err(anyhow!("cooldown_seconds must be between 30 and 86400"));
    }
    Ok(())
}

pub fn save_alert_rule(conn: &Connection, input: &AlertRuleInput) -> Result<AlertRule> {
    ensure_schema(conn)?;
    validate_rule(input)?;
    let id = input
        .id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let now = Utc::now().to_rfc3339();
    let created_at: String = conn
        .query_row(
            "SELECT created_at FROM alert_rules WHERE id=?1",
            params![id],
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or_else(|| now.clone());
    conn.execute(
        "INSERT INTO alert_rules (
            id,server_id,metric,comparison,threshold,consecutive_samples,cooldown_seconds,
            enabled,created_at,updated_at
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
         ON CONFLICT(id) DO UPDATE SET
            server_id=excluded.server_id, metric=excluded.metric,
            comparison=excluded.comparison, threshold=excluded.threshold,
            consecutive_samples=excluded.consecutive_samples,
            cooldown_seconds=excluded.cooldown_seconds, enabled=excluded.enabled,
            updated_at=excluded.updated_at",
        params![
            id,
            input.server_id,
            input.metric,
            input.comparison,
            input.threshold,
            input.consecutive_samples,
            input.cooldown_seconds,
            input.enabled as i64,
            created_at,
            now,
        ],
    )?;
    get_alert_rule(conn, &id)
}

fn row_alert_rule(row: &rusqlite::Row<'_>) -> rusqlite::Result<AlertRule> {
    Ok(AlertRule {
        id: row.get(0)?,
        server_id: row.get(1)?,
        metric: row.get(2)?,
        comparison: row.get(3)?,
        threshold: row.get(4)?,
        consecutive_samples: row.get(5)?,
        cooldown_seconds: row.get(6)?,
        enabled: row.get::<_, i64>(7)? != 0,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn get_alert_rule(conn: &Connection, id: &str) -> Result<AlertRule> {
    Ok(conn.query_row(
        "SELECT id,server_id,metric,comparison,threshold,consecutive_samples,
                cooldown_seconds,enabled,created_at,updated_at
         FROM alert_rules WHERE id=?1",
        params![id],
        row_alert_rule,
    )?)
}

pub fn alert_rules(conn: &Connection) -> Result<Vec<AlertRule>> {
    ensure_schema(conn)?;
    let mut statement = conn.prepare(
        "SELECT id,server_id,metric,comparison,threshold,consecutive_samples,
                cooldown_seconds,enabled,created_at,updated_at
         FROM alert_rules ORDER BY updated_at DESC",
    )?;
    let rows = statement.query_map([], row_alert_rule)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn delete_alert_rule(conn: &Connection, id: &str) -> Result<()> {
    ensure_schema(conn)?;
    conn.execute("DELETE FROM alert_rules WHERE id=?1", params![id])?;
    Ok(())
}

fn metric_value(rule: &AlertRule, snapshot: &HealthSnapshot) -> f64 {
    match rule.metric.as_str() {
        "cpu_percent" => snapshot.cpu_percent,
        "mem_percent" => snapshot.mem_percent,
        "swap_percent" => snapshot.swap_percent,
        "load1" => snapshot.load1,
        "max_disk_percent" => max_disk_percent(snapshot),
        "failed_services" => snapshot.failed_services.len() as f64,
        _ => 0.0,
    }
}

fn matches_threshold(rule: &AlertRule, value: f64) -> bool {
    match rule.comparison.as_str() {
        "gt" => value > rule.threshold,
        "gte" => value >= rule.threshold,
        "lt" => value < rule.threshold,
        "lte" => value <= rule.threshold,
        _ => false,
    }
}

fn historical_metric(point: &HealthPoint, metric: &str) -> f64 {
    match metric {
        "cpu_percent" => point.cpu_percent,
        "mem_percent" => point.mem_percent,
        "swap_percent" => point.swap_percent,
        "load1" => point.load1,
        "max_disk_percent" => point.max_disk_percent,
        "failed_services" => point.failed_services as f64,
        _ => 0.0,
    }
}

fn alert_is_in_cooldown(conn: &Connection, rule: &AlertRule, server_id: &str) -> Result<bool> {
    let last: Option<String> = conn
        .query_row(
            "SELECT fired_at FROM operator_alerts WHERE rule_id=?1 AND server_id=?2
             ORDER BY fired_at DESC LIMIT 1",
            params![rule.id, server_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(last
        .as_deref()
        .and_then(parse_time)
        .map(|time| (Utc::now() - time).num_seconds() < rule.cooldown_seconds as i64)
        .unwrap_or(false))
}

fn evaluate_alerts(
    conn: &Connection,
    server_id: &str,
    snapshot: &HealthSnapshot,
) -> Result<Vec<OperatorAlert>> {
    let rules = alert_rules(conn)?;
    let history = health_history(conn, server_id, 20)?;
    let mut fired = Vec::new();
    for rule in rules
        .into_iter()
        .filter(|rule| rule.enabled && rule.server_id.as_deref().map_or(true, |id| id == server_id))
    {
        let current = metric_value(&rule, snapshot);
        if !matches_threshold(&rule, current) || alert_is_in_cooldown(conn, &rule, server_id)? {
            continue;
        }
        let needed_previous = rule.consecutive_samples.saturating_sub(1) as usize;
        if needed_previous > 0 {
            // history[0] is the sample that triggered this evaluation; only prior
            // persisted 30-second samples count toward the consecutive gate.
            let available_previous = history.len().saturating_sub(1);
            let all_previous_match = history
                .iter()
                .skip(1)
                .take(needed_previous)
                .all(|point| matches_threshold(&rule, historical_metric(point, &rule.metric)));
            if available_previous < needed_previous || !all_previous_match {
                continue;
            }
        }
        let alert = OperatorAlert {
            id: uuid::Uuid::new_v4().to_string(),
            rule_id: rule.id.clone(),
            server_id: server_id.to_string(),
            metric: rule.metric.clone(),
            value: current,
            threshold: rule.threshold,
            message: format!(
                "{} {} {} (current {:.2})",
                rule.metric, rule.comparison, rule.threshold, current
            ),
            fired_at: Utc::now().to_rfc3339(),
            acknowledged_at: None,
        };
        conn.execute(
            "INSERT INTO operator_alerts (
                id,rule_id,server_id,metric,value,threshold,message,fired_at,acknowledged_at
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,NULL)",
            params![
                alert.id,
                alert.rule_id,
                alert.server_id,
                alert.metric,
                alert.value,
                alert.threshold,
                alert.message,
                alert.fired_at,
            ],
        )?;
        fired.push(alert);
    }
    Ok(fired)
}

fn row_alert(row: &rusqlite::Row<'_>) -> rusqlite::Result<OperatorAlert> {
    Ok(OperatorAlert {
        id: row.get(0)?,
        rule_id: row.get(1)?,
        server_id: row.get(2)?,
        metric: row.get(3)?,
        value: row.get(4)?,
        threshold: row.get(5)?,
        message: row.get(6)?,
        fired_at: row.get(7)?,
        acknowledged_at: row.get(8)?,
    })
}

pub fn alerts(conn: &Connection, limit: i64) -> Result<Vec<OperatorAlert>> {
    ensure_schema(conn)?;
    let mut statement = conn.prepare(
        "SELECT id,rule_id,server_id,metric,value,threshold,message,fired_at,acknowledged_at
         FROM operator_alerts ORDER BY fired_at DESC LIMIT ?1",
    )?;
    let rows = statement.query_map(params![limit.clamp(1, 1000)], row_alert)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn acknowledge_alert(conn: &Connection, id: &str) -> Result<()> {
    ensure_schema(conn)?;
    conn.execute(
        "UPDATE operator_alerts SET acknowledged_at=?2 WHERE id=?1",
        params![id, Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

pub fn save_tunnel_policy(conn: &Connection, input: &TunnelPolicyInput) -> Result<TunnelPolicy> {
    ensure_schema(conn)?;
    if input.tunnel_id.trim().is_empty() {
        return Err(anyhow!("tunnel_id is required"));
    }
    if !(5..=300).contains(&input.health_interval_secs) {
        return Err(anyhow!("health_interval_secs must be between 5 and 300"));
    }
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO tunnel_policies (tunnel_id,autostart,auto_reconnect,health_interval_secs,updated_at)
         VALUES (?1,?2,?3,?4,?5)
         ON CONFLICT(tunnel_id) DO UPDATE SET
            autostart=excluded.autostart,
            auto_reconnect=excluded.auto_reconnect,
            health_interval_secs=excluded.health_interval_secs,
            updated_at=excluded.updated_at",
        params![
            input.tunnel_id,
            input.autostart as i64,
            input.auto_reconnect as i64,
            input.health_interval_secs,
            now,
        ],
    )?;
    Ok(TunnelPolicy {
        tunnel_id: input.tunnel_id.clone(),
        autostart: input.autostart,
        auto_reconnect: input.auto_reconnect,
        health_interval_secs: input.health_interval_secs,
        updated_at: now,
    })
}

pub fn tunnel_policies(conn: &Connection) -> Result<Vec<TunnelPolicy>> {
    ensure_schema(conn)?;
    let mut statement = conn.prepare(
        "SELECT tunnel_id,autostart,auto_reconnect,health_interval_secs,updated_at
         FROM tunnel_policies ORDER BY updated_at DESC",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(TunnelPolicy {
            tunnel_id: row.get(0)?,
            autostart: row.get::<_, i64>(1)? != 0,
            auto_reconnect: row.get::<_, i64>(2)? != 0,
            health_interval_secs: row.get(3)?,
            updated_at: row.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn save_multi_host_run(conn: &Connection, run: &MultiHostRun) -> Result<()> {
    ensure_schema(conn)?;
    conn.execute(
        "INSERT INTO multi_host_runs (id,command,status,started_at,ended_at,results_json)
         VALUES (?1,?2,?3,?4,?5,?6)",
        params![
            run.id,
            run.command,
            run.status,
            run.started_at,
            run.ended_at,
            serde_json::to_string(&run.results)?,
        ],
    )?;
    Ok(())
}

pub fn multi_host_runs(conn: &Connection, limit: i64) -> Result<Vec<MultiHostRun>> {
    ensure_schema(conn)?;
    let mut statement = conn.prepare(
        "SELECT id,command,status,started_at,ended_at,results_json
         FROM multi_host_runs ORDER BY started_at DESC LIMIT ?1",
    )?;
    let rows = statement.query_map(params![limit.clamp(1, 200)], |row| {
        let results_json: String = row.get(5)?;
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            results_json,
        ))
    })?;
    let mut runs = Vec::new();
    for row in rows {
        let (id, command, status, started_at, ended_at, results_json) = row?;
        runs.push(MultiHostRun {
            id,
            command,
            status,
            started_at,
            ended_at,
            results: serde_json::from_str(&results_json)?,
        });
    }
    Ok(runs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alert_rule_validation_rejects_unbounded_inputs() {
        let mut input = AlertRuleInput {
            id: None,
            server_id: None,
            metric: "cpu_percent".into(),
            comparison: "gt".into(),
            threshold: 90.0,
            consecutive_samples: 2,
            cooldown_seconds: 60,
            enabled: true,
        };
        assert!(validate_rule(&input).is_ok());
        input.metric = "shell".into();
        assert!(validate_rule(&input).is_err());
        input.metric = "cpu_percent".into();
        input.consecutive_samples = 0;
        assert!(validate_rule(&input).is_err());
    }
}
