use std::time::Duration;

use crate::port::check_port;

#[derive(Debug, Clone)]
pub enum HealthCheckType {
    Port(u16),
    Http(String),
}

pub enum HealthStatus {
    Healthy,
    Unhealthy(String),
    Timeout,
}

pub async fn check_health(check: &HealthCheckType, timeout_secs: u64) -> HealthStatus {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);

    loop {
        if tokio::time::Instant::now() >= deadline {
            return HealthStatus::Timeout;
        }

        match check {
            HealthCheckType::Port(port) => {
                let info = check_port(*port);
                if info.in_use {
                    return HealthStatus::Healthy;
                }
            }
            HealthCheckType::Http(url) => match http_get_status(url).await {
                Ok(code) if code >= 200 && code < 400 => return HealthStatus::Healthy,
                _ => {}
            },
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

async fn http_get_status(url: &str) -> Result<u16, String> {
    let output = tokio::process::Command::new("curl")
        .args([
            "-s",
            "-o",
            "/dev/null",
            "-w",
            "%{http_code}",
            "--max-time",
            "2",
            url,
        ])
        .output()
        .await
        .map_err(|e| format!("Curl failed: {}", e))?;

    let code_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    code_str
        .parse::<u16>()
        .map_err(|e| format!("Parse error: {}", e))
}
