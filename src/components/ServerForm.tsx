import { useState } from "react";
import { useStore } from "../store";
import * as api from "../api";
import type { AuthType, Environment, Protocol, Server, ServerInput } from "../types";

interface Props {
  server: Server | null; // null = create new
  onClose: () => void;
}

const ALL_PROTOCOLS: Protocol[] = ["ssh", "sftp", "rdp", "vnc"];

/** Modal to create / edit a server profile. The secret field is write-only:
 *  it is sent to the keyring on save and never read back into the UI. */
export function ServerForm({ server, onClose }: Props) {
  const loadServers = useStore((s) => s.loadServers);
  const pushAlert = useStore((s) => s.pushAlert);

  const [name, setName] = useState(server?.name ?? "");
  const [host, setHost] = useState(server?.host ?? "");
  const [port, setPort] = useState(server?.port ?? 22);
  const [username, setUsername] = useState(server?.username ?? "");
  const [protocols, setProtocols] = useState<Protocol[]>(server?.protocols ?? ["ssh"]);
  const [authType, setAuthType] = useState<AuthType>(server?.auth_type ?? "key");
  const [keyPath, setKeyPath] = useState(server?.private_key_path ?? "");
  const [secret, setSecret] = useState("");
  const [tags, setTags] = useState((server?.tags ?? []).join(", "));
  const [group, setGroup] = useState(server?.group_name ?? "");
  const [environment, setEnvironment] = useState<Environment>(server?.environment ?? "dev");
  const [notes, setNotes] = useState(server?.notes ?? "");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  function toggleProtocol(p: Protocol) {
    setProtocols((cur) => (cur.includes(p) ? cur.filter((x) => x !== p) : [...cur, p]));
  }

  function applyDefaults(nextAuthType: AuthType) {
    setAuthType(nextAuthType);
    if (nextAuthType === "password" && !protocols.includes("ssh")) {
      setProtocols((current) => [...current, "ssh"]);
    }
  }

  async function save() {
    setError(null);
    if (!name.trim() || !host.trim() || !username.trim()) {
      setError("Name, host and username are required.");
      return;
    }
    if (protocols.length === 0) {
      setError("Pick at least one protocol.");
      return;
    }
    const input: ServerInput = {
      id: server?.id,
      name: name.trim(),
      host: host.trim(),
      port: Number(port) || 22,
      username: username.trim(),
      protocols,
      auth_type: authType,
      private_key_path: keyPath.trim() || null,
      tags: tags.split(",").map((t) => t.trim()).filter(Boolean),
      group_name: group.trim() || null,
      environment,
      notes: notes.trim() || null,
      secret: secret ? secret : null,
    };
    setSaving(true);
    try {
      await api.serverSave(input);
      pushAlert("info", `Saved server "${input.name}"`);
      await loadServers();
      onClose();
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="modal">
        <div className="modal-head">
          <div>
            <span className="eyebrow">{server ? "Edit profile" : "New profile"}</span>
            <strong>{server ? server.name : "Add server"}</strong>
          </div>
          <button className="ghost tiny" onClick={onClose}>✕</button>
        </div>
        <div className="modal-body">
          <div className="form-section">
            <span className="section-kicker">Identity</span>
            <p>Give the host a recognizable name and environment for fast filtering.</p>
          </div>
          <div className="form-row three">
            <div><label>Name</label><input value={name} onChange={(e) => setName(e.target.value)} placeholder="prod-db-1" /></div>
            <div><label>Group / folder</label><input value={group} onChange={(e) => setGroup(e.target.value)} placeholder="Production" /></div>
            <div>
              <label>Environment</label>
              <select value={environment} onChange={(e) => setEnvironment(e.target.value as Environment)}>
                <option value="production">production</option>
                <option value="staging">staging</option>
                <option value="dev">dev</option>
              </select>
            </div>
          </div>

          <div className="form-row three">
            <div><label>Host</label><input value={host} onChange={(e) => setHost(e.target.value)} placeholder="10.0.0.5 / host.example.com" /></div>
            <div><label>Port</label><input type="number" value={port} onChange={(e) => setPort(Number(e.target.value))} /></div>
            <div><label>Username</label><input value={username} onChange={(e) => setUsername(e.target.value)} placeholder="root" /></div>
          </div>

          <div className="form-section">
            <span className="section-kicker">Access</span>
            <p>Choose every workflow this profile should expose in the sidebar.</p>
          </div>
          <div>
            <label>Protocols</label>
            <div className="checks protocol-checks">
              {ALL_PROTOCOLS.map((p) => (
                <label key={p} className="check">
                  <input type="checkbox" checked={protocols.includes(p)} onChange={() => toggleProtocol(p)} />
                  <span>{p.toUpperCase()}</span>
                </label>
              ))}
            </div>
          </div>

          <div className="form-row">
            <div>
              <label>Auth type</label>
              <select value={authType} onChange={(e) => applyDefaults(e.target.value as AuthType)}>
                <option value="key">Private key</option>
                <option value="password">Password</option>
              </select>
            </div>
            <div>
              <label>{authType === "key" ? "Secret (key passphrase, optional)" : "Password"}</label>
              <input
                type="password"
                value={secret}
                onChange={(e) => setSecret(e.target.value)}
                placeholder={server ? "•••• (unchanged)" : authType === "key" ? "optional" : "stored in OS keyring"}
              />
            </div>
          </div>

          {authType === "key" && (
            <div>
              <label>Private key path</label>
              <input value={keyPath} onChange={(e) => setKeyPath(e.target.value)} placeholder="~/.ssh/id_ed25519" />
            </div>
          )}

          <div className="form-section">
            <span className="section-kicker">Context</span>
            <p>Tags and notes make the cockpit useful during incidents.</p>
          </div>
          <div>
            <label>Tags (comma separated)</label>
            <input value={tags} onChange={(e) => setTags(e.target.value)} placeholder="db, postgres, eu-west" />
          </div>

          <div>
            <label>Notes</label>
            <textarea rows={3} value={notes} onChange={(e) => setNotes(e.target.value)} placeholder="Runbooks, contacts, gotchas…" />
          </div>

          <div className="muted" style={{ fontSize: 11 }}>
            🔒 Secrets are stored in the OS keyring (Secret Service), never in the database.
          </div>
          {error && <div className="error-text">{error}</div>}
        </div>
        <div className="modal-foot">
          <button onClick={onClose}>Cancel</button>
          <button className="primary" onClick={() => void save()} disabled={saving}>
            {saving ? "Saving…" : "Save profile"}
          </button>
        </div>
      </div>
    </div>
  );
}
