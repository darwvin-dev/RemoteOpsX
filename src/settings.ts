import { RemoteOpsError } from "./errors";

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

export type DeepReadonly<T> = T extends object
  ? { readonly [Key in keyof T]: DeepReadonly<T[Key]> }
  : T;

export const DEFAULT_SETTINGS: DeepReadonly<AppSettings> = Object.freeze({
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

export function patchSettings(current: DeepReadonly<AppSettings>, patch: SettingsPatch): AppSettings {
  return {
    ...current,
    ...patch,
    default_ports: {
      ...current.default_ports,
      ...patch.default_ports,
    },
  };
}

function invalid(field: string, message: string): never {
  throw new RemoteOpsError(message, "validation.invalid_value", false, null, { field });
}

function validateInteger(field: string, value: number, minimum: number, maximum: number): void {
  if (!Number.isInteger(value) || value < minimum || value > maximum) {
    invalid(field, `must be an integer between ${minimum} and ${maximum}`);
  }
}

export function validateSettings(settings: AppSettings): void {
  if (settings.schema_version !== 1) {
    invalid("schema_version", "unsupported settings schema version; supported schema version is 1");
  }
  if (!(["system", "dark", "light"] as unknown[]).includes(settings.theme)) {
    invalid("theme", "must be system, dark, or light");
  }
  if (!(["ask", "overwrite", "rename", "skip"] as unknown[]).includes(settings.transfer_conflict_policy)) {
    invalid("transfer_conflict_policy", "must be ask, overwrite, rename, or skip");
  }

  const ports = settings.default_ports as DefaultPorts | null | undefined;
  if (typeof ports !== "object" || ports === null) {
    invalid("default_ports", "must be an object");
  }
  for (const protocol of ["ssh", "ftp", "rdp", "vnc"] as const) {
    validateInteger(`default_ports.${protocol}`, ports[protocol], 1, 65_535);
  }

  validateInteger("health_refresh_interval_ms", settings.health_refresh_interval_ms, 1000, 60_000);
  validateInteger("history_retention_days", settings.history_retention_days, 1, 3650);
  validateInteger("app_lock_timeout_minutes", settings.app_lock_timeout_minutes, 1, 1440);

  for (const field of [
    "desktop_clipboard_enabled",
    "desktop_audio_enabled",
    "desktop_notifications_enabled",
  ] as const) {
    if (typeof settings[field] !== "boolean") {
      invalid(field, "must be a boolean");
    }
  }
}
