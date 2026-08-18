import { describe, expect, it } from "vitest";
import {
  TERMINAL_CURSOR_OPTIONS,
  TERMINAL_FONT_OPTIONS,
  THEME_OPTIONS,
  UI_DENSITY_OPTIONS,
  UI_FONT_OPTIONS,
  resolveTheme,
  resolveThemePreset,
  terminalFontStack,
  terminalTheme,
  uiFontStack,
} from "./theme";

describe("appearance presets", () => {
  it("resolves system mode to the matching base palette", () => {
    expect(resolveTheme("system", true)).toBe("dark");
    expect(resolveThemePreset("system", true)).toBe("dark");
    expect(resolveTheme("system", false)).toBe("light");
    expect(resolveThemePreset("system", false)).toBe("light");
  });

  it("marks only light presets as light mode", () => {
    expect(resolveTheme("light", true)).toBe("light");
    expect(resolveTheme("solarized_light", true)).toBe("light");
    for (const theme of ["dark", "nord", "dracula", "tokyo_night", "solarized_dark"] as const) {
      expect(resolveTheme(theme, false)).toBe("dark");
    }
  });

  it("ships complete appearance choice sets without duplicate ids", () => {
    expect(THEME_OPTIONS).toHaveLength(8);
    expect(new Set(THEME_OPTIONS.map((option) => option.value)).size).toBe(THEME_OPTIONS.length);
    expect(UI_FONT_OPTIONS.length).toBeGreaterThanOrEqual(6);
    expect(TERMINAL_FONT_OPTIONS.length).toBeGreaterThanOrEqual(7);
    expect(UI_DENSITY_OPTIONS.map((option) => option.value)).toEqual(["compact", "comfortable", "spacious"]);
    expect(TERMINAL_CURSOR_OPTIONS.map((option) => option.value)).toEqual(["block", "underline", "bar"]);
  });

  it("returns deterministic font stacks with safe fallbacks", () => {
    expect(uiFontStack("ibm_plex_sans")).toContain("IBM Plex Sans");
    expect(uiFontStack("ibm_plex_sans")).toContain("sans-serif");
    expect(terminalFontStack("fira_code")).toContain("Fira Code");
    expect(terminalFontStack("fira_code")).toContain("monospace");
  });

  it("provides terminal colors for every resolved theme", () => {
    for (const option of THEME_OPTIONS) {
      const palette = terminalTheme(option.value, true);
      expect(palette.background).toMatch(/^#[0-9a-f]{6}$/i);
      expect(palette.foreground).toMatch(/^#[0-9a-f]{6}$/i);
      expect(palette.cursor).toMatch(/^#[0-9a-f]{6}$/i);
    }
    expect(terminalTheme("dracula", true).background).not.toBe(terminalTheme("nord", true).background);
  });

  it("keeps opaque terminal colors stable and converts lower opacity to rgba", () => {
    expect(terminalTheme("dark", true, 100).background).toBe("#05080d");
    expect(terminalTheme("dark", true, 80).background).toBe("rgba(5, 8, 13, 0.8)");
    expect(terminalTheme("solarized_light", false, 55).background).toBe("rgba(253, 246, 227, 0.55)");
  });
});
