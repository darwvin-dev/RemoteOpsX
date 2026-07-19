import { beforeEach, describe, expect, it } from "vitest";
import { useStore } from "./store";
import type { Server } from "./types";

const server = (id: string): Server => ({
  id,
  name: id,
  host: `${id}.example.com`,
  port: 22,
  ftp_port: null,
  rdp_port: null,
  vnc_port: null,
  username: "root",
  protocols: ["ssh"],
  auth_type: "key",
  private_key_path: null,
  tags: [],
  group_name: null,
  environment: "dev",
  notes: null,
  created_at: "",
  updated_at: "",
});

describe("workspace tab focus", () => {
  beforeEach(() => {
    useStore.setState({ tabs: [], activeTabId: null, focusedServerId: null });
  });

  it("focuses the fallback tab server after closing the active tab", () => {
    const first = useStore.getState().openTab("ssh", server("one"));
    const second = useStore.getState().openTab("ssh", server("two"));

    useStore.getState().closeTab(second);

    expect(useStore.getState().activeTabId).toBe(first);
    expect(useStore.getState().focusedServerId).toBe("one");
  });

  it("clears server focus after closing the final tab", () => {
    const only = useStore.getState().openTab("ssh", server("only"));

    useStore.getState().closeTab(only);

    expect(useStore.getState().activeTabId).toBeNull();
    expect(useStore.getState().focusedServerId).toBeNull();
  });
});
