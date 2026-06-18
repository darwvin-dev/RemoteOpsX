import { useStore } from "../store";
import { TerminalTab } from "./TerminalTab";
import { RemoteDesktopTab } from "./RemoteDesktopTab";
import { RunbookRunner } from "./RunbookRunner";
import { SftpPanel } from "./SftpPanel";
import { LogsPanel } from "./LogsPanel";

/** Renders all open tabs but only displays the active one. Terminal tabs stay
 *  mounted so SSH sessions and scrollback survive tab switches. */
export function TabContent() {
  const tabs = useStore((s) => s.tabs);
  const activeTabId = useStore((s) => s.activeTabId);
  const servers = useStore((s) => s.servers);

  if (tabs.length === 0) {
    return (
      <div className="tab-content">
        <div className="empty-state">
          <h2>Welcome to RemoteOpsX</h2>
          <p className="muted" style={{ maxWidth: 460 }}>
            A unified Linux remote-operations workspace — not just a terminal. Add a server, open an
            SSH session and watch live agentless health on the right. Run a built-in runbook to
            diagnose a host in one click.
          </p>
          <p className="muted">
            Pick a server on the left, then choose <span className="kbd">SSH</span>{" "}
            <span className="kbd">SFTP</span> <span className="kbd">RDP</span>{" "}
            <span className="kbd">VNC</span> or <span className="kbd">Logs</span>.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="tab-content">
      {tabs.map((t) => {
        const active = t.id === activeTabId;
        const server = servers.find((s) => s.id === t.serverId);
        if (!server) return null;
        return (
          <div key={t.id} className="tab-pane" style={{ display: active ? "flex" : "none" }}>
            {t.kind === "ssh" && <TerminalTab tabId={t.id} server={server} active={active} />}
            {(t.kind === "rdp" || t.kind === "vnc") && (
              <RemoteDesktopTab kind={t.kind} server={server} />
            )}
            {t.kind === "runbook" && t.runbookId && (
              <RunbookRunner runbookId={t.runbookId} server={server} />
            )}
            {t.kind === "sftp" && <SftpPanel server={server} active={active} />}
            {t.kind === "logs" && <LogsPanel server={server} />}
          </div>
        );
      })}
    </div>
  );
}
