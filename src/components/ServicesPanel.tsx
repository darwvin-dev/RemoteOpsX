import { useEffect, useState } from "react";
import * as api from "../api";
import { useStore } from "../store";
import type { Server } from "../types";

interface FailedUnit {
  unit: string;
  load: string;
  active: string;
  sub: string;
}

/** systemd services panel: list failed units, inspect, and (with confirmation)
 *  start/stop/restart. Every destructive action shows its exact command. */
export function ServicesPanel({ server }: { server: Server }) {
  const pushAlert = useStore((s) => s.pushAlert);
  const pushOutput = useStore((s) => s.pushOutput);
  const setBottomPanel = useStore((s) => s.setBottomPanel);
  const [failed, setFailed] = useState<FailedUnit[]>([]);
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState(false);
  const [confirm, setConfirm] = useState<{ action: string; unit: string; cmd: string } | null>(null);

  async function loadFailed() {
    setBusy(true);
    try {
      const out = await api.serviceAction(server.id, "list-failed", "");
      const units: FailedUnit[] = out.stdout
        .split("\n")
        .map((l) => l.trim())
        .filter(Boolean)
        .map((l) => {
          const [unit, load, active, sub] = l.split(/\s+/);
          return { unit, load, active, sub };
        })
        .filter((u) => u.unit && u.unit.includes("."));
      setFailed(units);
    } catch (err) {
      pushAlert("error", `list failed services: ${err}`);
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => { void loadFailed(); }, [server.id]);

  async function inspect(action: "status" | "logs", unit: string) {
    try {
      const out = await api.serviceAction(server.id, action, unit);
      pushOutput(`$ ${action} ${unit}\n${out.stdout || out.stderr}`);
      setBottomPanel("output");
    } catch (err) {
      pushAlert("error", `${action} ${unit}: ${err}`);
    }
  }

  function askAction(action: "start" | "stop" | "restart", unit: string) {
    const cmd = `sudo systemctl ${action} ${unit}`;
    setConfirm({ action, unit, cmd });
  }

  async function runAction() {
    if (!confirm) return;
    setBusy(true);
    try {
      const out = await api.serviceAction(server.id, confirm.action, confirm.unit);
      pushOutput(`$ ${confirm.cmd}\n${out.stdout || out.stderr || "(ok)"}`);
      pushAlert(out.success ? "info" : "error", `${confirm.action} ${confirm.unit} → exit ${out.exit_code}`);
      setBottomPanel("output");
      setConfirm(null);
      await loadFailed();
    } catch (err) {
      pushAlert("error", `${confirm.action} ${confirm.unit}: ${err}`);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div>
      <div className="flex" style={{ justifyContent: "space-between" }}>
        <strong>systemd services</strong>
        <button className="tiny" disabled={busy} onClick={() => void loadFailed()}>↻ Refresh</button>
      </div>

      <input style={{ margin: "8px 0" }} placeholder="Inspect a unit, e.g. nginx" value={query} onChange={(e) => setQuery(e.target.value)} />
      {query.trim() && (
        <div className="flex" style={{ marginBottom: 10, flexWrap: "wrap" }}>
          <button className="tiny" onClick={() => void inspect("status", query.trim())}>Status</button>
          <button className="tiny" onClick={() => void inspect("logs", query.trim())}>Logs (200)</button>
          <button className="tiny" onClick={() => askAction("restart", query.trim())}>Restart</button>
          <button className="tiny" onClick={() => askAction("start", query.trim())}>Start</button>
          <button className="tiny danger" onClick={() => askAction("stop", query.trim())}>Stop</button>
        </div>
      )}

      <div className="section-title">Failed units {failed.length > 0 && `(${failed.length})`}</div>
      {failed.length === 0 ? (
        <div className="warn-banner ok">✓ No failed units</div>
      ) : (
        failed.map((u) => (
          <div key={u.unit} className="metric-card" style={{ marginBottom: 6 }}>
            <div className="flex" style={{ justifyContent: "space-between" }}>
              <span className="mono" style={{ color: "var(--crit)" }}>● {u.unit}</span>
              <span className="status-badge status-crit">{u.active}/{u.sub}</span>
            </div>
            <div className="flex" style={{ marginTop: 6, flexWrap: "wrap" }}>
              <button className="tiny" onClick={() => void inspect("status", u.unit)}>Status</button>
              <button className="tiny" onClick={() => void inspect("logs", u.unit)}>Logs</button>
              <button className="tiny" onClick={() => askAction("restart", u.unit)}>Restart</button>
            </div>
          </div>
        ))
      )}

      {confirm && (
        <div className="confirm-box">
          Confirm <strong>{confirm.action}</strong> of <span className="mono">{confirm.unit}</span>:
          <div className="cmd-preview">{confirm.cmd}</div>
          <div className="flex" style={{ justifyContent: "flex-end" }}>
            <button className="tiny" onClick={() => setConfirm(null)}>Cancel</button>
            <button className="tiny primary" disabled={busy} onClick={() => void runAction()}>Run</button>
          </div>
        </div>
      )}
    </div>
  );
}
