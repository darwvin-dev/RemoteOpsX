# Operator Experience

RemoteOpsX treats the workspace home, Runbook Studio, Command Palette, and Operator Center as one local-first operator workflow.

## Fleet dashboard

When no session tab is open, the workspace shows persisted server health, unacknowledged alerts, tunnel state, and recent automation. Health status is derived from the persisted operator data plane rather than transient component state.

## Runbook Studio and execution

Studio accepts bounded YAML files and validates them in the Rust backend. Variables are rendered server-side for dry-run and again immediately before execution. Malformed or unresolved placeholders block preparation. Destructive commands are detected after rendering and always require confirmation even when the YAML step did not request it.

RunbookRunner executes only backend-prepared commands. Retry-from-failure prepares the runbook again with the current variables, starts from the failed original step, and recalculates every confirmation boundary. The preview, import/export, saved-run preparation, and fleet-dashboard handlers are registered in the Tauri command surface rather than simulated in React.

## Universal command palette

Ctrl/Cmd+K indexes workspace actions, servers, health and diagnostics views, open tabs, runbooks, and saved command snippets. Snippets require a focused SSH server and explicit confirmation; production targets are identified before execution.

## Multi-host cancellation

Multi-host runs carry a caller-generated run ID. Cancellation is cooperative: it prevents future bounded batches from starting, while SSH commands already in flight finish and remain in the persisted audit result. The multi-host worker runs off the Tauri command thread so the cancellation command remains responsive while a batch is active.
