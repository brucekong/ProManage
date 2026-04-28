mod commands;
mod config;
mod health;
mod log;
mod port;
mod process;
mod project;
mod scheduler;
mod types;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};

use commands::{AppState, PendingUpdate};
use config::{config_path, logs_dir, projects_path};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Determine app data directory
    let app_dir = get_app_dir();

    // Initialize logging
    log::setup_logging(&logs_dir(&app_dir));

    // Load config and projects
    let loaded_config = config::load_config(&config_path(&app_dir));
    let loaded_projects = project::load_projects(&projects_path(&app_dir));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            projects: Mutex::new(loaded_projects),
            config: Mutex::new(loaded_config),
            process_manager: Arc::new(process::ProcessManager::new()),
            app_dir: app_dir.clone(),
        })
        .manage(PendingUpdate(Mutex::new(None)))
        .setup(|app| {
            // Attach AppHandle to process manager for event emission
            let app_handle = app.handle().clone();
            let state = app.state::<AppState>();
            state.process_manager.attach(app_handle);

            // Build system tray
            let show_i = MenuItem::with_id(app, "show", "Show ProStation", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::scan_projects,
            commands::list_projects,
            commands::add_project,
            commands::remove_project,
            commands::update_project,
            commands::reorder_projects,
            commands::save_projects_config,
            commands::read_package_scripts,
            commands::get_git_statuses,
            commands::start_project,
            commands::start_project_command,
            commands::stop_project,
            commands::restart_project,
            commands::start_all_projects,
            commands::stop_all_projects,
            commands::get_process_statuses,
            commands::write_project_input,
            commands::resize_pty,
            commands::check_port_usage,
            commands::find_free_port_cmd,
            commands::kill_port_process,
            commands::get_config,
            commands::update_config,
            commands::open_project_in_ide,
            commands::check_app_update,
            commands::install_app_update,
            commands::relaunch_app,
            commands::check_project_health,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn get_app_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join("com.xueleikong.prostation");

    std::fs::create_dir_all(&dir).ok();
    std::fs::create_dir_all(dir.join("logs")).ok();

    dir
}
