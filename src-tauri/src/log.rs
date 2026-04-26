use std::path::PathBuf;

use chrono::Local;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

pub fn setup_logging(log_dir: &PathBuf) {
    std::fs::create_dir_all(log_dir).ok();

    let log_file = log_dir.join(format!(
        "prostation-{}.log",
        Local::now().format("%Y-%m-%d")
    ));

    let file_appender = {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file)
            .expect("Failed to open log file");

        fmt::layer()
            .with_writer(std::sync::Mutex::new(file))
            .with_ansi(false)
            .json()
    };

    let stdout_layer = fmt::layer().with_ansi(true).with_target(false);

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(stdout_layer)
        .with(file_appender)
        .init();
}
