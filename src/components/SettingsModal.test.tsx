// @vitest-environment jsdom

import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_SETTINGS } from "../settings";
import { useSettingsStore } from "../settingsStore";
import { SettingsModal } from "./SettingsModal";

const api = vi.hoisted(() => ({
  settingsGet: vi.fn(),
  settingsSave: vi.fn(),
}));

vi.mock("../api", () => api);

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe("SettingsModal focus", () => {
  let container: HTMLDivElement;
  let root: ReturnType<typeof createRoot>;

  beforeEach(() => {
    api.settingsSave.mockReset();
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

  function setInput(input: HTMLInputElement, value: string) {
    const valueSetter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
    valueSetter?.call(input, value);
    input.dispatchEvent(new Event("input", { bubbles: true }));
  }

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.restoreAllMocks();
  });

  it("keeps focus on a controlled input when editing makes settings dirty", () => {
    act(() => root.render(<SettingsModal onClose={vi.fn()} />));
    const input = container.querySelector<HTMLInputElement>("#settings-notifications");
    expect(input).not.toBeNull();

    input!.focus();
    act(() => input!.click());

    expect(useSettingsStore.getState().dirty).toBe(true);
    expect(document.activeElement).toBe(input);
  });

  it("keeps blank numeric drafts without corrupting settings", () => {
    act(() => root.render(<SettingsModal onClose={vi.fn()} />));
    const input = container.querySelector<HTMLInputElement>("#settings-history")!;
    act(() => setInput(input, ""));

    expect(input.value).toBe("");
    expect(useSettingsStore.getState().settings.history_retention_days).toBe(90);

    input.focus();
    act(() => input.blur());
    expect(input.getAttribute("aria-invalid")).toBe("true");
    expect(useSettingsStore.getState().persisted.history_retention_days).toBe(90);

    const discard = [...container.querySelectorAll<HTMLButtonElement>("button")].find((button) => button.textContent === "Discard changes")!;
    expect(discard.disabled).toBe(false);
    act(() => discard.click());
    expect(input.value).toBe("90");
    expect(useSettingsStore.getState().persisted.history_retention_days).toBe(90);
  });

  it("restores focus to the connected invoking control on unmount", () => {
    const trigger = document.createElement("button");
    document.body.appendChild(trigger);
    trigger.focus();
    act(() => root.render(<SettingsModal onClose={vi.fn()} />));
    expect(document.activeElement).not.toBe(trigger);

    act(() => root.unmount());
    expect(document.activeElement).toBe(trigger);
    trigger.remove();
    root = createRoot(container);
  });

  it("wraps Tab and Shift+Tab between actual enabled controls", () => {
    act(() => root.render(<SettingsModal onClose={vi.fn()} />));
    const enabled = [...container.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), select:not(:disabled)')];
    const first = enabled[0];
    const last = enabled[enabled.length - 1];

    last.focus();
    act(() => window.dispatchEvent(new KeyboardEvent("keydown", { key: "Tab", bubbles: true })));
    expect(document.activeElement).toBe(first);

    first.focus();
    act(() => window.dispatchEvent(new KeyboardEvent("keydown", { key: "Tab", shiftKey: true, bubbles: true })));
    expect(document.activeElement).toBe(last);
  });

  it("blocks every close path while saving and closes once after one successful save", async () => {
    let resolveSave!: (settings: typeof DEFAULT_SETTINGS) => void;
    api.settingsSave.mockImplementation(() => new Promise((resolve) => { resolveSave = resolve; }));
    const onClose = vi.fn();
    act(() => root.render(<SettingsModal onClose={onClose} />));
    act(() => useSettingsStore.getState().patch({ theme: "dark" }));

    const form = container.querySelector<HTMLFormElement>("form")!;
    await act(async () => {
      form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
      form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
      await Promise.resolve();
    });
    expect(useSettingsStore.getState().saving).toBe(true);
    expect(container.querySelector("fieldset")?.hasAttribute("disabled")).toBe(true);

    act(() => window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true })));
    act(() => container.querySelector<HTMLElement>(".modal-backdrop")?.dispatchEvent(new MouseEvent("mousedown", { bubbles: true })));
    const close = container.querySelector<HTMLButtonElement>('[aria-label="Close settings"]')!;
    const cancel = [...container.querySelectorAll<HTMLButtonElement>("button")].find((button) => button.textContent === "Cancel")!;
    const discard = [...container.querySelectorAll<HTMLButtonElement>("button")].find((button) => button.textContent === "Discard changes")!;
    act(() => { close.click(); cancel.click(); discard.click(); });
    expect(onClose).not.toHaveBeenCalled();

    await act(async () => {
      resolveSave({ ...DEFAULT_SETTINGS, theme: "dark", default_ports: { ...DEFAULT_SETTINGS.default_ports } });
      await Promise.resolve();
    });
    expect(api.settingsSave).toHaveBeenCalledOnce();
    expect(onClose).toHaveBeenCalledOnce();
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
