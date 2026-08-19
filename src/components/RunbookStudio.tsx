import { useCallback, useEffect, useMemo, useState } from "react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import * as api from "../api";
import * as experienceApi from "../experienceApi";
import { useStore } from "../store";
import type { Runbook } from "../types";
import type { RunbookPreview } from "../experienceTypes";

const NEW_RUNBOOK = `name: New Runbook
description: Describe the operator outcome.
target_os: linux
variables:
  service: nginx
steps:
  - name: Inspect service
    command: systemctl status {{service}} --no-pager || true
  - name: Example guarded action
    command: sudo systemctl restart {{service}}
    requires_confirmation: true
  - name: Verify
    command: systemctl is-active {{service}}
    success_pattern: active
`;

export function RunbookStudio({ onClose }: { onClose: () => void }) {
  const pushAlert = useStore((state) => state.pushAlert);
  const [runbooks, setRunbooks] = useState<Runbook[]>([]);
  const [selectedId, setSelectedId] = useState<string>("");
  const [content, setContent] = useState(NEW_RUNBOOK);
  const [preview, setPreview] = useState<RunbookPreview | null>(null);
  const [variables, setVariables] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const selected = useMemo(
    () => runbooks.find((runbook) => runbook.id === selectedId) ?? null,
    [runbooks, selectedId],
  );

  const load = useCallback(async () => {
    try {
      setRunbooks(await api.runbooksList());
    } catch (reason) {
      setError(String(reason));
    }
  }, []);

  useEffect(() => { void load(); }, [load]);

  useEffect(() => {
    if (!selected) return;
    setContent(selected.content_yaml);
    setPreview(null);
    setVariables({});
    setError(null);
  }, [selected]);

  async function validate(overrides = variables): Promise<RunbookPreview | null> {
    setBusy(true);
    setError(null);
    try {
      const next = await experienceApi.runbookPreviewYaml(content, overrides);
      setPreview(next);
      const merged = { ...next.spec.variables, ...overrides };
      setVariables(merged);
      return next;
    } catch (reason) {
      setPreview(null);
      setError(String(reason));
      return null;
    } finally {
      setBusy(false);
    }
  }

  async function saveRunbook() {
    const next = await validate();
    if (!next || !next.valid) return;
    setBusy(true);
    try {
      const id = selected && !selected.builtin ? selected.id : undefined;
      const savedId = await api.runbookSave(
        next.spec.name,
        next.spec.description,
        content,
        id,
      );
      await load();
      setSelectedId(savedId);
      pushAlert("info", selected?.builtin
        ? `Saved a custom copy of ${next.spec.name}`
        : `Saved runbook ${next.spec.name}`);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function importYaml() {
    const picked = await openDialog({
      multiple: false,
      directory: false,
      filters: [{ name: "Runbook YAML", extensions: ["yaml", "yml"] }],
    });
    if (!picked || Array.isArray(picked)) return;
    setBusy(true);
    try {
      const yaml = await experienceApi.runbookImportYaml(picked);
      setSelectedId("");
      setContent(yaml);
      setVariables({});
      setPreview(await experienceApi.runbookPreviewYaml(yaml));
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function exportYaml() {
    const next = preview ?? await validate();
    if (!next) return;
    const suggested = `${next.spec.name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "runbook"}.yaml`;
    const path = await saveDialog({
      defaultPath: suggested,
      filters: [{ name: "Runbook YAML", extensions: ["yaml", "yml"] }],
    });
    if (!path) return;
    try {
      await experienceApi.runbookExportYaml(path, content);
      pushAlert("info", `Exported runbook YAML to ${path}`);
    } catch (reason) {
      setError(String(reason));
    }
  }

  function createNew() {
    setSelectedId("");
    setContent(NEW_RUNBOOK);
    setPreview(null);
    setVariables({ service: "nginx" });
    setError(null);
  }

  return (
    <div className="modal-backdrop" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
      <div className="modal wide runbook-studio" role="dialog" aria-modal="true" aria-label="Runbook Studio">
        <div className="modal-head">
          <div><span className="eyebrow">Automation authoring</span><strong>Runbook Studio</strong></div>
          <button className="ghost tiny" onClick={onClose}>✕</button>
        </div>
        <div className="modal-body">
          <div className="form-row three">
            <div>
              <label>Runbook</label>
              <select value={selectedId} onChange={(event) => setSelectedId(event.target.value)}>
                <option value="">New / imported runbook</option>
                {runbooks.map((runbook) => (
                  <option key={runbook.id} value={runbook.id}>{runbook.builtin ? "Built-in · " : ""}{runbook.name}</option>
                ))}
              </select>
            </div>
            <div><label>&nbsp;</label><button onClick={createNew}>New</button></div>
            <div className="flex" style={{ alignItems: "end" }}>
              <button onClick={() => void importYaml()}>Import YAML</button>
              <button onClick={() => void exportYaml()}>Export YAML</button>
            </div>
          </div>

          {selected?.builtin ? (
            <div className="warn-banner">Built-ins are immutable templates in Studio. Saving creates a user-owned copy instead of replacing the shipped runbook.</div>
          ) : null}

          <div className="runbook-studio-grid">
            <div>
              <label htmlFor="runbook-yaml">YAML</label>
              <textarea
                id="runbook-yaml"
                className="mono"
                spellCheck={false}
                rows={26}
                value={content}
                onChange={(event) => {
                  setContent(event.target.value);
                  setPreview(null);
                  setError(null);
                }}
              />
            </div>
            <div>
              <div className="section-title">Dry-run preview</div>
              {preview ? (
                <>
                  <div className={preview.valid ? "warn-banner ok" : "warn-banner"}>
                    {preview.valid
                      ? `✓ Valid · ${preview.steps.length} step${preview.steps.length === 1 ? "" : "s"}`
                      : `⚠ Unresolved: ${preview.unresolved_variables.join(", ")}`}
                  </div>
                  {Object.keys(preview.spec.variables).length ? (
                    <div className="studio-variables">
                      {Object.keys(preview.spec.variables).map((key) => (
                        <div key={key}>
                          <label>{key}</label>
                          <input
                            value={variables[key] ?? ""}
                            onChange={(event) => setVariables((current) => ({ ...current, [key]: event.target.value }))}
                          />
                        </div>
                      ))}
                      <button className="tiny" onClick={() => void validate()}>Re-render variables</button>
                    </div>
                  ) : null}
                  <div className="studio-step-list">
                    {preview.steps.map((step) => (
                      <div key={step.index} className="studio-step">
                        <div className="flex">
                          <strong>{step.index + 1}. {step.name}</strong>
                          {step.requires_confirmation ? <span className="status-badge status-warn">confirmation</span> : null}
                          {step.destructive ? <span className="status-badge status-crit">destructive</span> : null}
                        </div>
                        <code>{step.command}</code>
                        {step.unresolved_variables.length ? <small className="field-error">Unresolved: {step.unresolved_variables.join(", ")}</small> : null}
                      </div>
                    ))}
                  </div>
                </>
              ) : <div className="panel-hint">Validate the YAML to preview exactly rendered commands. Dry-run never opens SSH or executes a step.</div>}
            </div>
          </div>
          {error ? <div className="warn-banner">⚠ {error}</div> : null}
        </div>
        <div className="modal-foot">
          <button onClick={onClose}>Close</button>
          <button disabled={busy} onClick={() => void validate()}>{busy ? "Validating…" : "Validate / Dry run"}</button>
          <button className="primary" disabled={busy || Boolean(preview && !preview.valid)} onClick={() => void saveRunbook()}>Save runbook</button>
        </div>
      </div>
    </div>
  );
}
