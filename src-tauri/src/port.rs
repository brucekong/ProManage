use crate::types::PortInfo;

pub fn check_port(port: u16) -> PortInfo {
    let output = if cfg!(target_os = "macos") {
        std::process::Command::new("lsof")
            .args(["-ti", &format!(":{}", port), "-sTCP:LISTEN"])
            .output()
            .ok()
    } else {
        std::process::Command::new("netstat")
            .args(["-ano", "|", "findstr", &format!(":{}", port)])
            .output()
            .ok()
    };

    match output {
        Some(out) if !out.stdout.is_empty() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let pid_line = stdout
                .lines()
                .next()
                .and_then(|l| l.trim().split_whitespace().last());
            let pid = pid_line.and_then(|p| p.parse::<u32>().ok());

            PortInfo {
                port,
                in_use: true,
                pid,
                process_name: None,
            }
        }
        _ => PortInfo {
            port,
            in_use: false,
            pid: None,
            process_name: None,
        },
    }
}

pub fn find_free_port(range_start: u16, range_end: u16) -> Result<u16, String> {
    for port in range_start..=range_end {
        let info = check_port(port);
        if !info.in_use {
            return Ok(port);
        }
    }
    Err(format!(
        "No free port found in range {}-{}",
        range_start, range_end
    ))
}

pub fn kill_port(port: u16) -> Result<(), String> {
    if cfg!(target_os = "macos") {
        let output = std::process::Command::new("lsof")
            .args(["-ti", &format!(":{}", port), "-sTCP:LISTEN"])
            .output()
            .map_err(|e| format!("Failed to check port: {}", e))?;

        if !output.stdout.is_empty() {
            let pid_str = String::from_utf8_lossy(&output.stdout);
            for pid in pid_str.lines() {
                let pid = pid.trim();
                if !pid.is_empty() {
                    std::process::Command::new("kill")
                        .args(["-9", pid])
                        .output()
                        .map_err(|e| format!("Failed to kill process {}: {}", pid, e))?;
                    tracing::info!(port = %port, pid = %pid, "Port process killed");
                }
            }
            Ok(())
        } else {
            Err(format!("No process found on port {}", port))
        }
    } else {
        // Windows
        let output = std::process::Command::new("netstat")
            .args(["-ano", "|", "findstr", &format!(":{}", port)])
            .output()
            .map_err(|e| format!("Failed to check port: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Some(pid) = line.trim().split_whitespace().last() {
                std::process::Command::new("taskkill")
                    .args(["/F", "/PID", pid])
                    .output()
                    .map_err(|e| format!("Failed to kill process: {}", e))?;
            }
        }
        Ok(())
    }
}
