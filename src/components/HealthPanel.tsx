import { useEffect, useRef, useState } from "react";
import * as api from "../api";
import { useStore } from "../store";
import type { HealthSnapshot, Server } from "../types";

/** Live agentless health for the focused server. Polls `health_collect` on the
 *  configurable interval and renders metric cards, sparklines and warnings. */
export function HealthPanel({ server }: { server: Server }) {
  const intervalMs = useStore((s) => s.healthIntervalMs);
  const setInterval_ = useStore((s) => s.setHealthInterval);
  const pushAlert = useStore((s) => s.pushAlert);
  const [snap, setSnap] = useState<HealthSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [paused, setPaused] = useState(false);

  // Rolling history for sparklines.
  const cpuHist = useRef<number[]>([]);
  const memHist = useRef<number[]>([]);
  const netHist = useRef<number[]>([]);
  const lastWarnKey = useRef<string>("");

  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;

    async function tick() {
      if (paused) { timer = setTimeout(tick, intervalMs); return; }
      try {
        const s = await api.healthCollect(server.id);
        if (cancelled) return;
        setSnap(s);
        setError(null);
        cpuHist.current = [...cpuHist.current, s.cpu_percent].slice(-40);
        memHist.current = [...memHist.current, s.mem_percent].slice(-40);
        netHist.current = [...netHist.current, s.net_rx_rate + s.net_tx_rate].slice(-40);
        // Surface new warnings to the alert log (dedup on content).
        const key = s.warnings.join("|");
        if (s.warnings.length && key !== lastWarnKey.current) {
          s.warnings.forEach((w) => pushAlert("warn", `${server.name}: ${w}`, server.id));
          lastWarnKey.current = key;
        }
        if (!s.warnings.length) lastWarnKey.current = "";
      } catch (err) {
        if (cancelled) return;
        setError(String(err));
      }
      timer = setTimeout(tick, intervalMs);
    }
    void tick();
    return () => { cancelled = true; clearTimeout(timer); };
  }, [server.id, intervalMs, paused, pushAlert, server.name]);

  return (
    <div>
      <div className="health-head">
        <div>
          <div className="host">{snap?.hostname || server.host}</div>
          <div className="os">{snap?.os_name} · {snap?.kernel}</div>
        </div>
        <div className="flex">
          <button className="tiny ghost" onClick={() => setPaused((p) => !p)}>{paused ? "▶" : "⏸"}</button>
          <select
            style={{ width: "auto" }}
            value={intervalMs}
            onChange={(e) => setInterval_(Number(e.target.value))}
            title="Refresh interval"
          >
            <option value={2000}>2s</option>
            <option value={3000}>3s</option>
            <option value={5000}>5s</option>
            <option value={10000}>10s</option>
          </select>
        </div>
      </div>

      {error && <div className="warn-banner">⚠ {error}</div>}
      {!snap && !error && <div className="panel-hint">Collecting metrics…</div>}

      {snap && (
        <>
          {snap.warnings.length === 0 ? (
            <div className="warn-banner ok">✓ All clear</div>
          ) : (
            snap.warnings.map((w, i) => <div key={i} className="warn-banner">⚠ {w}</div>)
          )}

          <div className="card-grid">
            <Metric label="CPU" value={`${snap.cpu_percent.toFixed(0)}%`} pct={snap.cpu_percent} crit={90} warn={70} />
            <Metric label="RAM" value={`${snap.mem_percent.toFixed(0)}%`} sub={`${fmtKb(snap.mem_used_kb)} / ${fmtKb(snap.mem_total_kb)}`} pct={snap.mem_percent} crit={90} warn={75} />
            <Metric label="Swap" value={`${snap.swap_percent.toFixed(0)}%`} sub={snap.swap_total_kb ? `${fmtKb(snap.swap_used_kb)} / ${fmtKb(snap.swap_total_kb)}` : "none"} pct={snap.swap_percent} crit={50} warn={25} />
            <Metric label="Load (1m)" value={snap.load1.toFixed(2)} sub={`5m ${snap.load5.toFixed(2)} · 15m ${snap.load15.toFixed(2)}`} />
          </div>

          <div className="card-grid" style={{ marginTop: 8 }}>
            <div className="metric-card">
              <div className="mc-label">CPU history</div>
              <Spark data={cpuHist.current} max={100} />
            </div>
            <div className="metric-card">
              <div className="mc-label">RAM history</div>
              <Spark data={memHist.current} max={100} />
            </div>
            <div className="metric-card span-2">
              <div className="mc-label">Network · ↓ {fmtRate(snap.net_rx_rate)} ↑ {fmtRate(snap.net_tx_rate)}</div>
              <Spark data={netHist.current} max={Math.max(1, ...netHist.current)} blue />
            </div>
            <div className="metric-card span-2">
              <div className="mc-label">Uptime</div>
              <div className="mc-value" style={{ fontSize: 16 }}>{fmtUptime(snap.uptime_secs)}</div>
            </div>
          </div>

          <div className="section-title">Disks</div>
          <table className="data">
            <thead><tr><th>Filesystem</th><th>Mount</th><th>Use</th></tr></thead>
            <tbody>
              {snap.disks.map((d, i) => (
                <tr key={i}>
                  <td className="mono">{d.filesystem}</td>
                  <td className="mono">{d.mount}</td>
                  <td>
                    <div className="flex">
                      <div className={`bar ${d.use_percent > 85 ? "crit" : d.use_percent > 70 ? "warn" : ""}`} style={{ width: 60 }}>
                        <span style={{ width: `${d.use_percent}%` }} />
                      </div>
                      <span className="mono">{d.use_percent.toFixed(0)}%</span>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>

          <div className="section-title">Top processes (CPU)</div>
          <table className="data">
            <thead><tr><th>PID</th><th>Command</th><th>%CPU</th><th>%MEM</th></tr></thead>
            <tbody>
              {snap.top_cpu.slice(0, 6).map((p, i) => (
                <tr key={i}>
                  <td className="mono">{p.pid}</td>
                  <td className="mono">{p.command}</td>
                  <td className="mono">{p.cpu.toFixed(1)}</td>
                  <td className="mono">{p.mem.toFixed(1)}</td>
                </tr>
              ))}
            </tbody>
          </table>

          {snap.failed_services.length > 0 && (
            <>
              <div className="section-title">Failed services</div>
              {snap.failed_services.map((s, i) => (
                <div key={i} className="mono" style={{ fontSize: 11, color: "var(--crit)" }}>● {s}</div>
              ))}
            </>
          )}

          <div className="section-title">Listening ports</div>
          <div className="mono" style={{ fontSize: 10, color: "var(--text-1)", maxHeight: 120, overflowY: "auto", whiteSpace: "pre" }}>
            {snap.listening_ports.slice(0, 20).join("\n") || "—"}
          </div>
        </>
      )}
    </div>
  );
}

function Metric({ label, value, sub, pct, warn, crit }: { label: string; value: string; sub?: string; pct?: number; warn?: number; crit?: number }) {
  const cls = pct === undefined ? "" : crit && pct > crit ? "crit" : warn && pct > warn ? "warn" : "";
  return (
    <div className="metric-card">
      <div className="mc-label">{label}</div>
      <div className="mc-value">{value}</div>
      {sub && <div className="mc-sub">{sub}</div>}
      {pct !== undefined && (
        <div className={`bar ${cls}`}><span style={{ width: `${Math.min(100, pct)}%` }} /></div>
      )}
    </div>
  );
}

function Spark({ data, max, blue }: { data: number[]; max: number; blue?: boolean }) {
  return (
    <div className={`spark${blue ? " blue" : ""}`}>
      {data.length === 0 && <span style={{ height: 1 }} />}
      {data.map((v, i) => (
        <span key={i} style={{ height: `${Math.max(2, (v / (max || 1)) * 100)}%` }} />
      ))}
    </div>
  );
}

function fmtKb(kb: number): string {
  const mb = kb / 1024;
  if (mb < 1024) return `${mb.toFixed(0)}M`;
  return `${(mb / 1024).toFixed(1)}G`;
}
function fmtRate(bytesPerSec: number): string {
  if (bytesPerSec < 1024) return `${bytesPerSec.toFixed(0)} B/s`;
  if (bytesPerSec < 1024 * 1024) return `${(bytesPerSec / 1024).toFixed(1)} KB/s`;
  return `${(bytesPerSec / 1024 / 1024).toFixed(2)} MB/s`;
}
function fmtUptime(secs: number): string {
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return `${d}d ${h}h ${m}m`;
}
