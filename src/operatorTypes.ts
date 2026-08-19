import type { CommandOutput } from "./types";

export interface HealthPoint {
  server_id: string;
  sampled_at: string;
  cpu_percent: number;
  mem_percent: number;
  swap_percent: number;
  load1: number;
  net_rx_rate: number;
  net_tx_rate: number;
  max_disk_percent: number;
  failed_services: number;
}

export interface AlertRule {
  id: string;
  server_id?: string | null;
  metric: string;
  comparison: "gt" | "gte" | "lt" | "lte";
  threshold: number;
  consecutive_samples: number;
  cooldown_seconds: number;
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface AlertRuleInput {
  id?: string | null;
  server_id?: string | null;
  metric: string;
  comparison: "gt" | "gte" | "lt" | "lte";
  threshold: number;
  consecutive_samples: number;
  cooldown_seconds: number;
  enabled: boolean;
}

export interface OperatorAlert {
  id: string;
  rule_id: string;
  server_id: string;
  metric: string;
  value: number;
  threshold: number;
  message: string;
  fired_at: string;
  acknowledged_at?: string | null;
}

export interface TunnelPolicy {
  tunnel_id: string;
  autostart: boolean;
  auto_reconnect: boolean;
  health_interval_secs: number;
  updated_at: string;
}

export interface TunnelPolicyInput {
  tunnel_id: string;
  autostart: boolean;
  auto_reconnect: boolean;
  health_interval_secs: number;
}

export type TransferDirection = "upload" | "download";
export interface TransferRequest {
  server_id: string;
  direction: TransferDirection;
  source: string;
  destination: string;
  recursive: boolean;
}
export interface TransferJob extends TransferRequest {
  id: string;
  status: "running" | "completed" | "failed" | "cancelled" | string;
  total_bytes?: number | null;
  transferred_bytes?: number | null;
  progress_percent?: number | null;
  started_at: string;
  ended_at?: string | null;
  error?: string | null;
}

export interface MultiHostRequest {
  run_id?: string | null;
  server_ids: string[];
  command: string;
  concurrency: number;
  production_confirmed: boolean;
  destructive_confirmed: boolean;
}
export interface MultiHostResult {
  server_id: string;
  server_name: string;
  environment: string;
  output: CommandOutput;
}
export interface MultiHostRun {
  id: string;
  command: string;
  status: string;
  started_at: string;
  ended_at: string;
  results: MultiHostResult[];
}

export interface TunnelReconcileReport {
  active: number;
  restarted: number;
  failed: number;
  stopped: number;
}
export interface OperatorBootstrapReport { tunnel_reconcile: TunnelReconcileReport; }

export interface BackupRestoreReport {
  servers: number;
  runbooks: number;
  snippets: number;
  jump_hosts: number;
  alert_rules: number;
  tunnels: number;
  password_reentry_server_ids: string[];
}
