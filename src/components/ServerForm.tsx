import { useMemo, useState } from "react";
import { useStore } from "../store";
import * as api from "../api";
import type { AuthType, Environment, Protocol, Server, ServerInput } from "../types";

interface Props {
  server: Server | null; // null = create new
  initialFolder?: string;
  onClose: () => void;
}

const FOLDER_NONE = "__none__";
const FOLDER_NEW = "__new__";

const ENVIRONMENT_OPTIONS: { value: Environment; label: string; description: string }[] = [
  { value: "production", label: "Production", description: "Critical systems" },
  { value: "staging", label: "Staging", description: "Pre-production" },
  { value: "dev", label: "Dev", description: "Lab and test hosts" },
];

const AUTH_OPTIONS: { value: AuthType; label: string; description: string }[] = [
  { value: "key", label: "Private key", description: "Best for daily SSH/SFTP work" },
  { value: "password", label: "Password", description: "Needed for FTP or password-only hosts" },
];

const PROTOCOL_OPTIONS: { value: Protocol; label: string; detail: string; icon: string }[] = [
  { value: "ssh", label: "SSH", detail: "Terminal, health, runbooks", icon: "▰" },
  { value: "sftp", label: "SFTP", detail: "Encrypted file browser", icon: "⇅" },
  { value: "ftp", label: "FTP", detail: "Plaintext legacy file access", icon: "⇆" },
  { value: "rdp", label: "RDP", detail: "Launch FreeRDP", icon: "▣" },
  { value: "vnc", label: "VNC", detail: "Launch VNC viewer", icon: "◫" },
];

/** Modal to create / edit a server profile. The secret field is write-only:
 *  it is sent to the keyring on save and never read back into the UI. */
export function ServerForm({ server, initialFolder, onClose }: Props) {
  const servers = useStore((store) => store.servers);
  const loadServers = useStore((s) => s.loadServers);
  const pushAlert = useStore((s) => s.pushAlert);
  const existingFolders = useMemo(() => {
    const folders = new Set<string>();
    for (const profile of servers) {
      const folder = profile.group_name?.trim();
      if (folder) folders.add(folder);
    }
    return [...folders].sort((left, right) => left.localeCompare(right));
  }, [servers]);

  const defaultFolder = (server?.group_name ?? initialFolder ?? "").trim();
  const [name, setName] = useState(server?.name ?? "");
  const [host, setHost] = useState(server?.host ?? "");
  const [port, setPort] = useState(server?.port ?? 22);
  const [ftpPort, setFtpPort] = useState(server?.ftp_port ?? 21);
  const [rdpPort, setRdpPort] = useState(server?.rdp_port ?? 3389);
  const [vncPort, setVncPort] = useState(server?.vnc_port ?? 5900);
  const [username, setUsername] = useState(server?.username ?? "");
  const [protocols, setProtocols] = useState<Protocol[]>(server?.protocols ?? ["ssh"]);
  const [authType, setAuthType] = useState<AuthType>(server?.auth_type ?? "key");
  const [keyPath, setKeyPath] = useState(server?.private_key_path ?? "");
  const [secret, setSecret] = useState("");
  const [tags, setTags] = useState((server?.tags ?? []).join(", "));
  const [folderChoice, setFolderChoice] = useState(defaultFolder || FOLDER_NONE);
  const [folderName, setFolderName] = useState(defaultFolder);
  const [environment, setEnvironment] = useState<Environment>(server?.environment ?? "dev");
  const [notes, setNotes] = useState(server?.notes ?? "");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const folderOptions = useMemo(() => {
    if (
      folderChoice !== FOLDER_NONE &&
      folderChoice !== FOLDER_NEW &&
      !existingFolders.includes(folderChoice)
    ) {
      return [folderChoice, ...existingFolders];
    }
    return existingFolders;
  }, [existingFolders, folderChoice]);

  function toggleProtocol(protocol: Protocol) {
    setProtocols((current) => {
      if (current.includes(protocol)) {
        return current.filter((enabledProtocol) => enabledProtocol !== protocol);
      }
      if (protocol === "ftp") setAuthType("password");
      return [...current, protocol];
    });
  }

  function applyDefaults(nextAuthType: AuthType) {
    setAuthType(nextAuthType);
    if (nextAuthType === "key") setSecret("");
  }

  function selectFolder(nextChoice: string) {
    setFolderChoice(nextChoice);
    if (nextChoice === FOLDER_NONE) {
      setFolderName("");
      return;
    }
    if (nextChoice === FOLDER_NEW) {
      setFolderName("");
      return;
    }
    setFolderName(nextChoice);
  }

  function validatePort(label: string, value: number) {
    if (!Number.isInteger(Number(value)) || value < 1 || value > 65535) {
      setError(`${label} must be between 1 and 65535.`);
      return false;
    }
    return true;
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
    if (protocols.includes("ftp") && authType !== "password") {
      setError("FTP requires password authentication because the protocol does not support SSH keys.");
      return;
    }
    if (authType === "password" && !server && !secret.trim()) {
      setError("Enter the password to store in the OS keyring.");
      return;
    }
    if (folderChoice === FOLDER_NEW && !folderName.trim()) {
      setError("Enter a folder name or choose No folder.");
      return;
    }
    if (!validatePort("SSH port", Number(port))) return;
    if (protocols.includes("ftp") && !validatePort("FTP port", Number(ftpPort))) return;
    if (protocols.includes("rdp") && !validatePort("RDP port", Number(rdpPort))) return;
    if (protocols.includes("vnc") && !validatePort("VNC port", Number(vncPort))) return;

    const normalizedFolder = folderChoice === FOLDER_NONE ? "" : folderName.trim();
    const input: ServerInput = {
      id: server?.id,
      name: name.trim(),
      host: host.trim(),
      port: Number(port) || 22,
      ftp_port: protocols.includes("ftp") ? Number(ftpPort) || 21 : null,
      rdp_port: protocols.includes("rdp") ? Number(rdpPort) || 3389 : null,
      vnc_port: protocols.includes("vnc") ? Number(vncPort) || 5900 : null,
      username: username.trim(),
      protocols,
      auth_type: authType,
      private_key_path: keyPath.trim() || null,
      tags: tags.split(",").map((t) => t.trim()).filter(Boolean),
      group_name: normalizedFolder || null,
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
      <div className="modal wide server-modal">
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
            <p>Name the host, place it in a folder and mark its environment for safer day-to-day operations.</p>
          </div>

          <div className="form-row">
            <div>
              <label>Profile name <span className="required">*</span></label>
              <input value={name} onChange={(event) => setName(event.target.value)} placeholder="prod-db-1" />
              <small className="field-help">The label shown in the sidebar and command palette.</small>
            </div>
            <div>
              <label>Folder</label>
              <div className="folder-picker">
                <select value={folderChoice} onChange={(event) => selectFolder(event.target.value)}>
                  <option value={FOLDER_NONE}>No folder</option>
                  {folderOptions.map((folder) => (
                    <option key={folder} value={folder}>{folder}</option>
                  ))}
                  <option value={FOLDER_NEW}>+ Create new folder</option>
                </select>
                {folderChoice === FOLDER_NEW && (
                  <input
                    value={folderName}
                    onChange={(event) => setFolderName(event.target.value)}
                    placeholder="e.g. Production / EU"
                    autoFocus
                  />
                )}
              </div>
              <small className="field-help">Folders are shared by all profiles and appear in the sidebar.</small>
            </div>
          </div>

          <div className="form-row three">
            <div>
              <label>Host <span className="required">*</span></label>
              <input value={host} onChange={(event) => setHost(event.target.value)} placeholder="10.0.0.5 or host.example.com" />
            </div>
            <div>
              <label>Username <span className="required">*</span></label>
              <input value={username} onChange={(event) => setUsername(event.target.value)} placeholder="root" />
            </div>
            <div>
              <label>SSH port</label>
              <input type="number" min={1} max={65535} value={port} onChange={(event) => setPort(Number(event.target.value))} />
            </div>
          </div>

          <div>
            <label>Environment</label>
            <div className="choice-grid three" role="radiogroup" aria-label="Environment">
              {ENVIRONMENT_OPTIONS.map((option) => (
                <button
                  key={option.value}
                  type="button"
                  className={`choice-card env-choice env-${option.value}${environment === option.value ? " selected" : ""}`}
                  onClick={() => setEnvironment(option.value)}
                  aria-pressed={environment === option.value}
                >
                  <span className="choice-mark" />
                  <strong>{option.label}</strong>
                  <small>{option.description}</small>
                </button>
              ))}
            </div>
          </div>

          <div className="form-section">
            <span className="section-kicker">Access</span>
            <p>Choose every workflow this profile should expose. FTP is only for legacy hosts.</p>
          </div>
          <div>
            <label>Protocols</label>
            <div className="protocol-grid" role="group" aria-label="Enabled protocols">
              {PROTOCOL_OPTIONS.map((option) => (
                <button
                  key={option.value}
                  type="button"
                  className={`protocol-card ${option.value}${protocols.includes(option.value) ? " selected" : ""}`}
                  onClick={() => toggleProtocol(option.value)}
                  aria-pressed={protocols.includes(option.value)}
                >
                  <span className="protocol-icon">{option.icon}</span>
                  <span>
                    <strong>{option.label}</strong>
                    <small>{option.detail}</small>
                  </span>
                  <span className="choice-mark" />
                </button>
              ))}
            </div>
            {protocols.includes("ftp") && (
              <div className="warn-banner" style={{ marginTop: 8 }}>
                FTP is plaintext and forces password authentication. Prefer SFTP whenever possible.
              </div>
            )}
          </div>

          {(protocols.includes("ftp") || protocols.includes("rdp") || protocols.includes("vnc")) && (
            <div className="form-row three">
              {protocols.includes("ftp") && <div><label>FTP port</label><input aria-label="FTP port" type="number" min={1} max={65535} value={ftpPort} onChange={(e) => setFtpPort(Number(e.target.value))} /></div>}
              {protocols.includes("rdp") && <div><label>RDP port</label><input aria-label="RDP port" type="number" min={1} max={65535} value={rdpPort} onChange={(e) => setRdpPort(Number(e.target.value))} /></div>}
              {protocols.includes("vnc") && <div><label>VNC port</label><input aria-label="VNC port" type="number" min={1} max={65535} value={vncPort} onChange={(e) => setVncPort(Number(e.target.value))} /></div>}
            </div>
          )}

          <div>
            <label>Auth type</label>
            <div className="choice-grid two" role="radiogroup" aria-label="Authentication type">
              {AUTH_OPTIONS.map((option) => (
                <button
                  key={option.value}
                  type="button"
                  className={`choice-card${authType === option.value ? " selected" : ""}`}
                  onClick={() => applyDefaults(option.value)}
                  aria-pressed={authType === option.value}
                >
                  <span className="choice-mark" />
                  <strong>{option.label}</strong>
                  <small>{option.description}</small>
                </button>
              ))}
            </div>
          </div>

          {authType === "password" ? (
            <div>
              <label>Password</label>
              <input
                type="password"
                value={secret}
                onChange={(event) => setSecret(event.target.value)}
                placeholder={server ? "•••• (unchanged)" : "stored in OS keyring"}
              />
              <small className="field-help">Saved to the OS keyring. It is never written into SQLite.</small>
            </div>
          ) : (
            <div>
              <label>Private key path</label>
              <input value={keyPath} onChange={(event) => setKeyPath(event.target.value)} placeholder="~/.ssh/id_ed25519" />
              <small className="field-help">Encrypted keys use your SSH agent or the interactive SSH prompt.</small>
            </div>
          )}

          <div className="form-section">
            <span className="section-kicker">Context</span>
            <p>Tags and notes make the cockpit useful during incidents.</p>
          </div>
          <div>
            <label>Tags (comma separated)</label>
            <input value={tags} onChange={(event) => setTags(event.target.value)} placeholder="db, postgres, eu-west" />
          </div>

          <div>
            <label>Notes</label>
            <textarea rows={3} value={notes} onChange={(event) => setNotes(event.target.value)} placeholder="Runbooks, contacts, gotchas…" />
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
