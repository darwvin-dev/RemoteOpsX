import { useEffect, useState } from "react";
import * as api from "../api";
import { useStore } from "../store";
import type { Tunnel } from "../types";

/** SSH tunnel manager modal: create local (-L), remote (-R) or dynamic SOCKS
 *  (-D) forwards, list active tunnels and stop them. */
export function TunnelManager({ onClose }: { onClose: () => void }) {
  const servers = useStore((s) => s.servers);
  const pushAlert = useStore((s) => s.pushAlert);
  const focused = useStore((s) => s.focusedServerId);
  const [tunnels, setTunnels] = useState<Tunnel[]>([]);

  const [serverId, setServerId] = useState(focused ?? servers[0]?.id ?? "");
  const [type, setType] = useState<"local" | "remote" | "dynamic">("local");
  const [localPort, setLocalPort] = useState(8080);
  const [remoteHost, setRemoteHost] = useState("127.0.0.1");
  const [remotePort, setRemotePort] = useState(80);

  async function refresh() {
    try { setTunnels(await api.tunnelsList()); } catch (err) { pushAlert("error", `tunnels: ${err}`); }
  }
  useEffect(() => { void refresh(); }, []);

  async function start() {
    if (!serverId) return;
    try {
      await api.tunnelStart({
        server_id: serverId,
        type,
        local_host: "127.0.0.1",
        local_port: Number(localPort),
        remote_host: type === "dynamic" ? null : remoteHost,
        remote_port: type === "dynamic" ? null : Number(remotePort),
      });
      pushAlert("info", `Tunnel started (${type} :${localPort})`);
      await refresh();
    } catch (err) {
      pushAlert("error", `start tunnel: ${err}`);
    }
  }

  async function stop(id: string) {
    try { await api.tunnelStop(id); await refresh(); } catch (err) { pushAlert("error", `stop: ${err}`); }
  }

  const serverName = (id: string) => servers.find((s) => s.id === id)?.name ?? id.slice(0, 8);

  return (
    <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="modal">
        <div className="modal-head">SSH Tunnels<button className="ghost tiny" onClick={onClose}>✕</button></div>
        <div className="modal-body">
          <div className="form-row three">
            <div>
              <label>Server</label>
              <select value={serverId} onChange={(e) => setServerId(e.target.value)}>
                {servers.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}
              </select>
            </div>
            <div>
              <label>Type</label>
              <select value={type} onChange={(e) => setType(e.target.value as typeof type)}>
                <option value="local">Local (-L)</option>
                <option value="remote">Remote (-R)</option>
                <option value="dynamic">Dynamic SOCKS (-D)</option>
              </select>
            </div>
            <div><label>Local port</label><input type="number" value={localPort} onChange={(e) => setLocalPort(Number(e.target.value))} /></div>
          </div>

          {type !== "dynamic" && (
            <div className="form-row">
              <div><label>Remote host</label><input value={remoteHost} onChange={(e) => setRemoteHost(e.target.value)} /></div>
              <div><label>Remote port</label><input type="number" value={remotePort} onChange={(e) => setRemotePort(Number(e.target.value))} /></div>
            </div>
          )}

          <button className="primary" onClick={() => void start()}>Start tunnel</button>

          <div className="section-title">Tunnels</div>
          {tunnels.length === 0 ? (
            <div className="panel-hint">No tunnels yet.</div>
          ) : (
            <table className="data">
              <thead><tr><th>Server</th><th>Type</th><th>Forward</th><th>Status</th><th /></tr></thead>
              <tbody>
                {tunnels.map((t) => (
                  <tr key={t.id}>
                    <td>{serverName(t.server_id)}</td>
                    <td>{t.type}</td>
                    <td className="mono">
                      {t.type === "dynamic"
                        ? `socks5 :${t.local_port}`
                        : `:${t.local_port} → ${t.remote_host}:${t.remote_port}`}
                    </td>
                    <td><span className={`status-badge ${t.status === "active" ? "status-ok" : "status-warn"}`}>{t.status}</span></td>
                    <td>{t.status === "active" && <button className="tiny danger" onClick={() => void stop(t.id)}>Stop</button>}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </div>
    </div>
  );
}
