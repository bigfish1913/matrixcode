//! MatrixCode CLI - AI Code Agent
//!
//! Supports multiple modes:
//! - terminal: Terminal UI (default)
//! - service: Pure JSON output
//! - daemon: For plugin use (stdin/stdout)

use clap::{Parser, Subcommand};
use matrixcode_core::{Agent, AgentBuilder, AgentEvent, Config, EventCollector};
use matrixcode_tui::TerminalUI;
use anyhow::Result;

#[derive(Parser)]
#[command(name = "matrixcode")]
#[command(about = "AI Code Agent with multi-model support")]
#[command(version)]
struct Cli {
    /// Run mode
    #[arg(short, long, default_value = "terminal")]
    mode: String,

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
        /// Action type (explain, fix, test, refactor)
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

    // Daemon mode doesn't require subcommand
    if cli.mode == "daemon" {
        return run_daemon_mode();
    }

    match cli.mode.as_str() {
        "terminal" | "tui" => run_terminal_mode(cli),
        "service" | "json" => run_service_mode(cli),
        _ => {
            eprintln!("Unknown mode: {}. Use 'terminal', 'service', or 'daemon'", cli.mode);
            std::process::exit(1);
        }
    }
}

/// Terminal mode: use TerminalUI to render
fn run_terminal_mode(cli: Cli) -> Result<()> {
    let mut ui = TerminalUI::new();
    let config = Config::from_env();
    
    // TODO: Create real provider
    // For now, simulate events
    let events = match cli.command {
        Some(Commands::Chat { message }) => {
            let mut collector = EventCollector::new();
            collector.push(AgentEvent::session_started());
            
            if let Some(msg) = message {
                collector.push(AgentEvent::text_delta(format!("Processing: {}", msg)));
                collector.push(AgentEvent::text_end());
                collector.push(AgentEvent::usage(100, 50));
            } else {
                collector.push(AgentEvent::text_delta(
                    "Welcome to MatrixCode! Type your message."
                ));
            }
            
            collector.push(AgentEvent::session_ended());
            collector.events().to_vec()
        }
        Some(_) => {
            vec![AgentEvent::error("Command not implemented", None, None)]
        }
        None => {
            vec![AgentEvent::text_delta("Please specify a command.")]
        }
    };

    // UI renders events
    ui.handle_events(&events);

    Ok(())
}

/// Service mode: pure JSON output
fn run_service_mode(cli: Cli) -> Result<()> {
    match cli.command {
        Some(Commands::Chat { message }) => {
            let mut collector = EventCollector::new();
            collector.push(AgentEvent::session_started());
            
            if let Some(msg) = message {
                collector.push(AgentEvent::text_delta(msg));
            }
            
            collector.push(AgentEvent::session_ended());
            
            // Output JSON stream
            println!("{}", collector.output_json_lines()?);
        }
        Some(_) => {
            println!("{}", AgentEvent::error(
                "Command not implemented".to_string(),
                None,
                None,
            ).to_json()?);
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

        // End marker
        writeln!(stdout_lock, "---END---")?;
        stdout_lock.flush()?;
    }

    Ok(())
}

/// Daemon request
#[derive(serde::Deserialize)]
struct DaemonRequest {
    /// Request type
    #[serde(rename = "type")]
    request_type: String,
    /// Content
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    /// Action type
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