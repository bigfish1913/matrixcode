//! /init command handling

use std::path::{Path, PathBuf};

use matrixcode_core::overview::{OVERVIEW_FILENAME, MATRIXCODE_DIR};

/// Result of handling an init command
pub enum InitCommandResult {
    /// A simple message to display
    Message(String),
    /// Request to generate project overview (async operation)
    GenerateOverview,
}

/// Handle /init commands for project overview generation
pub fn handle_init_command(cmd: &str, project_path: Option<&Path>) -> InitCommandResult {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let subcmd = parts.get(1).copied().unwrap_or("");

    match subcmd {
        "" => InitCommandResult::GenerateOverview,
        "status" => handle_init_status(project_path),
        "clear" | "reset" => handle_init_reset(project_path),
        _ => InitCommandResult::Message(
            "Unknown init command. Use: /init, /init status, /init reset".into(),
        ),
    }
}

fn handle_init_status(project_path: Option<&Path>) -> InitCommandResult {
    let path = project_path
        .map(|p| p.to_path_buf())
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default();

    let overview_path = path.join(OVERVIEW_FILENAME);
    let matrix_dir = path.join(MATRIXCODE_DIR);
    let has_overview = overview_path.exists();
    let has_memory = matrix_dir.join("memory.json").exists();
    let has_session = matrix_dir.join("session.json").exists();

    let overview_info = if has_overview {
        if let Ok(content) = std::fs::read_to_string(&overview_path) {
            let lines = content.lines().count();
            format!("✓ exists ({} lines)", lines)
        } else {
            "✓ exists".into()
        }
    } else {
        "❌ not found (use /init to generate)".into()
    };

    InitCommandResult::Message(format!(
        "📊 Project: {}\n  Overview: {}\n  Memory: {}\n  Session: {}",
        path.display(),
        overview_info,
        if has_memory { "✓ exists" } else { "❌ none" },
        if has_session { "✓ exists" } else { "❌ none" }
    ))
}

fn handle_init_reset(project_path: Option<&Path>) -> InitCommandResult {
    let path = project_path
        .map(|p| p.to_path_buf())
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default();

    let overview_path = path.join(OVERVIEW_FILENAME);
    let matrix_dir = path.join(MATRIXCODE_DIR);

    let mut reset_msg = String::new();

    if overview_path.exists() {
        match std::fs::remove_file(&overview_path) {
            Ok(_) => reset_msg.push_str(&format!("✓ Removed overview: {}\n", overview_path.display())),
            Err(e) => reset_msg.push_str(&format!("❌ Failed to remove overview: {}\n", e)),
        }
    }

    if matrix_dir.exists() {
        match std::fs::remove_dir_all(&matrix_dir) {
            Ok(_) => reset_msg.push_str(&format!("✓ Removed config dir: {}\n", matrix_dir.display())),
            Err(e) => reset_msg.push_str(&format!("❌ Failed to remove config dir: {}\n", e)),
        }
    }

    if reset_msg.is_empty() {
        InitCommandResult::Message("⚠️ No project configuration found to reset.".into())
    } else {
        reset_msg.push_str("\nRun '/init' to regenerate project overview");
        InitCommandResult::Message(reset_msg)
    }
}