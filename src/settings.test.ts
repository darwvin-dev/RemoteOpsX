import { beforeEach, describe, expect, it, vi } from "vitest";

const { rawInvoke } = vi.hoisted(() => ({ rawInvoke: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke: rawInvoke }));

import { settingsGet } from "./api";
import { RemoteOpsError, normalizeRemoteError } from "./errors";
import { DEFAULT_SETTINGS, patchSettings } from "./settings";

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

    expect(error).toMatchObject({ message: "offline", code: "client.error", retryable: false });
  });

  it.each([{}, { message: 42 }, 7, true, null, undefined])(
    "maps unknown rejection %j safely",
    (rejection) => {
      const error = normalizeRemoteError(rejection);

      expect(error.code).toBe("client.unknown");
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
    expect(error.message).not.toContain("[object Object]");
  });
});
