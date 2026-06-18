import { useState } from "react";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import * as api from "../api";
import { useStore } from "../store";
import type { Server } from "../types";

type Mode = "file" | "journal";

/** Logs tab: tail a remote file or read journalctl, search, save locally, or
 *  build a one-shot diagnostic bundle. */
export function LogsPanel({ server }: { server: Server }) {
  const pushAlert = useStore((s) => s.pushAlert);
  const [mode, setMode] = useState<Mode>("journal");
  const [target, setTarget] = useState("");
  const [lines, setLines] = useState(200);
  const [filter, setFilter] = useState("");
  const [output, setOutput] = useState("");
  const [busy, setBusy] = useState(false);

  function buildCmd(): string {
    const grep = filter.trim() ? ` | grep -i ${shellQuote(filter.trim())}` : "";
    if (mode === "journal") {
      const unit = target.trim() ? `-u ${shellQuote(target.trim())} ` : "";
      return `journalctl ${unit}-n ${lines} --no-pager${grep}`;
    }
    const f = target.trim() || "/var/log/syslog";
    return `tail -n ${lines} ${shellQuote(f)}${grep}`;
  }

  async function run() {
    setBusy(true);
    try {
      const cmd = buildCmd();
      const out = await api.runRemote(server.id, cmd);
      setOutput(out.stdout || out.stderr || "(no output)");
    } catch (err) {
      pushAlert("error", `logs: ${err}`);
    } finally {
      setBusy(false);
    }
  }

  async function saveLocal(content: string, name: string) {
    const dest = await saveDialog({ defaultPath: name });
    if (!dest) return;
    try {
      await api.saveTextFile(dest, content);
      pushAlert("info", `Saved ${dest}`);
    } catch (err) {
      pushAlert("error", `save: ${err}`);
    }
  }

  // Collect a diagnostic bundle: system info + failed services + top procs +
  // disk + ports + (optional) selected unit logs.
  async function bundle() {
    setBusy(true);
    try {
      const unit = target.trim();
      const cmds: [string, string][] = [
        ["SYSTEM INFO", "hostnamectl; uname -a; cat /etc/os-release"],
        ["UPTIME / LOAD", "uptime"],
        ["MEMORY", "free -m"],
        ["DISK", "df -h"],
        ["FAILED SERVICES", "systemctl --failed --no-pager"],
        ["TOP PROCESSES", "ps -eo pid,comm,%cpu,%mem --sort=-%cpu | head -15"],
        ["LISTENING PORTS", "ss -tulpen | head -60"],
      ];
      if (unit) cmds.push([`LOGS: ${unit}`, `journalctl -u ${shellQuote(unit)} -n 200 --no-pager`]);

      const parts: string[] = [`# RemoteOpsX diagnostic bundle — ${server.name} (${server.host})`, `# ${new Date().toISOString()}`, ""];
      for (const [title, cmd] of cmds) {
        const out = await api.runRemote(server.id, cmd);
        parts.push(`===== ${title} =====`, `$ ${cmd}`, out.stdout || out.stderr || "(empty)", "");
      }
      const text = parts.join("\n");
      setOutput(text);
      await saveLocal(text, `diag-${server.name}-${Date.now()}.txt`);
    } catch (err) {
      pushAlert("error", `bundle: ${err}`);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="sftp">
      <div className="toolbar">
        <select style={{ width: "auto" }} value={mode} onChange={(e) => setMode(e.target.value as Mode)}>
          <option value="journal">journalctl</option>
          <option value="file">tail file</option>
        </select>
        <input
          style={{ flex: 2 }}
          placeholder={mode === "journal" ? "unit (optional), e.g. nginx" : "/var/log/syslog"}
          value={target}
          onChange={(e) => setTarget(e.target.value)}
        />
        <input style={{ width: 80 }} type="number" value={lines} onChange={(e) => setLines(Number(e.target.value))} title="lines" />
        <input style={{ flex: 1 }} placeholder="filter (grep -i)" value={filter} onChange={(e) => setFilter(e.target.value)} />
        <button className="primary tiny" disabled={busy} onClick={() => void run()}>Fetch</button>
        <button className="tiny" disabled={busy} onClick={() => void bundle()}>Diagnostic bundle</button>
        <button className="tiny" disabled={!output} onClick={() => void saveLocal(output, `logs-${server.name}.txt`)}>Save</button>
      </div>
      <div className="bottom-body log-output" style={{ flex: 1 }}>
        {busy ? "Fetching…" : output || "Choose a source and press Fetch."}
      </div>
    </div>
  );
}

function shellQuote(s: string): string {
  return `'${s.replace(/'/g, "'\\''")}'`;
}
