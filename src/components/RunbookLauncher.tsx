import { useEffect, useState } from "react";
import * as api from "../api";
import { useStore } from "../store";
import type { Runbook } from "../types";

/** Modal to pick a runbook + target server and open it in a runbook tab. */
export function RunbookLauncher({ onClose }: { onClose: () => void }) {
  const servers = useStore((s) => s.servers);
  const focused = useStore((s) => s.focusedServerId);
  const openTab = useStore((s) => s.openTab);
  const pushAlert = useStore((s) => s.pushAlert);
  const [runbooks, setRunbooks] = useState<Runbook[]>([]);
  const [serverId, setServerId] = useState(focused ?? servers[0]?.id ?? "");
  const [query, setQuery] = useState("");

  useEffect(() => {
    void api.runbooksList().then(setRunbooks).catch((err) => pushAlert("error", `runbooks: ${err}`));
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  function launch(rb: Runbook) {
    const server = servers.find((s) => s.id === serverId);
    if (!server) { pushAlert("warn", "Select a target server first."); return; }
    openTab("runbook", server, { runbookId: rb.id, title: `RB · ${rb.name}` });
    onClose();
  }

  const filtered = runbooks.filter((runbook) => {
    const needle = query.trim().toLowerCase();
    if (!needle) return true;
    return `${runbook.name} ${runbook.description}`.toLowerCase().includes(needle);
  });

  return (
    <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="modal wide">
        <div className="modal-head">
          <div>
            <span className="eyebrow">Automation library</span>
            <strong>Runbooks</strong>
          </div>
          <button className="ghost tiny" onClick={onClose}>✕</button>
        </div>
        <div className="modal-body">
          <div className="launcher-grid">
            <div>
              <label>Target server</label>
              <select value={serverId} onChange={(e) => setServerId(e.target.value)}>
                {servers.length === 0 && <option value="">— no servers —</option>}
                {servers.map((s) => <option key={s.id} value={s.id}>{s.name} ({s.host})</option>)}
              </select>
            </div>
            <div>
              <label>Search runbooks</label>
              <input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="health, disk, docker, voip…" />
            </div>
          </div>

          <div className="section-title">Available runbooks {filtered.length > 0 && `(${filtered.length})`}</div>
          <div className="runbook-library">
            {filtered.length === 0 ? (
              <div className="panel-hint">No runbooks match your search.</div>
            ) : filtered.map((rb) => (
              <div key={rb.id} className="runbook-card">
                <div className="runbook-card-head">
                  <div>
                    <div style={{ fontWeight: 600 }}>
                      {rb.name} {rb.builtin && <span className="pill">built-in</span>}
                    </div>
                    <div className="mc-sub">{rb.description}</div>
                  </div>
                  <button className="primary tiny" disabled={!serverId} onClick={() => launch(rb)}>Run</button>
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
