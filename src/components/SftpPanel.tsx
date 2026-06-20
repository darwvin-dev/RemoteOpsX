import { useEffect, useState } from "react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import * as api from "../api";
import { useStore } from "../store";
import type { RemoteFile, Server } from "../types";

type FileProtocol = "sftp" | "ftp";

/** Remote file browser over SFTP/scp or plain FTP/curl. */
export function SftpPanel({ server, active, protocol = "sftp" }: { server: Server; active: boolean; protocol?: FileProtocol }) {
  const pushAlert = useStore((s) => s.pushAlert);
  const [path, setPath] = useState(protocol === "ftp" ? "/" : `/home/${server.username}`);
  const [files, setFiles] = useState<RemoteFile[]>([]);
  const [busy, setBusy] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const label = protocol.toUpperCase();
  const commands = protocol === "ftp"
    ? {
        list: api.ftpList,
        upload: api.ftpUpload,
        download: api.ftpDownload,
        delete: api.ftpDelete,
        rename: api.ftpRename,
      }
    : {
        list: api.sftpList,
        upload: api.sftpUpload,
        download: api.sftpDownload,
        delete: api.sftpDelete,
        rename: api.sftpRename,
      };

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

  // Lazy first load when the tab becomes active.
  useEffect(() => {
    if (active && !loaded) void list(path);
  }, [active, loaded, path]); // eslint-disable-line react-hooks/exhaustive-deps

  function join(dir: string, name: string) {
    if (name === "..") {
      const parts = dir.replace(/\/+$/, "").split("/");
      parts.pop();
      return parts.join("/") || "/";
    }
    return `${dir.replace(/\/+$/, "")}/${name}`;
  }

  async function upload() {
    const picked = await openDialog({ multiple: false });
    if (!picked || Array.isArray(picked)) return;
    setBusy(true);
    try {
      await commands.upload(server.id, picked, path);
      pushAlert("info", `${label} uploaded to ${path}`);
      await list(path);
    } catch (err) {
      pushAlert("error", `${label} upload: ${err}`);
    } finally {
      setBusy(false);
    }
  }

  async function download(f: RemoteFile) {
    const dest = await saveDialog({ defaultPath: f.name });
    if (!dest) return;
    const localDir = dest.split("/").slice(0, -1).join("/") || "/";
    setBusy(true);
    try {
      await commands.download(server.id, join(path, f.name), localDir);
      pushAlert("info", `${label} downloaded ${f.name}`);
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

  return (
    <div className="sftp">
      <div className="sftp-bar">
        <span className={`pill ${protocol}`}>{label}</span>
        <button className="tiny" onClick={() => void list(join(path, ".."))}>↑ Up</button>
        <input
          className="sftp-path"
          value={path}
          onChange={(e) => setPath(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && void list(path)}
        />
        <button className="tiny" disabled={busy} onClick={() => void list(path)}>Go</button>
        <button className="tiny primary" disabled={busy} onClick={() => void upload()}>⬆ Upload</button>
      </div>
      <div className="sftp-list">
        <div className="file-row" style={{ color: "var(--text-2)" }} onClick={() => void list(join(path, ".."))}>
          <span className="fperm">drwx</span>
          <span className="fname dir">..</span>
        </div>
        {files.map((f) => (
          <div key={f.name} className={`file-row${f.is_dir ? " dir" : ""}`}>
            <span className="fperm">{f.permissions}</span>
            <span className="fname" onClick={() => f.is_dir && void list(join(path, f.name))}>
              {f.is_dir ? "📁 " : "📄 "}{f.name}
            </span>
            <span className="fsize">{f.is_dir ? "" : fmtSize(f.size)}</span>
            {!f.is_dir && <button className="tiny ghost" onClick={() => void download(f)}>⬇</button>}
            <button className="tiny ghost" onClick={() => void rename(f)}>✎</button>
            <button className="tiny ghost" onClick={() => void remove(f)}>🗑</button>
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
