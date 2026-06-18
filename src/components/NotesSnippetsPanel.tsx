import * as api from "../api";
import { useStore } from "../store";
import type { Server } from "../types";

/** Handy command snippets the operator can run against the focused server with
 *  one click (output lands in the bottom panel). */
const SNIPPETS: { label: string; cmd: string }[] = [
  { label: "Who is logged in", cmd: "w" },
  { label: "Last logins", cmd: "last -n 15" },
  { label: "Open files (limit)", cmd: "lsof | head -50" },
  { label: "Largest dirs in /var", cmd: "du -xhd1 /var 2>/dev/null | sort -rh | head" },
  { label: "Recent kernel msgs", cmd: "dmesg | tail -40" },
  { label: "TCP connections", cmd: "ss -tan | head -40" },
  { label: "Cron jobs (root)", cmd: "crontab -l 2>/dev/null; ls -la /etc/cron.d" },
  { label: "OOM kills", cmd: "journalctl -k | grep -i 'killed process' | tail -20" },
];

export function NotesSnippetsPanel({ server, showSnippets }: { server: Server; showSnippets: boolean }) {
  const pushOutput = useStore((s) => s.pushOutput);
  const pushAlert = useStore((s) => s.pushAlert);
  const setBottomPanel = useStore((s) => s.setBottomPanel);

  async function run(cmd: string) {
    try {
      const out = await api.runRemote(server.id, cmd);
      pushOutput(`$ ${cmd}\n${out.stdout || out.stderr}`);
      setBottomPanel("output");
    } catch (err) {
      pushAlert("error", `${cmd}: ${err}`);
    }
  }

  if (!showSnippets) {
    return (
      <div>
        <div className="section-title">Notes · {server.name}</div>
        {server.notes ? (
          <div style={{ whiteSpace: "pre-wrap", lineHeight: 1.5 }}>{server.notes}</div>
        ) : (
          <div className="panel-hint">No notes. Edit the server profile to add some.</div>
        )}
      </div>
    );
  }

  return (
    <div>
      <div className="section-title">Quick snippets</div>
      <div className="col">
        {SNIPPETS.map((s) => (
          <button key={s.cmd} style={{ textAlign: "left" }} onClick={() => void run(s.cmd)}>
            <div style={{ fontWeight: 600 }}>{s.label}</div>
            <div className="mono muted" style={{ fontSize: 10 }}>{s.cmd}</div>
          </button>
        ))}
      </div>
    </div>
  );
}
