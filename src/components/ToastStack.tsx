import { useMemo } from "react";
import { useStore } from "../store";

/** Lightweight recent-alert toasts so critical feedback is visible immediately. */
export function ToastStack() {
  const alerts = useStore((s) => s.alerts);
  const recent = useMemo(() => alerts.slice(0, 3), [alerts]);

  if (recent.length === 0) return null;

  return (
    <div className="toast-stack" aria-live="polite">
      {recent.map((alert) => (
        <div key={alert.id} className={`toast ${alert.level}`}>
          <span className="toast-dot" />
          <div>
            <strong>{alert.level}</strong>
            <p>{alert.message}</p>
          </div>
        </div>
      ))}
    </div>
  );
}
