import { useCallback, useEffect, useMemo, useState } from "react";
import * as api from "../api";
import * as operatorApi from "../operatorApi";
import type {
  AlertRule,
  AlertRuleInput,
  BackupRestoreReport,
  HealthPoint,
  MultiHostRun,
  OperatorAlert,
  TransferDirection,
  TransferJob,
  TunnelPolicy,
} from "../operatorTypes";
import { useStore } from "../store";
import type { Tunnel } from "../types";

export function OperatorCenter({ onClose }: { onClose: () => void }) {
  const servers = useStore((state) => state.servers);
  const pushAlert = useStore((state) => state.pushAlert);
  const [section, setSection] = useState<"health" | "transfers" | "multi" | "tunnels" | "backup">("health");
  const [error, setError] = useState<string | null>(null);

  const reportError = useCallback((reason: unknown) => {
    const message = String(reason);
    setError(message);
    pushAlert("error", message);
  }, [pushAlert]);

  return (
    <div className="modal-backdrop" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
      <div className="modal wide" role="dialog" aria-modal="true" aria-label="Operator center">
        <div className="modal-head">
          <div><span className="eyebrow">Operations workspace</span><strong>Operator Center</strong></div>
          <button className="ghost tiny" onClick={onClose}>✕</button>
        </div>
        <div className="modal-body">
          <div className="segmented" style={{ marginBottom: 12 }}>
            {(["health", "transfers", "multi", "tunnels", "backup"] as const).map((item) => (
              <button key={item} className={section === item ? "active" : ""} onClick={() => { setSection(item); setError(null); }}>
                {item === "multi" ? "Multi-host" : item[0].toUpperCase() + item.slice(1)}
              </button>
            ))}
          </div>
          {error ? <div className="warn-banner">⚠ {error}</div> : null}
          {section === "health" && <HealthOps servers={servers} reportError={reportError} />}
          {section === "transfers" && <TransferOps servers={servers} reportError={reportError} />}
          {section === "multi" && <MultiHostOps servers={servers} reportError={reportError} />}
          {section === "tunnels" && <TunnelOps reportError={reportError} />}
          {section === "backup" && <BackupOps reportError={reportError} />}
        </div>
        <div className="modal-foot"><button onClick={onClose}>Close</button></div>
      </div>
    </div>
  );
}

function HealthOps({ servers, reportError }: { servers: ReturnType<typeof useStore.getState>["servers"]; reportError: (reason: unknown) => void }) {
  const [serverId, setServerId] = useState(servers[0]?.id ?? "");
  const [history, setHistory] = useState<HealthPoint[]>([]);
  const [rules, setRules] = useState<AlertRule[]>([]);
  const [alerts, setAlerts] = useState<OperatorAlert[]>([]);
  const [metric, setMetric] = useState("cpu_percent");
  const [threshold, setThreshold] = useState(90);
  const [samples, setSamples] = useState(2);
  const [cooldown, setCooldown] = useState(300);

  const refresh = useCallback(async () => {
    try {
      const [points, nextRules, nextAlerts] = await Promise.all([
        serverId ? operatorApi.healthHistory(serverId, 720) : Promise.resolve([]),
        operatorApi.alertRulesList(),
        operatorApi.alertsList(200),
      ]);
      setHistory(points.slice().reverse());
      setRules(nextRules);
      setAlerts(nextAlerts);
    } catch (reason) { reportError(reason); }
  }, [reportError, serverId]);

  useEffect(() => { void refresh(); }, [refresh]);

  async function saveRule() {
    const input: AlertRuleInput = {
      server_id: serverId || null,
      metric,
      comparison: "gt",
      threshold,
      consecutive_samples: samples,
      cooldown_seconds: cooldown,
      enabled: true,
    };
    try { await operatorApi.alertRuleSave(input); await refresh(); } catch (reason) { reportError(reason); }
  }

  return (
    <div>
      <div className="section-title">Historical health</div>
      <div className="form-row three">
        <div><label>Server</label><select value={serverId} onChange={(e) => setServerId(e.target.value)}>{servers.map((server) => <option key={server.id} value={server.id}>{server.name}</option>)}</select></div>
        <div><label>Window</label><div className="panel-hint">30s samples · bounded to 7 days</div></div>
        <div><label>&nbsp;</label><button onClick={() => void refresh()}>Refresh history</button></div>
      </div>
      <HealthChart points={history} />

      <div className="section-title">Alert rule</div>
      <div className="form-row three">
        <div><label>Metric</label><select value={metric} onChange={(e) => setMetric(e.target.value)}>
          <option value="cpu_percent">CPU %</option><option value="mem_percent">RAM %</option>
          <option value="swap_percent">Swap %</option><option value="max_disk_percent">Max disk %</option>
          <option value="load1">Load 1m</option><option value="failed_services">Failed services</option>
        </select></div>
        <div><label>Threshold &gt;</label><input type="number" value={threshold} onChange={(e) => setThreshold(Number(e.target.value))} /></div>
        <div><label>Consecutive samples</label><input type="number" min={1} max={20} value={samples} onChange={(e) => setSamples(Number(e.target.value))} /></div>
      </div>
      <div className="form-row three">
        <div><label>Cooldown seconds</label><input type="number" min={30} max={86400} value={cooldown} onChange={(e) => setCooldown(Number(e.target.value))} /></div>
        <div><label>Scope</label><div className="panel-hint">{serverId ? "Selected server" : "All servers"}</div></div>
        <div><label>&nbsp;</label><button className="primary" onClick={() => void saveRule()}>Add rule</button></div>
      </div>
      {rules.map((rule) => (
        <div key={rule.id} className="list-row"><span className="mono">{rule.metric} {rule.comparison} {rule.threshold}</span><span className="muted">{rule.consecutive_samples}× · {rule.cooldown_seconds}s</span><button className="tiny ghost" onClick={() => void operatorApi.alertRuleDelete(rule.id).then(refresh).catch(reportError)}>Delete</button></div>
      ))}

      <div className="section-title">Persisted alerts</div>
      {alerts.length === 0 ? <div className="panel-hint">No alert events yet.</div> : alerts.slice(0, 30).map((alert) => (
        <div key={alert.id} className="list-row">
          <span>{alert.acknowledged_at ? "✓" : "⚠"} {alert.message}</span>
          <span className="muted">{new Date(alert.fired_at).toLocaleString()}</span>
          {!alert.acknowledged_at ? <button className="tiny" onClick={() => void operatorApi.alertAcknowledge(alert.id).then(refresh).catch(reportError)}>Acknowledge</button> : null}
        </div>
      ))}
    </div>
  );
}

function HealthChart({ points }: { points: HealthPoint[] }) {
  const values = points.slice(-120).map((point) => point.cpu_percent);
  if (values.length < 2) return <div className="panel-hint">History will appear after health polling records samples.</div>;
  const coords = values.map((value, index) => `${(index / (values.length - 1)) * 100},${100 - Math.max(0, Math.min(100, value))}`).join(" ");
  return (
    <div style={{ height: 130, border: "1px solid var(--border)", borderRadius: 6, padding: 8, marginTop: 8 }}>
      <div className="mc-label">CPU · last {values.length} persisted samples</div>
      <svg viewBox="0 0 100 100" preserveAspectRatio="none" style={{ width: "100%", height: 96 }} aria-label="CPU history chart">
        <polyline points={coords} fill="none" stroke="currentColor" strokeWidth="1.5" vectorEffect="non-scaling-stroke" />
      </svg>
    </div>
  );
}

function TransferOps({ servers, reportError }: { servers: ReturnType<typeof useStore.getState>["servers"]; reportError: (reason: unknown) => void }) {
  const [serverId, setServerId] = useState(servers[0]?.id ?? "");
  const [direction, setDirection] = useState<TransferDirection>("upload");
  const [source, setSource] = useState("");
  const [destination, setDestination] = useState("");
  const [recursive, setRecursive] = useState(false);
  const [jobs, setJobs] = useState<TransferJob[]>([]);

  const refresh = useCallback(async () => {
    try { setJobs(await operatorApi.transferJobsList()); } catch (reason) { reportError(reason); }
  }, [reportError]);
  useEffect(() => {
    void refresh();
    const timer = window.setInterval(() => void refresh(), 1000);
    return () => window.clearInterval(timer);
  }, [refresh]);

  async function start() {
    try {
      await operatorApi.transferStart({ server_id: serverId, direction, source, destination, recursive });
      setSource("");
      await refresh();
    } catch (reason) { reportError(reason); }
  }

  return (
    <div>
      <div className="warn-banner ok">Persistent OpenSSH ControlMaster connections are reused across transfer jobs.</div>
      <div className="form-row three">
        <div><label>Server</label><select value={serverId} onChange={(e) => setServerId(e.target.value)}>{servers.map((server) => <option key={server.id} value={server.id}>{server.name}</option>)}</select></div>
        <div><label>Direction</label><select value={direction} onChange={(e) => setDirection(e.target.value as TransferDirection)}><option value="upload">Upload</option><option value="download">Download</option></select></div>
        <label className="settings-toggle"><span>Recursive</span><input type="checkbox" checked={recursive} onChange={(e) => setRecursive(e.target.checked)} /></label>
      </div>
      <div><label>{direction === "upload" ? "Local source" : "Remote source"}</label><input value={source} onChange={(e) => setSource(e.target.value)} /></div>
      <div><label>{direction === "upload" ? "Remote destination" : "Local destination"}</label><input value={destination} onChange={(e) => setDestination(e.target.value)} /></div>
      <button className="primary" disabled={!serverId || !source || !destination} onClick={() => void start()}>Start transfer</button>
      <div className="section-title">Transfer queue</div>
      {jobs.length === 0 ? <div className="panel-hint">No transfer jobs.</div> : jobs.map((job) => (
        <div key={job.id} className="list-row">
          <span className="mono">{job.direction} · {job.source} → {job.destination}</span>
          <span>{job.progress_percent == null ? job.status : `${job.progress_percent.toFixed(0)}% · ${job.status}`}</span>
          {job.status === "running" ? <button className="tiny" onClick={() => void operatorApi.transferCancel(job.id).then(refresh).catch(reportError)}>Cancel</button> : null}
          {job.error ? <span className="error-text">{job.error}</span> : null}
        </div>
      ))}
    </div>
  );
}

function MultiHostOps({ servers, reportError }: { servers: ReturnType<typeof useStore.getState>["servers"]; reportError: (reason: unknown) => void }) {
  const [selected, setSelected] = useState<string[]>([]);
  const [command, setCommand] = useState("");
  const [concurrency, setConcurrency] = useState(4);
  const [productionConfirmed, setProductionConfirmed] = useState(false);
  const [destructiveConfirmed, setDestructiveConfirmed] = useState(false);
  const [run, setRun] = useState<MultiHostRun | null>(null);
  const selectedServers = useMemo(() => servers.filter((server) => selected.includes(server.id)), [servers, selected]);
  const includesProduction = selectedServers.some((server) => server.environment === "production");
  const looksDestructive = /rm\s+-rf|mkfs|shutdown|poweroff|reboot|halt|dd\s+if=|systemctl\s+(stop|restart)|kubectl\s+delete/i.test(command);

  async function execute() {
    try {
      setRun(await operatorApi.multiHostRun({
        server_ids: selected,
        command,
        concurrency,
        production_confirmed: productionConfirmed,
        destructive_confirmed: destructiveConfirmed,
      }));
    } catch (reason) { reportError(reason); }
  }

  return (
    <div>
      <div className="section-title">Targets</div>
      <div style={{ maxHeight: 180, overflow: "auto", border: "1px solid var(--border)", borderRadius: 6 }}>
        {servers.map((server) => <label key={server.id} className="settings-toggle"><span>{server.name} · {server.environment}</span><input type="checkbox" checked={selected.includes(server.id)} onChange={(e) => setSelected((current) => e.target.checked ? [...current, server.id] : current.filter((id) => id !== server.id))} /></label>)}
      </div>
      <div><label>Command</label><textarea rows={4} value={command} onChange={(e) => setCommand(e.target.value)} /></div>
      <div className="form-row three">
        <div><label>Concurrency</label><input type="number" min={1} max={8} value={concurrency} onChange={(e) => setConcurrency(Number(e.target.value))} /></div>
        <label className="settings-toggle"><span>Confirm production targets</span><input type="checkbox" checked={productionConfirmed} onChange={(e) => setProductionConfirmed(e.target.checked)} /></label>
        <label className="settings-toggle"><span>Confirm destructive command</span><input type="checkbox" checked={destructiveConfirmed} onChange={(e) => setDestructiveConfirmed(e.target.checked)} /></label>
      </div>
      {includesProduction ? <div className="warn-banner">⚠ Selection includes production servers.</div> : null}
      {looksDestructive ? <div className="warn-banner">⚠ Command matches a destructive-operation family.</div> : null}
      <button className="primary" disabled={!selected.length || !command.trim()} onClick={() => void execute()}>Run on {selected.length} host{selected.length === 1 ? "" : "s"}</button>
      {run ? <>
        <div className="section-title">Result · {run.status}</div>
        {run.results.map((result) => <div key={result.server_id} className="list-row"><span>{result.output.success ? "✓" : "✕"} {result.server_name}</span><code>{result.output.success ? result.output.stdout.trim().slice(0, 180) : result.output.stderr.trim().slice(0, 180)}</code></div>)}
      </> : null}
    </div>
  );
}

function TunnelOps({ reportError }: { reportError: (reason: unknown) => void }) {
  const [tunnels, setTunnels] = useState<Tunnel[]>([]);
  const [policies, setPolicies] = useState<TunnelPolicy[]>([]);
  const refresh = useCallback(async () => {
    try {
      await operatorApi.tunnelsReconcile();
      const [nextTunnels, nextPolicies] = await Promise.all([api.tunnelsList(), operatorApi.tunnelPoliciesList()]);
      setTunnels(nextTunnels); setPolicies(nextPolicies);
    } catch (reason) { reportError(reason); }
  }, [reportError]);
  useEffect(() => { void refresh(); }, [refresh]);

  async function update(tunnel: Tunnel, patch: Partial<TunnelPolicy>) {
    const current = policies.find((policy) => policy.tunnel_id === tunnel.id);
    try {
      await operatorApi.tunnelPolicySave({
        tunnel_id: tunnel.id,
        autostart: patch.autostart ?? current?.autostart ?? false,
        auto_reconnect: patch.auto_reconnect ?? current?.auto_reconnect ?? false,
        health_interval_secs: patch.health_interval_secs ?? current?.health_interval_secs ?? 15,
      });
      await refresh();
    } catch (reason) { reportError(reason); }
  }

  return <div>
    <div className="warn-banner ok">Explicit Stop always disables the live process. Auto-reconnect only repairs unexpected failures; autostart applies after app startup.</div>
    {tunnels.length === 0 ? <div className="panel-hint">No persisted tunnels yet.</div> : tunnels.map((tunnel) => {
      const policy = policies.find((candidate) => candidate.tunnel_id === tunnel.id);
      return <div key={tunnel.id} className="list-row">
        <span className="mono">{tunnel.type} · :{tunnel.local_port} · {tunnel.status}</span>
        <label><input type="checkbox" checked={policy?.autostart ?? false} onChange={(e) => void update(tunnel, { autostart: e.target.checked })} /> autostart</label>
        <label><input type="checkbox" checked={policy?.auto_reconnect ?? false} onChange={(e) => void update(tunnel, { auto_reconnect: e.target.checked })} /> reconnect</label>
      </div>;
    })}
    <button onClick={() => void refresh()}>Reconcile now</button>
  </div>;
}

function BackupOps({ reportError }: { reportError: (reason: unknown) => void }) {
  const [path, setPath] = useState("");
  const [passphrase, setPassphrase] = useState("");
  const [report, setReport] = useState<BackupRestoreReport | null>(null);
  async function exportBackup() {
    try { await operatorApi.backupExport(path, passphrase); setReport(null); } catch (reason) { reportError(reason); }
  }
  async function importBackup() {
    if (!window.confirm("Restore this encrypted workspace backup? Existing profiles with matching IDs will be updated. Password secrets are never imported.")) return;
    try { setReport(await operatorApi.backupImport(path, passphrase)); } catch (reason) { reportError(reason); }
  }
  return <div>
    <div className="warn-banner ok">AES-256-CBC + PBKDF2 (250k iterations). Keyring passwords are never exported and the backup passphrase is never placed in process argv.</div>
    <div><label>Backup path</label><input value={path} onChange={(e) => setPath(e.target.value)} placeholder="/home/me/remoteopsx-backup.rox" /></div>
    <div><label>Passphrase (10+ characters)</label><input type="password" value={passphrase} onChange={(e) => setPassphrase(e.target.value)} /></div>
    <div className="flex"><button disabled={!path || passphrase.length < 10} onClick={() => void exportBackup()}>Export encrypted backup</button><button disabled={!path || passphrase.length < 10} onClick={() => void importBackup()}>Import backup</button></div>
    {report ? <div className="warn-banner">Restored {report.servers} servers, {report.runbooks} runbooks, {report.snippets} snippets and {report.alert_rules} alert rules. {report.password_reentry_server_ids.length} password profile(s) require credential re-entry.</div> : null}
  </div>;
}
