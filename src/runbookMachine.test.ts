import { describe, expect, it } from "vitest";
import {
  confirmStep,
  createRun,
  nextAction,
  recordResult,
  skipStep,
} from "./runbookMachine";
import type { RunbookStep, StepResult } from "./types";

const result = (name: string, status: StepResult["status"] = "success"): StepResult => ({
  name,
  command: `echo ${name}`,
  stdout: name,
  stderr: "",
  exit_code: status === "success" ? 0 : 1,
  status,
});

describe("runbook execution state", () => {
  it("pauses before a confirmation-gated step", () => {
    const steps: RunbookStep[] = [{ name: "restart", command: "restart", requires_confirmation: true }];
    const advanced = nextAction(createRun(steps, {}, "2026-06-20T00:00:00Z"));
    expect(advanced.action).toBeNull();
    expect(advanced.state.phase).toBe("waiting_confirmation");
    expect(advanced.state.pendingConfirmation).toBe(0);
    expect(advanced.state.results).toEqual([]);
  });

  it("confirms and resumes the same run without losing earlier results", () => {
    const steps: RunbookStep[] = [
      { name: "inspect", command: "inspect" },
      { name: "restart", command: "restart", requires_confirmation: true },
    ];
    let state = createRun(steps, {}, "2026-06-20T00:00:00Z");
    let advanced = nextAction(state);
    state = recordResult(advanced.state, result("inspect"));
    advanced = nextAction(state);
    state = confirmStep(advanced.state);
    advanced = nextAction(state);
    expect(advanced.action?.step.name).toBe("restart");
    state = recordResult(advanced.state, result("restart"));
    expect(state.startedAt).toBe("2026-06-20T00:00:00Z");
    expect(state.results.map((entry) => entry.name)).toEqual(["inspect", "restart"]);
    expect(nextAction(state).state.phase).toBe("complete");
  });

  it("records a skipped gate and continues with the next step", () => {
    const steps: RunbookStep[] = [
      { name: "restart", command: "restart", requires_confirmation: true },
      { name: "verify", command: "verify" },
    ];
    let advanced = nextAction(createRun(steps, {}, "start"));
    const skipped = skipStep(advanced.state);
    expect(skipped.results[0].status).toBe("skipped");
    advanced = nextAction(skipped);
    expect(advanced.action?.step.name).toBe("verify");
  });

  it("starts reruns cleanly with current variable values", () => {
    const steps: RunbookStep[] = [{ name: "service", command: "restart {{ service }}" }];
    const first = createRun(steps, { service: "nginx" }, "first");
    const second = createRun(steps, { service: "postgres" }, "second");
    expect(nextAction(first).action?.step.command).toBe("restart nginx");
    expect(nextAction(second).action?.step.command).toBe("restart postgres");
    expect(second.results).toEqual([]);
    expect(second.startedAt).toBe("second");
  });
});
