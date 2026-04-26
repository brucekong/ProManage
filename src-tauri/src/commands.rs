use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::Serialize;
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

fn updater_is_configured(cfg: &AppConfig) -> bool {
    !cfg.update_endpoint.trim().is_empty() && !cfg.updater_pubkey.trim().is_empty()
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
        projects[pos] = project;
        project::save_projects(&config::projects_path(&state.app_dir), &projects)?;
        Ok(projects.clone())
    } else {
        Err(format!("Project with id '{}' not found", project.id))
    }
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
    let (pid, _) = pm.start(&process_id, &process_name, &command, &project.path)?;

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
