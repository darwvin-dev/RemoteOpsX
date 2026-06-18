import { useEffect, useRef, useState } from "react";
import * as api from "../api";
import { useStore } from "../store";
import type { RunbookRun } from "../types";

/** Bottom dock: command output stream, runbook run history and the alert log. */
export function BottomPanel() {
  const open = useStore((s) => s.bottomPanelOpen);
  const view = useStore((s) => s.bottomPanel);
  const setView = useStore((s) => s.setBottomPanel);
  const toggle = useStore((s) => s.toggleBottomPanel);
  const outputLines = useStore((s) => s.outputLines);
  const clearOutput = useStore((s) => s.clearOutput);
  const alerts = useStore((s) => s.alerts);
  const clearAlerts = useStore((s) => s.clearAlerts);
  const servers = useStore((s) => s.servers);
  const [runs, setRuns] = useState<RunbookRun[]>([]);
  const outRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (view === "history" && open) {
      void api.runbookRunsList(50).then(setRuns).catch(() => {});
    }
  }, [view, open]);

  useEffect(() => {
    if (outRef.current) outRef.current.scrollTop = outRef.current.scrollHeight;
  }, [outputLines]);

  const serverName = (id: string) => servers.find((s) => s.id === id)?.name ?? id.slice(0, 8);

  return (
    <div className="bottom" style={{ maxHeight: open ? 240 : 32 }}>
      <div className="bottom-tabs">
        <button className={view === "output" ? "active" : ""} onClick={() => setView("output")}>Output</button>
        <button className={view === "history" ? "active" : ""} onClick={() => setView("history")}>Runbook history</button>
        <button className={view === "alerts" ? "active" : ""} onClick={() => setView("alerts")}>
          Alerts {alerts.length > 0 && `(${alerts.length})`}
        </button>
        <span style={{ flex: 1 }} />
        {view === "output" && <button className="tiny ghost" onClick={clearOutput}>Clear</button>}
        {view === "alerts" && <button className="tiny ghost" onClick={clearAlerts}>Clear</button>}
        <button className="tiny ghost" onClick={() => toggle()}>{open ? "▾" : "▸"}</button>
      </div>

      {open && view === "output" && (
        <div className="bottom-body log-output" ref={outRef}>
          {outputLines.length === 0 ? <span className="muted">Command output appears here.</span> : outputLines.join("\n\n")}
        </div>
      )}

      {open && view === "history" && (
        <div className="bottom-body">
          {runs.length === 0 ? (
            <span className="muted">No runbook runs yet.</span>
          ) : (
            runs.map((r) => {
              const ok = r.status === "success";
              return (
                <div key={r.id} className="alert-row">
                  <span className="at">{new Date(r.started_at).toLocaleString()}</span>
                  <span className={`status-badge ${ok ? "status-ok" : "status-crit"}`}>{r.status}</span>
                  <span>{serverName(r.server_id)} · {r.results.length} steps</span>
                </div>
              );
            })
          )}
        </div>
      )}

      {open && view === "alerts" && (
        <div className="bottom-body">
          {alerts.length === 0 ? (
            <span className="muted">No alerts.</span>
          ) : (
            alerts.map((a) => (
              <div key={a.id} className={`alert-row ${a.level}`}>
                <span className="at">{a.time}</span>
                <span className="am">{a.message}</span>
              </div>
            ))
          )}
        </div>
      )}
    </div>
  );
}
