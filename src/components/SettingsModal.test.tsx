// @vitest-environment jsdom

import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_SETTINGS } from "../settings";
import { useSettingsStore } from "../settingsStore";
import { SettingsModal } from "./SettingsModal";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe("SettingsModal focus", () => {
  let container: HTMLDivElement;
  let root: ReturnType<typeof createRoot>;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
    const settings = { ...DEFAULT_SETTINGS, default_ports: { ...DEFAULT_SETTINGS.default_ports } };
    useSettingsStore.setState({
      settings,
      persisted: { ...settings, default_ports: { ...settings.default_ports } },
      loading: false,
      saving: false,
      dirty: false,
      error: null,
    });
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.restoreAllMocks();
  });

  it("keeps focus on a controlled input when editing makes settings dirty", () => {
    act(() => root.render(<SettingsModal onClose={vi.fn()} />));
    const input = container.querySelector<HTMLInputElement>("#settings-history");
    expect(input).not.toBeNull();

    input!.focus();
    act(() => {
      const valueSetter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
      valueSetter?.call(input, "30");
      input!.dispatchEvent(new Event("input", { bubbles: true }));
    });

    expect(useSettingsStore.getState().dirty).toBe(true);
    expect(document.activeElement).toBe(input);
  });

  it("confirms dirty settings before Escape or backdrop close", () => {
    const onClose = vi.fn();
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    act(() => root.render(<SettingsModal onClose={onClose} />));
    act(() => useSettingsStore.getState().patch({ theme: "dark" }));

    act(() => window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true })));
    expect(confirm).toHaveBeenCalledOnce();
    expect(onClose).not.toHaveBeenCalled();

    confirm.mockReturnValue(true);
    const backdrop = container.querySelector<HTMLElement>(".modal-backdrop");
    act(() => backdrop?.dispatchEvent(new MouseEvent("mousedown", { bubbles: true })));
    expect(confirm).toHaveBeenCalledTimes(2);
    expect(onClose).toHaveBeenCalledOnce();
  });
});
