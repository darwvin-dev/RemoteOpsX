import { useEffect, useMemo, useRef, useState } from "react";
import { useStore } from "../store";
import type { RightPanelView, Server, TabKind } from "../types";

interface Props {
  open: boolean;
  onClose: () => void;
  onNewServer: () => void;
  onOpenRunbooks: () => void;
  onOpenTunnels: () => void;
  onOpenSettings: () => void;
}

interface PaletteAction {
  id: string;
  title: string;
  eyebrow: string;
  detail?: string;
  keywords: string;
  run: () => void;
}

const SERVER_ACTIONS: { kind: TabKind; label: string; requiresProtocol?: "ssh" | "sftp" | "ftp" | "rdp" | "vnc" }[] = [
  { kind: "ssh", label: "Open SSH", requiresProtocol: "ssh" },
  { kind: "sftp", label: "Open SFTP", requiresProtocol: "sftp" },
  { kind: "ftp", label: "Open FTP", requiresProtocol: "ftp" },
  { kind: "logs", label: "Open Logs" },
  { kind: "rdp", label: "Launch RDP", requiresProtocol: "rdp" },
  { kind: "vnc", label: "Launch VNC", requiresProtocol: "vnc" },
];

/** Keyboard-first command palette for jumping across servers and common actions. */
export function CommandPalette({ open, onClose, onNewServer, onOpenRunbooks, onOpenTunnels, onOpenSettings }: Props) {
  const servers = useStore((s) => s.servers);
  const tabs = useStore((s) => s.tabs);
  const activeTabId = useStore((s) => s.activeTabId);
  const openTab = useStore((s) => s.openTab);
  const setActiveTab = useStore((s) => s.setActiveTab);
  const setFocusedServer = useStore((s) => s.setFocusedServer);
  const setRightPanel = useStore((s) => s.setRightPanel);
  const setBottomPanel = useStore((s) => s.setBottomPanel);
  const toggleBottomPanel = useStore((s) => s.toggleBottomPanel);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const inputRef = useRef<HTMLInputElement | null>(null);

  const actions = useMemo<PaletteAction[]>(() => {
    const closeThen = (action: () => void) => () => {
      action();
      onClose();
    };

    const globalActions: PaletteAction[] = [
      {
        id: "settings",
        title: "Open application settings",
        eyebrow: "Application",
        detail: "Configure appearance, connections, retention and desktop integration",
        keywords: "settings preferences configuration theme ports",
        run: closeThen(onOpenSettings),
      },
      {
        id: "new-server",
        title: "Add server profile",
        eyebrow: "Workspace",
        detail: "Create a host profile with protocols, secrets, tags and notes",
        keywords: "add new server profile host",
        run: closeThen(onNewServer),
      },
      {
        id: "runbooks",
        title: "Open runbook launcher",
        eyebrow: "Automation",
        detail: "Pick a built-in runbook and target server",
        keywords: "runbook automation diagnose health",
        run: closeThen(onOpenRunbooks),
      },
      {
        id: "tunnels",
        title: "Manage SSH tunnels",
        eyebrow: "Networking",
        detail: "Create local, remote or dynamic SOCKS forwards",
        keywords: "ssh tunnels socks port forward",
        run: closeThen(onOpenTunnels),
      },
      {
        id: "alerts",
        title: "Show alerts",
        eyebrow: "Bottom panel",
        detail: "Open warnings and action notifications",
        keywords: "alerts warnings events bottom",
        run: closeThen(() => setBottomPanel("alerts")),
      },
      {
        id: "history",
        title: "Show runbook history",
        eyebrow: "Bottom panel",
        detail: "Review recent runbook executions",
        keywords: "history runbook runs bottom",
        run: closeThen(() => setBottomPanel("history")),
      },
      {
        id: "toggle-bottom",
        title: "Toggle bottom dock",
        eyebrow: "Layout",
        detail: "Collapse or expand output, history and alerts",
        keywords: "bottom output panel collapse expand",
        run: closeThen(() => toggleBottomPanel()),
      },
    ];

    const panelActions: PaletteAction[] = (["health", "services", "notes", "snippets"] as RightPanelView[]).map((view) => ({
      id: `panel-${view}`,
      title: `Focus ${view} panel`,
      eyebrow: "Right panel",
      detail: "Switch the operations side panel",
      keywords: `${view} right panel metrics services notes snippets`,
      run: closeThen(() => setRightPanel(view)),
    }));

    const tabActions = tabs.map((tab) => ({
      id: `tab-${tab.id}`,
      title: tab.title,
      eyebrow: tab.id === activeTabId ? "Active tab" : "Open tab",
      detail: `Switch to ${tab.kind.toUpperCase()} session`,
      keywords: `${tab.title} ${tab.kind} tab session`,
      run: closeThen(() => setActiveTab(tab.id)),
    }));

    const serverActions = servers.flatMap((server) => [
      {
        id: `focus-${server.id}`,
        title: `Focus ${server.name}`,
        eyebrow: server.environment,
        detail: `${server.username}@${server.host}:${server.port}`,
        keywords: serverKeywords(server),
        run: closeThen(() => setFocusedServer(server.id)),
      },
      ...SERVER_ACTIONS.filter((action) => !action.requiresProtocol || server.protocols.includes(action.requiresProtocol)).map((action) => ({
        id: `${action.kind}-${server.id}`,
        title: `${action.label} · ${server.name}`,
        eyebrow: server.group_name || "Server action",
        detail: `${server.username}@${server.host}:${server.port}`,
        keywords: `${serverKeywords(server)} ${action.kind} ${action.label}`,
        run: closeThen(() => openTab(action.kind, server)),
      })),
    ]);

    return [...globalActions, ...panelActions, ...tabActions, ...serverActions];
  }, [
    activeTabId,
    onClose,
    onNewServer,
    onOpenRunbooks,
    onOpenTunnels,
    onOpenSettings,
    openTab,
    servers,
    setActiveTab,
    setBottomPanel,
    setFocusedServer,
    setRightPanel,
    tabs,
    toggleBottomPanel,
  ]);

  const filtered = useMemo(() => {
    const terms = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
    if (terms.length === 0) return actions.slice(0, 18);
    return actions
      .map((action) => ({
        action,
        score: terms.reduce((score, term) => {
          const haystack = `${action.title} ${action.eyebrow} ${action.detail ?? ""} ${action.keywords}`.toLowerCase();
          if (action.title.toLowerCase().includes(term)) return score + 4;
          if (haystack.includes(term)) return score + 1;
          return score - 20;
        }, 0),
      }))
      .filter((item) => item.score >= terms.length)
      .sort((a, b) => b.score - a.score || a.action.title.localeCompare(b.action.title))
      .slice(0, 18)
      .map((item) => item.action);
  }, [actions, query]);

  useEffect(() => {
    if (!open) return;
    setQuery("");
    setSelected(0);
    const id = window.setTimeout(() => inputRef.current?.focus(), 20);
    return () => window.clearTimeout(id);
  }, [open]);

  useEffect(() => {
    setSelected(0);
  }, [query]);

  if (!open) return null;

  function onKeyDown(event: React.KeyboardEvent<HTMLInputElement>) {
    if (event.key === "Escape") {
      event.preventDefault();
      onClose();
      return;
    }
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setSelected((current) => Math.min(filtered.length - 1, current + 1));
      return;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      setSelected((current) => Math.max(0, current - 1));
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      filtered[selected]?.run();
    }
  }

  return (
    <div className="palette-backdrop" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
      <div className="palette" role="dialog" aria-label="Command palette">
        <div className="palette-search">
          <span>⌕</span>
          <input
            ref={inputRef}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={onKeyDown}
            placeholder="Search servers, tabs, panels or actions"
          />
          <kbd>Esc</kbd>
        </div>
        <div className="palette-list">
          {filtered.length === 0 ? (
            <div className="palette-empty">No matching actions.</div>
          ) : (
            filtered.map((action, index) => (
              <button
                key={action.id}
                className={`palette-item${selected === index ? " active" : ""}`}
                onMouseEnter={() => setSelected(index)}
                onClick={action.run}
              >
                <span className="palette-icon">{iconFor(action)}</span>
                <span className="palette-copy">
                  <strong>{action.title}</strong>
                  {action.detail && <small>{action.detail}</small>}
                </span>
                <span className="palette-eyebrow">{action.eyebrow}</span>
              </button>
            ))
          )}
        </div>
      </div>
    </div>
  );
}

function serverKeywords(server: Server): string {
  return [
    server.name,
    server.host,
    server.username,
    server.environment,
    server.group_name,
    ...server.protocols,
    ...server.tags,
  ].filter(Boolean).join(" ");
}

function iconFor(action: PaletteAction): string {
  if (action.id.startsWith("ssh-")) return "▰";
  if (action.id.startsWith("sftp-")) return "⇅";
  if (action.id.startsWith("ftp-")) return "⇅";
  if (action.id.startsWith("rdp-") || action.id.startsWith("vnc-")) return "▣";
  if (action.id.startsWith("focus-")) return "◉";
  if (action.id.startsWith("panel-")) return "◧";
  if (action.id.startsWith("tab-")) return "▱";
  if (action.id === "runbooks") return "▶";
  if (action.id === "tunnels") return "⇄";
  return "⌁";
}
