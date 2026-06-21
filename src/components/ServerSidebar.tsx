import { useMemo, useState } from "react";
import { useStore } from "../store";
import * as api from "../api";
import type { Environment, Server } from "../types";

interface Props {
  onNew: (folder?: string) => void;
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
  const [envFilter, setEnvFilter] = useState<Environment | "all">("all");

  // Filter by name/host/tag/group, then bucket by group.
  const groups = useMemo(() => {
    const q = query.trim().toLowerCase();
    const filtered = servers.filter((s) => {
      if (envFilter !== "all" && s.environment !== envFilter) return false;
      if (!q) return true;
      return (
        s.name.toLowerCase().includes(q) ||
        s.host.toLowerCase().includes(q) ||
        s.username.toLowerCase().includes(q) ||
        s.environment.toLowerCase().includes(q) ||
        (s.group_name ?? "").toLowerCase().includes(q) ||
        s.tags.some((t) => t.toLowerCase().includes(q))
      );
    });
    const map = new Map<string, Server[]>();
    for (const s of filtered.sort((a, b) => a.name.localeCompare(b.name))) {
      const g = s.group_name?.trim() || "Ungrouped";
      if (!map.has(g)) map.set(g, []);
      map.get(g)!.push(s);
    }
    return [...map.entries()].sort(([a], [b]) => a.localeCompare(b));
  }, [servers, query, envFilter]);

  const filteredCount = groups.reduce((count, [, items]) => count + items.length, 0);
  const environmentCounts = useMemo(() => ({
    production: servers.filter((server) => server.environment === "production").length,
    staging: servers.filter((server) => server.environment === "staging").length,
    dev: servers.filter((server) => server.environment === "dev").length,
  }), [servers]);

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

  function connect(s: Server, kind: "ssh" | "rdp" | "vnc" | "sftp" | "ftp" | "logs") {
    openTab(kind, s);
  }

  function defaultConnect(server: Server) {
    if (server.protocols.includes("ssh")) connect(server, "ssh");
    else if (server.protocols.includes("sftp")) connect(server, "sftp");
    else if (server.protocols.includes("ftp")) connect(server, "ftp");
    else if (server.protocols.includes("rdp")) connect(server, "rdp");
    else if (server.protocols.includes("vnc")) connect(server, "vnc");
    else connect(server, "logs");
  }

  return (
    <aside className="sidebar">
      <div className="sidebar-head">
        <div className="sidebar-title-row">
          <div>
            <strong>Servers</strong>
            <small>{filteredCount} shown · {servers.length} total</small>
          </div>
          <button className="primary tiny" onClick={() => onNew()}>+ Add</button>
        </div>
        <div className="sidebar-search">
          <input
            placeholder="Search name, host, user, tag…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
        <div className="env-filter" role="tablist" aria-label="Filter servers by environment">
          {(["all", "production", "staging", "dev"] as const).map((env) => (
            <button
              key={env}
              className={envFilter === env ? "active" : ""}
              onClick={() => setEnvFilter(env)}
            >
              {env === "all" ? "All" : env}
              {env !== "all" && <span>{environmentCounts[env]}</span>}
            </button>
          ))}
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
        {!loading && servers.length > 0 && filteredCount === 0 && (
          <div className="panel-hint">No servers match this filter.</div>
        )}
        {groups.map(([group, items]) => (
          <div key={group}>
            <div className="group-label">
              <span>{group}</span>
              <span className="group-actions">
                <button
                  className="tiny ghost"
                  title={`Add server to ${group}`}
                  onClick={(event) => {
                    event.stopPropagation();
                    onNew(group === "Ungrouped" ? undefined : group);
                  }}
                >
                  + Add
                </button>
                <span>{items.length}</span>
              </span>
            </div>
            {items.map((s) => (
              <div
                key={s.id}
                className={`server-item${focusedServerId === s.id ? " active" : ""}`}
                onClick={() => setFocusedServer(s.id)}
                onDoubleClick={() => defaultConnect(s)}
              >
                <div className="si-top">
                  <span className={`env-dot env-${s.environment}`} title={s.environment} />
                  <span className="si-name">{s.name}</span>
                  <span className="server-env">{s.environment}</span>
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
                    <button className="tiny" title="Open SSH terminal" onClick={(e) => { e.stopPropagation(); connect(s, "ssh"); }}>SSH</button>
                  )}
                  {s.protocols.includes("sftp") && (
                    <button className="tiny" title="Open file browser" onClick={(e) => { e.stopPropagation(); connect(s, "sftp"); }}>SFTP</button>
                  )}
                  {s.protocols.includes("ftp") && (
                    <button className="tiny" title="Open plaintext FTP browser" onClick={(e) => { e.stopPropagation(); connect(s, "ftp"); }}>FTP</button>
                  )}
                  {s.protocols.includes("rdp") && (
                    <button className="tiny" title="Launch RDP" onClick={(e) => { e.stopPropagation(); connect(s, "rdp"); }}>RDP</button>
                  )}
                  {s.protocols.includes("vnc") && (
                    <button className="tiny" title="Launch VNC" onClick={(e) => { e.stopPropagation(); connect(s, "vnc"); }}>VNC</button>
                  )}
                  <button className="tiny ghost" title="Open logs" onClick={(e) => { e.stopPropagation(); connect(s, "logs"); }}>Logs</button>
                  <span style={{ flex: 1 }} />
                  <button className="tiny icon-button" title="Edit profile" onClick={(e) => { e.stopPropagation(); onEdit(s); }}>✎</button>
                  <button className="tiny icon-button danger-ghost" title="Delete profile" onClick={(e) => { e.stopPropagation(); void remove(s); }}>🗑</button>
                </div>
              </div>
            ))}
          </div>
        ))}
      </div>
    </aside>
  );
}
