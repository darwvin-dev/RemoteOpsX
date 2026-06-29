export type RemoveListener = () => void;

export function terminalBackendSessionId(tabId: string, generation: number): string {
  return `${tabId}:${generation}`;
}

interface StartTerminalSessionOptions {
  tabId: string;
  listen: (event: string, handler: (payload: unknown) => void) => Promise<RemoveListener>;
  spawn: () => Promise<void>;
  onOutput: (payload: unknown) => void;
  onExit: () => void;
}

/** Establish event delivery before spawning so early PTY bytes cannot be lost. */
export async function startTerminalSession(options: StartTerminalSessionOptions): Promise<RemoveListener> {
  const removeOutput = await options.listen(`pty://output/${options.tabId}`, options.onOutput);
  let removeExit: RemoveListener | null = null;
  try {
    removeExit = await options.listen(`pty://exit/${options.tabId}`, options.onExit);
    await options.spawn();
  } catch (error) {
    removeOutput();
    removeExit?.();
    throw error;
  }
  return () => {
    removeOutput();
    removeExit?.();
  };
}
