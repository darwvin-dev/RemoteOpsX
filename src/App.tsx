import { useEffect, useLayoutEffect, useState } from "react";
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
import { CommandPalette } from "./components/CommandPalette";
import { ToastStack } from "./components/ToastStack";
import { SettingsModal } from "./components/SettingsModal";
import { useSettingsStore } from "./settingsStore";

export default function App() {
  const loadServers = useStore((s) => s.loadServers);
  const servers = useStore((s) => s.servers);
  const tabs = useStore((s) => s.tabs);
  const alerts = useStore((s) => s.alerts);
  const setBottomPanel = useStore((s) => s.setBottomPanel);
  const rightCollapsed = useStore((s) => s.tabs.length === 0 && s.focusedServerId === null);
  const [editing, setEditing] = useState<Server | null | undefined>(undefined); // undefined = closed
  const [initialFolder, setInitialFolder] = useState<string | undefined>(undefined);
  const [showRunbooks, setShowRunbooks] = useState(false);
  const [showTunnels, setShowTunnels] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const loadSettings = useSettingsStore((state) => state.load);
  const theme = useSettingsStore((state) => state.settings.theme);
  const settingsInitialized = useSettingsStore((state) => state.initialized);

  function openNewServer(folder?: string) {
    setInitialFolder(folder);
    setEditing(null);
  }

  function openEditServer(server: Server) {
    setInitialFolder(undefined);
    setEditing(server);
  }

  useEffect(() => {
    void loadServers();
    void loadSettings().catch(() => undefined);
  }, [loadServers, loadSettings]);

  useLayoutEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const applyTheme = () => {
      document.documentElement.dataset.theme = theme === "system" ? (media.matches ? "dark" : "light") : theme;
      if (settingsInitialized) document.documentElement.dataset.settingsReady = "true";
      else delete document.documentElement.dataset.settingsReady;
    };
    applyTheme();
    if (theme !== "system") return;
    media.addEventListener("change", applyTheme);
    return () => media.removeEventListener("change", applyTheme);
  }, [settingsInitialized, theme]);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setPaletteOpen(true);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  return (
    <div className={`app${rightCollapsed ? " right-collapsed" : ""}${settingsInitialized ? "" : " settings-pending"}`}>
      <header className="topbar">
        <div className="brand">
          <span className="logo" />
          <span>
            RemoteOpsX
            <small>remote operations cockpit</small>
          </span>
        </div>
        <button className="command-trigger" onClick={() => setPaletteOpen(true)}>
          <span>Search servers, actions, runbooks…</span>
          <kbd>Ctrl K</kbd>
        </button>
        <div className="spacer" />
        <div className="top-stats" aria-label="Workspace status">
          <button className="status-chip" onClick={() => setPaletteOpen(true)}>{servers.length} servers</button>
          <span className="status-chip">{tabs.length} tabs</span>
          <button className="status-chip alert-chip" onClick={() => setBottomPanel("alerts")}>
            {alerts.length} alerts
          </button>
        </div>
        <button className="tiny" onClick={() => setShowRunbooks(true)}>▶ Runbooks</button>
        <button className="tiny" onClick={() => setShowTunnels(true)}>⇄ Tunnels</button>
        <button className="tiny" onClick={() => setSettingsOpen(true)}>⚙ Settings</button>
      </header>

      <ServerSidebar onNew={openNewServer} onEdit={openEditServer} />

      <main className="main">
        <TabBar />
        <TabContent
          onNewServer={openNewServer}
          onOpenRunbooks={() => setShowRunbooks(true)}
          onOpenTunnels={() => setShowTunnels(true)}
        />
      </main>

      {!rightCollapsed && <RightPanel />}

      <BottomPanel />
      <ToastStack />

      <CommandPalette
        open={paletteOpen}
        onClose={() => setPaletteOpen(false)}
        onNewServer={() => openNewServer()}
        onOpenRunbooks={() => setShowRunbooks(true)}
        onOpenTunnels={() => setShowTunnels(true)}
        onOpenSettings={() => setSettingsOpen(true)}
      />

      {editing !== undefined && (
        <ServerForm
          server={editing}
          initialFolder={initialFolder}
          onClose={() => {
            setEditing(undefined);
            setInitialFolder(undefined);
          }}
        />
      )}
      {showRunbooks && <RunbookLauncher onClose={() => setShowRunbooks(false)} />}
      {showTunnels && <TunnelManager onClose={() => setShowTunnels(false)} />}
      {settingsOpen && <SettingsModal onClose={() => setSettingsOpen(false)} />}
    </div>
  );
}
