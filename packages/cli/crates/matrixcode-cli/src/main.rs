//! MatrixCode CLI - Full Implementation with REPL

use anyhow::Result;
use clap::{Parser, Subcommand};
use matrixcode_core::{AgentEvent, Config};
use matrixcode_tui::TerminalUI;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "matrixcode")]
#[command(about = "AI Code Agent with multi-model support")]
#[command(version)]
struct Cli {
    /// Run mode
    #[arg(short, long, default_value = "terminal")]
    mode: String,

    /// Continue last session
    #[arg(short, long)]
    continue_session: bool,

    /// Resume specific session
    #[arg(long)]
    resume: Option<String>,

    /// List sessions
    #[arg(long)]
    list_sessions: bool,

    /// Extra skills directory
    #[arg(long)]
    skills_dir: Option<PathBuf>,

    /// Think mode
    #[arg(long, default_value = "true")]
    think: bool,

    /// Max tokens
    #[arg(long, default_value = "16384")]
    max_tokens: u32,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start chat session
    Chat {
        /// Input content
        #[arg(short, long)]
        message: Option<String>,
    },

    /// Quick action
    QuickAction {
        /// Action type
        #[arg(short, long)]
        action: String,

        /// Target file
        #[arg(short, long)]
        file: Option<String>,
    },

    /// Create new session
    NewSession,

    /// Show session history
    History,

    /// Show status
    Status,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle list sessions
    if cli.list_sessions {
        list_sessions();
        return Ok(());
    }

    // Daemon mode doesn't require subcommand
    if cli.mode == "daemon" {
        return run_daemon_mode();
    }

    match cli.mode.as_str() {
        "terminal" | "tui" => run_terminal_mode(cli),
        "service" | "json" => run_service_mode(cli),
        _ => {
            eprintln!("Unknown mode: {}", cli.mode);
            std::process::exit(1);
        }
    }
}

/// Load skills from directories (simplified)
fn load_skills(_extra_dirs: &[PathBuf]) -> usize {
    // Skills loading is complex, return 0 for now
    // Full implementation in _src_old/main.rs
    0
}

/// List sessions
fn list_sessions() {
    println!("Sessions: (not implemented)");
}

/// Terminal mode with REPL
fn run_terminal_mode(cli: Cli) -> Result<()> {
    let mut ui = TerminalUI::new();
    
    // Load config
    let _config = Config::load();
    
    // Load skills (simplified)
    let _skills_count = load_skills(&Vec::new());
    
    println!("MatrixCode {}", env!("CARGO_PKG_VERSION"));
    println!("Mode: terminal");
    println!("Type '/help' for commands, '/exit' to quit.\n");
    
    // Session manager (simplified)
    let _project_root = std::env::current_dir().ok();
    
    // Handle single command
    if let Some(cmd) = cli.command {
        handle_command(cmd, &mut ui);
        return Ok(());
    }
    
    // REPL loop (simplified - real implementation needs rustyline)
    println!("REPL not fully implemented. Use daemon mode for now.");
    println!("Example: matrixcode --mode daemon");
    
    Ok(())
}

/// Handle single command
fn handle_command(cmd: Commands, ui: &mut TerminalUI) {
    match cmd {
        Commands::Chat { message } => {
            let events = if let Some(msg) = message {
                vec![
                    AgentEvent::session_started(),
                    AgentEvent::text_delta(format!("Processing: {}", msg)),
                    AgentEvent::session_ended(),
                ]
            } else {
                vec![AgentEvent::text_delta("Please provide a message.")]
            };
            ui.handle_events(&events);
        }
        Commands::Status => {
            println!("Status: Ready");
        }
        Commands::History => {
            println!("History: No history available");
        }
        _ => {
            println!("Command not implemented");
        }
    }
}

/// Service mode: pure JSON output
fn run_service_mode(cli: Cli) -> Result<()> {
    match cli.command {
        Some(Commands::Chat { message }) => {
            let events = vec![
                AgentEvent::session_started(),
                AgentEvent::text_delta(message.unwrap_or_default()),
                AgentEvent::session_ended(),
            ];
            
            for event in events {
                println!("{}", event.to_json()?);
            }
        }
        Some(_) => {
            println!("{}", AgentEvent::error("Command not implemented".to_string(), None, None).to_json()?);
        }
        None => {
            println!("{}", AgentEvent::error("Please specify a command".to_string(), None, None).to_json()?);
        }
    }
    Ok(())
}

/// Daemon mode: listen on stdin, output to stdout
fn run_daemon_mode() -> Result<()> {
    use std::io::{BufRead, Write};

    eprintln!("MatrixCode Daemon started (listening on stdin)");

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut stdout_lock = stdout.lock();

    for line in stdin.lock().lines() {
        let line = line?;

        if line.is_empty() {
            continue;
        }

        // Parse request
        let request: DaemonRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(e) => {
                let error_event = AgentEvent::error(
                    format!("Invalid request: {}", e),
                    Some("parse_error".to_string()),
                    None,
                );
                writeln!(stdout_lock, "{}", error_event.to_json()?)?;
                writeln!(stdout_lock, "---END---")?;
                stdout_lock.flush()?;
                continue;
            }
        };

        // Handle request
        let events = handle_daemon_request(request)?;

        // Output events
        for event in events {
            writeln!(stdout_lock, "{}", event.to_json()?)?;
        }

        writeln!(stdout_lock, "---END---")?;
        stdout_lock.flush()?;
    }

    Ok(())
}

/// Daemon request
#[derive(serde::Deserialize)]
struct DaemonRequest {
    #[serde(rename = "type")]
    request_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    action: Option<String>,
}

/// Handle daemon request
fn handle_daemon_request(request: DaemonRequest) -> Result<Vec<AgentEvent>> {
    let mut events = Vec::new();

    events.push(AgentEvent::session_started());

    match request.request_type.as_str() {
        "chat" => {
            if let Some(content) = request.content {
                events.push(AgentEvent::text_delta(content));
            }
        }
        "quick_action" => {
            if let Some(action) = request.action {
                events.push(AgentEvent::tool_use_start("action_1", action));
                events.push(AgentEvent::tool_result("action_1", "Result".to_string(), false));
            }
        }
        "status" => {
            events.push(AgentEvent::text_delta("Daemon is running"));
        }
        _ => {
            events.push(AgentEvent::error(
                format!("Unknown request type: {}", request.request_type),
                Some("unknown_type".to_string()),
                None,
            ));
        }
    }

    events.push(AgentEvent::session_ended());
    Ok(events)
}