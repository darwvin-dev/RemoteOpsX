import { useStore } from "../store";
import { HealthPanel } from "./HealthPanel";
import { ServicesPanel } from "./ServicesPanel";
import { DockerPanel } from "./DockerPanel";
import { NotesSnippetsPanel } from "./NotesSnippetsPanel";

const VIEWS = [
  { key: "health", label: "Health", icon: "◆" },
  { key: "services", label: "Services", icon: "●" },
  { key: "docker", label: "Docker", icon: "▣" },
  { key: "notes", label: "Notes", icon: "✦" },
  { key: "snippets", label: "Snippets", icon: "⌁" },
] as const;

/** Right-side operations panel, scoped to the focused server. */
export function RightPanel() {
  const view = useStore((s) => s.rightPanel);
  const setView = useStore((s) => s.setRightPanel);
  const focusedServerId = useStore((s) => s.focusedServerId);
  const server = useStore((s) => s.servers.find((x) => x.id === s.focusedServerId));

  return (
    <aside className="right">
      <div className="right-context">
        {server && focusedServerId ? (
          <>
            <div>
              <span className="eyebrow">Focused host</span>
              <strong>{server.name}</strong>
              <small>{server.username}@{server.host}:{server.port}</small>
            </div>
            <span className={`env-badge env-${server.environment}`}>{server.environment}</span>
          </>
        ) : (
          <div>
            <span className="eyebrow">Operations panel</span>
            <strong>No host selected</strong>
            <small>Choose a server to enable live tools</small>
          </div>
        )}
      </div>
      <div className="right-tabs">
        {VIEWS.map((v) => (
          <button key={v.key} className={view === v.key ? "active" : ""} onClick={() => setView(v.key)}>
            <span>{v.icon}</span>
            {v.label}
          </button>
        ))}
      </div>
      <div className="right-body">
        {!server || !focusedServerId ? (
          <div className="panel-hint">Select a server to see live operations data.</div>
        ) : view === "health" ? (
          <HealthPanel server={server} />
        ) : view === "services" ? (
          <ServicesPanel server={server} />
        ) : view === "docker" ? (
          <DockerPanel server={server} />
        ) : (
          <NotesSnippetsPanel server={server} showSnippets={view === "snippets"} />
        )}
      </div>
    </aside>
  );
}
