use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};
use std::time::Duration;

use super::{Tool, ToolDefinition};
use crate::approval::RiskLevel;

/// Monitor tool for watching processes and file changes
pub struct MonitorTool;

#[async_trait]
impl Tool for MonitorTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "monitor".to_string(),
            description: "Monitor external processes or wait for state changes. Use to: (1) Wait for build/test completion; (2) Watch for file changes; (3) Monitor background services; (4) Track process status. Returns when monitored condition is met or timeout expires.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "mode": {
                        "type": "string",
                        "enum": ["process", "file", "port", "timer"],
                        "description": "Monitor mode: 'process' watches a process, 'file' watches for file changes, 'port' waits for port availability, 'timer' is a simple countdown"
                    },
                    "target": {
                        "type": "string",
                        "description": "Target to monitor: PID or process name for 'process', file path for 'file', port number for 'port'"
                    },
                    "timeout": {
                        "type": "integer",
                        "default": 30000,
                        "description": "Timeout in milliseconds (default 30s)"
                    },
                    "condition": {
                        "type": "string",
                        "enum": ["exit", "running", "exists", "changed", "available"],
                        "default": "available",
                        "description": "Condition to wait for: 'exit' waits for process to finish, 'running' waits for process to start, 'exists' waits for file to exist, 'changed' waits for file modification, 'available' waits for port to be available"
                    }
                },
                "required": ["mode"]
            }),
        }
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe  // Read-only monitoring
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let mode = params["mode"].as_str().ok_or_else(|| anyhow::anyhow!("missing 'mode'"))?;
        let target = params["target"].as_str().map(|s| s.to_string());
        let timeout_ms = params["timeout"].as_u64().unwrap_or(30000);
        let condition = params["condition"].as_str().unwrap_or("available");

        let mode = mode.to_string();
        let condition = condition.to_string();

        // Use spawn_blocking for synchronous monitoring operations
        tokio::task::spawn_blocking(move || {
            let timeout = Duration::from_millis(timeout_ms);
            let start = std::time::Instant::now();

            match mode.as_str() {
                "process" => monitor_process(target.as_deref(), &condition, timeout, start),
                "file" => monitor_file(target.as_deref(), &condition, timeout, start),
                "port" => monitor_port(target.as_deref(), timeout, start),
                "timer" => monitor_timer(timeout_ms, start),
                _ => Ok(format!("Unknown monitor mode: {}", mode))
            }
        }).await?
    }
}

/// Monitor a process
fn monitor_process(target: Option<&str>, condition: &str, timeout: Duration, start: std::time::Instant) -> Result<String> {
    let target_str = target.ok_or_else(|| anyhow::anyhow!("missing 'target' for process monitoring"))?;

    // Parse target as PID or process name
    let pid: Option<u32> = target_str.parse().ok();

    match condition {
        "exit" => {
            // Wait for process to exit
            loop {
                if start.elapsed() > timeout {
                    return Ok(format!("Timeout: Process {} still running after {:.1}s", target_str, timeout.as_secs_f64()));
                }

                // Check if process is still running
                let running = if let Some(pid) = pid {
                    is_process_running_by_pid(pid)
                } else {
                    is_process_running_by_name(target_str)
                };

                if !running {
                    return Ok(format!("Process {} has exited", target_str));
                }

                std::thread::sleep(Duration::from_millis(500));
            }
        }
        "running" => {
            // Wait for process to start
            loop {
                if start.elapsed() > timeout {
                    return Ok(format!("Timeout: Process {} not found after {:.1}s", target_str, timeout.as_secs_f64()));
                }

                let running = if let Some(pid) = pid {
                    is_process_running_by_pid(pid)
                } else {
                    is_process_running_by_name(target_str)
                };

                if running {
                    return Ok(format!("Process {} is now running", target_str));
                }

                std::thread::sleep(Duration::from_millis(500));
            }
        }
        _ => Ok(format!("Unknown process condition: {}", condition))
    }
}

/// Monitor a file
fn monitor_file(target: Option<&str>, condition: &str, timeout: Duration, start: std::time::Instant) -> Result<String> {
    let target_str = target.ok_or_else(|| anyhow::anyhow!("missing 'target' for file monitoring"))?;
    let path = std::path::Path::new(target_str);

    let initial_mtime = path.metadata()
        .and_then(|m| m.modified())
        .ok();

    match condition {
        "exists" => {
            loop {
                if start.elapsed() > timeout {
                    return Ok(format!("Timeout: File {} does not exist after {:.1}s", target_str, timeout.as_secs_f64()));
                }

                if path.exists() {
                    return Ok(format!("File {} now exists", target_str));
                }

                std::thread::sleep(Duration::from_millis(500));
            }
        }
        "changed" => {
            loop {
                if start.elapsed() > timeout {
                    return Ok(format!("Timeout: File {} not changed after {:.1}s", target_str, timeout.as_secs_f64()));
                }

                let current_mtime = path.metadata()
                    .and_then(|m| m.modified())
                    .ok();

                if let (Some(initial), Some(current)) = (initial_mtime, current_mtime) {
                    if current > initial {
                        return Ok(format!("File {} has been modified", target_str));
                    }
                }

                std::thread::sleep(Duration::from_millis(500));
            }
        }
        _ => Ok(format!("Unknown file condition: {}", condition))
    }
}

/// Monitor a port
fn monitor_port(target: Option<&str>, timeout: Duration, start: std::time::Instant) -> Result<String> {
    let target_str = target.ok_or_else(|| anyhow::anyhow!("missing 'target' for port monitoring"))?;
    let port: u16 = target_str.parse()
        .map_err(|_| anyhow::anyhow!("invalid port number: {}", target_str))?;

    loop {
        if start.elapsed() > timeout {
            return Ok(format!("Timeout: Port {} not available after {:.1}s", port, timeout.as_secs_f64()));
        }

        // Try to connect to port
        let addr = format!("127.0.0.1:{}", port);
        if std::net::TcpStream::connect(&addr).is_ok() {
            return Ok(format!("Port {} is now available", port));
        }

        std::thread::sleep(Duration::from_millis(500));
    }
}

/// Simple timer countdown
fn monitor_timer(timeout_ms: u64, start: std::time::Instant) -> Result<String> {
    let duration = Duration::from_millis(timeout_ms);
    loop {
        let elapsed = start.elapsed();
        if elapsed >= duration {
            return Ok(format!("Timer completed after {:.1}s", duration.as_secs_f64()));
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Check if process is running by PID
fn is_process_running_by_pid(pid: u32) -> bool {
    #[cfg(unix)]
    {
        use std::process::Command;
        Command::new("ps")
            .arg("-p")
            .arg(pid.to_string())
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    #[cfg(windows)]
    {
        use std::process::Command;
        let pid_str = pid.to_string();
        let pid_bytes = pid_str.as_bytes();
        Command::new("tasklist")
            .arg("/FI")
            .arg(format!("PID eq {}", pid))
            .output()
            .map(|o| o.stdout.windows(4).any(|w| {
                // Look for PID in output
                w.windows(pid_bytes.len()).any(|w2| w2 == pid_bytes)
            }))
            .unwrap_or(false)
    }
}

/// Check if process is running by name
fn is_process_running_by_name(name: &str) -> bool {
    #[cfg(unix)]
    {
        use std::process::Command;
        Command::new("pgrep")
            .arg("-x")
            .arg(name)
            .output()
            .map(|o| !o.stdout.is_empty())
            .unwrap_or(false)
    }

    #[cfg(windows)]
    {
        use std::process::Command;
        Command::new("tasklist")
            .arg("/FI")
            .arg(format!("IMAGENAME eq {}", name))
            .output()
            .map(|o| o.stdout.windows(name.len()).any(|w| w == name.as_bytes()))
            .unwrap_or(false)
    }
}