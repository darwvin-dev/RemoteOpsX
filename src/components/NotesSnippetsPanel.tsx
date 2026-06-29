import { type FormEvent, useEffect, useMemo, useState } from "react";
import * as api from "../api";
import { useStore } from "../store";
import type { CommandSnippet, Server } from "../types";

/** Handy command snippets the operator can run against the focused server with
 *  one click (output lands in the bottom panel). */
const BUILTIN_SNIPPETS: CommandSnippet[] = [
  { id: "builtin-w", label: "Who is logged in", command: "w", tags: [], created_at: "", updated_at: "" },
  { id: "builtin-last", label: "Last logins", command: "last -n 15", tags: [], created_at: "", updated_at: "" },
  { id: "builtin-lsof", label: "Open files (limit)", command: "lsof | head -50", tags: [], created_at: "", updated_at: "" },
  { id: "builtin-var", label: "Largest dirs in /var", command: "du -xhd1 /var 2>/dev/null | sort -rh | head", tags: [], created_at: "", updated_at: "" },
  { id: "builtin-dmesg", label: "Recent kernel msgs", command: "dmesg | tail -40", tags: [], created_at: "", updated_at: "" },
  { id: "builtin-ss", label: "TCP connections", command: "ss -tan | head -40", tags: [], created_at: "", updated_at: "" },
  { id: "builtin-cron", label: "Cron jobs (root)", command: "crontab -l 2>/dev/null; ls -la /etc/cron.d", tags: [], created_at: "", updated_at: "" },
  { id: "builtin-oom", label: "OOM kills", command: "journalctl -k | grep -i 'killed process' | tail -20", tags: [], created_at: "", updated_at: "" },
];

export function NotesSnippetsPanel({ server, showSnippets }: { server: Server; showSnippets: boolean }) {
  const pushOutput = useStore((s) => s.pushOutput);
  const pushAlert = useStore((s) => s.pushAlert);
  const setBottomPanel = useStore((s) => s.setBottomPanel);
  const [snippets, setSnippets] = useState<CommandSnippet[]>([]);
  const [editing, setEditing] = useState<CommandSnippet | null>(null);
  const [label, setLabel] = useState("");
  const [command, setCommand] = useState("");
  const [tags, setTags] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!showSnippets) return;
    void loadSnippets();
  }, [showSnippets]);

  const visibleSnippets = useMemo(() => {
    const serverTags = new Set(server.tags.map((tag) => tag.toLowerCase()));
    const saved = snippets.filter((snippet) => snippet.tags.length === 0 || snippet.tags.some((tag) => serverTags.has(tag.toLowerCase())));
    return [...BUILTIN_SNIPPETS, ...saved];
  }, [server.tags, snippets]);

  async function loadSnippets() {
    try {
      setSnippets(await api.commandSnippetsList());
    } catch (err) {
      pushAlert("error", `snippets: ${err}`);
    }
  }

  async function run(snippet: CommandSnippet) {
    try {
      const out = await api.runRemote(server.id, snippet.command);
      pushOutput(`# ${snippet.label}\n$ ${snippet.command}\n${out.stdout || out.stderr}`);
      setBottomPanel("output");
    } catch (err) {
      pushAlert("error", `${snippet.label}: ${err}`);
    }
  }

  function startEdit(snippet?: CommandSnippet) {
    setEditing(snippet ?? null);
    setLabel(snippet?.label ?? "");
    setCommand(snippet?.command ?? "");
    setTags(snippet?.tags.join(", ") ?? "");
  }

  function cancelEdit() {
    setEditing(null);
    setLabel("");
    setCommand("");
    setTags("");
  }

  async function submit(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    try {
      const saved = await api.commandSnippetSave({
        id: editing?.id ?? null,
        label,
        command,
        tags: tags.split(",").map((tag) => tag.trim()).filter(Boolean),
      });
      setSnippets((current) => [saved, ...current.filter((snippet) => snippet.id !== saved.id)].sort((left, right) => left.label.localeCompare(right.label)));
      cancelEdit();
      pushAlert("info", `Saved snippet "${saved.label}"`);
    } catch (err) {
      pushAlert("error", `save snippet: ${err}`);
    } finally {
      setBusy(false);
    }
  }

  async function remove(snippet: CommandSnippet) {
    if (!confirm(`Delete snippet "${snippet.label}"?`)) return;
    setBusy(true);
    try {
      await api.commandSnippetDelete(snippet.id);
      setSnippets((current) => current.filter((item) => item.id !== snippet.id));
    } catch (err) {
      pushAlert("error", `delete snippet: ${err}`);
    } finally {
      setBusy(false);
    }
  }

  if (!showSnippets) {
    return (
      <div>
        <div className="section-title">Notes · {server.name}</div>
        {server.notes ? (
          <div style={{ whiteSpace: "pre-wrap", lineHeight: 1.5 }}>{server.notes}</div>
        ) : (
          <div className="panel-hint">No notes. Edit the server profile to add some.</div>
        )}
      </div>
    );
  }

  return (
    <div>
      <div className="section-title">Quick snippets</div>
      <div className="col">
        {visibleSnippets.map((snippet) => (
          <div key={snippet.id} className="snippet-card">
            <button style={{ textAlign: "left" }} onClick={() => void run(snippet)}>
              <div style={{ fontWeight: 600 }}>{snippet.label}</div>
              <div className="mono muted" style={{ fontSize: 10 }}>{snippet.command}</div>
              {snippet.tags.length > 0 && <div className="snippet-tags">{snippet.tags.map((tag) => <span key={tag}>{tag}</span>)}</div>}
            </button>
            {!snippet.id.startsWith("builtin-") && (
              <div className="snippet-actions">
                <button className="tiny ghost" disabled={busy} onClick={() => startEdit(snippet)}>Edit</button>
                <button className="tiny ghost danger-ghost" disabled={busy} onClick={() => void remove(snippet)}>Delete</button>
              </div>
            )}
          </div>
        ))}
      </div>
      <div className="section-title">{editing ? "Edit snippet" : "Add snippet"}</div>
      <form className="snippet-form" onSubmit={(event) => void submit(event)}>
        <input value={label} onChange={(event) => setLabel(event.target.value)} placeholder="Label" maxLength={80} required />
        <textarea rows={3} value={command} onChange={(event) => setCommand(event.target.value)} placeholder="Command to run over SSH" required />
        <input value={tags} onChange={(event) => setTags(event.target.value)} placeholder="Optional matching tags, comma-separated" />
        <div className="snippet-form-actions">
          {editing && <button type="button" className="tiny ghost" onClick={cancelEdit}>Cancel</button>}
          <button className="tiny primary" disabled={busy}>{editing ? "Save" : "Add"}</button>
        </div>
      </form>
    </div>
  );
}
