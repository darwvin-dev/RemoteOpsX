import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./styles.css", import.meta.url), "utf8");

function tokens(selector: string): Record<string, string> {
  const selectorStart = css.indexOf(selector);
  const blockStart = css.indexOf("{", selectorStart);
  const blockEnd = css.indexOf("}", blockStart);
  const body = selectorStart >= 0 && blockStart >= 0 && blockEnd >= 0 ? css.slice(blockStart + 1, blockEnd) : "";
  return Object.fromEntries([...body.matchAll(/--([\w-]+):\s*(#[\da-fA-F]{6})/g)].map((match) => [match[1], match[2]]));
}

function luminance(hex: string): number {
  const channels = [1, 3, 5].map((offset) => Number.parseInt(hex.slice(offset, offset + 2), 16) / 255)
    .map((value) => value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4);
  return 0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2];
}

function contrast(foreground: string, background: string): number {
  const [lighter, darker] = [luminance(foreground), luminance(background)].sort((a, b) => b - a);
  return (lighter + 0.05) / (darker + 0.05);
}

describe("semantic theme tokens", () => {
  it("provides distinct light tokens with WCAG AA contrast on semantic surfaces", () => {
    const dark = tokens(":root[data-theme=\"dark\"]");
    const light = tokens(":root[data-theme=\"light\"]");

    for (const semantic of ["danger", "warning", "success"]) {
      expect(light[`${semantic}-text`]).not.toBe(dark[`${semantic}-text`]);
      expect(contrast(dark[`${semantic}-text`], dark[`${semantic}-surface`])).toBeGreaterThanOrEqual(4.5);
      expect(contrast(light[`${semantic}-text`], light[`${semantic}-surface`])).toBeGreaterThanOrEqual(4.5);
    }
    expect(contrast(dark["warning-on-solid"], dark.warn)).toBeGreaterThanOrEqual(4.5);
    expect(contrast(light["warning-on-solid"], light.warn)).toBeGreaterThanOrEqual(4.5);
  });
});
