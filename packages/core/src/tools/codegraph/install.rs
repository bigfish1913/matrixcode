//! CodeGraph CLI installation and detection.

use anyhow::Result;
use std::path::PathBuf;
use tokio::process::Command;

/// Create a command with hidden window on Windows.
#[cfg(windows)]
fn create_command(program: &str) -> std::process::Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let mut cmd = std::process::Command::new(program);
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

#[cfg(not(windows))]
fn create_command(program: &str) -> std::process::Command {
    std::process::Command::new(program)
}

/// Create an async command with hidden window on Windows.
#[cfg(windows)]
fn create_async_command(program: &str) -> Command {
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let mut cmd = Command::new(program);
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

#[cfg(not(windows))]
fn create_async_command(program: &str) -> Command {
    Command::new(program)
}

/// Get CodeGraph installation directory (platform-specific).
fn get_codegraph_install_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|p| p.join("codegraph").join("current").join("bin"))
}

/// Get CodeGraph CLI executable name (platform-specific).
fn get_codegraph_exe_name() -> String {
    if cfg!(windows) {
        "codegraph.cmd".to_string()
    } else {
        "codegraph".to_string()
    }
}

/// Check if CodeGraph CLI is installed.
pub fn is_codegraph_installed() -> bool {
    // Try direct command (in PATH)
    if create_command("codegraph")
        .arg("--version")
        .output()
        .is_ok()
    {
        return true;
    }

    // Try platform-specific installation path
    if let Some(install_dir) = get_codegraph_install_dir() {
        let exe_name = get_codegraph_exe_name();
        let exe_path = install_dir.join(&exe_name);
        if exe_path.exists()
            && create_command(exe_path.to_str().unwrap_or("codegraph"))
                .arg("--version")
                .output()
                .is_ok()
        {
            return true;
        }
    }

    false
}

/// Get CodeGraph CLI path (returns the executable path or command name).
pub fn get_codegraph_path() -> Option<String> {
    // Try direct command first (in PATH)
    if create_command("codegraph")
        .arg("--version")
        .output()
        .is_ok()
    {
        return Some("codegraph".to_string());
    }

    // Try platform-specific installation path
    if let Some(install_dir) = get_codegraph_install_dir() {
        let exe_name = get_codegraph_exe_name();
        let exe_path = install_dir.join(&exe_name);
        if exe_path.exists() {
            return Some(exe_path.to_string_lossy().to_string());
        }
    }

    None
}

/// Auto-install CodeGraph CLI (Windows).
pub async fn install_codegraph() -> Result<()> {
    log::info!("Installing CodeGraph CLI...");

    // Windows PowerShell installer
    let result = create_async_command("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "irm https://raw.githubusercontent.com/colbymchenry/codegraph/main/install.ps1 | iex",
        ])
        .output()
        .await?;

    if result.status.success() {
        log::info!("CodeGraph CLI installed successfully");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&result.stderr);
        Err(anyhow::anyhow!("CodeGraph installation failed: {}", stderr))
    }
}

/// Ensure CodeGraph is available with optional auto-install.
pub async fn ensure_codegraph() -> Result<String> {
    if let Some(path) = get_codegraph_path() {
        return Ok(path);
    }

    install_codegraph().await?;
    get_codegraph_path().ok_or_else(|| anyhow::anyhow!("CodeGraph not found after installation"))
}
