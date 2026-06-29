import { useStore } from "../store";
import { TerminalTab } from "./TerminalTab";
import { RemoteDesktopTab } from "./RemoteDesktopTab";
import { RunbookRunner } from "./RunbookRunner";
import { SftpPanel } from "./SftpPanel";
import { LogsPanel } from "./LogsPanel";
import type { Server, TabKind } from "../types";

/** Renders all open tabs but only displays the active one. Terminal tabs stay
 *  mounted so SSH sessions and scrollback survive tab switches. */
export function TabContent({
  onNewServer,
  onOpenRunbooks,
  onOpenTunnels,
}: {
  onNewServer: () => void;
  onOpenRunbooks: () => void;
  onOpenTunnels: () => void;
}) {
  const tabs = useStore((s) => s.tabs);
  const activeTabId = useStore((s) => s.activeTabId);
  const servers = useStore((s) => s.servers);
  const focusedServerId = useStore((s) => s.focusedServerId);
  const openTab = useStore((s) => s.openTab);
  const setFocusedServer = useStore((s) => s.setFocusedServer);
  const focusedServer = servers.find((server) => server.id === focusedServerId) ?? servers[0] ?? null;

  if (tabs.length === 0) {
    return (
      <div className="tab-content">
        <StartDashboard
          servers={servers}
          focusedServer={focusedServer}
          onFocusServer={setFocusedServer}
          onOpenTab={openTab}
          onNewServer={onNewServer}
          onOpenRunbooks={onOpenRunbooks}
          onOpenTunnels={onOpenTunnels}
        />
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
            {t.kind === "sftp" && <SftpPanel server={server} active={active} protocol="sftp" />}
            {t.kind === "ftp" && <SftpPanel server={server} active={active} protocol="ftp" />}
            {t.kind === "logs" && <LogsPanel server={server} />}
          </div>
        );
      })}
    </div>
  );
}

function StartDashboard({
  servers,
  focusedServer,
  onFocusServer,
  onOpenTab,
  onNewServer,
  onOpenRunbooks,
  onOpenTunnels,
}: {
  servers: Server[];
  focusedServer: Server | null;
  onFocusServer: (id: string | null) => void;
  onOpenTab: (kind: TabKind, server: Server) => string;
  onNewServer: () => void;
  onOpenRunbooks: () => void;
  onOpenTunnels: () => void;
}) {
  const productionCount = servers.filter((server) => server.environment === "production").length;
  const protocolCount = new Set(servers.flatMap((server) => server.protocols)).size;
  const folderCount = new Set(servers.map((server) => server.group_name?.trim()).filter(Boolean)).size;

  return (
    <div className="start-dashboard">
      <section className="hero-card">
        <div className="hero-copy">
          <span className="eyebrow">Workspace</span>
          <h1>Remote operations, organized.</h1>
          <p>
            Connect to servers, inspect health, browse files, review logs and run controlled diagnostics.
          </p>
          <div className="hero-actions">
            <button className="primary" onClick={focusedServer ? () => onOpenTab("ssh", focusedServer) : onNewServer}>
              {focusedServer ? `SSH into ${focusedServer.name}` : "Add first server"}
            </button>
            <button onClick={onOpenRunbooks}>Run diagnostics</button>
            <button className="ghost" onClick={onOpenTunnels}>Tunnels</button>
          </div>
        </div>
      </section>

      <section className="dashboard-grid">
        <div className="insight-card">
          <span className="insight-value">{servers.length}</span>
          <span className="insight-label">Saved servers</span>
          <small>{productionCount} production targets</small>
        </div>
        <div className="insight-card">
          <span className="insight-value">{folderCount}</span>
          <span className="insight-label">Folders</span>
          <small>Group hosts by team, customer or environment</small>
        </div>
        <div className="insight-card">
          <span className="insight-value">{protocolCount}</span>
          <span className="insight-label">Protocols</span>
          <small>SSH, files, desktops and logs</small>
        </div>
        <div className="insight-card">
          <span className="insight-value">2–5s</span>
          <span className="insight-label">Health cadence</span>
          <small>Agentless metrics over SSH</small>
        </div>
      </section>

      <section className="quick-grid">
        <div className="quick-card primary-card">
          <div>
            <span className="eyebrow">Next best action</span>
            <h3>{focusedServer ? focusedServer.name : "Create your first profile"}</h3>
            <p>
              {focusedServer
                ? `${focusedServer.username}@${focusedServer.host}:${focusedServer.port}`
                : "Store host details, protocols, tags and keyring-backed secrets."}
            </p>
          </div>
          <div className="quick-actions">
            {focusedServer ? (
              <>
                {focusedServer.protocols.includes("ssh") && <button className="primary" onClick={() => onOpenTab("ssh", focusedServer)}>SSH</button>}
                {focusedServer.protocols.includes("sftp") && <button onClick={() => onOpenTab("sftp", focusedServer)}>SFTP</button>}
                {focusedServer.protocols.includes("ftp") && <button onClick={() => onOpenTab("ftp", focusedServer)}>FTP</button>}
                <button onClick={() => onOpenTab("logs", focusedServer)}>Logs</button>
              </>
            ) : (
              <button className="primary" onClick={onNewServer}>Add server</button>
            )}
          </div>
        </div>

        <div className="quick-card">
          <span className="eyebrow">Command palette</span>
          <h3>Jump to actions</h3>
          <p>Search servers, panels, tabs and runbooks from one entry point.</p>
          <div className="shortcut-row">
            <kbd>Ctrl</kbd><kbd>K</kbd>
          </div>
        </div>

        <div className="quick-card">
          <span className="eyebrow">Recent targets</span>
          {servers.length === 0 ? (
            <p>Add servers to build a grouped, searchable operations inventory.</p>
          ) : (
            <div className="mini-server-list">
              {servers.slice(0, 5).map((server) => (
                <button
                  key={server.id}
                  className="mini-server"
                  onClick={() => onFocusServer(server.id)}
                >
                  <span className={`env-dot env-${server.environment}`} />
                  <span>{server.name}</span>
                  <small>{server.host}</small>
                </button>
              ))}
            </div>
          )}
        </div>
      </section>
    </div>
  );
}
