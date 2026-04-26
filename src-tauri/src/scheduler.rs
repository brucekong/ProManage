use std::collections::{HashMap, HashSet};

use tauri::Emitter;

use crate::health::{check_health, HealthCheckType, HealthStatus};
use crate::port;
use crate::process::ProcessManager;
use crate::types::*;

/// Start a project and all its dependencies in order.
/// Dependencies are started sequentially with health checks.
/// The target project itself is started without blocking on health check.
pub async fn start_with_deps(
    target_id: &str,
    projects: &[ProjectConfig],
    process_manager: &ProcessManager,
    app_handle: &tauri::AppHandle,
    port_range: (u16, u16),
) -> Result<(), String> {
    // Build lookup map
    let map: HashMap<&str, &ProjectConfig> = projects.iter().map(|p| (p.id.as_str(), p)).collect();

    let target = map
        .get(target_id)
        .copied()
        .ok_or_else(|| format!("Project '{}' not found", target_id))?;

    // 1. Walk dependency chain to find ALL transitive deps in start order
    let chain = resolve_chain(target_id, &map)?;

    // 2. Start each dep (that isn't already running), wait for health
    for dep_id in &chain {
        // Skip the target project itself — it starts without blocking health check
        if *dep_id == target_id {
            continue;
        }

        let dep = map
            .get(*dep_id)
            .copied()
            .ok_or_else(|| format!("Dependency '{}' not found", dep_id))?;

        if is_running(dep_id, process_manager) {
            tracing::info!(dep = %dep.name, "Dependency already running, skipping");
            continue;
        }

        tracing::info!(dep = %dep.name, "Starting dependency...");
        start_single(dep, process_manager, app_handle, port_range)?;

        // Health check — wait for port to be ready (only for deps)
        let check = health_check_for(dep);
        match check {
            Some(health_check) => match check_health(&health_check, 60).await {
                HealthStatus::Healthy => {
                    tracing::info!(dep = %dep.name, "Dependency health check passed");
                }
                HealthStatus::Unhealthy(reason) => {
                    return Err(format!(
                        "Dependency '{}' failed health check: {}",
                        dep.name, reason
                    ));
                }
                HealthStatus::Timeout => {
                    return Err(format!(
                        "Dependency '{}' health check timed out (60s)",
                        dep.name
                    ));
                }
            },
            None => {
                tracing::warn!(dep = %dep.name, "No port configured, skipping health check");
            }
        }
    }

    // 3. Start the target project itself (non-blocking — status updates via events)
    if !is_running(target_id, process_manager) {
        tracing::info!(project = %target.name, "Starting target project...");
        start_single(target, process_manager, app_handle, port_range)?;
    }

    Ok(())
}

/// Resolve the dependency chain in start order (dependencies first).
/// Returns IDs in order: [earliest_dep, ..., target].
/// Detects circular dependencies.
fn resolve_chain<'a>(
    target_id: &'a str,
    map: &HashMap<&'a str, &'a ProjectConfig>,
) -> Result<Vec<&'a str>, String> {
    let mut visited: HashSet<&'a str> = HashSet::new();
    let mut stack: Vec<&'a str> = Vec::new();

    dfs(target_id, map, &mut visited, &mut stack)?;

    Ok(stack)
}

fn dfs<'a>(
    current: &'a str,
    map: &HashMap<&'a str, &'a ProjectConfig>,
    visited: &mut HashSet<&'a str>,
    stack: &mut Vec<&'a str>,
) -> Result<(), String> {
    if !visited.insert(current) {
        return Err(format!(
            "Circular dependency detected involving '{}'",
            current
        ));
    }

    if let Some(config) = map.get(current) {
        for dep_id in &config.depends_on {
            dfs(dep_id, map, visited, stack)?;
        }
    }

    stack.push(current);
    Ok(())
}

/// Start a single project process.
/// Returns the assigned port.
fn start_single(
    config: &ProjectConfig,
    process_manager: &ProcessManager,
    app_handle: &tauri::AppHandle,
    port_range: (u16, u16),
) -> Result<u16, String> {
    let assigned_port = if config.port == 0 {
        port::find_free_port(port_range.0, port_range.1)?
    } else {
        config.port
    };

    let (pid, _) =
        process_manager.start(&config.id, &config.name, &config.command, &config.path)?;

    let _ = app_handle.emit(
        "project-status-changed",
        serde_json::json!({
            "id": config.id,
            "status": "Starting",
            "pid": pid,
        }),
    );

    tracing::info!(project = %config.name, pid = %pid, port = %assigned_port, "Started via scheduler");
    Ok(assigned_port)
}

/// Determine health check type from project config.
fn health_check_for(config: &ProjectConfig) -> Option<HealthCheckType> {
    if config.port > 0 {
        Some(HealthCheckType::Port(config.port))
    } else {
        None
    }
}

fn is_running(project_id: &str, process_manager: &ProcessManager) -> bool {
    matches!(
        process_manager.get_process_status(project_id),
        ProjectStatus::Running
    )
}
