import { useMemo, useState } from "react";
import { useStore } from "../store";
import * as api from "../api";
import type { Server } from "../types";

interface Props {
  onNew: () => void;
  onEdit: (s: Server) => void;
}

/** Left sidebar: search + grouped server list with quick connect actions. */
export function ServerSidebar({ onNew, onEdit }: Props) {
  const servers = useStore((s) => s.servers);
  const loading = useStore((s) => s.loadingServers);
  const focusedServerId = useStore((s) => s.focusedServerId);
  const openTab = useStore((s) => s.openTab);
  const setFocusedServer = useStore((s) => s.setFocusedServer);
  const loadServers = useStore((s) => s.loadServers);
  const pushAlert = useStore((s) => s.pushAlert);
  const [query, setQuery] = useState("");

  // Filter by name/host/tag/group, then bucket by group.
  const groups = useMemo(() => {
    const q = query.trim().toLowerCase();
    const filtered = servers.filter((s) => {
      if (!q) return true;
      return (
        s.name.toLowerCase().includes(q) ||
        s.host.toLowerCase().includes(q) ||
        (s.group_name ?? "").toLowerCase().includes(q) ||
        s.tags.some((t) => t.toLowerCase().includes(q))
      );
    });
    const map = new Map<string, Server[]>();
    for (const s of filtered) {
      const g = s.group_name?.trim() || "Ungrouped";
      if (!map.has(g)) map.set(g, []);
      map.get(g)!.push(s);
    }
    return [...map.entries()];
  }, [servers, query]);

  async function remove(s: Server) {
    if (!confirm(`Delete profile "${s.name}"? This also removes its stored secret.`)) return;
    try {
      await api.serverDelete(s.id);
      pushAlert("info", `Deleted server "${s.name}"`);
      await loadServers();
    } catch (err) {
      pushAlert("error", `Delete failed: ${err}`);
    }
  }

  function connect(s: Server, kind: "ssh" | "rdp" | "vnc" | "sftp" | "logs") {
    openTab(kind, s);
  }

  return (
    <aside className="sidebar">
      <div className="sidebar-head">
        <div className="flex" style={{ justifyContent: "space-between" }}>
          <strong>Servers</strong>
          <button className="primary tiny" onClick={onNew}>+ Add</button>
        </div>
        <div className="sidebar-search">
          <input
            placeholder="Search name, host, tag, group…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
      </div>

      <div className="server-list">
        {loading && <div className="panel-hint">Loading…</div>}
        {!loading && servers.length === 0 && (
          <div className="panel-hint">
            No servers yet.<br />
            Click <strong>+ Add</strong> to create your first profile.
          </div>
        )}
        {groups.map(([group, items]) => (
          <div key={group}>
            <div className="group-label">{group}</div>
            {items.map((s) => (
              <div
                key={s.id}
                className={`server-item${focusedServerId === s.id ? " active" : ""}`}
                onClick={() => setFocusedServer(s.id)}
              >
                <div className="si-top">
                  <span className={`env-dot env-${s.environment}`} title={s.environment} />
                  <span className="si-name">{s.name}</span>
                </div>
                <span className="si-host">{s.username}@{s.host}:{s.port}</span>
                <div className="proto-pills">
                  {s.protocols.map((p) => (
                    <span key={p} className={`pill ${p}`}>{p}</span>
                  ))}
                </div>
                {s.tags.length > 0 && (
                  <div className="tag-row">
                    {s.tags.map((t) => <span key={t} className="tag">#{t}</span>)}
                  </div>
                )}
                <div className="server-actions">
                  {s.protocols.includes("ssh") && (
                    <button className="tiny" onClick={(e) => { e.stopPropagation(); connect(s, "ssh"); }}>SSH</button>
                  )}
                  {s.protocols.includes("sftp") && (
                    <button className="tiny" onClick={(e) => { e.stopPropagation(); connect(s, "sftp"); }}>SFTP</button>
                  )}
                  {s.protocols.includes("rdp") && (
                    <button className="tiny" onClick={(e) => { e.stopPropagation(); connect(s, "rdp"); }}>RDP</button>
                  )}
                  {s.protocols.includes("vnc") && (
                    <button className="tiny" onClick={(e) => { e.stopPropagation(); connect(s, "vnc"); }}>VNC</button>
                  )}
                  <button className="tiny ghost" onClick={(e) => { e.stopPropagation(); connect(s, "logs"); }}>Logs</button>
                  <span style={{ flex: 1 }} />
                  <button className="tiny ghost" onClick={(e) => { e.stopPropagation(); onEdit(s); }}>✎</button>
                  <button className="tiny ghost" onClick={(e) => { e.stopPropagation(); void remove(s); }}>🗑</button>
                </div>
              </div>
            ))}
          </div>
        ))}
      </div>
    </aside>
  );
}
