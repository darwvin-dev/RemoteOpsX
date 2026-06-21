//! Agentless live server health collection.
//!
//! A single SSH exec runs one shell snippet that emits every metric section
//! between `@@MARKER@@` lines (one round-trip per refresh). Rates that need two
//! samples (CPU %, network throughput) are derived in Rust by diffing against
//! the previous snapshot held per-server in `HealthState`.
//!
//! Nothing is installed on the remote host; only standard /proc, /sys and
//! coreutils/`ss`/`systemctl` reads are used.

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

        let mut snap = HealthSnapshot {
            os_name: parse_os_name(sections.get("OS").map(|s| s.as_str()).unwrap_or("")),
            kernel: sections
                .get("KERNEL")
                .cloned()
                .unwrap_or_default()
                .trim()
                .to_string(),
            hostname: sections
                .get("HOST")
                .cloned()
                .unwrap_or_default()
                .trim()
                .to_string(),
            ..HealthSnapshot::default()
        };

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
        let (cpu_idle, cpu_total) =
            parse_cpu(sections.get("CPU").map(|s| s.as_str()).unwrap_or(""));

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
            .map(|s| {
                s.lines()
                    .skip(1)
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty())
                    .collect()
            })
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
            w.push(format!(
                "Disk {} at {:.0}% ({})",
                d.mount, d.use_percent, d.filesystem
            ));
        }
    }
    if !s.failed_services.is_empty() {
        w.push(format!(
            "{} failed systemd service(s)",
            s.failed_services.len()
        ));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_idle_and_total() {
        // user nice system idle iowait irq softirq ...
        let (idle, total) = parse_cpu("cpu  100 0 50 800 50 0 0 0 0 0");
        assert_eq!(idle, 850, "idle = idle(800)+iowait(50)");
        assert_eq!(total, 1000, "sum of all jiffies");
    }

    #[test]
    fn cpu_empty_line_is_safe() {
        assert_eq!(parse_cpu(""), (0, 0));
    }

    #[test]
    fn meminfo_used_is_total_minus_available() {
        let mem = "MemTotal:       16000000 kB\nMemFree:  100000 kB\nMemAvailable:    4000000 kB\nSwapTotal:  2000000 kB\nSwapFree:   500000 kB\n";
        let (total, used, swap_total, swap_used) = parse_meminfo(mem);
        assert_eq!(total, 16_000_000);
        assert_eq!(used, 12_000_000); // total - available
        assert_eq!(swap_total, 2_000_000);
        assert_eq!(swap_used, 1_500_000);
    }

    #[test]
    fn disks_skip_pseudo_and_parse_percent() {
        let df = "Filesystem 1024-blocks Used Available Capacity Mounted on\n\
                  /dev/sda1 100000 80000 20000 80% /\n\
                  tmpfs 5000 10 4990 1% /run\n\
                  /dev/sdb1 200000 50000 150000 25% /data mount\n";
        let disks = parse_disks(df);
        assert_eq!(disks.len(), 2, "tmpfs is excluded");
        assert_eq!(disks[0].mount, "/");
        assert_eq!(disks[0].use_percent, 80.0);
        // mount paths with spaces are preserved
        assert_eq!(disks[1].mount, "/data mount");
    }

    #[test]
    fn net_sums_all_interfaces() {
        let (rx, tx) = parse_net("eth0 1000 2000\nwg0 500 250\n");
        assert_eq!(rx, 1500);
        assert_eq!(tx, 2250);
    }

    #[test]
    fn ps_parses_columns() {
        let ps = "PID COMMAND %CPU %MEM\n123 nginx 12.5 3.2\n456 postgres 4.0 8.1\n";
        let procs = parse_ps(ps);
        assert_eq!(procs.len(), 2);
        assert_eq!(procs[0].command, "nginx");
        assert_eq!(procs[0].cpu, 12.5);
        assert_eq!(procs[1].mem, 8.1);
    }

    #[test]
    fn sections_split_on_markers() {
        let raw = "@@OS@@\nPRETTY_NAME=\"Arch Linux\"\n@@KERNEL@@\n6.0.0\n@@END@@\nignored";
        let map = split_sections(raw);
        assert_eq!(parse_os_name(map.get("OS").unwrap()), "Arch Linux");
        assert_eq!(map.get("KERNEL").unwrap().trim(), "6.0.0");
        assert!(!map.contains_key("END"));
    }

    #[test]
    fn warnings_fire_on_thresholds() {
        let mut s = HealthSnapshot {
            cpu_percent: 95.0,
            mem_percent: 30.0,
            ..HealthSnapshot::default()
        };
        s.disks.push(DiskInfo {
            filesystem: "/dev/sda1".into(),
            size_kb: 100,
            used_kb: 90,
            use_percent: 90.0,
            mount: "/".into(),
        });
        s.failed_services.push("foo.service".into());
        let w = build_warnings(&s);
        assert!(w.iter().any(|x| x.contains("CPU")));
        assert!(w.iter().any(|x| x.contains("Disk")));
        assert!(w.iter().any(|x| x.contains("failed")));
        assert!(!w.iter().any(|x| x.contains("RAM")), "mem under threshold");
    }
}
