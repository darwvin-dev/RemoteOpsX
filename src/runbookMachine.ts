import type { RunbookStep, StepResult } from "./types";

export type RunPhase = "running" | "executing" | "waiting_confirmation" | "complete";

export interface PreparedRunbookStep extends RunbookStep {
  command: string;
  state: "pending" | "running" | "success" | "failure" | "skipped";
  result?: StepResult;
}

export interface RunState {
  startedAt: string;
  steps: PreparedRunbookStep[];
  cursor: number;
  results: StepResult[];
  overall: "success" | "failure";
  pendingConfirmation: number | null;
  confirmedCursor: boolean;
  phase: RunPhase;
}

export interface RunAction {
  index: number;
  step: PreparedRunbookStep;
}

function substitute(command: string, variables: Record<string, string>): string {
  return command.replace(/\{\{\s*(\w+)\s*\}\}/g, (_, key: string) => variables[key] ?? `{{${key}}}`);
}

export function createRun(
  steps: RunbookStep[],
  variables: Record<string, string>,
  startedAt = new Date().toISOString(),
): RunState {
  return {
    startedAt,
    steps: steps.map((step) => ({ ...step, command: substitute(step.command, variables), state: "pending" })),
    cursor: 0,
    results: [],
    overall: "success",
    pendingConfirmation: null,
    confirmedCursor: false,
    phase: "running",
  };
}

export function nextAction(state: RunState): { state: RunState; action: RunAction | null } {
  if (state.cursor >= state.steps.length) {
    return { state: { ...state, phase: "complete", pendingConfirmation: null }, action: null };
  }
  const step = state.steps[state.cursor];
  if (step.requires_confirmation && !state.confirmedCursor) {
    return {
      state: { ...state, phase: "waiting_confirmation", pendingConfirmation: state.cursor },
      action: null,
    };
  }
  const runningStep = { ...step, state: "running" as const };
  return {
    state: {
      ...state,
      phase: "executing",
      pendingConfirmation: null,
      steps: state.steps.map((entry, index) => index === state.cursor ? runningStep : entry),
    },
    action: { index: state.cursor, step: runningStep },
  };
}

export function confirmStep(state: RunState): RunState {
  if (state.pendingConfirmation !== state.cursor) return state;
  return { ...state, phase: "running", pendingConfirmation: null, confirmedCursor: true };
}

export function skipStep(state: RunState): RunState {
  if (state.pendingConfirmation !== state.cursor) return state;
  const step = state.steps[state.cursor];
  const result: StepResult = {
    name: step.name,
    command: step.command,
    stdout: "",
    stderr: "Skipped by user",
    exit_code: -1,
    status: "skipped",
  };
  return {
    ...state,
    cursor: state.cursor + 1,
    results: [...state.results, result],
    pendingConfirmation: null,
    confirmedCursor: false,
    phase: "running",
    steps: state.steps.map((entry, index) => index === state.cursor
      ? { ...entry, state: "skipped", result }
      : entry),
  };
}

export function recordResult(state: RunState, result: StepResult): RunState {
  if (state.cursor >= state.steps.length) return state;
  return {
    ...state,
    cursor: state.cursor + 1,
    results: [...state.results, result],
    overall: result.status === "failure" ? "failure" : state.overall,
    pendingConfirmation: null,
    confirmedCursor: false,
    phase: "running",
    steps: state.steps.map((entry, index) => index === state.cursor
      ? { ...entry, state: result.status, result }
      : entry),
  };
}
