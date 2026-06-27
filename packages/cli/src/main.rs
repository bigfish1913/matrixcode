//! MatrixCode CLI - Entry point

mod commands;
mod constants;
mod display;
mod helpers;
mod terminal_mode;
mod types;

use anyhow::Result;
use clap::Parser;
use commands::{run_daemon_mode, run_service_mode};
use terminal_mode::{run_terminal_mode, interactive_resume, list_sessions};
use types::Cli;

fn main() -> Result<()> {
    // Initialize logger
    env_logger::init();

    // Load .env file with multiple paths
    let current_dir = std::env::current_dir().unwrap_or_default();

    let mut env_paths: Vec<std::path::PathBuf> = vec![current_dir.join(".env")];
    let mut dir = current_dir.clone();
    for _ in 0..4 {
        if let Some(parent) = dir.parent() {
            env_paths.push(parent.join(".env"));
            dir = parent.to_path_buf();
        } else {
            break;
        }
    }

    let mut loaded_env = false;
    for path in &env_paths {
        if path.exists()
            && dotenvy::from_path(path).is_ok() {
                println!("[env: loaded from {}]", path.display());
                loaded_env = true;
                break;
            }
    }

    if !loaded_env {
        println!("[env: no .env file found, searched: {}]",
            env_paths.iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", "));
    }

    // Debug: show key env vars
    if let Ok(key) = std::env::var("API_KEY") {
        println!("[env: API_KEY={}...{}]", &key[..4.min(key.len())], &key[key.len()-4.min(key.len())..]);
    }
    if let Ok(model) = std::env::var("MODEL") {
        println!("[env: MODEL={}]", model);
    }
    if let Ok(provider) = std::env::var("PROVIDER") {
        println!("[env: PROVIDER={}]", provider);
    }

    let cli = Cli::parse();

    // Handle list sessions
    if cli.list_sessions {
        list_sessions();
        return Ok(());
    }

    // Handle interactive resume (-r)
    if cli.resume {
        return interactive_resume();
    }

    // Daemon mode
    if cli.mode == "daemon" {
        return run_daemon_mode();
    }

    // Run appropriate mode
    match cli.mode.as_str() {
        "terminal" | "tui" => run_terminal_mode(cli),
        "service" | "json" => run_service_mode(cli),
        _ => {
            eprintln!("Unknown mode: {}", cli.mode);
            std::process::exit(1);
        }
    }
}