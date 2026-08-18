import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { normalizeRemoteError } from "./errors";
import type { HealthSnapshot } from "./types";
import type {
  AlertRule,
  AlertRuleInput,
  BackupRestoreReport,
  HealthPoint,
  MultiHostRequest,
  MultiHostRun,
  OperatorAlert,
  OperatorBootstrapReport,
  TransferJob,
  TransferRequest,
  TunnelPolicy,
  TunnelPolicyInput,
  TunnelReconcileReport,
} from "./operatorTypes";

async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await tauriInvoke<T>(command, args);
  } catch (error) {
    throw normalizeRemoteError(error);
  }
}

export const bootstrap = () => invoke<OperatorBootstrapReport>("operator_bootstrap");
export const healthCollect = (serverId: string) =>
  invoke<HealthSnapshot>("operator_health_collect", { serverId });
export const healthHistory = (serverId: string, limit = 720) =>
  invoke<HealthPoint[]>("health_history_list", { serverId, limit });

export const alertRulesList = () => invoke<AlertRule[]>("alert_rules_list");
export const alertRuleSave = (input: AlertRuleInput) =>
  invoke<AlertRule>("alert_rule_save", { input });
export const alertRuleDelete = (id: string) => invoke<void>("alert_rule_delete", { id });
export const alertsList = (limit = 200) => invoke<OperatorAlert[]>("operator_alerts_list", { limit });
export const alertAcknowledge = (id: string) =>
  invoke<void>("operator_alert_acknowledge", { id });

export const transferStart = (request: TransferRequest) =>
  invoke<TransferJob>("transfer_start", { request });
export const transferCancel = (id: string) => invoke<void>("transfer_cancel", { id });
export const transferJobsList = () => invoke<TransferJob[]>("transfer_jobs_list");
export const transferChmod = (serverId: string, remotePath: string, mode: string) =>
  invoke<void>("transfer_chmod", { serverId, remotePath, mode });

export const tunnelPoliciesList = () => invoke<TunnelPolicy[]>("tunnel_policies_list");
export const tunnelPolicySave = (input: TunnelPolicyInput) =>
  invoke<TunnelPolicy>("tunnel_policy_save", { input });
export const tunnelsReconcile = () => invoke<TunnelReconcileReport>("tunnels_reconcile");

export const multiHostRun = (request: MultiHostRequest) =>
  invoke<MultiHostRun>("multi_host_run", { request });
export const multiHostRunsList = (limit = 50) =>
  invoke<MultiHostRun[]>("multi_host_runs_list", { limit });

export const backupExport = (path: string, passphrase: string) =>
  invoke<void>("workspace_backup_export", { path, passphrase });
export const backupImport = (path: string, passphrase: string) =>
  invoke<BackupRestoreReport>("workspace_backup_import", { path, passphrase });
