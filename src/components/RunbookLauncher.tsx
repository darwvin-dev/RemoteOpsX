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

  useEffect(() => {
    void api.runbooksList().then(setRunbooks).catch((err) => pushAlert("error", `runbooks: ${err}`));
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  function launch(rb: Runbook) {
    const server = servers.find((s) => s.id === serverId);
    if (!server) { pushAlert("warn", "Select a target server first."); return; }
    openTab("runbook", server, { runbookId: rb.id, title: `RB · ${rb.name}` });
    onClose();
  }

  return (
    <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="modal">
        <div className="modal-head">Runbooks<button className="ghost tiny" onClick={onClose}>✕</button></div>
        <div className="modal-body">
          <div>
            <label>Target server</label>
            <select value={serverId} onChange={(e) => setServerId(e.target.value)}>
              {servers.length === 0 && <option value="">— no servers —</option>}
              {servers.map((s) => <option key={s.id} value={s.id}>{s.name} ({s.host})</option>)}
            </select>
          </div>

          <div className="section-title">Available runbooks</div>
          <div className="col">
            {runbooks.map((rb) => (
              <div key={rb.id} className="metric-card">
                <div className="flex" style={{ justifyContent: "space-between" }}>
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
