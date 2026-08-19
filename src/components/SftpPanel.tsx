import { useEffect, useState } from "react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import * as api from "../api";
import * as operatorApi from "../operatorApi";
import { useStore } from "../store";
import type { RemoteFile, Server } from "../types";

type FileProtocol = "sftp" | "ftp";

/** Remote file browser over persistent SSH/SCP transfers or plain FTP/curl. */
export function SftpPanel({ server, active, protocol = "sftp" }: { server: Server; active: boolean; protocol?: FileProtocol }) {
  const pushAlert = useStore((s) => s.pushAlert);
  const [path, setPath] = useState(protocol === "ftp" ? "/" : server.username === "root" ? "/root" : `/home/${server.username}`);
  const [files, setFiles] = useState<RemoteFile[]>([]);
  const [busy, setBusy] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [dragActive, setDragActive] = useState(false);
  const label = protocol.toUpperCase();
  const commands = protocol === "ftp"
    ? { list: api.ftpList, upload: api.ftpUpload, download: api.ftpDownload, delete: api.ftpDelete, rename: api.ftpRename }
    : { list: api.sftpList, upload: api.sftpUpload, download: api.sftpDownload, delete: api.sftpDelete, rename: api.sftpRename };

  async function list(p: string) {
    setBusy(true);
    try {
      const f = await commands.list(server.id, p);
      setFiles(f);
      setPath(p);
      setLoaded(true);
    } catch (err) {
      pushAlert("error", `${label} list ${p}: ${err}`);
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => {
    if (active && !loaded) void list(path);
  }, [active, loaded, path]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (!active || protocol !== "sftp") return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void getCurrentWebview().onDragDropEvent((event) => {
      if (disposed) return;
      if (event.payload.type === "enter" || event.payload.type === "over") {
        setDragActive(true);
        return;
      }
      if (event.payload.type === "leave") {
        setDragActive(false);
        return;
      }
      if (event.payload.type === "drop") {
        setDragActive(false);
        const droppedPaths = event.payload.paths;
        void Promise.all(droppedPaths.map((source) => operatorApi.transferStart({
          server_id: server.id,
          direction: "upload",
          source,
          destination: path,
          recursive: true,
        }))).then((jobs) => {
          pushAlert("info", `${jobs.length} dropped item${jobs.length === 1 ? "" : "s"} queued for SFTP upload.`);
        }).catch((error) => pushAlert("error", `SFTP drag/drop: ${error}`));
      }
    }).then((cleanup) => {
      if (disposed) cleanup(); else unlisten = cleanup;
    }).catch((error) => pushAlert("error", `SFTP drag/drop listener: ${error}`));
    return () => {
      disposed = true;
      setDragActive(false);
      unlisten?.();
    };
  }, [active, path, protocol, pushAlert, server.id]);

  function join(dir: string, name: string) {
    if (name === "..") {
      const parts = dir.replace(/\/+$/, "").split("/");
      parts.pop();
      return parts.join("/") || "/";
    }
    return `${dir.replace(/\/+$/, "")}/${name}`;
  }

  async function upload(recursive = false) {
    const picked = await openDialog({ multiple: false, directory: recursive });
    if (!picked || Array.isArray(picked)) return;
    setBusy(true);
    try {
      if (protocol === "sftp") {
        const job = await operatorApi.transferStart({
          server_id: server.id,
          direction: "upload",
          source: picked,
          destination: path,
          recursive,
        });
        pushAlert("info", `${label} transfer queued (${job.id.slice(0, 8)}). Track it in Operator Center → Transfers.`);
      } else {
        if (recursive) throw new Error("Recursive folder upload is available for SFTP only.");
        await commands.upload(server.id, picked, path);
        pushAlert("info", `${label} uploaded to ${path}`);
        await list(path);
      }
    } catch (err) {
      pushAlert("error", `${label} upload: ${err}`);
    } finally {
      setBusy(false);
    }
  }

  async function download(f: RemoteFile) {
    let dest: string | null = null;
    if (f.is_dir) {
      const picked = await openDialog({ directory: true, multiple: false });
      dest = typeof picked === "string" ? picked : null;
    } else {
      dest = await saveDialog({ defaultPath: f.name });
    }
    if (!dest) return;
    setBusy(true);
    try {
      if (protocol === "sftp") {
        const job = await operatorApi.transferStart({
          server_id: server.id,
          direction: "download",
          source: join(path, f.name),
          destination: dest,
          recursive: f.is_dir,
        });
        pushAlert("info", `${label} transfer queued (${job.id.slice(0, 8)}). Track it in Operator Center → Transfers.`);
      } else {
        if (f.is_dir) throw new Error("Recursive folder download is available for SFTP only.");
        await commands.download(server.id, join(path, f.name), dest);
        pushAlert("info", `${label} downloaded ${f.name} to ${dest}`);
      }
    } catch (err) {
      pushAlert("error", `${label} download: ${err}`);
    } finally {
      setBusy(false);
    }
  }

  async function remove(f: RemoteFile) {
    if (!confirm(`Delete ${join(path, f.name)}?`)) return;
    try {
      await commands.delete(server.id, join(path, f.name));
      await list(path);
    } catch (err) {
      pushAlert("error", `${label} delete: ${err}`);
    }
  }

  async function rename(f: RemoteFile) {
    const to = prompt("Rename to:", f.name);
    if (!to || to === f.name) return;
    try {
      await commands.rename(server.id, join(path, f.name), join(path, to));
      await list(path);
    } catch (err) {
      pushAlert("error", `${label} rename: ${err}`);
    }
  }

  async function chmod(f: RemoteFile) {
    if (protocol !== "sftp") return;
    const mode = prompt("chmod mode (for example 644 or 0755):", f.is_dir ? "755" : "644");
    if (!mode) return;
    try {
      await operatorApi.transferChmod(server.id, join(path, f.name), mode);
      pushAlert("info", `chmod ${mode} applied to ${f.name}`);
      await list(path);
    } catch (err) {
      pushAlert("error", `chmod ${f.name}: ${err}`);
    }
  }

  return (
    <div className={`sftp${dragActive ? " drag-active" : ""}`}>
      <div className="sftp-bar">
        <span className={`pill ${protocol}`}>{label}</span>
        {protocol === "ftp" && <span className="status-badge status-warn" title="FTP traffic is not encrypted">plaintext</span>}
        {protocol === "sftp" && <span className="status-badge status-ok" title="Transfers reuse a persistent SSH ControlMaster">persistent</span>}
        {protocol === "sftp" && dragActive && <span className="status-badge status-ok">drop to upload</span>}
        <button className="tiny" onClick={() => void list(join(path, ".."))}>↑ Up</button>
        <input className="sftp-path" value={path} onChange={(e) => setPath(e.target.value)} onKeyDown={(e) => e.key === "Enter" && void list(path)} />
        <button className="tiny" disabled={busy} onClick={() => void list(path)}>Go</button>
        <button className="tiny primary" disabled={busy} onClick={() => void upload(false)}>⬆ Upload</button>
        {protocol === "sftp" && <button className="tiny" disabled={busy} onClick={() => void upload(true)}>⬆ Folder</button>}
      </div>
      <div className="sftp-list">
        <div className="file-row" style={{ color: "var(--text-2)" }}>
          <span className="fperm">drwx</span>
          <button type="button" className="fname file-name-button dir" onClick={() => void list(join(path, ".."))}>..</button>
        </div>
        {files.map((f) => (
          <div key={f.name} className={`file-row${f.is_dir ? " dir" : ""}`}>
            <span className="fperm">{f.permissions}</span>
            <button type="button" className="fname file-name-button" disabled={!f.is_dir} onClick={() => f.is_dir && void list(join(path, f.name))} title={f.name}>
              {f.is_dir ? "📁 " : "📄 "}{f.name}
            </button>
            <span className="fsize">{f.is_dir ? "" : fmtSize(f.size)}</span>
            {(protocol === "sftp" || !f.is_dir) && <button className="tiny ghost" aria-label={`Download ${f.name}`} onClick={() => void download(f)}>⬇</button>}
            {protocol === "sftp" && <button className="tiny ghost" aria-label={`Change permissions for ${f.name}`} onClick={() => void chmod(f)}>chmod</button>}
            <button className="tiny ghost" aria-label={`Rename ${f.name}`} onClick={() => void rename(f)}>✎</button>
            <button className="tiny ghost danger-ghost" aria-label={`Delete ${f.name}`} onClick={() => void remove(f)}>🗑</button>
          </div>
        ))}
        {busy && <div className="panel-hint">Working…</div>}
      </div>
    </div>
  );
}

function fmtSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
}