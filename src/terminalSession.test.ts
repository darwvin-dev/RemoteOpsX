import { describe, expect, it, vi } from "vitest";
import { startTerminalSession } from "./terminalSession";

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
});
