//! CodeGraph watcher management
//!
//! Handles starting, checking, and managing the CodeGraph file watcher.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use matrixcode_core::cancel::CancellationToken;
use matrixcode_core::tools::codegraph::CodeGraphWatcher;

/// Check if CodeGraph MCP daemon is running
pub fn is_daemon_running(project_path: &PathBuf) -> bool {
    // Method 1: Check daemon.pid file
    let daemon_pid_path = project_path.join(".codegraph").join("daemon.pid");
    if daemon_pid_path.exists() {
        let running = std::fs::read_to_string(&daemon_pid_path)
            .ok()
            .and_then(|pid| pid.trim().parse::<u32>().ok())
            .map(|pid| {
                #[cfg(target_os = "windows")]
                {
                    use std::os::windows::process::CommandExt;
                    const CREATE_NO_WINDOW: u32 = 0x08000000;
                    std::process::Command::new("tasklist")
                        .args(["/FI", &format!("PID eq {}", pid)])
                        .creation_flags(CREATE_NO_WINDOW)
                        .output()
                        .map(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()))
                        .unwrap_or(false)
                }
                #[cfg(not(target_os = "windows"))]
                std::path::Path::new("/proc").join(pid.to_string()).exists()
            })
            .unwrap_or(false);
        
        if running {
            return true;
        }
    }

    // Method 2: Check daemon.log for recent activity (last 60 seconds)
    let daemon_log_path = project_path.join(".codegraph").join("daemon.log");
    if daemon_log_path.exists() {
        if let Ok(metadata) = std::fs::metadata(&daemon_log_path) {
            if let Ok(modified) = metadata.modified() {
                let now = std::time::SystemTime::now();
                let elapsed = now.duration_since(modified).unwrap_or(std::time::Duration::MAX);
                if elapsed < std::time::Duration::from_secs(60) {
                    log::info!("CodeGraph: daemon.log recently modified, daemon likely active");
                    return true;
                }
            }
        }
    }

    false
}

/// Start CodeGraph watcher if no MCP daemon is running
pub fn start_watcher_if_needed(
    project_path: Option<&PathBuf>,
    cancel_token: CancellationToken,
    watcher_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
) {
    if let Some(path) = project_path {
        if is_daemon_running(path) {
            log::info!("CodeGraph MCP daemon detected, skipping our watcher to avoid conflict");
            return;
        }

        let watcher = CodeGraphWatcher::with_auto_detect(path.as_path());
        let handle = watcher.start(cancel_token);
        log::info!("CodeGraph watcher started (no MCP daemon detected)");
        *watcher_handle.lock().unwrap() = Some(handle);
    }
}

/// Check if watcher is running and start if needed (after /init)
pub fn ensure_watcher_running(
    project_path: &PathBuf,
    cancel_token: CancellationToken,
    watcher_handle: &Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
) {
    let mut handle_guard = watcher_handle.lock().unwrap();
    let watcher_running = handle_guard.is_some() &&
        handle_guard.as_ref().map(|h| !h.is_finished()).unwrap_or(false);

    if !watcher_running {
        if is_daemon_running(project_path) {
            log::info!("CodeGraph MCP daemon still running after /init, skipping watcher");
        } else {
            // No daemon, start our watcher
            let watcher = CodeGraphWatcher::with_auto_detect(project_path.as_path());
            let handle = watcher.start(cancel_token.clone());
            log::info!("CodeGraph watcher started after /init (no MCP daemon detected)");
            *handle_guard = Some(handle);
        }
    }
}

/// Cleanup watcher handle
pub fn cleanup_watcher(watcher_handle: &Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>) {
    let handle = watcher_handle.lock().unwrap();
    if let Some(ref h) = *handle
        && !h.is_finished() {
        log::info!("Aborting CodeGraph watcher...");
        h.abort();
    }
}