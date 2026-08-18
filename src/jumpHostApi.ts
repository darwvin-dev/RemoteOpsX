import { invoke } from "@tauri-apps/api/core";
import { normalizeRemoteError } from "./errors";
import type { HostIdentityReport, JumpHostConfig } from "./types";

async function call<T>(command: string, payload?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, payload);
  } catch (error) {
    throw normalizeRemoteError(error);
  }
}

export const jumpHostGet = (serverId: string) =>
  call<JumpHostConfig | null>("jump_host_get", { serverId });

export const jumpHostSave = (config: JumpHostConfig) =>
  call<JumpHostConfig>("jump_host_save", { config });

export const jumpHostDelete = (serverId: string) =>
  call<void>("jump_host_delete", { serverId });

export const jumpHostIdentityInspect = (serverId: string) =>
  call<HostIdentityReport>("ssh_jump_host_identity_inspect", { serverId });

export const jumpHostIdentityTrust = (serverId: string, expectedFingerprint: string, replace: boolean) =>
  call<HostIdentityReport>("ssh_jump_host_identity_trust", {
    serverId,
    expectedFingerprint,
    replace,
  });

export const jumpHostIdentityRemove = (serverId: string) =>
  call<void>("ssh_jump_host_identity_remove", { serverId });
