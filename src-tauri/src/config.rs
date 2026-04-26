use std::path::PathBuf;

use crate::types::AppConfig;

pub fn config_path(app_dir: &PathBuf) -> PathBuf {
    app_dir.join("config.json")
}

pub fn projects_path(app_dir: &PathBuf) -> PathBuf {
    app_dir.join("projects.json")
}

pub fn logs_dir(app_dir: &PathBuf) -> PathBuf {
    app_dir.join("logs")
}

pub fn load_config(path: &PathBuf) -> AppConfig {
    if path.exists() {
        match std::fs::read_to_string(path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => AppConfig::default(),
        }
    } else {
        AppConfig::default()
    }
}

pub fn save_config(path: &PathBuf, config: &AppConfig) -> Result<(), String> {
    let content = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(path, content).map_err(|e| e.to_string())
}
