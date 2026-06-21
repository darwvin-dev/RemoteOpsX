import { useEffect, useRef } from "react";
import { useSettingsStore } from "../settingsStore";
import type { DefaultPorts, SettingsPatch, Theme, TransferConflictPolicy } from "../settings";

interface Props {
  onClose: () => void;
}

const PORTS: { key: keyof DefaultPorts; label: string }[] = [
  { key: "ssh", label: "SSH default port" },
  { key: "ftp", label: "FTP default port" },
  { key: "rdp", label: "RDP default port" },
  { key: "vnc", label: "VNC default port" },
];

export function SettingsModal({ onClose }: Props) {
  const settings = useSettingsStore((state) => state.settings);
  const loading = useSettingsStore((state) => state.loading);
  const saving = useSettingsStore((state) => state.saving);
  const dirty = useSettingsStore((state) => state.dirty);
  const error = useSettingsStore((state) => state.error);
  const patch = useSettingsStore((state) => state.patch);
  const reset = useSettingsStore((state) => state.reset);
  const save = useSettingsStore((state) => state.save);
  const titleRef = useRef<HTMLHeadingElement>(null);
  const dialogRef = useRef<HTMLFormElement>(null);

  function requestClose() {
    if (dirty && !window.confirm("Discard your unsaved settings changes?")) return;
    if (dirty) reset();
    onClose();
  }

  useEffect(() => {
    titleRef.current?.focus();
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        event.preventDefault();
        requestClose();
        return;
      }
      if (event.key === "Tab") {
        const focusable = dialogRef.current?.querySelectorAll<HTMLElement>(
          'button:not(:disabled), input:not(:disabled), select:not(:disabled), [tabindex]:not([tabindex="-1"])',
        );
        if (!focusable?.length) return;
        const first = focusable[0];
        const last = focusable[focusable.length - 1];
        if (event.shiftKey && (document.activeElement === first || document.activeElement === titleRef.current)) {
          event.preventDefault();
          last.focus();
        } else if (!event.shiftKey && document.activeElement === last) {
          event.preventDefault();
          first.focus();
        }
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  // requestClose intentionally reads the current render's state.
  }, [dirty]);

  const patchNumber = (field: keyof Pick<typeof settings, "health_refresh_interval_ms" | "history_retention_days" | "app_lock_timeout_minutes">, value: string, multiplier = 1) => {
    patch({ [field]: Number(value) * multiplier } as SettingsPatch);
  };

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    if (!dirty || loading || saving) return;
    try {
      await save();
      onClose();
    } catch {
      // The normalized error is rendered from the settings store.
    }
  }

  return (
    <div className="modal-backdrop" onMouseDown={(event) => event.target === event.currentTarget && requestClose()}>
      <form ref={dialogRef} className="modal settings-modal" role="dialog" aria-modal="true" aria-labelledby="settings-title" onSubmit={submit}>
        <div className="modal-head">
          <div>
            <span className="eyebrow">Application</span>
            <h2 id="settings-title" ref={titleRef} tabIndex={-1}>Settings</h2>
          </div>
          <button type="button" className="ghost tiny" aria-label="Close settings" onClick={requestClose}>✕</button>
        </div>
        <div className="modal-body settings-body" aria-busy={loading || saving}>
          <section className="settings-section" aria-labelledby="appearance-heading">
            <h3 id="appearance-heading">Appearance</h3>
            <div>
              <label htmlFor="settings-theme">Theme</label>
              <select id="settings-theme" value={settings.theme} onChange={(event) => patch({ theme: event.target.value as Theme })}>
                <option value="system">Follow system</option>
                <option value="dark">Dark</option>
                <option value="light">Light</option>
              </select>
            </div>
          </section>

          <section className="settings-section" aria-labelledby="connections-heading">
            <h3 id="connections-heading">Connection defaults</h3>
            <div className="settings-grid ports-grid">
              {PORTS.map(({ key, label }) => (
                <div key={key}>
                  <label htmlFor={`settings-port-${key}`}>{label}</label>
                  <input id={`settings-port-${key}`} type="number" min={1} max={65535} step={1} required value={settings.default_ports[key]}
                    onChange={(event) => patch({ default_ports: { [key]: Number(event.target.value) } })} />
                </div>
              ))}
            </div>
          </section>

          <section className="settings-section" aria-labelledby="behavior-heading">
            <h3 id="behavior-heading">Behavior and retention</h3>
            <div className="settings-grid">
              <div>
                <label htmlFor="settings-health">Health refresh (seconds)</label>
                <input id="settings-health" type="number" min={1} max={60} step={1} required value={settings.health_refresh_interval_ms / 1000}
                  onChange={(event) => patchNumber("health_refresh_interval_ms", event.target.value, 1000)} />
              </div>
              <div>
                <label htmlFor="settings-history">History retention (days)</label>
                <input id="settings-history" type="number" min={1} max={3650} step={1} required value={settings.history_retention_days}
                  onChange={(event) => patchNumber("history_retention_days", event.target.value)} />
              </div>
              <div>
                <label htmlFor="settings-lock">App lock timeout (minutes)</label>
                <input id="settings-lock" type="number" min={1} max={1440} step={1} required value={settings.app_lock_timeout_minutes}
                  onChange={(event) => patchNumber("app_lock_timeout_minutes", event.target.value)} />
              </div>
              <div>
                <label htmlFor="settings-conflict">Transfer conflict policy</label>
                <select id="settings-conflict" value={settings.transfer_conflict_policy}
                  onChange={(event) => patch({ transfer_conflict_policy: event.target.value as TransferConflictPolicy })}>
                  <option value="ask">Ask every time</option>
                  <option value="overwrite">Overwrite</option>
                  <option value="rename">Keep both (rename)</option>
                  <option value="skip">Skip</option>
                </select>
              </div>
            </div>
          </section>

          <section className="settings-section" aria-labelledby="desktop-heading">
            <h3 id="desktop-heading">Desktop integration</h3>
            <div className="settings-toggles">
              <Toggle id="clipboard" label="Clipboard sharing" checked={settings.desktop_clipboard_enabled} onChange={(checked) => patch({ desktop_clipboard_enabled: checked })} />
              <Toggle id="audio" label="Remote audio" checked={settings.desktop_audio_enabled} onChange={(checked) => patch({ desktop_audio_enabled: checked })} />
              <Toggle id="notifications" label="Desktop notifications" checked={settings.desktop_notifications_enabled} onChange={(checked) => patch({ desktop_notifications_enabled: checked })} />
            </div>
          </section>

          {error ? (
            <div className="settings-error" role="alert">
              <strong>{error.message}</strong>
              <span>Code: <code>{error.code}</code></span>
              {error.correlationId ? <span>Correlation ID: <code>{error.correlationId}</code></span> : null}
            </div>
          ) : null}
        </div>
        <div className="modal-foot settings-foot">
          <button type="button" onClick={requestClose}>Cancel</button>
          <button type="button" onClick={reset} disabled={!dirty || loading || saving}>Discard changes</button>
          <button type="submit" className="primary" disabled={!dirty || loading || saving}>{saving ? "Saving…" : "Save settings"}</button>
        </div>
      </form>
    </div>
  );
}

function Toggle({ id, label, checked, onChange }: { id: string; label: string; checked: boolean; onChange: (checked: boolean) => void }) {
  return (
    <label className="settings-toggle" htmlFor={`settings-${id}`}>
      <span>{label}</span>
      <input id={`settings-${id}`} type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} />
    </label>
  );
}
