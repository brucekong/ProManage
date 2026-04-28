export type ProjectStatus = "Starting" | "Running" | "Stopped" | "Error";

export interface ProjectConfig {
  id: string;
  name: string;
  path: string;
  project_kind?: "folder" | "workspace";
  command: string;
  scripts?: [string, string][];
  has_custom_command?: boolean;
  port: number;
  group: string;
  note: string;
  auto_start: boolean;
  show_build_scripts?: boolean;
  depends_on: string[];
  env_vars: [string, string][];
}

export interface Project {
  id: string;
  name: string;
  path: string;
  project_kind?: "folder" | "workspace";
  command: string;
  scripts?: [string, string][];
  has_custom_command?: boolean;
  port: number;
  group: string;
  note: string;
  status: ProjectStatus;
  pid: number | null;
  start_time: string | null;
  auto_start: boolean;
  show_build_scripts?: boolean;
}

export interface AppConfig {
  scan_dirs: string[];
  port_range_start: number;
  port_range_end: number;
  log_retention_days: number;
  theme: "system" | "light" | "dark";
  minimize_to_tray: boolean;
  auto_restore: boolean;
  auto_check_updates: boolean;
  update_endpoint: string;
  updater_pubkey: string;
  ide_vscode_command: string;
  ide_antigravity_command: string;
}

export interface AppUpdateInfo {
  version: string;
  current_version: string;
  notes?: string | null;
  date?: string | null;
}

export interface PortInfo {
  port: number;
  in_use: boolean;
  pid: number | null;
  process_name: string | null;
}

export interface GitStatusInfo {
  project_id: string;
  branch: string | null;
  dirty: boolean;
  ahead: number;
  behind: number;
  has_git: boolean;
}
