import { useEffect, useRef, useState } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { listen } from "@tauri-apps/api/event";
import * as api from "../api";
import { useStore } from "../store";
import { startTerminalSession, terminalBackendSessionId, type RemoveListener } from "../terminalSession";
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
  const [conn, setConn] = useState<ConnState>("connecting");
  const [generation, setGeneration] = useState(0); // bumped on reconnect

  useEffect(() => {
    if (!hostRef.current) return;
    let disposed = false;
    const unlisteners: RemoveListener[] = [];
    let spawned = false;
    let ioErrorShown = false;
    const backendSessionId = terminalBackendSessionId(tabId, generation);

    const term = new Terminal({
      fontFamily: '"JetBrains Mono", "DejaVu Sans Mono", monospace',
      fontSize: 13,
      cursorBlink: true,
      scrollback: 5000,
      theme: {
        background: "#05080d",
        foreground: "#c9d4e0",
        cursor: "#2dd4bf",
        selectionBackground: "#1d4e73",
        black: "#0a0e14", red: "#f85149", green: "#3fb950", yellow: "#d29922",
        blue: "#4aa8ff", magenta: "#a371f7", cyan: "#2dd4bf", white: "#e6edf3",
      },
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.loadAddon(new WebLinksAddon());
    term.open(hostRef.current);
    termRef.current = term;
    fitRef.current = fit;

    try {
      fit.fit();
    } catch { /* ignore pre-layout fit */ }

    // Forward keystrokes to the PTY (encode to bytes).
    const enc = new TextEncoder();
    term.onData((data) => {
      void api.ptyWrite(backendSessionId, Array.from(enc.encode(data))).catch((error) => {
        if (!ioErrorShown) {
          ioErrorShown = true;
          pushAlert("error", `Terminal write failed (${server.name}): ${error}`, server.id);
        }
      });
    });

    // Keep the remote PTY size in sync with the xterm viewport.
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
          },
        });
        if (disposed) {
          removeListeners();
          if (spawned) void api.ptyClose(backendSessionId).catch(() => {});
          return;
        }
        unlisteners.push(removeListeners);
        setConn("connected");
      } catch (err) {
        if (disposed) return;
        setConn("closed");
        term.writeln(`\r\n\x1b[31m✖ connection failed: ${err}\x1b[0m`);
        pushAlert("error", `SSH connect failed (${server.name}): ${err}`, server.id);
        return;
      }

    }

    void start();

    // Refit on panel resize.
    const ro = new ResizeObserver(() => {
      try { fit.fit(); } catch { /* noop */ }
    });
    ro.observe(hostRef.current);

    return () => {
      disposed = true;
      ro.disconnect();
      unlisteners.forEach((u) => u());
      if (spawned) void api.ptyClose(backendSessionId).catch(() => {});
      term.dispose();
      termRef.current = null;
    };
    // generation in deps so "reconnect" tears down and rebuilds.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tabId, server.id, generation]);

  // Refit when this tab becomes visible again.
  useEffect(() => {
    if (active && fitRef.current) {
      const id = setTimeout(() => {
        try { fitRef.current?.fit(); termRef.current?.focus(); } catch { /* noop */ }
      }, 30);
      return () => clearTimeout(id);
    }
  }, [active]);

  function reconnect() {
    setConn("connecting");
    termRef.current?.clear();
    setGeneration((g) => g + 1);
  }

  return (
    <>
      <div className="term-toolbar">
        <div className="term-status">
          <span className={`conn-dot ${conn}`} />
          <span>{conn === "connected" ? "connected" : conn === "connecting" ? "connecting…" : "closed"}</span>
        </div>
        <span className="muted mono">{server.username}@{server.host}:{server.port}</span>
        <span className="grow" style={{ flex: 1 }} />
        <button className="tiny" onClick={reconnect}>Reconnect</button>
        <button className="tiny ghost" onClick={() => termRef.current?.clear()}>Clear</button>
      </div>
      <div className="terminal-host" ref={hostRef} />
    </>
  );
}
