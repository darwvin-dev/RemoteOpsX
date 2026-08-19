import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { normalizeRemoteError } from "./errors";
import type { DashboardSummary, RunbookPreview } from "./experienceTypes";

async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await tauriInvoke<T>(command, args);
  } catch (error) {
    throw normalizeRemoteError(error);
  }
}

export const runbookPreviewYaml = (contentYaml: string, variables?: Record<string, string>) =>
  invoke<RunbookPreview>("runbook_preview_yaml", {
    contentYaml,
    variables: variables ?? null,
  });

export const runbookPreviewSaved = (runbookId: string, variables?: Record<string, string>) =>
  invoke<RunbookPreview>("runbook_preview_saved", {
    runbookId,
    variables: variables ?? null,
  });

export const runbookImportYaml = (path: string) =>
  invoke<string>("runbook_import_yaml", { path });

export const runbookExportYaml = (path: string, contentYaml: string) =>
  invoke<void>("runbook_export_yaml", { path, contentYaml });

export const dashboardSummary = () =>
  invoke<DashboardSummary>("operator_dashboard_summary");
