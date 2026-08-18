import { useEffect, useRef, useState } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { listen } from "@tauri-apps/api/event";
import * as api from "../api";
import { useStore } from "../store";
import { useSettingsStore } from "../settingsStore";
import { SYSTEM_THEME_QUERY, terminalFontStack, terminalTheme } from "../theme";
import {
  nextTerminalConnectionAttempt,
  startTerminalSession,
  terminalBackendSessionId,
  type RemoveListener,
} from "../terminalSession";
import type { Server } from "../types";

interface Props {
  tabId: string;
  server: Server;
  active: boolean;
}

type ConnState = "connecting" | "connected" | "closed";

/** A single SSH terminal backed by a server-side PTY (system `ssh`).
 *  Output arrives via Tauri events; keystrokes are sent through `pty_write`. */
export function TerminalTab({ tabId, server, active }: Props) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const pushAlert = useStore((s) => s.pushAlert);
  const theme = useSettingsStore((state) => state.settings.theme);
  const terminalFont = useSettingsStore((state) => state.settings.terminal_font);
  const terminalFontSize = useSettingsStore((state) => state.settings.terminal_font_size);
  const terminalLineHeight = useSettingsStore((state) => state.settings.terminal_line_height_percent);
  const terminalCursorStyle = useSettingsStore((state) => state.settings.terminal_cursor_style);
  const terminalOpacity = useSettingsStore((state) => state.settings.terminal_background_opacity_percent);
  const [conn, setConn] = useState<ConnState>("connecting");
  const [generation, setGeneration] = useState(0);

  useEffect(() => {
    if (!hostRef.current) return;
    let disposed = false;
    const unlisteners: RemoveListener[] = [];
    let spawned = false;
    let ioErrorShown = false;
    const backendSessionId = terminalBackendSessionId(
      tabId,
      generation,
      nextTerminalConnectionAttempt(),
    );
    const systemPrefersDark = window.matchMedia(SYSTEM_THEME_QUERY).matches;

    const term = new Terminal({
      fontFamily: terminalFontStack(terminalFont),
      fontSize: terminalFontSize,
      lineHeight: terminalLineHeight / 100,
      cursorBlink: true,
      cursorStyle: terminalCursorStyle,
      allowTransparency: true,
      scrollback: 5000,
      theme: terminalTheme(theme, systemPrefersDark, terminalOpacity),
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.loadAddon(new WebLinksAddon());
    term.open(hostRef.current);
    termRef.current = term;
    fitRef.current = fit;

    try {
      fit.fit();
    } catch {
      // Ignore pre-layout fit.
    }

    const enc = new TextEncoder();
    term.onData((data) => {
      void api.ptyWrite(backendSessionId, Array.from(enc.encode(data))).catch((error) => {
        if (!ioErrorShown) {
          ioErrorShown = true;
          pushAlert("error", `Terminal write failed (${server.name}): ${error}`, server.id);
        }
      });
    });

    term.onResize(({ cols, rows }) => {
      if (spawned) void api.ptyResize(backendSessionId, cols, rows).catch(() => {});
    });

    async function start() {
      const cols = term.cols || 80;
      const rows = term.rows || 24;
      try {
        const removeListeners = await startTerminalSession({
          tabId: backendSessionId,
          listen: (event, handler) => listen(event, (message) => handler(message.payload)),
          spawn: async () => {
            await api.ptySpawn(backendSessionId, server.id, cols, rows);
            spawned = true;
          },
          onOutput: (payload) => term.write(new Uint8Array(payload as number[])),
          onExit: () => {
            setConn("closed");
            term.writeln("\r\n\x1b[33m● session closed\x1b[0m");
            if (spawned) {
              spawned = false;
              void api.ptyClose(backendSessionId).catch((error) => {
                pushAlert(
                  "error",
                  `Terminal session cleanup failed (${server.name}): ${error}`,
                  server.id,
                );
              });
            }
          },
        });
        if (disposed) {
          removeListeners();
          if (spawned) void api.ptyClose(backendSessionId).catch(() => {});
          return;
        }
        unlisteners.push(removeListeners);
        setConn("connected");
      } catch (error) {
        if (disposed) return;
        setConn("closed");
        term.writeln(`\r\n\x1b[31m✖ connection failed: ${error}\x1b[0m`);
        pushAlert("error", `SSH connect failed (${server.name}): ${error}`, server.id);
      }
    }

    void start();

    const resizeObserver = new ResizeObserver(() => {
      try {
        fit.fit();
      } catch {
        // No-op while hidden/pre-layout.
      }
    });
    resizeObserver.observe(hostRef.current);

    return () => {
      disposed = true;
      resizeObserver.disconnect();
      unlisteners.forEach((unlisten) => unlisten());
      if (spawned) void api.ptyClose(backendSessionId).catch(() => {});
      term.dispose();
      termRef.current = null;
    };
  }, [tabId, server.id, generation, pushAlert, server.name]);

  useEffect(() => {
    const media = window.matchMedia(SYSTEM_THEME_QUERY);
    const applyTerminalAppearance = () => {
      const term = termRef.current;
      if (!term) return;
      term.options.fontFamily = terminalFontStack(terminalFont);
      term.options.fontSize = terminalFontSize;
      term.options.lineHeight = terminalLineHeight / 100;
      term.options.cursorStyle = terminalCursorStyle;
      term.options.theme = terminalTheme(theme, media.matches, terminalOpacity);
      try {
        fitRef.current?.fit();
      } catch {
        // A hidden terminal may not be measurable yet.
      }
    };
    applyTerminalAppearance();
    if (theme !== "system") return;
    media.addEventListener("change", applyTerminalAppearance);
    return () => media.removeEventListener("change", applyTerminalAppearance);
  }, [
    terminalCursorStyle,
    terminalFont,
    terminalFontSize,
    terminalLineHeight,
    terminalOpacity,
    theme,
  ]);

  useEffect(() => {
    if (active && fitRef.current) {
      const id = setTimeout(() => {
        try {
          fitRef.current?.fit();
          termRef.current?.focus();
        } catch {
          // No-op while hidden/pre-layout.
        }
      }, 30);
      return () => clearTimeout(id);
    }
  }, [active]);

  function reconnect() {
    setConn("connecting");
    termRef.current?.clear();
    setGeneration((value) => value + 1);
  }

  return (
    <>
      <div className="term-toolbar">
        <div className="term-status">
          <span className={`conn-dot ${conn}`} />
          <span>
            {conn === "connected" ? "connected" : conn === "connecting" ? "connecting…" : "closed"}
          </span>
        </div>
        <span className="muted mono">
          {server.username}@{server.host}:{server.port}
        </span>
        <span className="grow" style={{ flex: 1 }} />
        <button className="tiny" onClick={reconnect}>
          Reconnect
        </button>
        <button className="tiny ghost" onClick={() => termRef.current?.clear()}>
          Clear
        </button>
      </div>
      <div className="terminal-host" ref={hostRef} />
    </>
  );
}
