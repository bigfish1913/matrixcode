//! MatrixCode CLI - Full Implementation with REPL

use anyhow::Result;
use clap::{Parser, Subcommand};
use matrixcode_core::{
    AgentEvent, Config, cancel::CancellationToken,
    agent::AgentBuilder,
    AnthropicProvider,
    SessionManager,
    tools::all_tools,
};
use matrixcode_tui::{TuiApp, setup_terminal, restore_terminal};
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
    0
}

/// List sessions
fn list_sessions() {
    println!("Sessions: (not implemented)");
}

/// Terminal mode with TUI
fn run_terminal_mode(cli: Cli) -> Result<()> {
    // Load config
    let config = Config::load();

    // Get API configuration
    let api_key = config.api_key.clone()
        .or_else(|| std::env::var("ANTHROPIC_AUTH_TOKEN").ok())
        .ok_or_else(|| anyhow::anyhow!("No API key found. Set ANTHROPIC_AUTH_TOKEN or configure in ~/.matrix/config.json"))?;

    let model = config.model.clone()
        .or_else(|| std::env::var("ANTHROPIC_MODEL").ok())
        .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());

    let base_url = config.base_url.clone()
        .or_else(|| std::env::var("ANTHROPIC_BASE_URL").ok())
        .unwrap_or_else(|| "https://api.anthropic.com".to_string());

    // Load skills (simplified)
    let _skills_count = load_skills(&Vec::new());

    // Handle single command without TUI
    if let Some(cmd) = cli.command {
        handle_command(cmd);
        return Ok(());
    }

    // Setup tokio runtime
    let rt = tokio::runtime::Runtime::new()?;

    // Create channels for Agent communication
    let (event_tx, event_rx) = tokio::sync::mpsc::channel(100);
    let (task_tx, mut task_rx) = tokio::sync::mpsc::channel::<String>(10);
    let (ask_tx, ask_rx) = tokio::sync::mpsc::channel::<String>(1);

    // Create cancellation token
    let cancel_token = CancellationToken::new();

    // Load session BEFORE spawning agent task so TUI can also display restored messages
    let project_path = std::env::current_dir().ok();
    let (restored_messages, session_mgr_state) = {
        let mut mgr = SessionManager::new().ok();
        let mut messages = Vec::new();
        
        if let Some(ref mut mgr) = mgr {
            if cli.continue_session || cli.resume.is_some() {
                let session = if let Some(ref query) = cli.resume {
                    mgr.resume(query, project_path.as_deref()).ok().flatten()
                } else {
                    mgr.continue_last(project_path.as_deref()).ok().flatten()
                };
                if let Some(s) = session {
                    messages = s.messages.clone();
                }
            } else {
                let _ = mgr.start_new(project_path.as_deref());
            }
        }
        (messages, mgr)
    };

    // Clone things needed in the agent task
    let agent_cancel = cancel_token.clone();
    let agent_event_tx = event_tx.clone();
    let agent_api_key = api_key.clone();
    let agent_model = model.clone();
    let agent_base_url = base_url.clone();
    let agent_think = cli.think;
    let agent_max_tokens = cli.max_tokens;
    let agent_restored_messages = restored_messages.clone();
    let agent_project_path = project_path.clone();

    // Spawn Agent task with real Agent
    let _agent_task = rt.spawn(async move {
        // Create provider
        let provider = AnthropicProvider::new(agent_api_key, agent_model.clone(), agent_base_url);

        // Load memory
        let project_path_ref = agent_project_path.as_deref();
        let mut memory_storage = matrixcode_core::memory::MemoryStorage::new(project_path_ref).ok();
        let memory_summary = memory_storage.as_ref()
            .and_then(|ms| ms.load_combined().ok())
            .map(|mem| mem.generate_prompt_summary(20))
            .unwrap_or_default();

        // Build system prompt with memory
        let system_prompt = if memory_summary.is_empty() {
            "You are a helpful AI coding assistant named MatrixCode.".to_string()
        } else {
            format!(
                "You are a helpful AI coding assistant named MatrixCode.\n\n{}", 
                memory_summary
            )
        };

        // Build agent with external event sender
        let mut agent = AgentBuilder::new(Box::new(provider))
            .system_prompt(system_prompt)
            .model_name(agent_model.clone())
            .max_tokens(agent_max_tokens)
            .think(agent_think)
            .tools(all_tools())
            .event_tx(agent_event_tx.clone())
            .build();

        // Restore messages from pre-loaded session
        if !agent_restored_messages.is_empty() {
            agent.set_messages(agent_restored_messages);
        }

        // Re-open session manager inside the task for saving
        let mut session_mgr = session_mgr_state;

        // Set cancel token
        agent.set_cancel_token(agent_cancel.clone());
        agent.set_ask_channel(ask_rx);

        while let Some(msg) = task_rx.recv().await {
            // Check cancellation
            if agent_cancel.is_cancelled() {
                agent_event_tx.send(AgentEvent::error(
                    "Operation interrupted by user".to_string(),
                    Some("interrupted".to_string()),
                    None,
                )).await.ok();
                agent_cancel.reset();
                continue;
            }

            // Handle special commands from TUI
            if msg == "/new" {
                agent.clear_history();
                if let Some(ref mut mgr) = session_mgr {
                    let _ = mgr.start_new(agent_project_path.as_deref());
                }
                continue;
            }
            if msg.starts_with("/mode:") {
                let mode = &msg[6..];
                match mode {
                    "ask" => agent.set_approve_mode(matrixcode_core::approval::ApproveMode::Ask),
                    "auto" => agent.set_approve_mode(matrixcode_core::approval::ApproveMode::Auto),
                    "strict" => agent.set_approve_mode(matrixcode_core::approval::ApproveMode::Strict),
                    _ => {}
                }
                continue;
            }

            // Run agent - events are sent directly via event_tx during run()
            match agent.run(msg.clone()).await {
                Ok(_) => {
                    // Auto-save session after each turn
                    if let Some(ref mut mgr) = session_mgr {
                        let (input_tokens, output_tokens) = agent.get_token_counts();
                        let messages = agent.get_messages();
                        mgr.set_messages(messages.to_vec());
                        mgr.update_stats(input_tokens as u32, output_tokens);
                        let _ = mgr.save_current();
                        
                        // Debug log: session save
                        matrixcode_core::debug::debug_log().session_save(messages.len(), output_tokens);
                    }
                    
                    // Auto-detect and save memories
                    if let Some(ref mut ms) = memory_storage {
                        let messages = agent.get_messages();
                        // Detect from last assistant message
                        if let Some(last_msg) = messages.last() {
                            let text = match &last_msg.content {
                                matrixcode_core::providers::MessageContent::Text(t) => t.clone(),
                                matrixcode_core::providers::MessageContent::Blocks(blocks) => {
                                    blocks.iter().filter_map(|b| match b {
                                        matrixcode_core::ContentBlock::Text { text } => Some(text.as_str()),
                                        _ => None,
                                    }).collect::<Vec<_>>().join("\n")
                                }
                            };
                            let detected = matrixcode_core::memory::detect_memories_from_text(
                                &text, None
                            );
                            if !detected.is_empty() {
                                let detected_count = detected.len();
                                if let Ok(mut mem) = ms.load_global() {
                                    for entry in detected {
                                        mem.add(entry);
                                    }
                                    let _ = ms.save_global(&mem);
                                    
                                    // Debug log: memory save
                                    matrixcode_core::debug_memory!(detected_count, text.len());
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    agent_event_tx.send(AgentEvent::error(
                        format!("Agent error: {}", e),
                        Some("agent_error".to_string()),
                        None,
                    )).await.ok();
                }
            }
        }
    });

    // Enter runtime context so tokio channels work in sync code
    let _guard = rt.enter();

    // Setup terminal for TUI
    let mut terminal = setup_terminal()?;

    // Create App and run it (TUI runs in sync context, but tokio channels are usable)
    let mut app = TuiApp::new(task_tx, event_rx, cancel_token)
        .with_ask_channel(ask_tx)
        .with_config(&model, cli.think, cli.max_tokens, None)
        .with_session_messages(&restored_messages);
    let result = app.run(&mut terminal);

    // Restore terminal
    restore_terminal()?;

    result
}

/// Handle single command
fn handle_command(cmd: Commands) {
    match cmd {
        Commands::Chat { message } => {
            if let Some(msg) = message {
                println!("Processing: {}", msg);
            } else {
                println!("Please provide a message with --message");
            }
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
                events.push(AgentEvent::tool_use_start("action_1", action, None));
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