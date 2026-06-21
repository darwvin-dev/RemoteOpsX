// Shared TypeScript types. These mirror the Rust serde models exactly.

export type Protocol = "ssh" | "sftp" | "ftp" | "rdp" | "vnc";
export type AuthType = "password" | "key";
export type Environment = "production" | "staging" | "dev";

export interface Server {
  id: string;
  name: string;
  host: string;
  port: number;
  ftp_port?: number | null;
  rdp_port?: number | null;
  vnc_port?: number | null;
  username: string;
  protocols: Protocol[];
  auth_type: AuthType;
  private_key_path?: string | null;
  tags: string[];
  group_name?: string | null;
  environment: Environment;
  notes?: string | null;
  created_at: string;
  updated_at: string;
}

export interface ServerInput {
  id?: string;
  name: string;
  host: string;
  port: number;
  ftp_port?: number | null;
  rdp_port?: number | null;
  vnc_port?: number | null;
  username: string;
  protocols: Protocol[];
  auth_type: AuthType;
  private_key_path?: string | null;
  tags: string[];
  group_name?: string | null;
  environment: Environment;
  notes?: string | null;
  secret?: string | null;
}

export interface CommandOutput {
  stdout: string;
  stderr: string;
  exit_code: number;
  success: boolean;
}

export interface DiskInfo {
  filesystem: string;
  size_kb: number;
  used_kb: number;
  use_percent: number;
  mount: string;
}

export interface ProcInfo {
  pid: string;
  command: string;
  cpu: number;
  mem: number;
}

export interface HealthSnapshot {
  os_name: string;
  kernel: string;
  hostname: string;
  uptime_secs: number;
  load1: number;
  load5: number;
  load15: number;
  cpu_percent: number;
  mem_percent: number;
  mem_total_kb: number;
  mem_used_kb: number;
  swap_percent: number;
  swap_total_kb: number;
  swap_used_kb: number;
  net_rx_rate: number;
  net_tx_rate: number;
  disks: DiskInfo[];
  top_cpu: ProcInfo[];
  top_mem: ProcInfo[];
  listening_ports: string[];
  failed_services: string[];
  warnings: string[];
}

export interface Runbook {
  id: string;
  name: string;
  description: string;
  content_yaml: string;
  builtin: boolean;
  created_at: string;
  updated_at: string;
}

export interface RunbookStep {
  name: string;
  command: string;
  requires_confirmation?: boolean;
  success_pattern?: string | null;
  failure_pattern?: string | null;
}

export interface RunbookSpec {
  name: string;
  description: string;
  target_os?: string | null;
  variables: Record<string, string>;
  steps: RunbookStep[];
}

export interface StepResult {
  name: string;
  command: string;
  stdout: string;
  stderr: string;
  exit_code: number;
  status: "success" | "failure" | "skipped";
}

export interface RunbookRun {
  id: string;
  runbook_id: string;
  server_id: string;
  started_at: string;
  ended_at?: string | null;
  status: string;
  results: StepResult[];
}

export interface Tunnel {
  id: string;
  server_id: string;
  type: "local" | "remote" | "dynamic";
  local_host?: string | null;
  local_port: number;
  remote_host?: string | null;
  remote_port?: number | null;
  status: string;
  created_at: string;
}

export interface RemoteFile {
  name: string;
  is_dir: boolean;
  size: number;
  permissions: string;
}

// ---- UI-only types ----

export type TabKind = "ssh" | "rdp" | "vnc" | "logs" | "runbook" | "sftp" | "ftp";

export interface Tab {
  id: string;
  kind: TabKind;
  serverId: string;
  title: string;
  // For runbook tabs: which runbook to run.
  runbookId?: string;
}

export type RightPanelView = "health" | "services" | "notes" | "snippets";
export type BottomPanelView = "output" | "history" | "alerts";
