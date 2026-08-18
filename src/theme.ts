import type { TerminalCursorStyle, TerminalFont, Theme, UiDensity, UiFont } from "./settings";

export const SYSTEM_THEME_QUERY = "(prefers-color-scheme: dark)";

export type ResolvedTheme = Exclude<Theme, "system">;
export type ThemeMode = "dark" | "light";

export interface ThemeOption {
  value: Theme;
  label: string;
  description: string;
  mode: "system" | ThemeMode;
}

export interface FontOption<T extends string> {
  value: T;
  label: string;
  stack: string;
}

export const THEME_OPTIONS: readonly ThemeOption[] = [
  { value: "system", label: "Follow system", description: "Use Paper or Obsidian with the OS color scheme.", mode: "system" },
  { value: "dark", label: "Obsidian", description: "RemoteOpsX's dense teal-and-blue dark workspace.", mode: "dark" },
  { value: "light", label: "Paper", description: "Clean light workspace with strong operational contrast.", mode: "light" },
  { value: "nord", label: "Nord", description: "Cool arctic blues with restrained status colors.", mode: "dark" },
  { value: "dracula", label: "Dracula", description: "Deep violet surfaces with bright terminal accents.", mode: "dark" },
  { value: "tokyo_night", label: "Tokyo Night", description: "Indigo-night palette with crisp blue highlights.", mode: "dark" },
  { value: "solarized_dark", label: "Solarized Dark", description: "Low-contrast blue-green palette for long sessions.", mode: "dark" },
  { value: "solarized_light", label: "Solarized Light", description: "Warm low-glare light palette.", mode: "light" },
];

export const UI_FONT_OPTIONS: readonly FontOption<UiFont>[] = [
  { value: "system", label: "System UI", stack: 'system-ui, -apple-system, "Segoe UI", sans-serif' },
  { value: "inter", label: "Inter", stack: '"Inter", system-ui, -apple-system, "Segoe UI", sans-serif' },
  { value: "ibm_plex_sans", label: "IBM Plex Sans", stack: '"IBM Plex Sans", "Noto Sans", system-ui, sans-serif' },
  { value: "noto_sans", label: "Noto Sans", stack: '"Noto Sans", system-ui, sans-serif' },
  { value: "ubuntu", label: "Ubuntu", stack: '"Ubuntu", "Noto Sans", system-ui, sans-serif' },
  { value: "roboto", label: "Roboto", stack: '"Roboto", "Noto Sans", system-ui, sans-serif' },
];

export const UI_DENSITY_OPTIONS: readonly { value: UiDensity; label: string; description: string }[] = [
  { value: "compact", label: "Compact", description: "More servers and controls on screen." },
  { value: "comfortable", label: "Comfortable", description: "Balanced default spacing." },
  { value: "spacious", label: "Spacious", description: "Larger touch targets and breathing room." },
];

export const TERMINAL_FONT_OPTIONS: readonly FontOption<TerminalFont>[] = [
  { value: "jetbrains_mono", label: "JetBrains Mono", stack: '"JetBrains Mono", "DejaVu Sans Mono", ui-monospace, monospace' },
  { value: "fira_code", label: "Fira Code", stack: '"Fira Code", "DejaVu Sans Mono", ui-monospace, monospace' },
  { value: "cascadia_code", label: "Cascadia Code", stack: '"Cascadia Code", "Cascadia Mono", "DejaVu Sans Mono", ui-monospace, monospace' },
  { value: "ibm_plex_mono", label: "IBM Plex Mono", stack: '"IBM Plex Mono", "DejaVu Sans Mono", ui-monospace, monospace' },
  { value: "source_code_pro", label: "Source Code Pro", stack: '"Source Code Pro", "DejaVu Sans Mono", ui-monospace, monospace' },
  { value: "dejavu_sans_mono", label: "DejaVu Sans Mono", stack: '"DejaVu Sans Mono", ui-monospace, monospace' },
  { value: "system_mono", label: "System monospace", stack: 'ui-monospace, "SFMono-Regular", Menlo, Monaco, Consolas, monospace' },
];

export const TERMINAL_CURSOR_OPTIONS: readonly { value: TerminalCursorStyle; label: string }[] = [
  { value: "block", label: "Block" },
  { value: "underline", label: "Underline" },
  { value: "bar", label: "Bar" },
];

const LIGHT_THEMES = new Set<ResolvedTheme>(["light", "solarized_light"]);

export function resolveTheme(theme: Theme, systemPrefersDark: boolean): ThemeMode {
  if (theme === "system") return systemPrefersDark ? "dark" : "light";
  return LIGHT_THEMES.has(theme) ? "light" : "dark";
}

export function resolveThemePreset(theme: Theme, systemPrefersDark: boolean): ResolvedTheme {
  if (theme === "system") return systemPrefersDark ? "dark" : "light";
  return theme;
}

export function uiFontStack(font: UiFont): string {
  return UI_FONT_OPTIONS.find((option) => option.value === font)?.stack ?? UI_FONT_OPTIONS[0].stack;
}

export function terminalFontStack(font: TerminalFont): string {
  return TERMINAL_FONT_OPTIONS.find((option) => option.value === font)?.stack ?? TERMINAL_FONT_OPTIONS[0].stack;
}

export interface TerminalTheme {
  background: string;
  foreground: string;
  cursor: string;
  selectionBackground: string;
  black: string;
  red: string;
  green: string;
  yellow: string;
  blue: string;
  magenta: string;
  cyan: string;
  white: string;
  brightBlack?: string;
  brightRed?: string;
  brightGreen?: string;
  brightYellow?: string;
  brightBlue?: string;
  brightMagenta?: string;
  brightCyan?: string;
  brightWhite?: string;
}

const TERMINAL_THEMES: Record<ResolvedTheme, TerminalTheme> = {
  dark: { background: "#05080d", foreground: "#c9d4e0", cursor: "#2dd4bf", selectionBackground: "#1d4e73", black: "#0a0e14", red: "#f85149", green: "#3fb950", yellow: "#d29922", blue: "#4aa8ff", magenta: "#a371f7", cyan: "#2dd4bf", white: "#e6edf3" },
  light: { background: "#f8fafc", foreground: "#263548", cursor: "#087f75", selectionBackground: "#c7ece8", black: "#263548", red: "#c52f2a", green: "#18823b", yellow: "#9a6500", blue: "#176fba", magenta: "#7048bd", cyan: "#087f75", white: "#d5e0ec" },
  nord: { background: "#2e3440", foreground: "#d8dee9", cursor: "#88c0d0", selectionBackground: "#434c5e", black: "#3b4252", red: "#bf616a", green: "#a3be8c", yellow: "#ebcb8b", blue: "#81a1c1", magenta: "#b48ead", cyan: "#88c0d0", white: "#e5e9f0", brightBlack: "#4c566a", brightWhite: "#eceff4" },
  dracula: { background: "#282a36", foreground: "#f8f8f2", cursor: "#f8f8f2", selectionBackground: "#44475a", black: "#21222c", red: "#ff5555", green: "#50fa7b", yellow: "#f1fa8c", blue: "#6272a4", magenta: "#ff79c6", cyan: "#8be9fd", white: "#f8f8f2", brightBlack: "#6272a4", brightWhite: "#ffffff" },
  tokyo_night: { background: "#1a1b26", foreground: "#c0caf5", cursor: "#7aa2f7", selectionBackground: "#33467c", black: "#15161e", red: "#f7768e", green: "#9ece6a", yellow: "#e0af68", blue: "#7aa2f7", magenta: "#bb9af7", cyan: "#7dcfff", white: "#a9b1d6", brightBlack: "#414868", brightWhite: "#c0caf5" },
  solarized_dark: { background: "#002b36", foreground: "#839496", cursor: "#93a1a1", selectionBackground: "#073642", black: "#073642", red: "#dc322f", green: "#859900", yellow: "#b58900", blue: "#268bd2", magenta: "#d33682", cyan: "#2aa198", white: "#eee8d5", brightBlack: "#586e75", brightWhite: "#fdf6e3" },
  solarized_light: { background: "#fdf6e3", foreground: "#657b83", cursor: "#586e75", selectionBackground: "#eee8d5", black: "#073642", red: "#dc322f", green: "#859900", yellow: "#b58900", blue: "#268bd2", magenta: "#d33682", cyan: "#2aa198", white: "#eee8d5", brightBlack: "#839496", brightWhite: "#fdf6e3" },
};

function hexWithOpacity(hex: string, opacityPercent: number): string {
  const match = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex);
  if (!match) return hex;
  const alpha = Math.min(100, Math.max(0, opacityPercent)) / 100;
  return `rgba(${parseInt(match[1], 16)}, ${parseInt(match[2], 16)}, ${parseInt(match[3], 16)}, ${alpha})`;
}

export function terminalTheme(theme: Theme, systemPrefersDark: boolean, opacityPercent = 100): TerminalTheme {
  const palette = TERMINAL_THEMES[resolveThemePreset(theme, systemPrefersDark)];
  if (opacityPercent >= 100) return palette;
  return { ...palette, background: hexWithOpacity(palette.background, opacityPercent) };
}

export function bootstrapSystemTheme(
  matchMedia: (query: string) => MediaQueryList = window.matchMedia.bind(window),
): ThemeMode {
  const prefersDark = matchMedia(SYSTEM_THEME_QUERY).matches;
  const mode = resolveTheme("system", prefersDark);
  document.documentElement.dataset.theme = mode;
  document.documentElement.dataset.themePreset = resolveThemePreset("system", prefersDark);
  document.documentElement.dataset.uiFont = "system";
  document.documentElement.dataset.density = "comfortable";
  return mode;
}
