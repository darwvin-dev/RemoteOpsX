import { useEffect, useMemo, useRef, useState } from "react";
import * as api from "../api";
import * as experienceApi from "../experienceApi";
import { useStore } from "../store";
import {
  confirmStep,
  createRun,
  nextAction,
  recordResult,
  skipStep,
  type RunState,
} from "../runbookMachine";
import type { RunbookPreview } from "../experienceTypes";
import type { RunbookSpec, RunbookStep, Server, StepResult } from "../types";

/** Executes one durable run state across confirmation boundaries. Every actual
 * execution is prepared by the Rust backend first, so variable rendering and
 * destructive-command confirmation use the same policy as Studio dry-run. */
export function RunbookRunner({ runbookId, server }: { runbookId: string; server: Server }) {
  const pushAlert = useStore((state) => state.pushAlert);
  const [spec, setSpec] = useState<RunbookSpec | null>(null);
  const [vars, setVars] = useState<Record<string, string>>({});
  const [prepared, setPrepared] = useState<RunbookPreview | null>(null);
  const [preparing, setPreparing] = useState(false);
  const [run, setRun] = useState<RunState | null>(null);
  const [runOriginIndex, setRunOriginIndex] = useState(0);
  const [expanded, setExpanded] = useState<Set<number>>(() => new Set());
  const recordedRun = useRef<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void api.runbookSpec(runbookId).then((loaded) => {
      if (cancelled) return;
      setSpec(loaded);
      setVars(loaded.variables ?? {});
      setPrepared(null);
      setRun(null);
      setRunOriginIndex(0);
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

  const previewSteps = useMemo(() => {
    if (prepared) {
      return createRun(preparedSteps(prepared), {}, "preview").steps;
    }
    return spec ? createRun(spec.steps, vars, "preview").steps : [];
  }, [prepared, spec, vars]);
  const steps = run?.steps ?? previewSteps;
  const completedSteps = steps.filter((step) => ["success", "failure", "skipped"].includes(step.state)).length;
  const progressPct = steps.length ? (completedSteps / steps.length) * 100 : 0;
  const active = preparing || Boolean(run && ["running", "executing", "waiting_confirmation"].includes(run.phase));
  const firstFailedResult = run?.phase === "complete"
    ? run.results.findIndex((result) => result.status === "failure")
    : -1;
  const retryOriginalIndex = firstFailedResult >= 0 ? runOriginIndex + firstFailedResult : -1;

  async function startFrom(index: number) {
    if (!spec || active) return;
    setPreparing(true);
    try {
      const nextPrepared = await experienceApi.runbookPreviewSaved(runbookId, vars);
      if (!nextPrepared.valid) {
        pushAlert(
          "warn",
          `Runbook has unresolved variables: ${nextPrepared.unresolved_variables.join(", ")}`,
          server.id,
        );
        return;
      }
      const executable = preparedSteps(nextPrepared);
      const bounded = Math.max(0, Math.min(index, executable.length - 1));
      recordedRun.current = null;
      setExpanded(new Set());
      setPrepared(nextPrepared);
      setRunOriginIndex(bounded);
      setRun(createRun(executable.slice(bounded), {}));
    } catch (error) {
      pushAlert("error", `prepare runbook: ${error}`, server.id);
    } finally {
      setPreparing(false);
    }
  }

  function start() {
    void startFrom(0);
  }

  function retryFromFailure() {
    if (retryOriginalIndex < 0) return;
    void startFrom(retryOriginalIndex);
  }

  function updateVariable(key: string, value: string) {
    setVars((current) => ({ ...current, [key]: value }));
    // Never imply an old server-rendered preview represents changed inputs.
    setPrepared(null);
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
          {runOriginIndex > 0 && run ? <small className="muted">Retry run starts at original step {runOriginIndex + 1}.</small> : null}
        </div>
        <div className="flex">
          {retryOriginalIndex >= 0 && !active ? (
            <button className="warn" onClick={retryFromFailure}>
              Retry from step {retryOriginalIndex + 1}
            </button>
          ) : null}
          <button className="primary" disabled={active} onClick={start}>
            {preparing ? "Preparing…" : active ? "Running…" : run?.phase === "complete" ? "Run from start" : "Run runbook"}
          </button>
        </div>
      </div>

      {prepared && !run ? (
        <div className="warn-banner ok">✓ Commands prepared by backend policy.</div>
      ) : null}

      {retryOriginalIndex >= 0 && !active ? (
        <div className="warn-banner">
          The previous run failed at <strong>{spec.steps[retryOriginalIndex]?.name}</strong>. Retry executes that step and every step after it, with confirmation recalculated from the current rendered commands.
        </div>
      ) : null}

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
                <input disabled={active} value={value} onChange={(event) => updateVariable(key, event.target.value)} />
              </div>
            ))}
          </div>
        </div>
      )}

      {steps.map((step, index) => {
        const isExpanded = expanded.has(index) || step.state === "running";
        const needsConfirmation = run?.pendingConfirmation === index;
        const originalIndex = run ? runOriginIndex + index : index;
        return (
          <div key={`${originalIndex}-${step.name}`} className={`step ${step.state}`}>
            <button className="step-head" onClick={() => toggleExpanded(index)}>
              <span className="step-idx">
                {step.state === "success" ? "✓" : step.state === "failure" ? "✕" : step.state === "skipped" ? "–" : step.state === "running" ? "•" : originalIndex + 1}
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

function preparedSteps(preview: RunbookPreview): RunbookStep[] {
  return preview.steps.map((step) => ({
    name: step.name,
    command: step.command,
    requires_confirmation: step.requires_confirmation,
    success_pattern: step.success_pattern ?? null,
    failure_pattern: step.failure_pattern ?? null,
  }));
}
