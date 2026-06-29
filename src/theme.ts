import type { Theme } from "./settings";

export const SYSTEM_THEME_QUERY = "(prefers-color-scheme: dark)";

export function resolveTheme(theme: Theme, systemPrefersDark: boolean): "dark" | "light" {
  return theme === "system" ? (systemPrefersDark ? "dark" : "light") : theme;
}

export function bootstrapSystemTheme(
  matchMedia: (query: string) => MediaQueryList = window.matchMedia.bind(window),
): "dark" | "light" {
  const theme = resolveTheme("system", matchMedia(SYSTEM_THEME_QUERY).matches);
  document.documentElement.dataset.theme = theme;
  return theme;
}
