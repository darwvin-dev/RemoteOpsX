import { useStore } from "../store";
import { TerminalTab } from "./TerminalTab";
import { RemoteDesktopTab } from "./RemoteDesktopTab";
import { RunbookRunner } from "./RunbookRunner";
import { SftpPanel } from "./SftpPanel";
import { LogsPanel } from "./LogsPanel";
import { OperationsDashboard } from "./OperationsDashboard";

/** Renders all open tabs but only displays the active one. Terminal tabs stay
 * mounted so SSH sessions and scrollback survive tab switches. */
export function TabContent({
  onNewServer,
  onOpenRunbooks,
  onOpenTunnels,
  onOpenOperations,
}: {
  onNewServer: () => void;
  onOpenRunbooks: () => void;
  onOpenTunnels: () => void;
  onOpenOperations: () => void;
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
        <OperationsDashboard
          servers={servers}
          focusedServer={focusedServer}
          onFocusServer={setFocusedServer}
          onOpenTab={openTab}
          onNewServer={onNewServer}
          onOpenRunbooks={onOpenRunbooks}
          onOpenTunnels={onOpenTunnels}
          onOpenOperations={onOpenOperations}
        />
      </div>
    );
  }

  return (
    <div className="tab-content">
      {tabs.map((tab) => {
        const active = tab.id === activeTabId;
        const server = servers.find((candidate) => candidate.id === tab.serverId);
        if (!server) return null;
        return (
          <div key={tab.id} className="tab-pane" style={{ display: active ? "flex" : "none" }}>
            {tab.kind === "ssh" && <TerminalTab tabId={tab.id} server={server} active={active} />}
            {(tab.kind === "rdp" || tab.kind === "vnc") && (
              <RemoteDesktopTab kind={tab.kind} server={server} />
            )}
            {tab.kind === "runbook" && tab.runbookId && (
              <RunbookRunner runbookId={tab.runbookId} server={server} />
            )}
            {tab.kind === "sftp" && <SftpPanel server={server} active={active} protocol="sftp" />}
            {tab.kind === "ftp" && <SftpPanel server={server} active={active} protocol="ftp" />}
            {tab.kind === "logs" && <LogsPanel server={server} />}
          </div>
        );
      })}
    </div>
  );
}
