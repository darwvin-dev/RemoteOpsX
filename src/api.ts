// Typed wrappers around Tauri commands. One function per backend command so
// components never touch `invoke` strings directly.

import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { normalizeRemoteError } from "./errors";
import { validateSettings } from "./settings";
import type { AppSettings } from "./settings";
import type {
  CommandOutput,
  HealthSnapshot,
  RemoteFile,
  Runbook,
  RunbookRun,
  RunbookSpec,
  RunbookStep,
  Server,
  ServerInput,
  StepResult,
  Tunnel,
} from "./types";

async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await tauriInvoke<T>(command, args);
  } catch (error) {
    throw normalizeRemoteError(error);
  }
}

// ---- Settings ----
export const settingsGet = () => invoke<AppSettings>("settings_get");
export const settingsSave = async (settings: AppSettings) => {
  validateSettings(settings);
  return invoke<AppSettings>("settings_save", { settings });
};

// ---- Servers ----
export const serversList = () => invoke<Server[]>("servers_list");
export const serverGet = (id: string) => invoke<Server>("server_get", { id });
export const serverSave = (input: ServerInput) => invoke<string>("server_save", { input });
export const serverDelete = (id: string) => invoke<void>("server_delete", { id });

// ---- SSH PTY ----
export const ptySpawn = (sessionId: string, serverId: string, cols: number, rows: number) =>
  invoke<void>("pty_spawn", { sessionId, serverId, cols, rows });
export const ptyWrite = (sessionId: string, data: number[]) => invoke<void>("pty_write", { sessionId, data });
export const ptyResize = (sessionId: string, cols: number, rows: number) =>
  invoke<void>("pty_resize", { sessionId, cols, rows });
export const ptyClose = (sessionId: string) => invoke<void>("pty_close", { sessionId });

// ---- Health ----
export const healthCollect = (serverId: string) => invoke<HealthSnapshot>("health_collect", { serverId });

// ---- Generic remote exec ----
export const runRemote = (serverId: string, command: string) =>
  invoke<CommandOutput>("run_remote", { serverId, command });

// ---- Runbooks ----
export const runbooksList = () => invoke<Runbook[]>("runbooks_list");
export const runbookGet = (id: string) => invoke<Runbook>("runbook_get", { id });
export const runbookSpec = (id: string) => invoke<RunbookSpec>("runbook_spec", { id });
export const runbookSave = (name: string, description: string, contentYaml: string, id?: string) =>
  invoke<string>("runbook_save", { id: id ?? null, name, description, contentYaml });
export const runbookRunStep = (serverId: string, step: RunbookStep) =>
  invoke<StepResult>("runbook_run_step", { serverId, step });
export const runbookRecordRun = (
  runbookId: string,
  serverId: string,
  startedAt: string,
  status: string,
  results: StepResult[],
) => invoke<string>("runbook_record_run", { runbookId, serverId, startedAt, status, results });
export const runbookRunsList = (limit = 50) => invoke<RunbookRun[]>("runbook_runs_list", { limit });

// ---- Services ----
export const serviceAction = (serverId: string, action: string, unit: string) =>
  invoke<CommandOutput>("service_action", { serverId, action, unit });

// ---- SFTP ----
export const sftpList = (serverId: string, path: string) => invoke<RemoteFile[]>("sftp_list", { serverId, path });
export const sftpUpload = (serverId: string, localPath: string, remoteDir: string) =>
  invoke<void>("sftp_upload", { serverId, localPath, remoteDir });
export const sftpDownload = (serverId: string, remotePath: string, localPath: string) =>
  invoke<void>("sftp_download", { serverId, remotePath, localPath });
export const sftpDelete = (serverId: string, remotePath: string) => invoke<void>("sftp_delete", { serverId, remotePath });
export const sftpRename = (serverId: string, from: string, to: string) =>
  invoke<void>("sftp_rename", { serverId, from, to });

// ---- FTP ----
export const ftpList = (serverId: string, path: string) => invoke<RemoteFile[]>("ftp_list", { serverId, path });
export const ftpUpload = (serverId: string, localPath: string, remoteDir: string) =>
  invoke<void>("ftp_upload", { serverId, localPath, remoteDir });
export const ftpDownload = (serverId: string, remotePath: string, localPath: string) =>
  invoke<void>("ftp_download", { serverId, remotePath, localPath });
export const ftpDelete = (serverId: string, remotePath: string) => invoke<void>("ftp_delete", { serverId, remotePath });
export const ftpRename = (serverId: string, from: string, to: string) =>
  invoke<void>("ftp_rename", { serverId, from, to });

// ---- Remote desktop ----
export const rdpLaunch = (serverId: string, options: { fullscreen: boolean; resolution?: string | null }) =>
  invoke<void>("rdp_launch", { serverId, options });
export const vncLaunch = (serverId: string, options: { fullscreen: boolean }) =>
  invoke<void>("vnc_launch", { serverId, options });

// ---- Tunnels ----
export const tunnelStart = (tunnel: Partial<Tunnel> & { server_id: string; type: string; local_port: number }) =>
  invoke<Tunnel>("tunnel_start", { tunnel });
export const tunnelStop = (id: string) => invoke<void>("tunnel_stop", { id });
export const tunnelsList = () => invoke<Tunnel[]>("tunnels_list");

// ---- Local file save (logs / diagnostic bundle) ----
export const saveTextFile = (path: string, content: string) => invoke<void>("save_text_file", { path, content });
