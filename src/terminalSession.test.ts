import { describe, expect, it, vi } from "vitest";
import { nextTerminalConnectionAttempt, startTerminalSession, terminalBackendSessionId } from "./terminalSession";

describe("terminal session startup", () => {
  it("registers output and exit listeners before spawning", async () => {
    const order: string[] = [];
    const listen = vi.fn(async (event: string) => {
      order.push(`listen:${event}`);
      return vi.fn();
    });
    const spawn = vi.fn(async () => { order.push("spawn"); });

    await startTerminalSession({
      tabId: "tab-1",
      listen,
      spawn,
      onOutput: vi.fn(),
      onExit: vi.fn(),
    });

    expect(order).toEqual([
      "listen:pty://output/tab-1",
      "listen:pty://exit/tab-1",
      "spawn",
    ]);
  });

  it("removes listeners when spawning fails", async () => {
    const removeOutput = vi.fn();
    const removeExit = vi.fn();
    const listen = vi.fn()
      .mockResolvedValueOnce(removeOutput)
      .mockResolvedValueOnce(removeExit);
    await expect(startTerminalSession({
      tabId: "tab-1",
      listen,
      spawn: async () => { throw new Error("failed"); },
      onOutput: vi.fn(),
      onExit: vi.fn(),
    })).rejects.toThrow("failed");
    expect(removeOutput).toHaveBeenCalledOnce();
    expect(removeExit).toHaveBeenCalledOnce();
  });

  it("uses a distinct backend session id for each connection attempt", () => {
    expect(terminalBackendSessionId("tab-1", 0, 1)).toBe("tab-1:0:1");
    expect(terminalBackendSessionId("tab-1", 0, 2)).toBe("tab-1:0:2");
    expect(terminalBackendSessionId("tab-1", 1, 3)).toBe("tab-1:1:3");
    expect(terminalBackendSessionId("tab-1", 0, 1)).not.toBe(terminalBackendSessionId("tab-1", 0, 2));
  });

  it("allocates monotonically increasing connection attempts", () => {
    const first = nextTerminalConnectionAttempt();
    const second = nextTerminalConnectionAttempt();

    expect(second).toBe(first + 1);
  });
});
