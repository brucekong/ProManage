use std::path::Path;

use serde_json::Value;
use uuid::Uuid;

use crate::types::{Project, ProjectConfig, ProjectStatus};

pub fn scan_directory(dir: &str) -> Vec<ProjectConfig> {
    let mut projects = Vec::new();
    let path = Path::new(dir);

    if !path.exists() || !path.is_dir() {
        tracing::warn!("Scan directory does not exist: {}", dir);
        return projects;
    }

    if let Some(project) = parse_split_package_project(path) {
        projects.push(project);
        return projects;
    }

    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(e) => {
            tracing::error!("Failed to read directory {}: {}", dir, e);
            return projects;
        }
    };

    for entry in entries.flatten() {
        let entry_path = entry.path();
        if !entry_path.is_dir() {
            if entry_path.file_name().and_then(|n| n.to_str()) == Some("package.json") {
                // Handle the case where the scanned dir itself IS a project
                if let Some(project) = parse_package_json(&entry_path, dir) {
                    projects.push(project);
                }
                if let Some(sub_projects) = resolve_package_workspaces(path) {
                    projects.extend(sub_projects);
                }
            }
            continue;
        }

        if let Some(project) = parse_split_package_project(&entry_path) {
            projects.push(project);
            continue;
        }

        let pkg_json = entry_path.join("package.json");
        if pkg_json.exists() {
            if let Some(project) = parse_package_json(&pkg_json, entry_path.to_str().unwrap()) {
                projects.push(project);
            }
            if let Some(sub_projects) = resolve_package_workspaces(&entry_path) {
                projects.extend(sub_projects);
            }
        }

        // Check for monorepo workspace configs
        let workspace_files = [
            entry_path.join("pnpm-workspace.yaml"),
            entry_path.join("lerna.json"),
        ];

        for ws_file in &workspace_files {
            if ws_file.exists() {
                if let Some(sub_projects) = resolve_workspace(&entry_path, ws_file) {
                    projects.extend(sub_projects);
                }
            }
        }
    }

    projects
}

fn parse_split_package_project(root_dir: &Path) -> Option<ProjectConfig> {
    let child_packages = find_child_package_jsons(root_dir);
    if child_packages.len() < 2 {
        return None;
    }

    let mut scripts = Vec::new();
    for (child_dir, pkg_path, role) in child_packages {
        let content = std::fs::read_to_string(&pkg_path).ok()?;
        let json: Value = serde_json::from_str(&content).ok()?;
        let relative_dir = child_dir
            .strip_prefix(root_dir)
            .ok()?
            .to_string_lossy()
            .to_string();

        for (script_name, _) in extract_scripts(&json) {
            let virtual_name = format!("{}:{}", role, script_name);
            let command = package_script_command(&relative_dir, &script_name);
            scripts.push((virtual_name, command));
        }
    }

    if !has_runnable_script(&scripts) {
        return None;
    }

    scripts.sort_by(|a, b| a.0.cmp(&b.0));
    let name = root_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    Some(ProjectConfig {
        id: Uuid::new_v4().to_string(),
        name,
        path: root_dir.to_string_lossy().to_string(),
        command: infer_default_command(&scripts),
        scripts,
        port: 0,
        group: "default".to_string(),
        note: String::new(),
        auto_start: false,
        show_build_scripts: false,
        depends_on: vec![],
        env_vars: vec![],
    })
}

fn find_child_package_jsons(
    root_dir: &Path,
) -> Vec<(std::path::PathBuf, std::path::PathBuf, String)> {
    let ignored = [
        ".git",
        ".next",
        ".nuxt",
        ".output",
        "cache",
        "dist",
        "node_modules",
        "target",
    ];

    let mut packages = Vec::new();
    let entries = match std::fs::read_dir(root_dir) {
        Ok(entries) => entries,
        Err(_) => return packages,
    };

    for entry in entries.flatten() {
        let child_dir = entry.path();
        if !child_dir.is_dir() {
            continue;
        }

        let dir_name = child_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if ignored.contains(&dir_name) {
            continue;
        }

        let pkg_path = child_dir.join("package.json");
        if !pkg_path.exists() {
            continue;
        }

        if let Some(role) = infer_child_package_role(dir_name, &pkg_path) {
            packages.push((child_dir, pkg_path, role));
        }
    }

    packages
}

fn infer_child_package_role(dir_name: &str, pkg_path: &Path) -> Option<String> {
    let lower_dir = dir_name.to_lowercase();
    if lower_dir.contains("front")
        || lower_dir == "web"
        || lower_dir.contains("client")
        || lower_dir.ends_with("-fe")
        || lower_dir == "app"
    {
        return Some("web".to_string());
    }

    if lower_dir.contains("server")
        || lower_dir.contains("api")
        || lower_dir.contains("backend")
        || lower_dir.ends_with("-be")
    {
        return Some("api".to_string());
    }

    let content = std::fs::read_to_string(pkg_path).ok()?;
    let lower_content = content.to_lowercase();
    if lower_content.contains("vite")
        || lower_content.contains("next")
        || lower_content.contains("react")
        || lower_content.contains("vue")
    {
        return Some("web".to_string());
    }

    if lower_content.contains("express")
        || lower_content.contains("nestjs")
        || lower_content.contains("fastify")
        || lower_content.contains("prisma")
    {
        return Some("api".to_string());
    }

    None
}

fn package_script_command(relative_dir: &str, script_name: &str) -> String {
    if script_name == "start" {
        format!("cd {} && npm start", shell_escape(relative_dir))
    } else {
        format!(
            "cd {} && npm run {}",
            shell_escape(relative_dir),
            shell_escape(script_name)
        )
    }
}

fn shell_escape(value: &str) -> String {
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '-' | '.' | ':'))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn parse_package_json(path: &Path, project_dir: &str) -> Option<ProjectConfig> {
    let content = std::fs::read_to_string(path).ok()?;
    let json: Value = serde_json::from_str(&content).ok()?;

    let name = json["name"].as_str().unwrap_or(
        path.parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("unknown"),
    );

    let scripts = extract_scripts(&json);

    // Detect if this is a monorepo workspace root
    let has_workspaces = json.get("workspaces").and_then(|v| v.as_array()).is_some()
        || json.get("private").and_then(|v| v.as_bool()) == Some(true);

    // Skip workspace roots that do not expose runnable scripts themselves.
    if has_workspaces && !has_runnable_script(&scripts) {
        return None;
    }

    let default_command = infer_default_command(&scripts);

    Some(ProjectConfig {
        id: Uuid::new_v4().to_string(),
        name: name.to_string(),
        path: project_dir.to_string(),
        command: default_command,
        scripts,
        port: 0,
        group: "default".to_string(),
        note: String::new(),
        auto_start: false,
        show_build_scripts: false,
        depends_on: vec![],
        env_vars: vec![],
    })
}

pub fn read_package_scripts(project_dir: &str) -> Result<Vec<(String, String)>, String> {
    let pkg_path = Path::new(project_dir).join("package.json");
    if !pkg_path.exists() {
        if let Some(project) = parse_split_package_project(Path::new(project_dir)) {
            return Ok(project.scripts);
        }
    }

    let content = std::fs::read_to_string(&pkg_path)
        .map_err(|e| format!("Failed to read {}: {}", pkg_path.display(), e))?;
    let json: Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    Ok(extract_scripts(&json))
}

pub fn enrich_project_config(mut config: ProjectConfig) -> ProjectConfig {
    if let Ok(scripts) = read_package_scripts(&config.path) {
        config.scripts = scripts;
        if should_replace_command(&config.command, &config.scripts) {
            config.command = infer_default_command(&config.scripts);
        }
    }
    config
}

pub fn repair_project_configs(projects: &mut [ProjectConfig]) -> bool {
    let mut changed = false;

    for project in projects {
        let original_command = project.command.clone();
        let original_scripts = project.scripts.clone();
        let repaired = enrich_project_config(project.clone());

        project.command = repaired.command;
        project.scripts = repaired.scripts;

        if project.command != original_command || project.scripts != original_scripts {
            changed = true;
        }
    }

    changed
}

fn extract_scripts(json: &Value) -> Vec<(String, String)> {
    let mut scripts: Vec<(String, String)> = json
        .get("scripts")
        .and_then(|value| value.as_object())
        .map(|scripts| {
            scripts
                .iter()
                .filter_map(|(name, command)| {
                    command
                        .as_str()
                        .map(|command| (name.to_string(), command.to_string()))
                })
                .collect()
        })
        .unwrap_or_default();

    scripts.sort_by(|a, b| a.0.cmp(&b.0));
    scripts
}

fn has_runnable_script(scripts: &[(String, String)]) -> bool {
    scripts.iter().any(|(name, _)| {
        name == "dev"
            || name == "start"
            || name == "serve"
            || name.starts_with("dev:")
            || name.starts_with("start:")
            || name.starts_with("serve:")
            || name.ends_with(":dev")
            || name.ends_with(":start")
            || name.ends_with(":serve")
            || name.starts_with("build:")
            || name.ends_with(":build")
    })
}

fn infer_default_command(scripts: &[(String, String)]) -> String {
    let priority = [
        "web:dev",
        "api:dev",
        "web:start",
        "api:start",
        "dev:web",
        "dev:app",
        "dev:api",
        "dev",
        "start:web",
        "start:api",
        "start",
        "serve",
    ];

    for preferred in priority {
        if let Some((name, command)) = scripts.iter().find(|(name, _)| name == preferred) {
            return script_to_command(name, command);
        }
    }

    if let Some((name, command)) = scripts
        .iter()
        .find(|(name, _)| name.starts_with("dev:") || name.contains("dev"))
    {
        return script_to_command(name, command);
    }

    if let Some((name, command)) = scripts
        .iter()
        .find(|(name, _)| name.starts_with("start:") || name.contains("start"))
    {
        return script_to_command(name, command);
    }

    if let Some((name, command)) = scripts.iter().find(|(name, _)| name.contains("build")) {
        return script_to_command(name, command);
    }

    scripts
        .first()
        .map(|(name, command)| script_to_command(name, command))
        .unwrap_or_else(|| "npm run dev".to_string())
}

fn script_to_command(script_name: &str, script_command: &str) -> String {
    if is_launch_command(script_command) {
        return script_command.to_string();
    }

    if script_name == "start" {
        "npm start".to_string()
    } else {
        format!("npm run {}", script_name)
    }
}

fn is_launch_command(command: &str) -> bool {
    let trimmed = command.trim_start();
    trimmed.starts_with("npm ")
        || trimmed.starts_with("pnpm ")
        || trimmed.starts_with("yarn ")
        || trimmed.starts_with("bun ")
        || trimmed.starts_with("cd ")
}

fn should_replace_command(command: &str, scripts: &[(String, String)]) -> bool {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return true;
    }

    if let Some(script_name) = trimmed.strip_prefix("npm run ") {
        let script_name = script_name.trim();
        return !scripts.iter().any(|(name, _)| name == script_name);
    }

    if trimmed == "npm start" {
        return !scripts.iter().any(|(name, _)| name == "start");
    }

    false
}

fn resolve_workspace(root_dir: &Path, ws_file: &Path) -> Option<Vec<ProjectConfig>> {
    let file_name = ws_file.file_name()?.to_str()?;
    let root_str = root_dir.to_str()?;

    match file_name {
        "pnpm-workspace.yaml" => resolve_pnpm_workspace(root_str),
        "lerna.json" => resolve_lerna_workspace(root_str),
        _ => None,
    }
}

fn resolve_package_workspaces(root_dir: &Path) -> Option<Vec<ProjectConfig>> {
    let pkg_path = root_dir.join("package.json");
    let content = std::fs::read_to_string(pkg_path).ok()?;
    let json: Value = serde_json::from_str(&content).ok()?;

    let patterns: Vec<String> = match json.get("workspaces") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.as_str().map(str::to_string))
            .collect(),
        Some(Value::Object(object)) => object
            .get("packages")
            .and_then(|packages| packages.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    };

    if patterns.is_empty() {
        return None;
    }

    let mut projects = Vec::new();
    for pattern in patterns {
        projects.extend(resolve_workspace_pattern(root_dir, &pattern));
    }

    Some(projects)
}

fn resolve_workspace_pattern(root_dir: &Path, pattern: &str) -> Vec<ProjectConfig> {
    let mut projects = Vec::new();
    let resolved = root_dir.join(pattern);
    if resolved.is_dir() {
        let pkg = resolved.join("package.json");
        if pkg.exists() {
            if let Some(project) = parse_package_json(&pkg, resolved.to_str().unwrap_or_default()) {
                projects.push(project);
            }
        }
    }

    if pattern.contains('*') {
        let parent_pattern = pattern.trim_end_matches("/*").trim_end_matches('*');
        let parent = root_dir.join(parent_pattern);
        if parent.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&parent) {
                for entry in entries.flatten() {
                    let child_path = entry.path();
                    if child_path.is_dir() {
                        let pkg = child_path.join("package.json");
                        if pkg.exists() {
                            if let Some(project) =
                                parse_package_json(&pkg, child_path.to_str().unwrap_or_default())
                            {
                                projects.push(project);
                            }
                        }
                    }
                }
            }
        }
    }

    projects
}

fn resolve_pnpm_workspace(root_dir: &str) -> Option<Vec<ProjectConfig>> {
    let yaml_path = Path::new(root_dir).join("pnpm-workspace.yaml");
    let content = std::fs::read_to_string(yaml_path).ok()?;

    // Simple YAML parsing for workspace packages (avoid heavy dependency)
    let mut projects = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("- ") || (trimmed.starts_with('\'') || trimmed.starts_with('"')) {
            let pattern = trimmed
                .trim_start_matches("- ")
                .trim_matches('\'')
                .trim_matches('"')
                .trim();

            let resolved = Path::new(root_dir).join(pattern);
            if resolved.is_dir() {
                let pkg = resolved.join("package.json");
                if pkg.exists() {
                    if let Some(project) = parse_package_json(&pkg, resolved.to_str()?) {
                        projects.push(project);
                    }
                }
            }
            // Basic glob support: replace * with a scan
            if pattern.contains('*') {
                let parent_pattern = pattern.trim_end_matches("/*").trim_end_matches('*');
                let parent = Path::new(root_dir).join(parent_pattern);
                if parent.is_dir() {
                    if let Ok(entries) = std::fs::read_dir(&parent) {
                        for entry in entries.flatten() {
                            let child_path = entry.path();
                            if child_path.is_dir() {
                                let pkg = child_path.join("package.json");
                                if pkg.exists() {
                                    if let Some(project) =
                                        parse_package_json(&pkg, child_path.to_str()?)
                                    {
                                        projects.push(project);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Some(projects)
}

fn resolve_lerna_workspace(root_dir: &str) -> Option<Vec<ProjectConfig>> {
    let json_path = Path::new(root_dir).join("lerna.json");
    let content = std::fs::read_to_string(json_path).ok()?;
    let json: Value = serde_json::from_str(&content).ok()?;

    let packages = json["packages"].as_array()?;

    let mut projects = Vec::new();
    for pkg_pattern in packages {
        let pattern = pkg_pattern.as_str()?;
        let resolved = Path::new(root_dir).join(pattern);
        if resolved.is_dir() {
            let pkg = resolved.join("package.json");
            if pkg.exists() {
                if let Some(project) = parse_package_json(&pkg, resolved.to_str()?) {
                    projects.push(project);
                }
            }
        }
    }

    Some(projects)
}

pub fn merge_scanned_projects(
    existing: &[ProjectConfig],
    scanned: Vec<ProjectConfig>,
) -> Vec<ProjectConfig> {
    let mut merged = existing.to_vec();
    for project in scanned {
        if let Some(existing_project) = merged.iter_mut().find(|p| p.path == project.path) {
            existing_project.scripts = project.scripts.clone();
            if should_replace_command(&existing_project.command, &project.scripts) {
                existing_project.command = project.command.clone();
            }
            if existing_project.name.trim().is_empty() {
                existing_project.name = project.name.clone();
            }
        } else {
            merged.push(project);
        }
    }
    merged
}

pub fn load_projects(path: &std::path::PathBuf) -> Vec<ProjectConfig> {
    if path.exists() {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|content| serde_json::from_str(&content).ok())
            .unwrap_or_default()
    } else {
        Vec::new()
    }
}

pub fn save_projects(path: &std::path::PathBuf, projects: &[ProjectConfig]) -> Result<(), String> {
    let content = serde_json::to_string_pretty(projects).map_err(|e| e.to_string())?;
    std::fs::write(path, content).map_err(|e| e.to_string())
}

pub fn config_to_project(config: &ProjectConfig) -> Project {
    Project {
        id: config.id.clone(),
        name: config.name.clone(),
        path: config.path.clone(),
        command: config.command.clone(),
        port: config.port,
        group: config.group.clone(),
        note: config.note.clone(),
        status: ProjectStatus::Stopped,
        pid: None,
        start_time: None,
        auto_start: config.auto_start,
        show_build_scripts: config.show_build_scripts,
    }
}

pub fn configs_to_projects(configs: &[ProjectConfig]) -> Vec<Project> {
    configs.iter().map(config_to_project).collect()
}
