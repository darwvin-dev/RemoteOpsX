import { useEffect, useMemo, useState } from "react";
import * as api from "../api";
import * as jumpApi from "../jumpHostApi";
import { useStore } from "../store";
import type { HostIdentityReport, JumpHostConfig, SshKeyInfo } from "../types";

export function JumpHostManager({ onClose }: { onClose: () => void }) {
  const servers = useStore((state) => state.servers);
  const pushAlert = useStore((state) => state.pushAlert);
  const [serverId, setServerId] = useState(servers[0]?.id ?? "");
  const [enabled, setEnabled] = useState(false);
  const [host, setHost] = useState("");
  const [port, setPort] = useState(22);
  const [username, setUsername] = useState("");
  const [keyPath, setKeyPath] = useState("");
  const [keys, setKeys] = useState<SshKeyInfo[]>([]);
  const [identity, setIdentity] = useState<HostIdentityReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const selectedServer = useMemo(() => servers.find((server) => server.id === serverId), [servers, serverId]);

  useEffect(() => {
    void api.sshKeysList().then(setKeys).catch(() => setKeys([]));
  }, []);

  useEffect(() => {
    let cancelled = false;
    setError(null);
    setIdentity(null);
    if (!serverId) return;
    setBusy(true);
    void jumpApi.jumpHostGet(serverId)
      .then((config) => {
        if (cancelled) return;
        setEnabled(Boolean(config));
        setHost(config?.host ?? "");
        setPort(config?.port ?? 22);
        setUsername(config?.username ?? "");
        setKeyPath(config?.private_key_path ?? "");
        if (config) {
          void jumpApi.jumpHostIdentityInspect(serverId)
            .then((report) => !cancelled && setIdentity(report))
            .catch((reason) => !cancelled && setError(String(reason)));
        }
      })
      .catch((reason) => !cancelled && setError(String(reason)))
      .finally(() => !cancelled && setBusy(false));
    return () => { cancelled = true; };
  }, [serverId]);

  async function save() {
    if (!serverId) return;
    setError(null);
    if (!enabled) {
      setBusy(true);
      try {
        await jumpApi.jumpHostDelete(serverId);
        setIdentity(null);
        pushAlert("info", `${selectedServer?.name ?? "Server"}: jump host disabled`, serverId);
      } catch (reason) {
        setError(String(reason));
      } finally {
        setBusy(false);
      }
      return;
    }
    if (!host.trim() || !username.trim() || !keyPath.trim()) {
      setError("Host, username and a private key are required for a jump host.");
      return;
    }
    if (!Number.isInteger(port) || port < 1 || port > 65535) {
      setError("Jump-host port must be between 1 and 65535.");
      return;
    }
    const config: JumpHostConfig = {
      server_id: serverId,
      host: host.trim(),
      port,
      username: username.trim(),
      private_key_path: keyPath.trim(),
    };
    setBusy(true);
    try {
      await jumpApi.jumpHostSave(config);
      setIdentity(await jumpApi.jumpHostIdentityInspect(serverId));
      pushAlert("info", `${selectedServer?.name ?? "Server"}: jump host saved`, serverId);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function trust(fingerprint: string, replace: boolean) {
    setBusy(true);
    setError(null);
    try {
      setIdentity(await jumpApi.jumpHostIdentityTrust(serverId, fingerprint, replace));
      pushAlert("info", `${selectedServer?.name ?? "Server"}: bastion fingerprint ${replace ? "replaced" : "trusted"}`, serverId);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function removeTrust() {
    setBusy(true);
    setError(null);
    try {
      await jumpApi.jumpHostIdentityRemove(serverId);
      setIdentity(await jumpApi.jumpHostIdentityInspect(serverId));
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="modal-backdrop" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
      <div className="modal wide" role="dialog" aria-modal="true" aria-label="Jump hosts">
        <div className="modal-head">
          <div><span className="eyebrow">SSH routing</span><strong>Jump hosts / Bastions</strong></div>
          <button className="ghost tiny" onClick={onClose}>✕</button>
        </div>
        <div className="modal-body">
          <div className="warn-banner">
            Bastions use SSH keys only. Both the bastion and destination fingerprints must be explicitly verified; RemoteOpsX never uses accept-new.
          </div>
          <div>
            <label>Server profile</label>
            <select value={serverId} onChange={(event) => setServerId(event.target.value)}>
              {servers.map((server) => <option key={server.id} value={server.id}>{server.name} · {server.host}</option>)}
            </select>
          </div>
          {!servers.length ? <div className="panel-hint">Create a server profile first.</div> : null}
          {serverId ? (
            <>
              <label className="settings-toggle">
                <span>Route this profile through a bastion</span>
                <input type="checkbox" checked={enabled} onChange={(event) => setEnabled(event.target.checked)} />
              </label>
              {enabled ? (
                <>
                  <div className="form-row three">
                    <div><label>Bastion host</label><input value={host} onChange={(event) => setHost(event.target.value)} placeholder="bastion.example.com" /></div>
                    <div><label>Username</label><input value={username} onChange={(event) => setUsername(event.target.value)} placeholder="ops" /></div>
                    <div><label>SSH port</label><input type="number" min={1} max={65535} value={port} onChange={(event) => setPort(Number(event.target.value))} /></div>
                  </div>
                  <div>
                    <label>Private key</label>
                    <div className="key-picker">
                      <select value={keys.some((key) => key.path === keyPath) ? keyPath : ""} onChange={(event) => setKeyPath(event.target.value)}>
                        <option value="">Choose from ~/.ssh</option>
                        {keys.map((key) => <option key={key.path} value={key.path}>{key.name}</option>)}
                      </select>
                      <input value={keyPath} onChange={(event) => setKeyPath(event.target.value)} placeholder="~/.ssh/id_ed25519" />
                    </div>
                  </div>
                  <div className="section-title">Bastion identity</div>
                  {identity?.status === "trusted" ? <div className="warn-banner ok">✓ Bastion identity explicitly trusted</div> : null}
                  {identity?.status === "unseen" ? <div className="warn-banner">Verify one fingerprint before using this route.</div> : null}
                  {identity?.status === "changed" ? <div className="warn-banner">⚠ Bastion identity changed. Verify out-of-band before Replace.</div> : null}
                  {identity ? (
                    <table className="data"><tbody>{identity.candidates.map((candidate) => (
                      <tr key={`${candidate.key_type}-${candidate.fingerprint}`}>
                        <td><div>{candidate.key_type}</div><div className="mono" style={{ fontSize: 10 }}>{candidate.fingerprint}</div></td>
                        <td>{identity.status === "unseen" ? <button className="tiny" disabled={busy} onClick={() => void trust(candidate.fingerprint, false)}>Trust</button> : identity.status === "changed" ? <button className="tiny" disabled={busy} onClick={() => void trust(candidate.fingerprint, true)}>Replace</button> : "verified"}</td>
                      </tr>
                    ))}</tbody></table>
                  ) : null}
                  {identity?.trusted_fingerprints.length ? <button className="tiny ghost" disabled={busy} onClick={() => void removeTrust()}>Remove stored bastion trust</button> : null}
                  <div className="panel-hint">After trusting the bastion, open the selected server’s Diagnostics panel to verify the destination fingerprint through the bastion.</div>
                </>
              ) : null}
            </>
          ) : null}
          {error ? <div className="error-text">{error}</div> : null}
        </div>
        <div className="modal-foot"><button onClick={onClose}>Close</button><button className="primary" disabled={busy || !serverId} onClick={() => void save()}>{busy ? "Saving…" : "Save routing"}</button></div>
      </div>
    </div>
  );
}
