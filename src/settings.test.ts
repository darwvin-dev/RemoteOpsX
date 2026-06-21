import { beforeEach, describe, expect, it, vi } from "vitest";

const { rawInvoke } = vi.hoisted(() => ({ rawInvoke: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke: rawInvoke }));

import { settingsGet, settingsSave } from "./api";
import { RemoteOpsError, normalizeRemoteError } from "./errors";
import { DEFAULT_SETTINGS, patchSettings } from "./settings";
import type { AppSettings } from "./settings";
import { createSettingsState } from "./settingsStore";

const darkSettings = (): AppSettings => ({
  ...DEFAULT_SETTINGS,
  theme: "dark",
  default_ports: { ...DEFAULT_SETTINGS.default_ports },
});

describe("settings contracts", () => {
  beforeEach(() => {
    rawInvoke.mockReset();
  });

  it("mirrors the Rust defaults exactly", () => {
    expect(DEFAULT_SETTINGS).toEqual({
      schema_version: 1,
      theme: "system",
      default_ports: { ssh: 22, ftp: 21, rdp: 3389, vnc: 5900 },
      health_refresh_interval_ms: 3000,
      history_retention_days: 90,
      app_lock_timeout_minutes: 15,
      transfer_conflict_policy: "ask",
      desktop_clipboard_enabled: true,
      desktop_audio_enabled: true,
      desktop_notifications_enabled: true,
    });
  });

  it("patches nested ports immutably and preserves sibling ports", () => {
    const current = { ...DEFAULT_SETTINGS, default_ports: { ...DEFAULT_SETTINGS.default_ports } };
    const result = patchSettings(current, { default_ports: { ssh: 2222 } });

    expect(result.default_ports).toEqual({ ssh: 2222, ftp: 21, rdp: 3389, vnc: 5900 });
    expect(current.default_ports).toEqual(DEFAULT_SETTINGS.default_ports);
    expect(result).not.toBe(current);
    expect(result.default_ports).not.toBe(current.default_ports);
    expect(DEFAULT_SETTINGS.default_ports.ssh).toBe(22);
  });

  it.each([
    ["schema_version", (settings: AppSettings): void => { settings.schema_version = 2; }],
    ["theme", (settings: AppSettings): void => { (settings as { theme: string }).theme = "blue"; }],
    [
      "transfer_conflict_policy",
      (settings: AppSettings): void => {
        (settings as { transfer_conflict_policy: string }).transfer_conflict_policy = "replace";
      },
    ],
    ["default_ports.ssh", (settings: AppSettings): void => { settings.default_ports.ssh = 65_536; }],
    ["default_ports.ftp", (settings: AppSettings): void => { settings.default_ports.ftp = Number.NaN; }],
    ["default_ports.rdp", (settings: AppSettings): void => { settings.default_ports.rdp = 3389.5; }],
    [
      "health_refresh_interval_ms",
      (settings: AppSettings): void => { settings.health_refresh_interval_ms = 999; },
    ],
    ["history_retention_days", (settings: AppSettings): void => { settings.history_retention_days = 3651; }],
    ["app_lock_timeout_minutes", (settings: AppSettings): void => { settings.app_lock_timeout_minutes = 0; }],
    [
      "desktop_clipboard_enabled",
      (settings: AppSettings): void => {
        (settings as unknown as { desktop_clipboard_enabled: string }).desktop_clipboard_enabled = "yes";
      },
    ],
  ] as const)("rejects invalid %s before invoking Tauri", async (field, mutate) => {
    const settings: AppSettings = {
      ...DEFAULT_SETTINGS,
      default_ports: { ...DEFAULT_SETTINGS.default_ports },
    };
    mutate(settings);

    await expect(settingsSave(settings)).rejects.toMatchObject({
      code: "validation.invalid_value",
      retryable: false,
      correlationId: null,
      context: { field },
    });
    expect(rawInvoke).not.toHaveBeenCalled();
  });

  it("normalizes structured invoke rejections without losing transport metadata", () => {
    const error = normalizeRemoteError({
      message: "try later",
      code: "network.unavailable",
      retryable: true,
      correlation_id: "request-123",
      context: { host: "example.test", attempt: "2" },
    });

    expect(error).toBeInstanceOf(RemoteOpsError);
    expect(error.message).toBe("try later");
    expect(error.code).toBe("network.unavailable");
    expect(error.retryable).toBe(true);
    expect(error.correlationId).toBe("request-123");
    expect(error.context).toEqual({ host: "example.test", attempt: "2" });
  });

  it("normalizes errors rejected by the Tauri invoke boundary", async () => {
    rawInvoke.mockImplementation(() => {
      throw {
        message: "try later",
        code: "network.unavailable",
        retryable: true,
        correlation_id: "request-123",
        context: { host: "example.test" },
      };
    });

    await expect(settingsGet()).rejects.toSatisfy(
      (error: unknown) => error instanceof RemoteOpsError && error.code === "network.unavailable",
    );
  });

  it("maps native errors to client.error", () => {
    const error = normalizeRemoteError(new Error("offline"));

    expect(error).toMatchObject({
      message: "offline",
      code: "client.error",
      retryable: false,
      correlationId: null,
    });
  });

  it.each([{}, { message: 42 }, 7, true, null, undefined])(
    "maps unknown rejection %j safely",
    (rejection) => {
      const error = normalizeRemoteError(rejection);

      expect(error.code).toBe("client.unknown");
      expect(error.correlationId).toBeNull();
      expect(error.message).not.toContain("[object Object]");
    },
  );

  it("rejects malformed structured context", () => {
    const error = normalizeRemoteError({
      message: "bad payload",
      code: "remote.bad",
      retryable: false,
      correlation_id: "request-456",
      context: { valid: "yes", invalid: 1 },
    });

    expect(error.code).toBe("client.unknown");
    expect(error.correlationId).toBeNull();
    expect(error.message).not.toContain("[object Object]");
  });

  it.each([{}, { correlation_id: "" }])(
    "rejects malformed correlation metadata %j with a null fallback",
    (correlation) => {
      const error = normalizeRemoteError({
        message: "bad payload",
        code: "remote.bad",
        retryable: false,
        context: {},
        ...correlation,
      });

      expect(error.code).toBe("client.unknown");
      expect(error.correlationId).toBeNull();
    },
  );

  it("defensively freezes error context", () => {
    const context = { field: "theme" };
    const error = new RemoteOpsError("invalid", "validation.invalid_value", false, null, context);
    context.field = "changed";

    expect(error.context).toEqual({ field: "theme" });
    expect(Object.isFrozen(error.context)).toBe(true);
  });
});

describe("settings state", () => {
  it("loads, patches, and saves backend-returned settings", async () => {
    const loaded = darkSettings();
    const saved = { ...loaded, theme: "light" as const };
    const store = createSettingsState({
      load: async () => loaded,
      save: async () => saved,
    });

    await store.getState().load();
    expect(store.getState()).toMatchObject({ settings: loaded, persisted: loaded, dirty: false });

    store.getState().patch({ theme: "light" });
    expect(store.getState()).toMatchObject({ settings: saved, persisted: loaded, dirty: true });

    await store.getState().save();
    expect(store.getState()).toMatchObject({
      settings: saved,
      persisted: saved,
      dirty: false,
      saving: false,
      error: null,
    });
  });

  it("rolls settings and persisted state back when save fails", async () => {
    const loaded = darkSettings();
    const store = createSettingsState({
      load: async () => loaded,
      save: async () => { throw new Error("disk full"); },
    });

    await store.getState().load();
    store.getState().patch({ theme: "light" });

    await expect(store.getState().save()).rejects.toMatchObject({
      message: "disk full",
      code: "client.error",
    });
    expect(store.getState()).toMatchObject({
      settings: loaded,
      persisted: loaded,
      dirty: false,
      saving: false,
      error: { message: "disk full", code: "client.error" },
    });
  });

  it("resets edits without mutating persisted settings", async () => {
    const loaded = darkSettings();
    const store = createSettingsState({ load: async () => loaded, save: async (settings) => settings });

    await store.getState().load();
    store.getState().patch({ default_ports: { ssh: 2222 } });
    store.getState().reset();

    expect(store.getState()).toMatchObject({ settings: loaded, persisted: loaded, dirty: false, error: null });
    expect(store.getState().settings).not.toBe(store.getState().persisted);
  });
});
