use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::process::Command;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::Instant;

use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use serde_json::Value;
use sysinfo::{Pid, ProcessesToUpdate, System};
use tauri::Emitter;

use crate::types::ProjectStatus;

pub struct ProcessManager {
    processes: Mutex<HashMap<String, ProcessHandle>>,
    system: Mutex<System>,
    app_handle: Mutex<Option<tauri::AppHandle>>,
}

struct ProcessHandle {
    killer: Box<dyn ChildKiller + Send + Sync>,
    writer: Option<Box<dyn Write + Send>>,
    master: Box<dyn MasterPty + Send>,
    pid: u32,
    started_at: Instant,
    project_name: String,
    stop_requested: Arc<AtomicBool>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            processes: Mutex::new(HashMap::new()),
            system: Mutex::new(System::new()),
            app_handle: Mutex::new(None),
        }
    }

    /// Attach a Tauri AppHandle so the process manager can emit events.
    pub fn attach(&self, handle: tauri::AppHandle) {
        if let Ok(mut slot) = self.app_handle.lock() {
            *slot = Some(handle);
        }
    }

    pub fn start(
        &self,
        project_id: &str,
        project_name: &str,
        command: &str,
        working_dir: &str,
    ) -> Result<(u32, ProjectStatus), String> {
        let (shell_cmd, shell_flag) = default_shell();
        let command = prepare_shell_command(command, working_dir);

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 30,
                cols: 120,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to open PTY: {}", e))?;

        let mut cmd = CommandBuilder::new(shell_cmd);
        cmd.arg(shell_flag);
        cmd.arg(&command);
        cmd.cwd(working_dir);
        apply_node_toolchain_env(&mut cmd);

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("Failed to clone PTY reader: {}", e))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("Failed to open PTY writer: {}", e))?;
        let mut child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("Failed to start process in PTY: {}", e))?;
        let pid = child.process_id().unwrap_or(0);
        let killer = child.clone_killer();
        let stop_requested = Arc::new(AtomicBool::new(false));

        let handle = ProcessHandle {
            killer,
            writer: Some(writer),
            master: pair.master,
            pid,
            started_at: Instant::now(),
            project_name: project_name.to_string(),
            stop_requested: stop_requested.clone(),
        };

        self.processes
            .lock()
            .map_err(|e| e.to_string())?
            .insert(project_id.to_string(), handle);

        // Get a clone of AppHandle for the reader threads
        let emitter = self.app_handle.lock().ok().and_then(|g| g.clone());

        if let Some(ref em) = emitter {
            let em = em.clone();
            let pid = project_id.to_string();
            std::thread::spawn(move || {
                let mut buffer = [0_u8; 4096];
                loop {
                    match reader.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(size) => {
                            let text = String::from_utf8_lossy(&buffer[..size]).to_string();
                            let _ = em.emit(
                                "process-output",
                                serde_json::json!({
                                    "project_id": pid,
                                    "text": text,
                                    "stream": "pty",
                                }),
                            );
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        if let Some(ref em) = emitter {
            let em = em.clone();
            let process_id = project_id.to_string();
            std::thread::spawn(move || {
                let status = child.wait();
                let stop_requested = stop_requested.load(Ordering::SeqCst);
                let next_status = match status {
                    Ok(exit_status) if stop_requested || exit_status.success() => "Stopped",
                    Ok(_) | Err(_) => "Error",
                };

                let _ = em.emit(
                    "project-status-changed",
                    serde_json::json!({
                        "id": process_id,
                        "status": next_status,
                    }),
                );
            });
        }

        tracing::info!(project = %project_name, pid = %pid, "Process started");

        Ok((pid, ProjectStatus::Running))
    }

    pub fn write_input(&self, project_id: &str, input: &str) -> Result<(), String> {
        let mut processes = self.processes.lock().map_err(|e| e.to_string())?;
        let handle = processes
            .get_mut(project_id)
            .ok_or_else(|| format!("No running process found for project {}", project_id))?;
        let writer = handle
            .writer
            .as_mut()
            .ok_or_else(|| format!("Process for project {} is not accepting input", project_id))?;

        writer
            .write_all(input.as_bytes())
            .map_err(|e| format!("Failed to write to process stdin: {}", e))?;
        writer
            .flush()
            .map_err(|e| format!("Failed to flush process stdin: {}", e))?;
        Ok(())
    }

    pub fn resize_pty(&self, project_id: &str, rows: u16, cols: u16) -> Result<(), String> {
        let processes = self.processes.lock().map_err(|e| e.to_string())?;
        let handle = processes
            .get(project_id)
            .ok_or_else(|| format!("No running process found for project {}", project_id))?;
        handle
            .master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to resize PTY: {}", e))?;
        Ok(())
    }

    pub fn stop(&self, project_id: &str) -> Result<(), String> {
        let mut processes = self.processes.lock().map_err(|e| e.to_string())?;

        if let Some(handle) = processes.get_mut(project_id) {
            let pid = handle.pid;
            handle.stop_requested.store(true, Ordering::SeqCst);

            // Try graceful shutdown first
            #[cfg(target_os = "macos")]
            {
                let _ = Command::new("kill")
                    .args(["-TERM", &pid.to_string()])
                    .output();
            }

            let _ = handle.killer.kill();

            tracing::info!(project = %handle.project_name, pid = %pid, "Process stopped");
            processes.remove(project_id);
            Ok(())
        } else {
            Err(format!(
                "No running process found for project {}",
                project_id
            ))
        }
    }

    pub fn stop_all(&self) -> Vec<(String, String)> {
        let ids: Vec<String> = self
            .processes
            .lock()
            .map(|p| p.keys().cloned().collect())
            .unwrap_or_default();

        let mut results = Vec::new();
        for id in &ids {
            match self.stop(id) {
                Ok(_) => results.push((id.clone(), "ok".to_string())),
                Err(e) => results.push((id.clone(), e)),
            }
        }
        results
    }

    pub fn get_process_status(&self, project_id: &str) -> ProjectStatus {
        let pid = {
            let processes = match self.processes.lock() {
                Ok(p) => p,
                Err(_) => return ProjectStatus::Error("Lock error".to_string()),
            };
            processes.get(project_id).map(|h| h.pid)
        };

        match pid {
            None => ProjectStatus::Stopped,
            Some(pid) => {
                let sys_pid = Pid::from_u32(pid);
                let mut system = match self.system.lock() {
                    Ok(s) => s,
                    Err(_) => return ProjectStatus::Error("Lock error".to_string()),
                };
                system.refresh_processes(ProcessesToUpdate::All, false);

                if system.process(sys_pid).is_some() {
                    ProjectStatus::Running
                } else {
                    ProjectStatus::Stopped
                }
            }
        }
    }

    pub fn cleanup(&self) {
        let ids: Vec<String> = self
            .processes
            .lock()
            .map(|p| p.keys().cloned().collect())
            .unwrap_or_default();

        for id in ids {
            let _ = self.stop(&id);
        }
    }

    pub fn is_any_running(&self) -> bool {
        let processes = match self.processes.lock() {
            Ok(p) => p,
            Err(_) => return false,
        };
        !processes.is_empty()
    }
}

fn default_shell() -> (String, String) {
    if cfg!(target_os = "windows") {
        return ("cmd".to_string(), "/C".to_string());
    }

    if Path::new("/bin/zsh").exists() {
        return ("/bin/zsh".to_string(), "-lic".to_string());
    }

    if let Ok(shell) = std::env::var("SHELL") {
        if shell.contains("zsh") || shell.contains("bash") {
            return (shell, "-lic".to_string());
        }
    }

    if Path::new("/bin/bash").exists() {
        return ("/bin/bash".to_string(), "-lic".to_string());
    }

    ("/bin/sh".to_string(), "-c".to_string())
}

fn prepare_shell_command(command: &str, working_dir: &str) -> String {
    if cfg!(target_os = "windows") {
        return command.to_string();
    }

    let command = apply_volta_run(command, working_dir);
    let mut prefixes = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        let volta_home = Path::new(&home).join(".volta");
        let volta_bin = volta_home.join("bin");
        if volta_bin.is_dir() {
            prefixes.push(format!(
                "export VOLTA_HOME={}; export PATH={}:$PATH",
                shell_escape(&volta_home.to_string_lossy()),
                shell_escape(&volta_bin.to_string_lossy())
            ));
        }
    }

    if prefixes.is_empty() {
        command
    } else {
        format!("{}; {}", prefixes.join("; "), command)
    }
}

fn apply_volta_run(command: &str, working_dir: &str) -> String {
    if command.contains("volta run") {
        return command.to_string();
    }

    let (target_dir, inner_command, cd_prefix) = command_target(command, working_dir);
    let Some(toolchain) = read_volta_toolchain(&target_dir) else {
        return command.to_string();
    };

    let mut options = Vec::new();
    if let Some(node) = toolchain.node {
        options.push(format!("--node {}", shell_escape(&node)));
    }
    if let Some(npm) = toolchain.npm {
        options.push(format!("--npm {}", shell_escape(&npm)));
    }
    if let Some(pnpm) = toolchain.pnpm {
        options.push(format!("--pnpm {}", shell_escape(&pnpm)));
    }
    if let Some(yarn) = toolchain.yarn {
        options.push(format!("--yarn {}", shell_escape(&yarn)));
    }

    if options.is_empty() {
        return command.to_string();
    }

    format!(
        "{}volta run {} -- {}",
        cd_prefix,
        options.join(" "),
        inner_command
    )
}

fn command_target(command: &str, working_dir: &str) -> (std::path::PathBuf, String, String) {
    let trimmed = command.trim();
    let Some(rest) = trimmed.strip_prefix("cd ") else {
        return (
            Path::new(working_dir).to_path_buf(),
            trimmed.to_string(),
            String::new(),
        );
    };

    let Some((dir, inner)) = rest.split_once("&&") else {
        return (
            Path::new(working_dir).to_path_buf(),
            trimmed.to_string(),
            String::new(),
        );
    };

    let dir = dir.trim();
    let clean_dir = dir
        .trim_matches('"')
        .trim_matches('\'')
        .replace("'\\''", "'");
    let target_dir = Path::new(working_dir).join(&clean_dir);
    let cd_prefix = format!("cd {} && ", shell_escape(&clean_dir));

    (target_dir, inner.trim().to_string(), cd_prefix)
}

struct VoltaToolchain {
    node: Option<String>,
    npm: Option<String>,
    pnpm: Option<String>,
    yarn: Option<String>,
}

fn read_volta_toolchain(project_dir: &Path) -> Option<VoltaToolchain> {
    let mut cursor = Some(project_dir);
    while let Some(dir) = cursor {
        let pkg_path = dir.join("package.json");
        if let Ok(content) = std::fs::read_to_string(pkg_path) {
            if let Ok(json) = serde_json::from_str::<Value>(&content) {
                if let Some(volta) = json.get("volta").and_then(|value| value.as_object()) {
                    return Some(VoltaToolchain {
                        node: volta
                            .get("node")
                            .and_then(|value| value.as_str())
                            .map(str::to_string),
                        npm: volta
                            .get("npm")
                            .and_then(|value| value.as_str())
                            .map(str::to_string),
                        pnpm: volta
                            .get("pnpm")
                            .and_then(|value| value.as_str())
                            .map(str::to_string),
                        yarn: volta
                            .get("yarn")
                            .and_then(|value| value.as_str())
                            .map(str::to_string),
                    });
                }
            }
        }
        cursor = dir.parent();
    }

    None
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

fn apply_node_toolchain_env(cmd: &mut CommandBuilder) {
    if cfg!(target_os = "windows") {
        return;
    }

    if let Some(home) = std::env::var_os("HOME") {
        let volta_home = Path::new(&home).join(".volta");
        let volta_bin = volta_home.join("bin");
        if volta_bin.is_dir() {
            cmd.env("VOLTA_HOME", volta_home.as_os_str());
            prepend_path(cmd, &volta_bin);
        }
    }

    if let Some(nvm_dir) = std::env::var_os("NVM_DIR") {
        cmd.env("NVM_DIR", nvm_dir);
    }
}

fn prepend_path(cmd: &mut CommandBuilder, path: &Path) {
    let current_path = cmd
        .get_env("PATH")
        .map(|value| value.to_string_lossy().to_string())
        .or_else(|| std::env::var("PATH").ok())
        .unwrap_or_default();

    let path = path.to_string_lossy();
    let next_path = if current_path.is_empty() {
        path.to_string()
    } else {
        format!("{}:{}", path, current_path)
    };

    cmd.env("PATH", next_path);
}

impl Drop for ProcessManager {
    fn drop(&mut self) {
        self.cleanup();
    }
}
