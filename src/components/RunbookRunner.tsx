import { useEffect, useMemo, useRef, useState } from "react";
import * as api from "../api";
import { useStore } from "../store";
import {
  confirmStep,
  createRun,
  nextAction,
  recordResult,
  skipStep,
  type RunState,
} from "../runbookMachine";
import type { RunbookSpec, Server, StepResult } from "../types";

/** Executes one durable frontend run state across confirmation boundaries. */
export function RunbookRunner({ runbookId, server }: { runbookId: string; server: Server }) {
  const pushAlert = useStore((state) => state.pushAlert);
  const [spec, setSpec] = useState<RunbookSpec | null>(null);
  const [vars, setVars] = useState<Record<string, string>>({});
  const [run, setRun] = useState<RunState | null>(null);
  const [expanded, setExpanded] = useState<Set<number>>(() => new Set());
  const recordedRun = useRef<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void api.runbookSpec(runbookId).then((loaded) => {
      if (cancelled) return;
      setSpec(loaded);
      setVars(loaded.variables ?? {});
      setRun(null);
    }).catch((error) => pushAlert("error", `load runbook: ${error}`));
    return () => { cancelled = true; };
  }, [pushAlert, runbookId]);

  useEffect(() => {
    if (!run || run.phase !== "running") return;
    const advanced = nextAction(run);
    setRun(advanced.state);
  }, [run]);

  useEffect(() => {
    if (!run || run.phase !== "executing") return;
    const step = run.steps[run.cursor];
    if (!step) return;
    let cancelled = false;
    void api.runbookRunStep(server.id, step).then((result) => {
      if (!cancelled) setRun((current) => current ? recordResult(current, result) : current);
    }).catch((error) => {
      if (cancelled) return;
      const result: StepResult = {
        name: step.name,
        command: step.command,
        stdout: "",
        stderr: String(error),
        exit_code: -1,
        status: "failure",
      };
      setRun((current) => current ? recordResult(current, result) : current);
    });
    return () => { cancelled = true; };
  }, [run, server.id]);

  useEffect(() => {
    if (!run || run.phase !== "complete" || recordedRun.current === run.startedAt) return;
    recordedRun.current = run.startedAt;
    void api.runbookRecordRun(runbookId, server.id, run.startedAt, run.overall, run.results)
      .then(() => pushAlert(
        run.overall === "success" ? "info" : "warn",
        `Runbook "${spec?.name}" finished: ${run.overall}`,
        server.id,
      ))
      .catch((error) => pushAlert("error", `record run: ${error}`));
  }, [pushAlert, run, runbookId, server.id, spec?.name]);

  const previewSteps = useMemo(
    () => spec ? createRun(spec.steps, vars, "preview").steps : [],
    [spec, vars],
  );
  const steps = run?.steps ?? previewSteps;
  const completedSteps = steps.filter((step) => ["success", "failure", "skipped"].includes(step.state)).length;
  const progressPct = steps.length ? (completedSteps / steps.length) * 100 : 0;
  const active = run?.phase === "running" || run?.phase === "executing";

  function start() {
    if (!spec) return;
    recordedRun.current = null;
    setExpanded(new Set());
    setRun(createRun(spec.steps, vars));
  }

  function toggleExpanded(index: number) {
    setExpanded((current) => {
      const next = new Set(current);
      if (next.has(index)) next.delete(index); else next.add(index);
      return next;
    });
  }

  if (!spec) return <div className="panel-hint">Loading runbook…</div>;

  return (
    <div className="runbook-runner">
      <div className="rb-header">
        <div>
          <h2>{spec.name}</h2>
          <p>{spec.description} · target <span className="mono">{server.name}</span></p>
        </div>
        <button className="primary" disabled={active} onClick={start}>
          {active ? "Running…" : run?.phase === "complete" ? "Run again" : "Run runbook"}
        </button>
      </div>

      <div className="run-progress">
        <div>
          <strong>{completedSteps}/{steps.length}</strong>
          <span>{active ? "Running steps" : run?.phase === "complete" ? "Run complete" : "Ready to execute"}</span>
        </div>
        <div className="progress-track"><span style={{ width: `${progressPct}%` }} /></div>
      </div>

      {Object.keys(vars).length > 0 && (
        <div className="metric-card" style={{ marginBottom: 12 }}>
          <div className="mc-label">Variables</div>
          <div className="form-row" style={{ marginTop: 6 }}>
            {Object.entries(vars).map(([key, value]) => (
              <div key={key}>
                <label>{key}</label>
                <input disabled={active} value={value} onChange={(event) => setVars((current) => ({ ...current, [key]: event.target.value }))} />
              </div>
            ))}
          </div>
        </div>
      )}

      {steps.map((step, index) => {
        const isExpanded = expanded.has(index) || step.state === "running";
        const needsConfirmation = run?.pendingConfirmation === index;
        return (
          <div key={`${index}-${step.name}`} className={`step ${step.state}`}>
            <button className="step-head" onClick={() => toggleExpanded(index)}>
              <span className="step-idx">
                {step.state === "success" ? "✓" : step.state === "failure" ? "✕" : step.state === "skipped" ? "–" : step.state === "running" ? "•" : index + 1}
              </span>
              <span className="step-name">
                {step.name}
                {step.requires_confirmation && <span className="pill" style={{ marginLeft: 8 }}>confirm</span>}
              </span>
              <span className="step-cmd">{step.command}</span>
            </button>

            {needsConfirmation && run && (
              <div className="confirm-box">
                This step requires confirmation. It will run:
                <div className="cmd-preview">{step.command}</div>
                <div className="flex" style={{ justifyContent: "flex-end" }}>
                  <button className="tiny" onClick={() => setRun(skipStep(run))}>Skip</button>
                  <button className="tiny primary" onClick={() => setRun(confirmStep(run))}>Confirm & run</button>
                </div>
              </div>
            )}

            {isExpanded && step.result && (
              <div className="step-body">
                <pre>{step.result.stdout}{step.result.stderr ? `\n[stderr]\n${step.result.stderr}` : ""}{`\n— exit ${step.result.exit_code}`}</pre>
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
