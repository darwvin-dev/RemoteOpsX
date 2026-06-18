//! Agentless live server health collection.
//!
//! A single SSH exec runs one shell snippet that emits every metric section
//! between `@@MARKER@@` lines (one round-trip per refresh). Rates that need two
//! samples (CPU %, network throughput) are derived in Rust by diffing against
//! the previous snapshot held per-server in `HealthState`.
//!
//! Nothing is installed on the remote host; only standard /proc, /sys and
//! coreutils/`ss`/`systemctl`/`docker` reads are used.

use std::collections::HashMap;
use std::sync::Mutex;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::models::Server;
use crate::ssh_manager;

/// The remote probe script. Kept POSIX-sh compatible. Each section is framed
/// by `@@NAME@@` so the parser can split deterministically.
const PROBE: &str = r#"
echo '@@OS@@'; cat /etc/os-release 2>/dev/null;
echo '@@KERNEL@@'; uname -r 2>/dev/null;
echo '@@HOST@@'; hostname 2>/dev/null;
echo '@@UPTIME@@'; cat /proc/uptime 2>/dev/null;
echo '@@LOAD@@'; cat /proc/loadavg 2>/dev/null;
echo '@@CPU@@'; grep '^cpu ' /proc/stat 2>/dev/null;
echo '@@MEM@@'; cat /proc/meminfo 2>/dev/null;
echo '@@DISK@@'; df -P 2>/dev/null;
echo '@@NET@@'; for d in /sys/class/net/*; do n=$(basename "$d"); [ "$n" = lo ] && continue; r=$(cat "$d/statistics/rx_bytes" 2>/dev/null); t=$(cat "$d/statistics/tx_bytes" 2>/dev/null); [ -n "$r" ] && echo "$n $r $t"; done;
echo '@@PSCPU@@'; ps -eo pid,comm,%cpu,%mem --sort=-%cpu 2>/dev/null | head -11;
echo '@@PSMEM@@'; ps -eo pid,comm,%cpu,%mem --sort=-%mem 2>/dev/null | head -11;
echo '@@PORTS@@'; (ss -tulpen 2>/dev/null || ss -tuln 2>/dev/null) | head -60;
echo '@@FAILED@@'; systemctl --failed --no-pager --plain --no-legend 2>/dev/null | head -40;
echo '@@DOCKERPS@@'; if command -v docker >/dev/null 2>&1; then docker ps --format '{{.Names}}|{{.Status}}|{{.Image}}' 2>/dev/null; fi;
echo '@@DOCKERSTATS@@'; if command -v docker >/dev/null 2>&1; then docker stats --no-stream --format '{{.Name}}|{{.CPUPerc}}|{{.MemPerc}}' 2>/dev/null; fi;
echo '@@END@@'
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    pub filesystem: String,
    pub size_kb: u64,
    pub used_kb: u64,
    pub use_percent: f64,
    pub mount: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcInfo {
    pub pid: String,
    pub command: String,
    pub cpu: f64,
    pub mem: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerContainer {
    pub name: String,
    pub status: String,
    pub image: String,
    pub cpu_percent: Option<f64>,
    pub mem_percent: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HealthSnapshot {
    pub os_name: String,
    pub kernel: String,
    pub hostname: String,
    pub uptime_secs: u64,
    pub load1: f64,
    pub load5: f64,
    pub load15: f64,
    pub cpu_percent: f64,
    pub mem_percent: f64,
    pub mem_total_kb: u64,
    pub mem_used_kb: u64,
    pub swap_percent: f64,
    pub swap_total_kb: u64,
    pub swap_used_kb: u64,
    pub net_rx_rate: f64, // bytes/sec
    pub net_tx_rate: f64, // bytes/sec
    pub disks: Vec<DiskInfo>,
    pub top_cpu: Vec<ProcInfo>,
    pub top_mem: Vec<ProcInfo>,
    pub listening_ports: Vec<String>,
    pub failed_services: Vec<String>,
    pub docker: Vec<DockerContainer>,
    pub docker_available: bool,
    pub warnings: Vec<String>,
}

/// Per-server raw counters used to compute rates between refreshes.
#[derive(Default, Clone)]
struct Sample {
    cpu_idle: u64,
    cpu_total: u64,
    net_rx: u64,
    net_tx: u64,
    epoch_ms: i64,
}

#[derive(Default)]
pub struct HealthState {
    last: Mutex<HashMap<String, Sample>>,
}

impl HealthState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Collect one snapshot for a server, computing rates against the previous
    /// sample.
    pub fn collect(&self, server: &Server) -> Result<HealthSnapshot> {
        let out = ssh_manager::run_remote(server, PROBE)?;
        if out.stdout.trim().is_empty() && !out.stderr.trim().is_empty() {
            anyhow::bail!("health probe failed: {}", out.stderr.trim());
        }
        let sections = split_sections(&out.stdout);
        let now_ms = chrono::Utc::now().timestamp_millis();

        let mut snap = HealthSnapshot::default();

        // OS / kernel / host
        snap.os_name = parse_os_name(sections.get("OS").map(|s| s.as_str()).unwrap_or(""));
        snap.kernel = sections.get("KERNEL").cloned().unwrap_or_default().trim().to_string();
        snap.hostname = sections.get("HOST").cloned().unwrap_or_default().trim().to_string();

        // uptime
        if let Some(u) = sections.get("UPTIME") {
            if let Some(first) = u.split_whitespace().next() {
                snap.uptime_secs = first.parse::<f64>().unwrap_or(0.0) as u64;
            }
        }

        // load
        if let Some(l) = sections.get("LOAD") {
            let parts: Vec<&str> = l.split_whitespace().collect();
            snap.load1 = parts.first().and_then(|v| v.parse().ok()).unwrap_or(0.0);
            snap.load5 = parts.get(1).and_then(|v| v.parse().ok()).unwrap_or(0.0);
            snap.load15 = parts.get(2).and_then(|v| v.parse().ok()).unwrap_or(0.0);
        }

        // cpu (needs previous sample)
        let (cpu_idle, cpu_total) = parse_cpu(sections.get("CPU").map(|s| s.as_str()).unwrap_or(""));

        // memory
        let mem = parse_meminfo(sections.get("MEM").map(|s| s.as_str()).unwrap_or(""));
        snap.mem_total_kb = mem.0;
        snap.mem_used_kb = mem.1;
        snap.mem_percent = pct(mem.1, mem.0);
        snap.swap_total_kb = mem.2;
        snap.swap_used_kb = mem.3;
        snap.swap_percent = pct(mem.3, mem.2);

        // disks
        snap.disks = parse_disks(sections.get("DISK").map(|s| s.as_str()).unwrap_or(""));

        // net (needs previous sample)
        let (net_rx, net_tx) = parse_net(sections.get("NET").map(|s| s.as_str()).unwrap_or(""));

        // processes
        snap.top_cpu = parse_ps(sections.get("PSCPU").map(|s| s.as_str()).unwrap_or(""));
        snap.top_mem = parse_ps(sections.get("PSMEM").map(|s| s.as_str()).unwrap_or(""));

        // ports
        snap.listening_ports = sections
            .get("PORTS")
            .map(|s| s.lines().skip(1).map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
            .unwrap_or_default();

        // failed services
        snap.failed_services = sections
            .get("FAILED")
            .map(|s| {
                s.lines()
                    .filter_map(|l| l.split_whitespace().next().map(|x| x.to_string()))
                    .filter(|x| !x.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        // docker
        let (docker, available) = parse_docker(
            sections.get("DOCKERPS").map(|s| s.as_str()).unwrap_or(""),
            sections.get("DOCKERSTATS").map(|s| s.as_str()).unwrap_or(""),
        );
        snap.docker = docker;
        snap.docker_available = available;

        // rates from previous sample
        {
            let mut guard = self.last.lock().unwrap();
            if let Some(prev) = guard.get(&server.id) {
                let dt = ((now_ms - prev.epoch_ms) as f64 / 1000.0).max(0.001);
                // CPU%
                let d_total = cpu_total.saturating_sub(prev.cpu_total);
                let d_idle = cpu_idle.saturating_sub(prev.cpu_idle);
                if d_total > 0 {
                    snap.cpu_percent = ((d_total - d_idle) as f64 / d_total as f64) * 100.0;
                }
                // network
                snap.net_rx_rate = (net_rx.saturating_sub(prev.net_rx)) as f64 / dt;
                snap.net_tx_rate = (net_tx.saturating_sub(prev.net_tx)) as f64 / dt;
            }
            guard.insert(
                server.id.clone(),
                Sample {
                    cpu_idle,
                    cpu_total,
                    net_rx,
                    net_tx,
                    epoch_ms: now_ms,
                },
            );
        }

        snap.warnings = build_warnings(&snap);
        Ok(snap)
    }

    /// Drop cached sampling state (e.g. when a server is disconnected).
    pub fn forget(&self, server_id: &str) {
        self.last.lock().unwrap().remove(server_id);
    }
}

fn build_warnings(s: &HealthSnapshot) -> Vec<String> {
    let mut w = Vec::new();
    if s.cpu_percent > 90.0 {
        w.push(format!("CPU usage high: {:.0}%", s.cpu_percent));
    }
    if s.mem_percent > 90.0 {
        w.push(format!("RAM usage high: {:.0}%", s.mem_percent));
    }
    for d in &s.disks {
        if d.use_percent > 85.0 {
            w.push(format!("Disk {} at {:.0}% ({})", d.mount, d.use_percent, d.filesystem));
        }
    }
    if !s.failed_services.is_empty() {
        w.push(format!("{} failed systemd service(s)", s.failed_services.len()));
    }
    for c in &s.docker {
        if c.status.to_lowercase().contains("exited") {
            w.push(format!("Docker container '{}' exited", c.name));
        }
    }
    w
}

// ---------- parsing helpers ----------

fn split_sections(raw: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut current: Option<String> = None;
    let mut buf = String::new();
    for line in raw.lines() {
        let t = line.trim();
        if t.starts_with("@@") && t.ends_with("@@") {
            if let Some(name) = current.take() {
                map.insert(name, std::mem::take(&mut buf));
            }
            let name = t.trim_matches('@').to_string();
            if name == "END" {
                break;
            }
            current = Some(name);
        } else if current.is_some() {
            buf.push_str(line);
            buf.push('\n');
        }
    }
    if let Some(name) = current.take() {
        map.insert(name, buf);
    }
    map
}

fn pct(used: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        (used as f64 / total as f64) * 100.0
    }
}

fn parse_os_name(s: &str) -> String {
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("PRETTY_NAME=") {
            return rest.trim_matches('"').to_string();
        }
    }
    "Linux".to_string()
}

/// Returns (idle, total) jiffies from a `cpu ...` line.
fn parse_cpu(line: &str) -> (u64, u64) {
    let nums: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .filter_map(|v| v.parse().ok())
        .collect();
    if nums.is_empty() {
        return (0, 0);
    }
    let total: u64 = nums.iter().sum();
    // idle = idle (field 3) + iowait (field 4)
    let idle = nums.get(3).copied().unwrap_or(0) + nums.get(4).copied().unwrap_or(0);
    (idle, total)
}

/// Returns (mem_total, mem_used, swap_total, swap_used) in kB.
fn parse_meminfo(s: &str) -> (u64, u64, u64, u64) {
    let get = |key: &str| -> u64 {
        for line in s.lines() {
            if let Some(rest) = line.strip_prefix(key) {
                return rest
                    .trim()
                    .trim_start_matches(':')
                    .trim()
                    .split_whitespace()
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
            }
        }
        0
    };
    let mem_total = get("MemTotal");
    let mem_available = get("MemAvailable");
    let mem_used = mem_total.saturating_sub(mem_available);
    let swap_total = get("SwapTotal");
    let swap_free = get("SwapFree");
    let swap_used = swap_total.saturating_sub(swap_free);
    (mem_total, mem_used, swap_total, swap_used)
}

fn parse_disks(s: &str) -> Vec<DiskInfo> {
    let mut out = Vec::new();
    for line in s.lines().skip(1) {
        let p: Vec<&str> = line.split_whitespace().collect();
        // Filesystem 1024-blocks Used Available Capacity Mounted on
        if p.len() >= 6 {
            let fs = p[0].to_string();
            // skip pseudo filesystems
            if fs.starts_with("tmpfs") || fs.starts_with("devtmpfs") || fs == "none" {
                continue;
            }
            out.push(DiskInfo {
                filesystem: fs,
                size_kb: p[1].parse().unwrap_or(0),
                used_kb: p[2].parse().unwrap_or(0),
                use_percent: p[4].trim_end_matches('%').parse().unwrap_or(0.0),
                mount: p[5..].join(" "),
            });
        }
    }
    out
}

/// Sum rx/tx across all non-loopback interfaces.
fn parse_net(s: &str) -> (u64, u64) {
    let mut rx = 0u64;
    let mut tx = 0u64;
    for line in s.lines() {
        let p: Vec<&str> = line.split_whitespace().collect();
        if p.len() == 3 {
            rx += p[1].parse::<u64>().unwrap_or(0);
            tx += p[2].parse::<u64>().unwrap_or(0);
        }
    }
    (rx, tx)
}

fn parse_ps(s: &str) -> Vec<ProcInfo> {
    let mut out = Vec::new();
    for line in s.lines().skip(1) {
        let p: Vec<&str> = line.split_whitespace().collect();
        if p.len() >= 4 {
            out.push(ProcInfo {
                pid: p[0].to_string(),
                command: p[1].to_string(),
                cpu: p[2].parse().unwrap_or(0.0),
                mem: p[3].parse().unwrap_or(0.0),
            });
        }
    }
    out
}

fn parse_docker(ps: &str, stats: &str) -> (Vec<DockerContainer>, bool) {
    let ps = ps.trim();
    // No docker binary -> the section is empty.
    if ps.is_empty() && stats.trim().is_empty() {
        return (Vec::new(), false);
    }
    let mut stat_map: HashMap<String, (Option<f64>, Option<f64>)> = HashMap::new();
    for line in stats.lines() {
        let p: Vec<&str> = line.split('|').collect();
        if p.len() == 3 {
            let cpu = p[1].trim_end_matches('%').parse().ok();
            let mem = p[2].trim_end_matches('%').parse().ok();
            stat_map.insert(p[0].to_string(), (cpu, mem));
        }
    }
    let mut out = Vec::new();
    for line in ps.lines() {
        let p: Vec<&str> = line.split('|').collect();
        if p.len() >= 3 {
            let name = p[0].to_string();
            let (cpu, mem) = stat_map.get(&name).copied().unwrap_or((None, None));
            out.push(DockerContainer {
                name,
                status: p[1].to_string(),
                image: p[2].to_string(),
                cpu_percent: cpu,
                mem_percent: mem,
            });
        }
    }
    (out, true)
}
