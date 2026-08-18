import { useCallback, useEffect, useState } from "react";
import * as experienceApi from "../experienceApi";
import { useStore } from "../store";
import type { DashboardSummary } from "../experienceTypes";
import type { Server, TabKind } from "../types";

export function OperationsDashboard({
  servers,
  focusedServer,
  onFocusServer,
  onOpenTab,
  onNewServer,
  onOpenRunbooks,
  onOpenTunnels,
  onOpenOperations,
}: {
  servers: Server[];
  focusedServer: Server | null;
  onFocusServer: (id: string | null) => void;
  onOpenTab: (kind: TabKind, server: Server) => string;
  onNewServer: () => void;
  onOpenRunbooks: () => void;
  onOpenTunnels: () => void;
  onOpenOperations: () => void;
}) {
  const pushAlert = useStore((state) => state.pushAlert);
  const [summary, setSummary] = useState<DashboardSummary | null>(null);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      setSummary(await experienceApi.dashboardSummary());
    } catch (reason) {
      pushAlert("error", `Dashboard refresh failed: ${reason}`);
    } finally {
      setLoading(false);
    }
  }, [pushAlert]);

  useEffect(() => {
    void refresh();
    const timer = window.setInterval(() => void refresh(), 15_000);
    return () => window.clearInterval(timer);
  }, [refresh]);

  const serverById = (id: string) => servers.find((server) => server.id === id) ?? null;

  return (
    <div className="start-dashboard operations-dashboard">
      <section className="hero-card operations-hero">
        <div className="hero-copy">
          <span className="eyebrow">Operations overview</span>
          <h1>{summary?.critical ? `${summary.critical} critical target${summary.critical === 1 ? "" : "s"}` : "Infrastructure at a glance."}</h1>
          <p>
            Persisted health, alert events, tunnels and recent automation are summarized here without deploying an agent.
          </p>
          <div className="hero-actions">
            <button className="primary" onClick={focusedServer ? () => onOpenTab("ssh", focusedServer) : onNewServer}>
              {focusedServer ? `SSH into ${focusedServer.name}` : "Add first server"}
            </button>
            <button onClick={onOpenOperations}>Operator Center</button>
            <button onClick={onOpenRunbooks}>Runbooks</button>
            <button className="ghost" onClick={onOpenTunnels}>Tunnels</button>
            <button className="ghost" disabled={loading} onClick={() => void refresh()}>Refresh</button>
          </div>
        </div>
      </section>

      <section className="dashboard-grid">
        <MetricCard value={summary?.servers_total ?? servers.length} label="Servers" detail={`${summary?.unknown ?? servers.length} without persisted health`} />
        <MetricCard value={summary?.healthy ?? 0} label="Healthy" detail={`${summary?.warning ?? 0} warning`} tone="ok" />
        <MetricCard value={summary?.critical ?? 0} label="Critical" detail={`${summary?.unacknowledged_alerts ?? 0} unacknowledged alerts`} tone={summary?.critical ? "critical" : undefined} />
        <MetricCard value={summary?.active_tunnels ?? 0} label="Active tunnels" detail={`${summary?.failed_tunnels ?? 0} failed`} />
      </section>

      <section className="operations-columns">
        <div className="quick-card operations-server-card">
          <div className="flex" style={{ justifyContent: "space-between" }}>
            <div><span className="eyebrow">Fleet</span><h3>Server state</h3></div>
            <small className="muted">30s persisted samples</small>
          </div>
          {!summary || summary.servers.length === 0 ? (
            <div className="panel-hint">{servers.length ? "Health samples appear after polling succeeds." : "Add a server to start."}</div>
          ) : (
            <div className="operations-server-list">
              {summary.servers.slice(0, 18).map((rollup) => {
                const server = serverById(rollup.server_id);
                return (
                  <div key={rollup.server_id} className={`operations-server-row status-${rollup.status}`}>
                    <button className="mini-server" onClick={() => onFocusServer(rollup.server_id)}>
                      <span className={`conn-dot ${statusDot(rollup.status)}`} />
                      <span>{rollup.name}</span>
                      <small>{rollup.environment}</small>
                    </button>
                    <div className="operations-metrics mono">
                      <span>CPU {formatPercent(rollup.cpu_percent)}</span>
                      <span>RAM {formatPercent(rollup.mem_percent)}</span>
                      <span>Disk {formatPercent(rollup.max_disk_percent)}</span>
                      {rollup.failed_services ? <span className="error-text">{rollup.failed_services} failed svc</span> : null}
                    </div>
                    {server?.protocols.includes("ssh") ? <button className="tiny" onClick={() => onOpenTab("ssh", server)}>SSH</button> : null}
                  </div>
                );
              })}
            </div>
          )}
        </div>

        <div className="quick-card">
          <span className="eyebrow">Alerts</span>
          <h3>Recent events</h3>
          {!summary?.recent_alerts.length ? <div className="panel-hint">No persisted alert events.</div> : summary.recent_alerts.slice(0, 8).map((alert) => (
            <button key={alert.id} className="dashboard-event" onClick={() => onFocusServer(alert.server_id)}>
              <span>{alert.acknowledged_at ? "✓" : "⚠"}</span>
              <span>{alert.message}</span>
              <small>{new Date(alert.fired_at).toLocaleTimeString()}</small>
            </button>
          ))}
          <button className="tiny" onClick={onOpenOperations}>Manage alerts</button>
        </div>

        <div className="quick-card">
          <span className="eyebrow">Automation</span>
          <h3>Recent runs</h3>
          {!summary?.recent_runbooks.length && !summary?.recent_multi_host.length ? (
            <div className="panel-hint">No recent automation runs.</div>
          ) : (
            <div className="dashboard-run-list">
              {summary?.recent_runbooks.slice(0, 5).map((run) => (
                <div key={run.id} className="dashboard-event static"><span>{run.status === "success" ? "✓" : "●"}</span><span>Runbook · {run.status}</span><small>{new Date(run.started_at).toLocaleTimeString()}</small></div>
              ))}
              {summary?.recent_multi_host.slice(0, 5).map((run) => (
                <div key={run.id} className="dashboard-event static"><span>{run.status === "success" ? "✓" : "●"}</span><span>Multi-host · {run.status}</span><small>{run.results.length} hosts</small></div>
              ))}
            </div>
          )}
          <div className="flex"><button className="tiny" onClick={onOpenRunbooks}>Runbooks</button><button className="tiny" onClick={onOpenOperations}>Multi-host</button></div>
        </div>
      </section>
    </div>
  );
}

function MetricCard({ value, label, detail, tone }: { value: number; label: string; detail: string; tone?: "ok" | "critical" }) {
  return <div className={`insight-card${tone ? ` metric-${tone}` : ""}`}><span className="insight-value">{value}</span><span className="insight-label">{label}</span><small>{detail}</small></div>;
}

function formatPercent(value?: number | null) {
  return value == null ? "—" : `${value.toFixed(0)}%`;
}

function statusDot(status: string) {
  if (status === "healthy") return "connected";
  if (status === "critical") return "closed";
  return "connecting";
}
