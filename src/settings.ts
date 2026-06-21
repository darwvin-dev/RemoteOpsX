export type Theme = "system" | "dark" | "light";

export type TransferConflictPolicy = "ask" | "overwrite" | "rename" | "skip";

export interface DefaultPorts {
  ssh: number;
  ftp: number;
  rdp: number;
  vnc: number;
}

export interface AppSettings {
  schema_version: number;
  theme: Theme;
  default_ports: DefaultPorts;
  health_refresh_interval_ms: number;
  history_retention_days: number;
  app_lock_timeout_minutes: number;
  transfer_conflict_policy: TransferConflictPolicy;
  desktop_clipboard_enabled: boolean;
  desktop_audio_enabled: boolean;
  desktop_notifications_enabled: boolean;
}

export type SettingsPatch = Omit<Partial<AppSettings>, "default_ports"> & {
  default_ports?: Partial<DefaultPorts>;
};

export const DEFAULT_SETTINGS: Readonly<AppSettings> = Object.freeze({
  schema_version: 1,
  theme: "system",
  default_ports: Object.freeze({ ssh: 22, ftp: 21, rdp: 3389, vnc: 5900 }),
  health_refresh_interval_ms: 3000,
  history_retention_days: 90,
  app_lock_timeout_minutes: 15,
  transfer_conflict_policy: "ask",
  desktop_clipboard_enabled: true,
  desktop_audio_enabled: true,
  desktop_notifications_enabled: true,
});

export function patchSettings(current: AppSettings, patch: SettingsPatch): AppSettings {
  return {
    ...current,
    ...patch,
    default_ports: {
      ...current.default_ports,
      ...patch.default_ports,
    },
  };
}
