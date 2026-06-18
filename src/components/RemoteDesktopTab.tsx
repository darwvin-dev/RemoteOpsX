import { useState } from "react";
import * as api from "../api";
import { useStore } from "../store";
import type { Server } from "../types";

/** RDP / VNC launcher tab. MVP launches the external system client
 *  (xfreerdp / vncviewer) while the session record stays in RemoteOpsX.
 *  The adapter is structured so an embedded canvas can replace this later. */
export function RemoteDesktopTab({ kind, server }: { kind: "rdp" | "vnc"; server: Server }) {
  const pushAlert = useStore((s) => s.pushAlert);
  const [fullscreen, setFullscreen] = useState(false);
  const [resolution, setResolution] = useState("1600x900");
  const [launched, setLaunched] = useState(false);

  const defaultPort = kind === "rdp" ? 3389 : 5900;
  const port = server.port === 22 ? defaultPort : server.port;

  async function launch() {
    try {
      if (kind === "rdp") {
        await api.rdpLaunch(server.id, { fullscreen, resolution });
      } else {
        await api.vncLaunch(server.id, { fullscreen });
      }
      setLaunched(true);
      pushAlert("info", `${kind.toUpperCase()} session launched for ${server.name}`, server.id);
    } catch (err) {
      pushAlert("error", `${kind.toUpperCase()} launch failed: ${err}`, server.id);
    }
  }

  return (
    <div className="runbook-runner">
      <div className="rb-header">
        <div>
          <h2>{kind.toUpperCase()} · {server.name}</h2>
          <p className="mono">{server.host}:{port} · {server.username}</p>
        </div>
      </div>

      <div className="metric-card" style={{ maxWidth: 460 }}>
        <div className="col">
          <label className="check">
            <input type="checkbox" checked={fullscreen} onChange={(e) => setFullscreen(e.target.checked)} />
            Fullscreen
          </label>
          {kind === "rdp" && (
            <div>
              <label>Resolution</label>
              <select value={resolution} onChange={(e) => setResolution(e.target.value)}>
                <option>1280x720</option>
                <option>1600x900</option>
                <option>1920x1080</option>
                <option>2560x1440</option>
              </select>
            </div>
          )}
          <button className="primary" onClick={() => void launch()}>
            Launch {kind === "rdp" ? "xfreerdp" : "VNC viewer"}
          </button>
          {launched && <div className="warn-banner ok">✓ External {kind.toUpperCase()} window launched.</div>}
        </div>
      </div>

      <p className="muted" style={{ marginTop: 14, maxWidth: 520 }}>
        MVP launches the system {kind === "rdp" ? "FreeRDP" : "VNC"} client as a separate window. Ensure{" "}
        <span className="mono">{kind === "rdp" ? "xfreerdp / xfreerdp3" : "a VNC viewer (tigervnc, remmina…)"}</span>{" "}
        is installed. Embedded {kind.toUpperCase()} rendering is on the roadmap.
      </p>
    </div>
  );
}
