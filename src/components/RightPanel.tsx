import { useStore } from "../store";
import { HealthPanel } from "./HealthPanel";
import { ServicesPanel } from "./ServicesPanel";
import { DockerPanel } from "./DockerPanel";
import { NotesSnippetsPanel } from "./NotesSnippetsPanel";

const VIEWS = [
  { key: "health", label: "Health" },
  { key: "services", label: "Services" },
  { key: "docker", label: "Docker" },
  { key: "notes", label: "Notes" },
  { key: "snippets", label: "Snippets" },
] as const;

/** Right-side operations panel, scoped to the focused server. */
export function RightPanel() {
  const view = useStore((s) => s.rightPanel);
  const setView = useStore((s) => s.setRightPanel);
  const focusedServerId = useStore((s) => s.focusedServerId);
  const server = useStore((s) => s.servers.find((x) => x.id === s.focusedServerId));

  return (
    <aside className="right">
      <div className="right-tabs">
        {VIEWS.map((v) => (
          <button key={v.key} className={view === v.key ? "active" : ""} onClick={() => setView(v.key)}>
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
