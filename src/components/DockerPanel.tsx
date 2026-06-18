import { useEffect, useState } from "react";
import * as api from "../api";
import { useStore } from "../store";
import type { Server } from "../types";

interface Row {
  name: string;
  status: string;
  image: string;
  ports: string;
  cpu?: string;
  mem?: string;
}

/** Docker panel: container list with status/resource usage and lifecycle
 *  actions, plus `docker compose ps`. */
export function DockerPanel({ server }: { server: Server }) {
  const pushAlert = useStore((s) => s.pushAlert);
  const pushOutput = useStore((s) => s.pushOutput);
  const setBottomPanel = useStore((s) => s.setBottomPanel);
  const [rows, setRows] = useState<Row[]>([]);
  const [available, setAvailable] = useState(true);
  const [busy, setBusy] = useState(false);

  async function load() {
    setBusy(true);
    try {
      const [ps, stats] = await Promise.all([
        api.dockerAction(server.id, "ps"),
        api.dockerAction(server.id, "stats"),
      ]);
      if (!ps.success && ps.stderr.toLowerCase().includes("not found")) {
        setAvailable(false);
        return;
      }
      const statMap = new Map<string, { cpu: string; mem: string }>();
      stats.stdout.split("\n").filter(Boolean).forEach((l) => {
        const [name, cpu, mem] = l.split("|");
        if (name) statMap.set(name, { cpu, mem });
      });
      const parsed: Row[] = ps.stdout.split("\n").filter(Boolean).map((l) => {
        const [name, status, image, ports] = l.split("|");
        const st = statMap.get(name);
        return { name, status, image, ports: ports ?? "", cpu: st?.cpu, mem: st?.mem };
      });
      setRows(parsed);
      setAvailable(true);
    } catch (err) {
      pushAlert("error", `docker ps: ${err}`);
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => { void load(); }, [server.id]);

  async function action(act: "start" | "stop" | "restart" | "logs", name: string) {
    if ((act === "stop" || act === "restart") && !confirm(`docker ${act} ${name}?`)) return;
    try {
      const out = await api.dockerAction(server.id, act, name);
      if (act === "logs") {
        pushOutput(`$ docker logs ${name}\n${out.stdout || out.stderr}`);
        setBottomPanel("output");
      } else {
        pushAlert(out.success ? "info" : "error", `docker ${act} ${name} → exit ${out.exit_code}`);
        await load();
      }
    } catch (err) {
      pushAlert("error", `docker ${act} ${name}: ${err}`);
    }
  }

  if (!available) return <div className="panel-hint">Docker not detected on this host.</div>;

  return (
    <div>
      <div className="flex" style={{ justifyContent: "space-between" }}>
        <strong>Docker containers</strong>
        <button className="tiny" disabled={busy} onClick={() => void load()}>↻ Refresh</button>
      </div>

      {rows.length === 0 ? (
        <div className="panel-hint">No containers.</div>
      ) : (
        rows.map((r) => {
          const exited = r.status.toLowerCase().includes("exited");
          return (
            <div key={r.name} className="metric-card" style={{ marginTop: 8 }}>
              <div className="flex" style={{ justifyContent: "space-between" }}>
                <span className="mono" style={{ fontWeight: 600 }}>{r.name}</span>
                <span className={`status-badge ${exited ? "status-crit" : "status-ok"}`}>{r.status}</span>
              </div>
              <div className="mc-sub mono">{r.image}</div>
              {(r.cpu || r.mem) && <div className="mc-sub">CPU {r.cpu ?? "—"} · MEM {r.mem ?? "—"}</div>}
              {r.ports && <div className="mc-sub mono" style={{ fontSize: 10 }}>{r.ports}</div>}
              <div className="flex" style={{ marginTop: 6, flexWrap: "wrap" }}>
                <button className="tiny" onClick={() => void action("logs", r.name)}>Logs</button>
                {exited
                  ? <button className="tiny" onClick={() => void action("start", r.name)}>Start</button>
                  : <button className="tiny" onClick={() => void action("restart", r.name)}>Restart</button>}
                {!exited && <button className="tiny danger" onClick={() => void action("stop", r.name)}>Stop</button>}
              </div>
            </div>
          );
        })
      )}
    </div>
  );
}
