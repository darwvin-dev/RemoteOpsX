// Global UI state via Zustand. Keeps server list, open tabs, the active tab,
// panel selections and a lightweight alert log.

import { create } from "zustand";
import * as api from "./api";
import type {
  BottomPanelView,
  RightPanelView,
  Server,
  Tab,
  TabKind,
} from "./types";

let tabCounter = 0;
const nextTabId = () => `tab-${Date.now()}-${tabCounter++}`;

export interface Alert {
  id: string;
  time: string;
  level: "info" | "warn" | "error";
  message: string;
  serverId?: string;
}

interface AppStore {
  // data
  servers: Server[];
  loadingServers: boolean;
  // layout
  tabs: Tab[];
  activeTabId: string | null;
  rightPanel: RightPanelView;
  bottomPanelOpen: boolean;
  bottomPanel: BottomPanelView;
  // the server whose health/services the right panel reflects
  focusedServerId: string | null;
  healthIntervalMs: number;
  alerts: Alert[];
  outputLines: string[];

  // actions
  loadServers: () => Promise<void>;
  setRightPanel: (v: RightPanelView) => void;
  setBottomPanel: (v: BottomPanelView) => void;
  toggleBottomPanel: (open?: boolean) => void;
  setFocusedServer: (id: string | null) => void;
  setHealthInterval: (ms: number) => void;
  openTab: (kind: TabKind, server: Server, opts?: { runbookId?: string; title?: string }) => string;
  closeTab: (id: string) => void;
  setActiveTab: (id: string) => void;
  pushAlert: (level: Alert["level"], message: string, serverId?: string) => void;
  clearAlerts: () => void;
  pushOutput: (text: string) => void;
  clearOutput: () => void;
}

export const useStore = create<AppStore>((set, get) => ({
  servers: [],
  loadingServers: false,
  tabs: [],
  activeTabId: null,
  rightPanel: "health",
  bottomPanelOpen: true,
  bottomPanel: "output",
  focusedServerId: null,
  healthIntervalMs: 3000,
  alerts: [],
  outputLines: [],

  loadServers: async () => {
    set({ loadingServers: true });
    try {
      const servers = await api.serversList();
      set({ servers, loadingServers: false });
    } catch (err) {
      set({ loadingServers: false });
      get().pushAlert("error", `Failed to load servers: ${err}`);
    }
  },

  setRightPanel: (v) => set({ rightPanel: v }),
  setBottomPanel: (v) => set({ bottomPanel: v, bottomPanelOpen: true }),
  toggleBottomPanel: (open) => set((s) => ({ bottomPanelOpen: open ?? !s.bottomPanelOpen })),
  setFocusedServer: (id) => set({ focusedServerId: id }),
  setHealthInterval: (ms) => set({ healthIntervalMs: ms }),

  openTab: (kind, server, opts) => {
    const id = nextTabId();
    const title = opts?.title ?? `${kind.toUpperCase()} · ${server.name}`;
    const tab: Tab = { id, kind, serverId: server.id, title, runbookId: opts?.runbookId };
    set((s) => ({
      tabs: [...s.tabs, tab],
      activeTabId: id,
      focusedServerId: server.id,
    }));
    return id;
  },

  closeTab: (id) => {
    set((s) => {
      const idx = s.tabs.findIndex((t) => t.id === id);
      const tabs = s.tabs.filter((t) => t.id !== id);
      let activeTabId = s.activeTabId;
      if (s.activeTabId === id) {
        const fallback = tabs[idx] ?? tabs[idx - 1] ?? tabs[tabs.length - 1] ?? null;
        activeTabId = fallback ? fallback.id : null;
      }
      return { tabs, activeTabId };
    });
  },

  setActiveTab: (id) => {
    const tab = get().tabs.find((t) => t.id === id);
    set({ activeTabId: id, focusedServerId: tab ? tab.serverId : get().focusedServerId });
  },

  pushAlert: (level, message, serverId) =>
    set((s) => ({
      alerts: [
        { id: `${Date.now()}-${Math.random().toString(36).slice(2, 7)}`, time: new Date().toLocaleTimeString(), level, message, serverId },
        ...s.alerts,
      ].slice(0, 200),
    })),

  clearAlerts: () => set({ alerts: [] }),

  pushOutput: (text) =>
    set((s) => ({ outputLines: [...s.outputLines, text].slice(-500) })),
  clearOutput: () => set({ outputLines: [] }),
}));
