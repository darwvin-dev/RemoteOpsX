import { useEffect, useState } from "react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import * as api from "../api";
import { useStore } from "../store";
import type { RemoteFile, Server } from "../types";

/** Remote file browser over SFTP/scp: list/upload/download/delete/rename. */
export function SftpPanel({ server, active }: { server: Server; active: boolean }) {
  const pushAlert = useStore((s) => s.pushAlert);
  const [path, setPath] = useState(`/home/${server.username}`);
  const [files, setFiles] = useState<RemoteFile[]>([]);
  const [busy, setBusy] = useState(false);
  const [loaded, setLoaded] = useState(false);

  async function list(p: string) {
    setBusy(true);
    try {
      const f = await api.sftpList(server.id, p);
      setFiles(f);
      setPath(p);
      setLoaded(true);
    } catch (err) {
      pushAlert("error", `list ${p}: ${err}`);
    } finally {
      setBusy(false);
    }
  }

  // Lazy first load when the tab becomes active.
  useEffect(() => { if (active && !loaded) void list(path); }, [active]); // eslint-disable-line react-hooks/exhaustive-deps

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
      await api.sftpUpload(server.id, picked, path);
      pushAlert("info", `Uploaded to ${path}`);
      await list(path);
    } catch (err) {
      pushAlert("error", `upload: ${err}`);
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
      await api.sftpDownload(server.id, join(path, f.name), localDir);
      pushAlert("info", `Downloaded ${f.name}`);
    } catch (err) {
      pushAlert("error", `download: ${err}`);
    } finally {
      setBusy(false);
    }
  }

  async function remove(f: RemoteFile) {
    if (!confirm(`Delete ${join(path, f.name)}?`)) return;
    try {
      await api.sftpDelete(server.id, join(path, f.name));
      await list(path);
    } catch (err) {
      pushAlert("error", `delete: ${err}`);
    }
  }

  async function rename(f: RemoteFile) {
    const to = prompt("Rename to:", f.name);
    if (!to || to === f.name) return;
    try {
      await api.sftpRename(server.id, join(path, f.name), join(path, to));
      await list(path);
    } catch (err) {
      pushAlert("error", `rename: ${err}`);
    }
  }

  return (
    <div className="sftp">
      <div className="sftp-bar">
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
