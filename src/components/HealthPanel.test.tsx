// @vitest-environment jsdom

import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_SETTINGS } from "../settings";
import { useSettingsStore } from "../settingsStore";
import type { HealthSnapshot, Server } from "../types";
import { HealthPanel } from "./HealthPanel";

const api = vi.hoisted(() => ({ healthCollect: vi.fn(), settingsGet: vi.fn(), settingsSave: vi.fn() }));
vi.mock("../api", () => api);
(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const server: Server = {
  id: "server-1", name: "Server", host: "host", port: 22, username: "root", protocols: ["ssh"],
  auth_type: "key", tags: [], environment: "dev", created_at: "", updated_at: "",
};
const snapshot: HealthSnapshot = {
  os_name: "Linux", kernel: "6", hostname: "host", uptime_secs: 1, load1: 0, load5: 0, load15: 0,
  cpu_percent: 1, mem_percent: 1, mem_total_kb: 100, mem_used_kb: 1, swap_percent: 0,
  swap_total_kb: 0, swap_used_kb: 0, net_rx_rate: 0, net_tx_rate: 0, disks: [], top_cpu: [], top_mem: [],
  listening_ports: [], failed_services: [], warnings: [],
};

describe("HealthPanel polling", () => {
  let container: HTMLDivElement;
  let root: ReturnType<typeof createRoot>;

  beforeEach(() => {
    vi.useFakeTimers();
    api.healthCollect.mockReset().mockResolvedValue(snapshot);
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
    const settings = { ...DEFAULT_SETTINGS, default_ports: { ...DEFAULT_SETTINGS.default_ports } };
    useSettingsStore.setState({ settings, persisted: { ...settings, default_ports: { ...settings.default_ports } }, loading: false, saving: false });
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.useRealTimers();
  });

  it("restarts on interval changes and clears the timer on unmount", async () => {
    await act(async () => { root.render(<HealthPanel server={server} />); await Promise.resolve(); });
    expect(api.healthCollect).toHaveBeenCalledTimes(1);

    await act(async () => {
      useSettingsStore.getState().patch({ health_refresh_interval_ms: 5000 });
      await Promise.resolve();
    });
    expect(api.healthCollect).toHaveBeenCalledTimes(2);

    await act(async () => { await vi.advanceTimersByTimeAsync(4999); });
    expect(api.healthCollect).toHaveBeenCalledTimes(2);
    await act(async () => { await vi.advanceTimersByTimeAsync(1); });
    expect(api.healthCollect).toHaveBeenCalledTimes(3);

    act(() => root.unmount());
    await vi.advanceTimersByTimeAsync(10_000);
    expect(api.healthCollect).toHaveBeenCalledTimes(3);
    root = createRoot(container);
  });
});
