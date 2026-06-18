import { useEffect, useState } from "react";
import * as api from "../api";
import { useStore } from "../store";
import type { RunbookSpec, RunbookStep, Server, StepResult } from "../types";

interface StepView extends RunbookStep {
  // resolved command after variable substitution
  resolved: string;
  state: "pending" | "running" | "success" | "failure" | "skipped";
  result?: StepResult;
  expanded: boolean;
}

/** Runbook execution view: preview steps, confirm destructive ones, run them
 *  one-by-one over SSH, capture per-step output and persist the run. */
export function RunbookRunner({ runbookId, server }: { runbookId: string; server: Server }) {
  const pushAlert = useStore((s) => s.pushAlert);
  const [spec, setSpec] = useState<RunbookSpec | null>(null);
  const [vars, setVars] = useState<Record<string, string>>({});
  const [steps, setSteps] = useState<StepView[]>([]);
  const [running, setRunning] = useState(false);
  const [confirmIdx, setConfirmIdx] = useState<number | null>(null);
  const [done, setDone] = useState(false);

  useEffect(() => {
    void api.runbookSpec(runbookId).then((sp) => {
      setSpec(sp);
      setVars(sp.variables ?? {});
      resetSteps(sp, sp.variables ?? {});
    }).catch((err) => pushAlert("error", `load runbook: ${err}`));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [runbookId]);

  function substitute(cmd: string, v: Record<string, string>): string {
    return cmd.replace(/\{\{\s*(\w+)\s*\}\}/g, (_, k) => v[k] ?? `{{${k}}}`);
  }

  function resetSteps(sp: RunbookSpec, v: Record<string, string>) {
    setSteps(sp.steps.map((s) => ({
      ...s,
      resolved: substitute(s.command, v),
      state: "pending",
      expanded: false,
    })));
    setDone(false);
  }

  function setStep(i: number, patch: Partial<StepView>) {
    setSteps((cur) => cur.map((s, idx) => (idx === i ? { ...s, ...patch } : s)));
  }

  // Execute steps sequentially, pausing at confirmation gates.
  async function runFrom(startIdx: number) {
    setRunning(true);
    const startedAt = new Date().toISOString();
    const collected: StepResult[] = [];
    let overall: "success" | "failure" = "success";

    for (let i = startIdx; i < steps.length; i++) {
      const step = steps[i];
      if (step.requires_confirmation && i !== startIdx - 1) {
        // gate: stop and ask for confirmation, unless we just confirmed this one
        if (confirmIdx !== i) {
          setConfirmIdx(i);
          setRunning(false);
          return; // resumes when user confirms
        }
      }
      setStep(i, { state: "running", expanded: true });
      const resolved = substitute(step.command, vars);
      try {
        const res = await api.runbookRunStep(server.id, { ...step, command: resolved });
        collected.push(res);
        setStep(i, { state: res.status, result: res });
        if (res.status === "failure") overall = "failure";
      } catch (err) {
        const res: StepResult = {
          name: step.name, command: resolved, stdout: "", stderr: String(err), exit_code: -1, status: "failure",
        };
        collected.push(res);
        setStep(i, { state: "failure", result: res });
        overall = "failure";
      }
      setConfirmIdx(null);
    }

    setRunning(false);
    setDone(true);
    try {
      await api.runbookRecordRun(runbookId, server.id, startedAt, overall, collected);
      pushAlert(overall === "success" ? "info" : "warn", `Runbook "${spec?.name}" finished: ${overall}`, server.id);
    } catch (err) {
      pushAlert("error", `record run: ${err}`);
    }
  }

  function start() {
    if (spec) resetSteps(spec, vars);
    // allow state to flush before running
    setTimeout(() => void runFrom(0), 0);
  }

  function confirmAndContinue() {
    const i = confirmIdx;
    if (i === null) return;
    // Mark as confirmed by setting confirmIdx to i then resuming from i.
    setConfirmIdx(i);
    setTimeout(() => void runFrom(i), 0);
  }

  if (!spec) return <div className="panel-hint">Loading runbook…</div>;

  return (
    <div className="runbook-runner">
      <div className="rb-header">
        <div>
          <h2>{spec.name}</h2>
          <p>{spec.description} · target <span className="mono">{server.name}</span></p>
        </div>
        <div className="flex">
          <button className="primary" disabled={running} onClick={start}>
            {running ? "Running…" : done ? "Run again" : "Run runbook"}
          </button>
        </div>
      </div>

      {Object.keys(vars).length > 0 && (
        <div className="metric-card" style={{ marginBottom: 12 }}>
          <div className="mc-label">Variables</div>
          <div className="form-row" style={{ marginTop: 6 }}>
            {Object.entries(vars).map(([k, v]) => (
              <div key={k}>
                <label>{k}</label>
                <input value={v} onChange={(e) => setVars((cur) => ({ ...cur, [k]: e.target.value }))} />
              </div>
            ))}
          </div>
        </div>
      )}

      {steps.map((s, i) => (
        <div key={i} className={`step ${s.state}`}>
          <div className="step-head" onClick={() => setStep(i, { expanded: !s.expanded })}>
            <span className="step-idx">
              {s.state === "success" ? "✓" : s.state === "failure" ? "✕" : s.state === "running" ? "•" : i + 1}
            </span>
            <span className="step-name">
              {s.name}
              {s.requires_confirmation && <span className="pill" style={{ marginLeft: 8 }}>confirm</span>}
            </span>
            <span className="step-cmd">{s.resolved}</span>
          </div>

          {confirmIdx === i && (
            <div className="confirm-box">
              This step requires confirmation. It will run:
              <div className="cmd-preview">{s.resolved}</div>
              <div className="flex" style={{ justifyContent: "flex-end" }}>
                <button className="tiny" onClick={() => { setConfirmIdx(null); setStep(i, { state: "skipped" }); }}>Skip</button>
                <button className="tiny primary" onClick={confirmAndContinue}>Confirm & run</button>
              </div>
            </div>
          )}

          {s.expanded && s.result && (
            <div className="step-body">
              <pre>{s.result.stdout || ""}{s.result.stderr ? `\n\x1b[stderr]\n${s.result.stderr}` : ""}{`\n— exit ${s.result.exit_code}`}</pre>
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
