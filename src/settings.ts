import { RemoteOpsError } from "./errors";

export type Theme =
  | "system"
  | "dark"
  | "light"
  | "nord"
  | "dracula"
  | "tokyo_night"
  | "solarized_dark"
  | "solarized_light";

export type UiFont = "system" | "inter" | "ibm_plex_sans" | "noto_sans" | "ubuntu" | "roboto";

export type UiDensity = "compact" | "comfortable" | "spacious";

export type TerminalFont =
  | "jetbrains_mono"
  | "fira_code"
  | "cascadia_code"
  | "ibm_plex_mono"
  | "source_code_pro"
  | "dejavu_sans_mono"
  | "system_mono";

export type TerminalCursorStyle = "block" | "underline" | "bar";

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
  ui_font: UiFont;
  ui_density: UiDensity;
  terminal_font: TerminalFont;
  terminal_font_size: number;
  terminal_line_height_percent: number;
  terminal_cursor_style: TerminalCursorStyle;
  terminal_background_opacity_percent: number;
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
  ui_font: "system",
  ui_density: "comfortable",
  terminal_font: "jetbrains_mono",
  terminal_font_size: 13,
  terminal_line_height_percent: 100,
  terminal_cursor_style: "block",
  terminal_background_opacity_percent: 100,
  default_ports: Object.freeze({ ssh: 22, ftp: 21, rdp: 3389, vnc: 5900 }),
  health_refresh_interval_ms: 3000,
  history_retention_days: 90,
  app_lock_timeout_minutes: 15,
  transfer_conflict_policy: "ask",
  desktop_clipboard_enabled: true,
  desktop_audio_enabled: true,
  desktop_notifications_enabled: true,
});

export const THEMES: readonly Theme[] = [
  "system",
  "dark",
  "light",
  "nord",
  "dracula",
  "tokyo_night",
  "solarized_dark",
  "solarized_light",
];

export const UI_FONTS: readonly UiFont[] = [
  "system",
  "inter",
  "ibm_plex_sans",
  "noto_sans",
  "ubuntu",
  "roboto",
];

export const UI_DENSITIES: readonly UiDensity[] = ["compact", "comfortable", "spacious"];

export const TERMINAL_FONTS: readonly TerminalFont[] = [
  "jetbrains_mono",
  "fira_code",
  "cascadia_code",
  "ibm_plex_mono",
  "source_code_pro",
  "dejavu_sans_mono",
  "system_mono",
];

export const TERMINAL_CURSOR_STYLES: readonly TerminalCursorStyle[] = ["block", "underline", "bar"];

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
  if (!(THEMES as readonly unknown[]).includes(settings.theme)) {
    invalid("theme", `must be one of: ${THEMES.join(", ")}`);
  }
  if (!(UI_FONTS as readonly unknown[]).includes(settings.ui_font)) {
    invalid("ui_font", `must be one of: ${UI_FONTS.join(", ")}`);
  }
  if (!(UI_DENSITIES as readonly unknown[]).includes(settings.ui_density)) {
    invalid("ui_density", `must be one of: ${UI_DENSITIES.join(", ")}`);
  }
  if (!(TERMINAL_FONTS as readonly unknown[]).includes(settings.terminal_font)) {
    invalid("terminal_font", `must be one of: ${TERMINAL_FONTS.join(", ")}`);
  }
  if (!(TERMINAL_CURSOR_STYLES as readonly unknown[]).includes(settings.terminal_cursor_style)) {
    invalid("terminal_cursor_style", `must be one of: ${TERMINAL_CURSOR_STYLES.join(", ")}`);
  }
  validateInteger("terminal_font_size", settings.terminal_font_size, 10, 24);
  validateInteger("terminal_line_height_percent", settings.terminal_line_height_percent, 100, 200);
  validateInteger(
    "terminal_background_opacity_percent",
    settings.terminal_background_opacity_percent,
    55,
    100,
  );
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
