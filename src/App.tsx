import { useEffect, useState } from "react";
import { useStore } from "./store";
import type { Server } from "./types";
import { ServerSidebar } from "./components/ServerSidebar";
import { ServerForm } from "./components/ServerForm";
import { TabBar } from "./components/TabBar";
import { TabContent } from "./components/TabContent";
import { RightPanel } from "./components/RightPanel";
import { BottomPanel } from "./components/BottomPanel";
import { RunbookLauncher } from "./components/RunbookLauncher";
import { TunnelManager } from "./components/TunnelManager";

export default function App() {
  const loadServers = useStore((s) => s.loadServers);
  const rightCollapsed = useStore((s) => s.tabs.length === 0 && s.focusedServerId === null);
  const [editing, setEditing] = useState<Server | null | undefined>(undefined); // undefined = closed
  const [showRunbooks, setShowRunbooks] = useState(false);
  const [showTunnels, setShowTunnels] = useState(false);

  useEffect(() => {
    void loadServers();
  }, [loadServers]);

  return (
    <div className={`app${rightCollapsed ? " right-collapsed" : ""}`}>
      <header className="topbar">
        <div className="brand">
          <span className="logo" />
          RemoteOpsX <small>· remote operations workspace</small>
        </div>
        <div className="spacer" />
        <button className="tiny" onClick={() => setShowRunbooks(true)}>▶ Runbooks</button>
        <button className="tiny" onClick={() => setShowTunnels(true)}>⇄ Tunnels</button>
        <div className="meta">Linux remote-ops client</div>
      </header>

      <ServerSidebar onNew={() => setEditing(null)} onEdit={(s) => setEditing(s)} />

      <main className="main">
        <TabBar />
        <TabContent />
      </main>

      {!rightCollapsed && <RightPanel />}

      <BottomPanel />

      {editing !== undefined && (
        <ServerForm server={editing} onClose={() => setEditing(undefined)} />
      )}
      {showRunbooks && <RunbookLauncher onClose={() => setShowRunbooks(false)} />}
      {showTunnels && <TunnelManager onClose={() => setShowTunnels(false)} />}
    </div>
  );
}
