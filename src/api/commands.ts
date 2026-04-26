import { invoke } from "@tauri-apps/api/core";
import type { ProjectConfig, AppConfig, PortInfo, AppUpdateInfo } from "../types/project";

// ─── Project Commands ───────────────────────

export async function scanProjects(dir: string): Promise<ProjectConfig[]> {
  return invoke("scan_projects", { dir });
}

export async function listProjects(): Promise<ProjectConfig[]> {
  return invoke("list_projects");
}

export async function addProject(project: ProjectConfig): Promise<ProjectConfig[]> {
  return invoke("add_project", { project });
}

export async function removeProject(id: string): Promise<ProjectConfig[]> {
  return invoke("remove_project", { id });
}

export async function updateProject(project: ProjectConfig): Promise<ProjectConfig[]> {
  return invoke("update_project", { project });
}

export async function saveProjectsConfig(): Promise<void> {
  return invoke("save_projects_config");
}

export async function readPackageScripts(dir: string): Promise<[string, string][]> {
  return invoke("read_package_scripts", { dir });
}

// ─── Process Commands ───────────────────────

export async function startProject(id: string): Promise<void> {
  return invoke("start_project", { id });
}

export async function startProjectCommand(
  id: string,
  processId: string,
  label: string,
  command: string
): Promise<void> {
  return invoke("start_project_command", { id, processId, label, command });
}

export async function stopProject(id: string): Promise<void> {
  return invoke("stop_project", { id });
}

export async function restartProject(id: string): Promise<void> {
  return invoke("restart_project", { id });
}

export async function getProcessStatuses(): Promise<[string, string][]> {
  return invoke("get_process_statuses");
}

export async function writeProjectInput(id: string, input: string): Promise<void> {
  return invoke("write_project_input", { id, input });
}

export async function resizePty(id: string, rows: number, cols: number): Promise<void> {
  return invoke("resize_pty", { id, rows, cols });
}

export async function stopAllProjects(): Promise<void> {
  return invoke("stop_all_projects");
}

export async function startAllProjects(): Promise<void> {
  return invoke("start_all_projects");
}

// ─── Port Commands ─────────────────────────

export async function checkPortUsage(port: number): Promise<PortInfo> {
  return invoke("check_port_usage", { port });
}

export async function findFreePort(): Promise<number> {
  return invoke("find_free_port_cmd");
}

export async function killPortProcess(port: number): Promise<void> {
  return invoke("kill_port_process", { port });
}

// ─── Config Commands ───────────────────────

export async function getConfig(): Promise<AppConfig> {
  return invoke("get_config");
}

export async function updateConfig(config: AppConfig): Promise<void> {
  return invoke("update_config", { config });
}

// ─── App Update Commands ───────────────────

export async function checkAppUpdate(): Promise<AppUpdateInfo | null> {
  return invoke("check_app_update");
}

export async function installAppUpdate(): Promise<void> {
  return invoke("install_app_update");
}

export async function relaunchApp(): Promise<void> {
  return invoke("relaunch_app");
}

// ─── Health Commands ───────────────────────

export async function checkProjectHealth(port: number, url: string | null = null): Promise<string> {
  return invoke("check_project_health", { port, url });
}
