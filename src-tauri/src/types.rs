use serde::{Deserialize, Serialize};

pub const DEFAULT_UPDATE_ENDPOINT: &str =
    "https://github.com/brucekong/ProManage/releases/latest/download/latest.json";
pub const DEFAULT_UPDATER_PUBKEY: &str =
    "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEZGQ0Q2NTM3MzFCNDhBMjIKUldRaWlyUXhOMlhOLzQxRUZ0YVd5WFlSamRKWDY2L3NSVlE1eXY1NkYyRGJJMDFJbDZhUFI4V1IK";

fn default_project_kind() -> String {
    "folder".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProjectStatus {
    Running,
    Stopped,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(default = "default_project_kind")]
    pub project_kind: String,
    pub command: String,
    #[serde(default)]
    pub has_custom_command: bool,
    pub port: u16,
    pub group: String,
    pub note: String,
    pub status: ProjectStatus,
    pub pid: Option<u32>,
    pub start_time: Option<String>,
    pub auto_start: bool,
    #[serde(default)]
    pub is_favorite: bool,
    #[serde(default)]
    pub show_build_scripts: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(default = "default_project_kind")]
    pub project_kind: String,
    pub command: String,
    #[serde(default)]
    pub scripts: Vec<(String, String)>,
    #[serde(default)]
    pub has_custom_command: bool,
    pub port: u16,
    pub group: String,
    pub note: String,
    pub auto_start: bool,
    #[serde(default)]
    pub is_favorite: bool,
    #[serde(default)]
    pub show_build_scripts: bool,
    pub depends_on: Vec<String>,
    pub env_vars: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub scan_dirs: Vec<String>,
    pub port_range_start: u16,
    pub port_range_end: u16,
    pub log_retention_days: u32,
    pub theme: String, // "system" | "light" | "dark"
    pub language: String, // "en" | "zh"
    pub minimize_to_tray: bool,
    pub auto_restore: bool,
    pub auto_check_updates: bool,
    pub update_endpoint: String,
    pub updater_pubkey: String,
    pub ide_vscode_command: String,
    pub ide_antigravity_command: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            scan_dirs: vec![],
            port_range_start: 3000,
            port_range_end: 4000,
            log_retention_days: 7,
            theme: "system".to_string(),
            language: "en".to_string(),
            minimize_to_tray: true,
            auto_restore: false,
            auto_check_updates: true,
            update_endpoint: DEFAULT_UPDATE_ENDPOINT.to_string(),
            updater_pubkey: DEFAULT_UPDATER_PUBKEY.to_string(),
            ide_vscode_command: "code".to_string(),
            ide_antigravity_command: "ag".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortInfo {
    pub port: u16,
    pub in_use: bool,
    pub pid: Option<u32>,
    pub process_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub project_id: Option<String>,
    pub message: String,
}
