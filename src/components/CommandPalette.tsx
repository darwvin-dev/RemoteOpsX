import { useEffect, useMemo, useRef, useState } from "react";
import * as api from "../api";
import { useStore } from "../store";
import type {
  CommandSnippet,
  RightPanelView,
  Runbook,
  Server,
  TabKind,
} from "../types";

interface Props {
  open: boolean;
  onClose: () => void;
  onNewServer: () => void;
  onOpenRunbooks: () => void;
  onOpenRunbookStudio: () => void;
  onOpenTunnels: () => void;
  onOpenOperations: () => void;
  onOpenSettings: () => void;
}

interface PaletteAction {
  id: string;
  title: string;
  eyebrow: string;
  detail?: string;
  keywords: string;
  run: () => void | Promise<void>;
}

const SERVER_ACTIONS: {
  kind: TabKind;
  label: string;
  requiresProtocol?: "ssh" | "sftp" | "ftp" | "rdp" | "vnc";
}[] = [
  { kind: "ssh", label: "Open SSH", requiresProtocol: "ssh" },
  { kind: "sftp", label: "Open SFTP", requiresProtocol: "sftp" },
  { kind: "ftp", label: "Open FTP", requiresProtocol: "ftp" },
  { kind: "logs", label: "Open Logs" },
  { kind: "rdp", label: "Launch RDP", requiresProtocol: "rdp" },
  { kind: "vnc", label: "Launch VNC", requiresProtocol: "vnc" },
];

/** Keyboard-first universal operator launcher. It indexes application actions,
 * servers, protocol actions, panels, runbooks, snippets and open tabs. */
export function CommandPalette({
  open,
  onClose,
  onNewServer,
  onOpenRunbooks,
  onOpenRunbookStudio,
  onOpenTunnels,
  onOpenOperations,
  onOpenSettings,
}: Props) {
  const servers = useStore((s) => s.servers);
  const tabs = useStore((s) => s.tabs);
  const activeTabId = useStore((s) => s.activeTabId);
  const focusedServerId = useStore((s) => s.focusedServerId);
  const openTab = useStore((s) => s.openTab);
  const setActiveTab = useStore((s) => s.setActiveTab);
  const setFocusedServer = useStore((s) => s.setFocusedServer);
  const setRightPanel = useStore((s) => s.setRightPanel);
  const setBottomPanel = useStore((s) => s.setBottomPanel);
  const toggleBottomPanel = useStore((s) => s.toggleBottomPanel);
  const pushAlert = useStore((s) => s.pushAlert);
  const pushOutput = useStore((s) => s.pushOutput);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const [runbooks, setRunbooks] = useState<Runbook[]>([]);
  const [snippets, setSnippets] = useState<CommandSnippet[]>([]);
  const inputRef = useRef<HTMLInputElement | null>(null);

  const focusedServer = useMemo(
    () => servers.find((server) => server.id === focusedServerId) ?? null,
    [focusedServerId, servers],
  );

  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    void Promise.all([api.runbooksList(), api.commandSnippetsList()])
      .then(([nextRunbooks, nextSnippets]) => {
        if (cancelled) return;
        setRunbooks(nextRunbooks);
        setSnippets(nextSnippets);
      })
      .catch((error) => {
        if (!cancelled) pushAlert("warn", `Palette index refresh failed: ${error}`);
      });
    return () => {
      cancelled = true;
    };
  }, [open, pushAlert]);

  const actions = useMemo<PaletteAction[]>(() => {
    const closeThen = (action: () => void) => () => {
      action();
      onClose();
    };

    const focusPanel = (server: Server, view: RightPanelView) => {
      setFocusedServer(server.id);
      setRightPanel(view);
      onClose();
    };

    const globalActions: PaletteAction[] = [
      {
        id: "operations-dashboard",
        title: "Open operations dashboard",
        eyebrow: "Operations",
        detail: "Fleet health, alerts, tunnels and recent automation",
        keywords: "operations dashboard fleet unhealthy critical warning noc overview health",
        run: closeThen(onOpenOperations),
      },
      {
        id: "operator-center",
        title: "Open Operator Center",
        eyebrow: "Operations",
        detail: "Alerts, transfers, multi-host commands, tunnels and backup",
        keywords: "operator center alerts transfers multi host broadcast backup restore tunnel",
        run: closeThen(onOpenOperations),
      },
      {
        id: "runbook-studio",
        title: "Open Runbook Studio",
        eyebrow: "Automation",
        detail: "Author, validate, dry-run, import and export runbooks",
        keywords: "runbook studio yaml editor dry run automation import export",
        run: closeThen(onOpenRunbookStudio),
      },
      {
        id: "runbooks",
        title: "Open runbook launcher",
        eyebrow: "Automation",
        detail: "Pick a runbook and target server",
        keywords: "runbook automation diagnose health execute",
        run: closeThen(onOpenRunbooks),
      },
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
        id: "sessions",
        title: "Show SSH session history",
        eyebrow: "Bottom panel",
        detail: "Review opened and closed terminal sessions",
        keywords: "history ssh sessions terminal bottom",
        run: closeThen(() => setBottomPanel("sessions")),
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

    const panelActions: PaletteAction[] = focusedServer
      ? (["health", "diagnostics", "services", "notes", "snippets"] as RightPanelView[]).map((view) => ({
          id: `panel-${view}-${focusedServer.id}`,
          title: `${capitalize(view)} · ${focusedServer.name}`,
          eyebrow: "Focused server",
          detail: `Open the ${view} panel for ${focusedServer.host}`,
          keywords: `${view} right panel metrics health diagnostics services notes snippets ${serverKeywords(focusedServer)}`,
          run: () => focusPanel(focusedServer, view),
        }))
      : [];

    const tabActions: PaletteAction[] = tabs.map((tab) => ({
      id: `tab-${tab.id}`,
      title: tab.title,
      eyebrow: tab.id === activeTabId ? "Active tab" : "Open tab",
      detail: `Switch to ${tab.kind.toUpperCase()} session`,
      keywords: `${tab.title} ${tab.kind} tab session`,
      run: closeThen(() => setActiveTab(tab.id)),
    }));

    const serverActions: PaletteAction[] = servers.flatMap((server) => [
      {
        id: `focus-${server.id}`,
        title: `Focus ${server.name}`,
        eyebrow: server.environment,
        detail: `${server.username}@${server.host}:${server.port}`,
        keywords: serverKeywords(server),
        run: closeThen(() => setFocusedServer(server.id)),
      },
      {
        id: `health-${server.id}`,
        title: `Health · ${server.name}`,
        eyebrow: server.environment,
        detail: "Focus host and open live/persisted health",
        keywords: `${serverKeywords(server)} health cpu ram disk load unhealthy metrics`,
        run: () => focusPanel(server, "health"),
      },
      {
        id: `diagnostics-${server.id}`,
        title: `Diagnostics · ${server.name}`,
        eyebrow: server.environment,
        detail: "SSH trust, runtime readiness and authenticated probe",
        keywords: `${serverKeywords(server)} diagnostics ssh trust fingerprint probe connectivity`,
        run: () => focusPanel(server, "diagnostics"),
      },
      ...SERVER_ACTIONS.filter(
        (action) => !action.requiresProtocol || server.protocols.includes(action.requiresProtocol),
      ).map((action) => ({
        id: `${action.kind}-${server.id}`,
        title: `${action.label} · ${server.name}`,
        eyebrow: server.group_name || "Server action",
        detail: `${server.username}@${server.host}:${server.port}`,
        keywords: `${serverKeywords(server)} ${action.kind} ${action.label}`,
        run: closeThen(() => openTab(action.kind, server)),
      })),
    ]);

    const runbookActions: PaletteAction[] = runbooks.map((runbook) => {
      const target = focusedServer;
      return {
        id: `runbook-${runbook.id}`,
        title: target ? `Run ${runbook.name} · ${target.name}` : runbook.name,
        eyebrow: runbook.builtin ? "Built-in runbook" : "Runbook",
        detail: target
          ? `Open controlled execution on ${target.name}`
          : "Focus a server first, or open the runbook launcher",
        keywords: `runbook automation ${runbook.name} ${runbook.description} ${target ? serverKeywords(target) : ""}`,
        run: target
          ? closeThen(() => openTab("runbook", target, { runbookId: runbook.id, title: `Runbook · ${runbook.name} · ${target.name}` }))
          : closeThen(onOpenRunbooks),
      };
    });

    const snippetActions: PaletteAction[] = snippets
      .filter((snippet) => !focusedServer || snippet.tags.length === 0 || snippet.tags.some((tag) => focusedServer.tags.includes(tag)))
      .map((snippet) => ({
        id: `snippet-${snippet.id}`,
        title: focusedServer ? `${snippet.label} · ${focusedServer.name}` : snippet.label,
        eyebrow: "Command snippet",
        detail: focusedServer
          ? `Requires confirmation before execution on ${focusedServer.environment}`
          : "Focus an SSH server before executing this snippet",
        keywords: `snippet command ${snippet.label} ${snippet.tags.join(" ")} ${focusedServer ? serverKeywords(focusedServer) : ""}`,
        run: async () => {
          const target = focusedServer;
          if (!target) {
            pushAlert("warn", "Focus a server before executing a command snippet.");
            return;
          }
          if (!target.protocols.includes("ssh")) {
            pushAlert("warn", `${target.name} does not expose SSH.` , target.id);
            return;
          }
          const production = target.environment === "production";
          const confirmed = window.confirm(
            `${production ? "PRODUCTION TARGET\n\n" : ""}Execute snippet “${snippet.label}” on ${target.name} (${target.host})?\n\n${snippet.command}`,
          );
          if (!confirmed) return;
          onClose();
          try {
            const output = await api.runRemote(target.id, snippet.command);
            pushOutput(`$ ${snippet.label} · ${target.name}\n${output.stdout}${output.stderr}`);
            setBottomPanel("output");
            pushAlert(output.success ? "info" : "error", `${snippet.label} ${output.success ? "completed" : "failed"} on ${target.name}`, target.id);
          } catch (error) {
            pushAlert("error", `${snippet.label} failed on ${target.name}: ${error}`, target.id);
          }
        },
      }));

    return [
      ...globalActions,
      ...panelActions,
      ...runbookActions,
      ...snippetActions,
      ...tabActions,
      ...serverActions,
    ];
  }, [
    activeTabId,
    focusedServer,
    onClose,
    onNewServer,
    onOpenOperations,
    onOpenRunbookStudio,
    onOpenRunbooks,
    onOpenSettings,
    onOpenTunnels,
    openTab,
    pushAlert,
    pushOutput,
    runbooks,
    servers,
    setActiveTab,
    setBottomPanel,
    setFocusedServer,
    setRightPanel,
    snippets,
    tabs,
    toggleBottomPanel,
  ]);

  const filtered = useMemo(() => {
    const terms = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
    if (terms.length === 0) return actions.slice(0, 24);
    return actions
      .map((action) => ({
        action,
        score: terms.reduce((score, term) => {
          const title = action.title.toLowerCase();
          const haystack = `${action.title} ${action.eyebrow} ${action.detail ?? ""} ${action.keywords}`.toLowerCase();
          if (title.startsWith(term)) return score + 8;
          if (title.includes(term)) return score + 4;
          if (haystack.includes(term)) return score + 1;
          return score - 20;
        }, 0),
      }))
      .filter((item) => item.score >= terms.length)
      .sort((a, b) => b.score - a.score || a.action.title.localeCompare(b.action.title))
      .slice(0, 24)
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
      setSelected((current) => Math.min(Math.max(0, filtered.length - 1), current + 1));
      return;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      setSelected((current) => Math.max(0, current - 1));
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      void filtered[selected]?.run();
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
            placeholder="Search servers, health, runbooks, snippets or actions"
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
                onClick={() => void action.run()}
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

function capitalize(value: string) {
  return value.charAt(0).toUpperCase() + value.slice(1);
}

function iconFor(action: PaletteAction): string {
  if (action.id.startsWith("ssh-")) return "▰";
  if (action.id.startsWith("sftp-") || action.id.startsWith("ftp-")) return "⇅";
  if (action.id.startsWith("rdp-") || action.id.startsWith("vnc-")) return "▣";
  if (action.id.startsWith("focus-")) return "◉";
  if (action.id.startsWith("health-") || action.id.startsWith("diagnostics-")) return "◌";
  if (action.id.startsWith("panel-")) return "◧";
  if (action.id.startsWith("tab-")) return "▱";
  if (action.id.startsWith("runbook-")) return "▶";
  if (action.id.startsWith("snippet-")) return ">_";
  if (action.id.includes("operations")) return "◎";
  if (action.id.includes("runbook")) return "▶";
  if (action.id === "tunnels") return "⇄";
  return "⌁";
}
