use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_updater::{Update, UpdaterExt};
use url::Url;

use crate::health::{self, HealthCheckType};

use crate::config;
use crate::port;
use crate::process::ProcessManager;
use crate::project;
use crate::types::*;

pub struct AppState {
    pub projects: Mutex<Vec<ProjectConfig>>,
    pub config: Mutex<AppConfig>,
    pub process_manager: Arc<ProcessManager>,
    pub app_dir: PathBuf,
}

pub struct PendingUpdate(pub Mutex<Option<Update>>);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct UpdateMetadata {
    pub version: String,
    pub current_version: String,
    pub notes: Option<String>,
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct GitStatusMetadata {
    pub project_id: String,
    pub branch: Option<String>,
    pub dirty: bool,
    pub ahead: u32,
    pub behind: u32,
    pub has_git: bool,
}

fn updater_is_configured(cfg: &AppConfig) -> bool {
    !cfg.update_endpoint.trim().is_empty() && !cfg.updater_pubkey.trim().is_empty()
}

fn default_shell() -> (String, String) {
    if cfg!(target_os = "windows") {
        return ("cmd".to_string(), "/C".to_string());
    }

    if std::path::Path::new("/bin/zsh").exists() {
        return ("/bin/zsh".to_string(), "-lic".to_string());
    }

    if let Ok(shell) = std::env::var("SHELL") {
        if shell.contains("zsh") || shell.contains("bash") {
            return (shell, "-lic".to_string());
        }
    }

    if std::path::Path::new("/bin/bash").exists() {
        return ("/bin/bash".to_string(), "-lic".to_string());
    }

    ("/bin/sh".to_string(), "-c".to_string())
}

fn shell_escape(value: &str) -> String {
    let escaped = value.replace('\'', r"'\''");
    format!("'{}'", escaped)
}

fn project_working_dir(project: &ProjectConfig) -> String {
    if project.project_kind == "workspace" {
        return Path::new(&project.path)
            .parent()
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_else(|| project.path.clone());
    }

    project.path.clone()
}

fn git_output(dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn is_inside_git_work_tree(dir: &Path) -> bool {
    git_output(dir, &["rev-parse", "--is-inside-work-tree"])
        .map(|value| value == "true")
        .unwrap_or(false)
}

fn resolve_workspace_folder_path(root_dir: &Path, raw_path: &str) -> PathBuf {
    let folder_path = Path::new(raw_path);
    if folder_path.is_absolute() {
        folder_path.to_path_buf()
    } else {
        root_dir.join(folder_path)
    }
}

fn parse_cd_target(command: &str) -> Option<PathBuf> {
    let rest = command.trim().strip_prefix("cd ")?;
    let (dir, _) = rest.split_once("&&")?;
    let mut clean_dir = dir.trim().to_string();

    if clean_dir.len() >= 2 {
        let first = clean_dir.chars().next();
        let last = clean_dir.chars().last();
        if matches!((first, last), (Some('\''), Some('\'')) | (Some('"'), Some('"'))) {
            clean_dir = clean_dir[1..clean_dir.len() - 1].to_string();
        }
    }

    Some(PathBuf::from(clean_dir.replace("'\\''", "'")))
}

fn workspace_git_candidate_dirs(project: &ProjectConfig) -> Vec<PathBuf> {
    let workspace_path = PathBuf::from(&project.path);
    let root_dir = workspace_path.parent().unwrap_or_else(|| Path::new(""));
    let mut dirs = Vec::new();
    let mut seen = HashSet::new();

    let mut push_dir = |dir: PathBuf| {
        let key = dir.to_string_lossy().to_string();
        if seen.insert(key) {
            dirs.push(dir);
        }
    };

    if let Ok(content) = std::fs::read_to_string(&workspace_path) {
        if let Ok(json) = serde_json::from_str::<Value>(&content) {
            if let Some(folders) = json.get("folders").and_then(|value| value.as_array()) {
                for folder in folders {
                    if let Some(raw_path) = folder.get("path").and_then(|value| value.as_str()) {
                        push_dir(resolve_workspace_folder_path(root_dir, raw_path));
                    }
                }
            }
        }
    }

    for (_, command) in &project.scripts {
        if let Some(dir) = parse_cd_target(command) {
            push_dir(dir);
        }
    }

    push_dir(root_dir.to_path_buf());
    dirs
}

fn git_working_dir_for_project(project: &ProjectConfig) -> Option<PathBuf> {
    let candidates = if project.project_kind == "workspace" {
        workspace_git_candidate_dirs(project)
    } else {
        vec![PathBuf::from(project_working_dir(project))]
    };

    candidates
        .into_iter()
        .find(|dir| dir.exists() && is_inside_git_work_tree(dir))
}

fn git_status_for_project(project: &ProjectConfig) -> GitStatusMetadata {
    let Some(working_dir) = git_working_dir_for_project(project) else {
        return GitStatusMetadata {
            project_id: project.id.clone(),
            branch: None,
            dirty: false,
            ahead: 0,
            behind: 0,
            has_git: false,
        };
    };

    let branch = git_output(&working_dir, &["branch", "--show-current"])
        .filter(|value| !value.is_empty())
        .or_else(|| git_output(&working_dir, &["rev-parse", "--short", "HEAD"]));
    let dirty = git_output(&working_dir, &["status", "--porcelain"])
        .map(|value| !value.is_empty())
        .unwrap_or(false);
    let (ahead, behind) = git_output(&working_dir, &["rev-list", "--left-right", "--count", "HEAD...@{upstream}"])
        .and_then(|value| {
            let mut parts = value.split_whitespace();
            let ahead = parts.next()?.parse::<u32>().ok()?;
            let behind = parts.next()?.parse::<u32>().ok()?;
            Some((ahead, behind))
        })
        .unwrap_or((0, 0));

    GitStatusMetadata {
        project_id: project.id.clone(),
        branch,
        dirty,
        ahead,
        behind,
        has_git: true,
    }
}

// ─── Project Commands ───────────────────────────────────────

#[tauri::command]
pub fn scan_projects(
    state: State<'_, AppState>,
    dir: String,
) -> Result<Vec<ProjectConfig>, String> {
    let scanned = project::scan_directory(&dir);
    let mut existing = state.projects.lock().map_err(|e| e.to_string())?;
    let merged = project::merge_scanned_projects(&existing, scanned);
    *existing = merged.clone();
    project::save_projects(&config::projects_path(&state.app_dir), &existing)?;
    Ok(merged)
}

#[tauri::command]
pub fn list_projects(state: State<'_, AppState>) -> Result<Vec<ProjectConfig>, String> {
    let mut projects = state.projects.lock().map_err(|e| e.to_string())?;
    if project::repair_project_configs(&mut projects) {
        project::save_projects(&config::projects_path(&state.app_dir), &projects)?;
    }
    Ok(projects.clone())
}

#[tauri::command]
pub fn add_project(
    state: State<'_, AppState>,
    project: ProjectConfig,
) -> Result<Vec<ProjectConfig>, String> {
    let mut projects = state.projects.lock().map_err(|e| e.to_string())?;
    // Check for duplicate path
    if projects.iter().any(|p| p.path == project.path) {
        return Err(format!("Project at path '{}' already exists", project.path));
    }
    let mut p = project::enrich_project_config(project);
    if p.id.is_empty() {
        p.id = uuid::Uuid::new_v4().to_string();
    }
    projects.push(p);
    project::save_projects(&config::projects_path(&state.app_dir), &projects)?;
    Ok(projects.clone())
}

#[tauri::command]
pub fn remove_project(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<ProjectConfig>, String> {
    let mut projects = state.projects.lock().map_err(|e| e.to_string())?;
    projects.retain(|p| p.id != id);
    project::save_projects(&config::projects_path(&state.app_dir), &projects)?;
    Ok(projects.clone())
}

#[tauri::command]
pub fn update_project(
    state: State<'_, AppState>,
    project: ProjectConfig,
) -> Result<Vec<ProjectConfig>, String> {
    let mut projects = state.projects.lock().map_err(|e| e.to_string())?;
    if let Some(pos) = projects.iter().position(|p| p.id == project.id) {
        projects[pos] = project::enrich_project_config(project);
        project::save_projects(&config::projects_path(&state.app_dir), &projects)?;
        Ok(projects.clone())
    } else {
        Err(format!("Project with id '{}' not found", project.id))
    }
}

#[tauri::command]
pub fn reorder_projects(
    state: State<'_, AppState>,
    project_ids: Vec<String>,
) -> Result<Vec<ProjectConfig>, String> {
    let mut projects = state.projects.lock().map_err(|e| e.to_string())?;
    let mut ordered = Vec::with_capacity(projects.len());

    for id in &project_ids {
        if let Some(pos) = projects.iter().position(|project| &project.id == id) {
            ordered.push(projects.remove(pos));
        }
    }

    ordered.append(&mut projects);
    project::save_projects(&config::projects_path(&state.app_dir), &ordered)?;
    *projects = ordered.clone();
    Ok(ordered)
}

#[tauri::command]
pub fn save_projects_config(state: State<'_, AppState>) -> Result<(), String> {
    let projects = state.projects.lock().map_err(|e| e.to_string())?;
    project::save_projects(&config::projects_path(&state.app_dir), &projects)
}

#[tauri::command]
pub fn read_package_scripts(dir: String) -> Result<Vec<(String, String)>, String> {
    project::read_package_scripts(&dir)
}

#[tauri::command]
pub fn get_git_statuses(state: State<'_, AppState>) -> Result<Vec<GitStatusMetadata>, String> {
    let projects = state.projects.lock().map_err(|e| e.to_string())?.clone();
    Ok(projects.iter().map(git_status_for_project).collect())
}

// ─── Process Commands ───────────────────────────────────────

#[tauri::command]
pub async fn start_project(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    // Extract data from State before any await
    let (config, all_projects, port_range, pm) = {
        let mut projects = state.projects.lock().map_err(|e| e.to_string())?;
        if project::repair_project_configs(&mut projects) {
            project::save_projects(&config::projects_path(&state.app_dir), &projects)?;
        }
        let cfg = state.config.lock().map_err(|e| e.to_string())?;
        let target = projects.iter().find(|p| p.id == id).cloned();
        let all = projects.clone();
        let range = (cfg.port_range_start, cfg.port_range_end);
        (target, all, range, state.process_manager.clone())
    };

    let config = config.ok_or_else(|| format!("Project '{}' not found", id))?;
    if config.project_kind == "workspace" {
        return Err("Workspace projects can be opened in an IDE, but cannot be started.".to_string());
    }

    // Use scheduler for dependency chain (async, pm is Arc so Send)
    crate::scheduler::start_with_deps(&id, &all_projects, &pm, &app, port_range).await?;

    tracing::info!(project = %config.name, "Project started with deps");
    Ok(())
}

#[tauri::command]
pub fn start_project_command(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    process_id: String,
    label: String,
    command: String,
) -> Result<(), String> {
    let (project, pm) = {
        let projects = state.projects.lock().map_err(|e| e.to_string())?;
        let project = projects
            .iter()
            .find(|p| p.id == id)
            .cloned()
            .ok_or_else(|| format!("Project '{}' not found", id))?;
        (project, state.process_manager.clone())
    };

    let process_name = format!("{} · {}", project.name, label);
    let working_dir = project_working_dir(&project);
    let (pid, _) = pm.start(&process_id, &process_name, &command, &working_dir)?;

    let _ = app.emit(
        "project-status-changed",
        serde_json::json!({
            "id": process_id,
            "status": "Starting",
            "pid": pid,
        }),
    );

    Ok(())
}

#[tauri::command]
pub fn stop_project(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    let pm = state.process_manager.clone();
    pm.stop(&id)?;

    let _ = app.emit(
        "project-status-changed",
        serde_json::json!({
            "id": id,
            "status": "Stopped",
        }),
    );

    tracing::info!(project_id = %id, "Project stopped");
    Ok(())
}

#[tauri::command]
pub async fn restart_project(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let pm = state.process_manager.clone();
    pm.stop(&id).ok();

    let _ = app.emit(
        "project-status-changed",
        serde_json::json!({
            "id": id.clone(),
            "status": "Stopped",
        }),
    );

    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

    start_project(app, state, id).await
}

#[tauri::command]
pub fn get_process_statuses(state: State<'_, AppState>) -> Vec<(String, String)> {
    let projects = state.projects.lock().map(|p| p.clone()).unwrap_or_default();
    let pm = state.process_manager.clone();
    projects
        .iter()
        .map(|p| {
            let status = match pm.get_process_status(&p.id) {
                ProjectStatus::Running => "Running",
                ProjectStatus::Stopped => "Stopped",
                ProjectStatus::Error(_) => "Error",
            };
            (p.id.clone(), status.to_string())
        })
        .collect()
}

#[tauri::command]
pub fn write_project_input(
    state: State<'_, AppState>,
    id: String,
    input: String,
) -> Result<(), String> {
    state.process_manager.write_input(&id, &input)
}

#[tauri::command]
pub fn resize_pty(
    state: State<'_, AppState>,
    id: String,
    rows: u16,
    cols: u16,
) -> Result<(), String> {
    state.process_manager.resize_pty(&id, rows, cols)
}

#[tauri::command]
pub fn stop_all_projects(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let pm = state.process_manager.clone();
    let results = pm.stop_all();
    for (id, _) in &results {
        let _ = app.emit(
            "project-status-changed",
            serde_json::json!({
                "id": id,
                "status": "Stopped",
            }),
        );
    }
    Ok(())
}

#[tauri::command]
pub async fn start_all_projects(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let ids: Vec<String> = state
        .projects
        .lock()
        .map_err(|e| e.to_string())?
        .iter()
        .map(|p| p.id.clone())
        .collect();
    for id in ids {
        let _ = start_project(app.clone(), state.clone(), id).await;
    }
    Ok(())
}

// ─── Port Commands ──────────────────────────────────────────

#[tauri::command]
pub fn check_port_usage(port: u16) -> PortInfo {
    port::check_port(port)
}

#[tauri::command]
pub fn find_free_port_cmd(state: State<'_, AppState>) -> Result<u16, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    port::find_free_port(config.port_range_start, config.port_range_end)
}

#[tauri::command]
pub fn kill_port_process(port: u16) -> Result<(), String> {
    port::kill_port(port)
}

// ─── Config Commands ────────────────────────────────────────

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    Ok(config.clone())
}

#[tauri::command]
pub fn update_config(state: State<'_, AppState>, config: AppConfig) -> Result<(), String> {
    let mut current = state.config.lock().map_err(|e| e.to_string())?;
    *current = config.clone();
    config::save_config(&config::config_path(&state.app_dir), &current)
}

#[tauri::command]
pub fn open_project_in_ide(project_path: String, ide_command: String) -> Result<(), String> {
    let ide = ide_command.trim();
    if ide.is_empty() {
        return Err("IDE command is not configured".to_string());
    }

    let command = if cfg!(target_os = "windows") {
        format!(r#"{ide} "{}""#, project_path)
    } else {
        format!("{} {}", ide, shell_escape(&project_path))
    };

    let (shell_cmd, shell_flag) = default_shell();
    Command::new(shell_cmd)
        .arg(shell_flag)
        .arg(command)
        .spawn()
        .map_err(|e| format!("Failed to open project in IDE: {e}"))?;

    Ok(())
}

// ─── Updater Commands ───────────────────────────────────────

#[tauri::command]
pub async fn check_app_update(
    app: AppHandle,
    state: State<'_, AppState>,
    pending_update: State<'_, PendingUpdate>,
) -> Result<Option<UpdateMetadata>, String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?.clone();

    if !updater_is_configured(&cfg) {
        *pending_update.0.lock().map_err(|e| e.to_string())? = None;
        return Ok(None);
    }

    let endpoint = Url::parse(cfg.update_endpoint.trim()).map_err(|e| {
        format!("Invalid update endpoint URL: {e}")
    })?;

    let updater = app
        .updater_builder()
        .pubkey(cfg.updater_pubkey.trim().to_string())
        .endpoints(vec![endpoint])
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;

    let update = updater.check().await.map_err(|e| e.to_string())?;
    let metadata = update.as_ref().map(|item| UpdateMetadata {
        version: item.version.clone(),
        current_version: item.current_version.clone(),
        notes: item.body.clone(),
        date: item.date.as_ref().map(|date| date.to_string()),
    });

    *pending_update.0.lock().map_err(|e| e.to_string())? = update;
    Ok(metadata)
}

#[tauri::command]
pub async fn install_app_update(
    pending_update: State<'_, PendingUpdate>,
) -> Result<(), String> {
    let update = pending_update
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .take()
        .ok_or_else(|| "No pending update available".to_string())?;

    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn relaunch_app(app: AppHandle) {
    app.restart();
}

// ─── Health Commands ────────────────────────────────────────

#[tauri::command]
pub async fn check_project_health(port: u16, url: Option<String>) -> Result<String, String> {
    let check = match url {
        Some(u) => HealthCheckType::Http(u),
        None => HealthCheckType::Port(port),
    };

    match health::check_health(&check, 10).await {
        health::HealthStatus::Healthy => Ok("Healthy".to_string()),
        health::HealthStatus::Unhealthy(msg) => Err(msg),
        health::HealthStatus::Timeout => Err("Health check timed out".to_string()),
    }
}
