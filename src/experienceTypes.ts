import type { RunbookRun, RunbookSpec } from "./types";
import type { MultiHostRun, OperatorAlert } from "./operatorTypes";

export interface RunbookPreviewStep {
  index: number;
  name: string;
  command: string;
  requires_confirmation: boolean;
  destructive: boolean;
  unresolved_variables: string[];
}

export interface RunbookPreview {
  spec: RunbookSpec;
  steps: RunbookPreviewStep[];
  unresolved_variables: string[];
  valid: boolean;
}

export interface DashboardServer {
  server_id: string;
  name: string;
  environment: string;
  status: "healthy" | "warning" | "critical" | "unknown" | string;
  sampled_at?: string | null;
  cpu_percent?: number | null;
  mem_percent?: number | null;
  max_disk_percent?: number | null;
  failed_services?: number | null;
  unacknowledged_alerts: number;
}

export interface DashboardSummary {
  servers_total: number;
  healthy: number;
  warning: number;
  critical: number;
  unknown: number;
  active_tunnels: number;
  failed_tunnels: number;
  unacknowledged_alerts: number;
  servers: DashboardServer[];
  recent_alerts: OperatorAlert[];
  recent_runbooks: RunbookRun[];
  recent_multi_host: MultiHostRun[];
}
