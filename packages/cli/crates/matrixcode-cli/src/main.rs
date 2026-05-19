//! MatrixCode CLI - Full Implementation with REPL

use anyhow::Result;
use clap::{Parser, Subcommand};
use matrixcode_core::{
    AgentEvent, Config, cancel::CancellationToken,
    agent::AgentBuilder,
    AnthropicProvider,
    SessionManager,
    tools::all_tools,
    memory::MemoryStorage,
};
use matrixcode_tui::{TuiApp, setup_terminal, restore_terminal};
use std::path::{PathBuf, Path};

// Handle /init commands for project overview generation
// Note: For async operations, we return a special command that will be handled in the agent task
fn handle_init_command(cmd: &str, project_path: Option<&Path>) -> InitCommandResult {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let subcmd = parts.get(1).copied().unwrap_or("");
    
    match subcmd {
        "" => {
            // /init without subcommand - generate project overview
            InitCommandResult::GenerateOverview
        }
        "status" => {
            // Show current project overview status
            if let Some(path) = project_path {
                let overview_path = path.join(matrixcode_core::overview::OVERVIEW_FILENAME);
                let matrix_dir = path.join(matrixcode_core::overview::MATRIXCODE_DIR);
                let has_overview = overview_path.exists();
                let has_memory = matrix_dir.join("memory.json").exists();
                let has_session = matrix_dir.join("session.json").exists();
                
                let overview_info = if has_overview {
                    if let Ok(metadata) = std::fs::metadata(&overview_path) {
                        if let Ok(modified) = metadata.modified() {
                            let modified_time: chrono::DateTime<chrono::Local> = modified.into();
                            format!("✓ exists (modified: {})", modified_time.format("%Y-%m-%d %H:%M"))
                        } else {
                            "✓ exists".into()
                        }
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
            } else {
                InitCommandResult::Message("⚠️ No project path set. Use: matrixcode --project <path>".into())
            }
        }
        "clear" | "reset" => {
            // Clear project overview
            if let Some(path) = project_path {
                let overview_path = path.join(matrixcode_core::overview::OVERVIEW_FILENAME);
                if overview_path.exists() {
                    match std::fs::remove_file(&overview_path) {
                        Ok(_) => InitCommandResult::Message(format!("✓ Project overview cleared: {}", overview_path.display())),
                        Err(e) => InitCommandResult::Message(format!("❌ Failed to clear overview: {}", e)),
                    }
                } else {
                    InitCommandResult::Message("⚠️ No project overview found to clear.".into())
                }
            } else {
                InitCommandResult::Message("⚠️ No project path set.".into())
            }
        }
        _ => {
            InitCommandResult::Message("⚠️ Unknown init command. Use: /init, /init status, /init clear".into())
        }
    }
}

/// Result of handling an init command
enum InitCommandResult {
    /// A simple message to display
    Message(String),
    /// Request to generate project overview (async operation)
    GenerateOverview,
}

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

/// Load skills from directories
fn load_skills(extra_dirs: &[PathBuf]) -> Vec<matrixcode_core::skills::Skill> {
    use matrixcode_core::skills::discover_skills;
    use std::path::PathBuf;
    
    // Build list of skill directories to search
    let mut roots: Vec<PathBuf> = Vec::new();
    
    // 1. User's global skills directory (~/.matrix/skills)
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join(".matrix").join("skills"));
    }
    
    // 2. Project-local skills directory (.matrix/skills)
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd.join(".matrix").join("skills"));
    }
    
    // 3. Extra directories from CLI option
    roots.extend(extra_dirs.iter().cloned());
    
    // Discover and load skills
    let skills = discover_skills(&roots);
    
    if !skills.is_empty() {
        eprintln!("[skills] Loaded {} skill(s)", skills.len());
    }
    
    skills
}

/// List sessions
fn list_sessions() {
    use matrixcode_core::session::SessionManager;
    
    let mgr = SessionManager::new().ok();
    if let Some(mgr) = mgr {
        let sessions = mgr.list_sessions();
        if sessions.is_empty() {
            println!("No sessions found.");
            println!("\nTip: Use 'matrixcode' to start a new session.");
        } else {
            println!("Sessions:\n");
            for (i, session) in sessions.iter().enumerate() {
                let status = if mgr.has_current() && mgr.current_id() == Some(session.id.as_str()) { 
                    " [current]" 
                } else { 
                    "" 
                };
                let project = session.project_path.as_deref().unwrap_or("unknown");
                println!("  {}. {} ({}){}", 
                    i + 1, 
                    session.short_id(),
                    project,
                    status
                );
            }
            println!("\nTotal: {} sessions", sessions.len());
            println!("\nResume: matrixcode --resume <id>");
        }
    } else {
        println!("No session manager available.");
        println!("Sessions directory: ~/.matrix/sessions/");
    }
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

    // Load skills
    let skills_dirs: Vec<PathBuf> = cli.skills_dir.iter().cloned().collect();
    let skills = load_skills(&skills_dirs);
    
    // Handle single command without TUI
    if let Some(cmd) = cli.command {
        handle_command(cmd, &skills);
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
    let agent_approve_mode = config.approve_mode.as_ref()
        .map(|m| matrixcode_core::approval::ApproveMode::parse(m))
        .unwrap_or(matrixcode_core::approval::ApproveMode::Ask);
    
    // Clone skills for agent task
    let agent_skills = skills.clone();

    // Spawn Agent task with real Agent
    let _agent_task = rt.spawn(async move {
        // Create provider (clone values so they can be reused for overview generation)
        let provider = AnthropicProvider::new(agent_api_key.clone(), agent_model.clone(), agent_base_url.clone());

        // Load memory
        let project_path_ref = agent_project_path.as_deref();
        let mut memory_storage = matrixcode_core::memory::MemoryStorage::new(project_path_ref).ok();
        let memory = memory_storage.as_ref()
            .and_then(|ms| ms.load_combined().ok());
        
        // Send MemoryLoaded event if we have entries
        if let Some(ref mem) = memory
            && !mem.entries.is_empty() {
            let _ = agent_event_tx.send(matrixcode_core::AgentEvent::with_data(
                matrixcode_core::EventType::MemoryLoaded,
                matrixcode_core::EventData::Memory {
                    summary: mem.generate_prompt_summary(10),
                    entries_count: mem.entries.len(),
                },
            )).await;
        }
        
        let memory_summary = memory
            .map(|mem| mem.generate_prompt_summary(20))
            .unwrap_or_default();

        // Load project overview (MATRIX.md)
        let project_overview = project_path_ref
            .and_then(|path| matrixcode_core::overview::ProjectOverview::load(path).ok().flatten());
        
        // Log overview loading
        if let Some(ref overview) = project_overview {
            matrixcode_core::debug::debug_log().log("overview", &format!("Loaded project overview: {} chars", overview.content.len()));
        }

        // Build system prompt with memory, project overview and skills
        let system_prompt = matrixcode_core::prompt::build_system_prompt(
            &matrixcode_core::prompt::PromptProfile::Default,
            &agent_skills,
            project_overview.as_ref().map(|o| o.content.as_str()),
            if memory_summary.is_empty() { None } else { Some(&memory_summary) },
        );

        // Build agent with external event sender
        let mut agent = AgentBuilder::new(Box::new(provider))
            .system_prompt(system_prompt)
            .model_name(agent_model.clone())
            .max_tokens(agent_max_tokens)
            .think(agent_think)
            .tools(all_tools())
            .event_tx(agent_event_tx.clone())
            .approve_mode(agent_approve_mode)
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

            // Extract keywords from user message for debug
            let keywords = matrixcode_core::memory::extract_context_keywords(&msg);
            if !keywords.is_empty() {
                matrixcode_core::debug_keywords!(&keywords, &msg);
                // Send KeywordsExtracted event to TUI
                let _ = agent_event_tx.send(matrixcode_core::AgentEvent::with_data(
                    matrixcode_core::EventType::KeywordsExtracted,
                    matrixcode_core::EventData::Keywords {
                        keywords: keywords.clone(),
                        source: msg.clone(),
                    },
                )).await;
            }

            // Handle special commands from TUI
            if msg == "/new" {
                agent.clear_history();
                if let Some(ref mut mgr) = session_mgr {
                    let _ = mgr.start_new(agent_project_path.as_deref());
                }
                // Send session ended event
                let _ = agent_event_tx.send(matrixcode_core::AgentEvent::session_ended()).await;
                continue;
            }
            
            // Handle /init commands
            if msg.starts_with("/init") {
                let result = handle_init_command(&msg, agent_project_path.as_deref());
                match result {
                    InitCommandResult::Message(msg) => {
                        let _ = agent_event_tx.send(matrixcode_core::AgentEvent::with_data(
                            matrixcode_core::EventType::Progress,
                            matrixcode_core::EventData::Progress {
                                message: msg,
                                percentage: None,
                            },
                        )).await;
                    }
                    InitCommandResult::GenerateOverview => {
                        // Generate project overview using AI
                        let _ = agent_event_tx.send(matrixcode_core::AgentEvent::with_data(
                            matrixcode_core::EventType::Progress,
                            matrixcode_core::EventData::Progress {
                                message: "🔄 Generating project overview...".into(),
                                percentage: Some(10),
                            },
                        )).await;
                        
                        if let Some(ref path) = agent_project_path {
                            // Create a new provider for overview generation
                            let overview_provider = AnthropicProvider::new(
                                agent_api_key.clone(),
                                agent_model.clone(),
                                agent_base_url.clone(),
                            );
                            
                            match matrixcode_core::overview::ProjectOverview::generate_with_ai(path.as_path(), &overview_provider).await {
                                Ok(overview) => {
                                    let _ = agent_event_tx.send(matrixcode_core::AgentEvent::with_data(
                                        matrixcode_core::EventType::Progress,
                                        matrixcode_core::EventData::Progress {
                                            message: format!("✓ Project overview generated: {}", overview.path.display()),
                                            percentage: Some(100),
                                        },
                                    )).await;
                                    
                                    // Log overview content for debug
                                    matrixcode_core::debug::debug_log().log("overview", &format!("Generated overview with {} chars", overview.content.len()));
                                }
                                Err(e) => {
                                    let _ = agent_event_tx.send(matrixcode_core::AgentEvent::error(
                                        format!("Failed to generate overview: {}", e),
                                        Some("overview_error".into()),
                                        None,
                                    )).await;
                                }
                            }
                        } else {
                            let _ = agent_event_tx.send(matrixcode_core::AgentEvent::error(
                                String::from("No project path set. Cannot generate overview."),
                                Some("no_project".into()),
                                None,
                            )).await;
                        }
                    }
                }
                continue;
            }
            
            // Handle /skills commands
            if msg == "/skills" || msg.starts_with("/skills ") {
                let parts: Vec<&str> = msg.split_whitespace().collect();
                let subcmd = parts.get(1).copied().unwrap_or("");
                
                let response = if subcmd.is_empty() || subcmd == "list" {
                    // List all available skills
                    if agent_skills.is_empty() {
                        "📚 No skills loaded.\n\nSkills directories searched:\n  - ~/.matrix/skills\n  - .matrix/skills\n\nTo add a skill, create a SKILL.md file in a subdirectory.".to_string()
                    } else {
                        let mut info = format!("📚 Loaded skills ({}):\n\n", agent_skills.len());
                        for skill in &agent_skills {
                            info.push_str(&format!("• {}: {}\n", skill.name, skill.description));
                            info.push_str(&format!("  Source: {}\n", skill.source_file.display()));
                        }
                        info.push_str("\nUse `/skills <name>` to view a skill's content.");
                        info
                    }
                } else if subcmd == "reload" {
                    // Reload skills from directories
                    let skills_dirs: Vec<PathBuf> = Vec::new();
                    let new_skills = load_skills(&skills_dirs);
                    let count = new_skills.len();
                    // Note: we can't actually update agent_skills in the async task,
                    // but we can show the reload result
                    format!("🔄 Skills reloaded: {} skill(s) found.\n\nNote: Restart MatrixCode to use new skills.", count)
                } else {
                    // Show specific skill content
                    let skill_name = subcmd;
                    if let Some(skill) = agent_skills.iter().find(|s| s.name == skill_name) {
                        format!("📚 Skill: {}\n\n{}\n\nSource: {}", 
                            skill.name, 
                            skill.body,
                            skill.source_file.display()
                        )
                    } else {
                        format!("❌ Skill '{}' not found.\n\nUse `/skills list` to see available skills.", skill_name)
                    }
                };
                
                let _ = agent_event_tx.send(matrixcode_core::AgentEvent::with_data(
                    matrixcode_core::EventType::Progress,
                    matrixcode_core::EventData::Progress {
                        message: response,
                        percentage: None,
                    },
                )).await;
                continue;
            }
            
            if msg == "/compact" || msg == "/compress" {
                // Manual compression request
                let original_tokens = matrixcode_core::compress::estimate_total_tokens(agent.get_messages());
                if original_tokens > 100 {
                    // Send compression triggered event
                    let _ = agent_event_tx.send(matrixcode_core::AgentEvent::with_data(
                        matrixcode_core::EventType::CompressionTriggered,
                        matrixcode_core::EventData::Progress {
                            message: format!("Compressing {} tokens...", original_tokens),
                            percentage: None,
                        },
                    )).await;
                    
                    // Perform compression
                    match matrixcode_core::compress::compress_messages(
                        agent.get_messages(),
                        matrixcode_core::compress::CompressionStrategy::SlidingWindow,
                        &matrixcode_core::compress::CompressionConfig::default(),
                    ) {
                        Ok(compressed) => {
                            let compressed_tokens = matrixcode_core::compress::estimate_total_tokens(&compressed);
                            agent.set_messages(compressed);
                            let ratio = compressed_tokens as f32 / original_tokens as f32;
                            
                            // Send completion event
                            let _ = agent_event_tx.send(matrixcode_core::AgentEvent::with_data(
                                matrixcode_core::EventType::CompressionCompleted,
                                matrixcode_core::EventData::Compression {
                                    original_tokens: original_tokens as u64,
                                    compressed_tokens: compressed_tokens as u64,
                                    ratio,
                                },
                            )).await;
                            
                            // Debug log
                            matrixcode_core::debug_compress!(original_tokens as u32, compressed_tokens, ratio);
                        }
                        Err(e) => {
                            let _ = agent_event_tx.send(matrixcode_core::AgentEvent::error(
                                format!("Compression failed: {}", e),
                                None,
                                None,
                            )).await;
                        }
                    }
                } else {
                    let _ = agent_event_tx.send(matrixcode_core::AgentEvent::progress(
                        "Context too small, no need to compress",
                        None,
                    )).await;
                }
                continue;
            }
            if let Some(mode) = msg.strip_prefix("/mode:") {
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
                                    
                                    // Send event to TUI
                                    let _ = agent_event_tx.send(matrixcode_core::AgentEvent::with_data(
                                        matrixcode_core::EventType::MemoryDetected,
                                        matrixcode_core::EventData::Memory {
                                            summary: format!("Detected {} memory entries", detected_count),
                                            entries_count: detected_count,
                                        },
                                    )).await;
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
        .with_config(&model, cli.think, cli.max_tokens, None);
    
    // Load restored messages if any
    if !restored_messages.is_empty() {
        app.load_messages(restored_messages);
    }
    let result = app.run(&mut terminal);

    // Restore terminal
    restore_terminal()?;

    result
}

/// Handle single command with actual agent execution
fn handle_command(cmd: Commands, skills: &[matrixcode_core::skills::Skill]) {
    // Load config
    let config = Config::load();
    
    // Get API configuration
    let api_key = config.api_key.clone()
        .or_else(|| std::env::var("ANTHROPIC_AUTH_TOKEN").ok())
        .unwrap_or_else(|| {
            eprintln!("❌ No API key found. Set ANTHROPIC_AUTH_TOKEN or configure in ~/.matrix/config.json");
            std::process::exit(1);
        });

    let model = config.model.clone()
        .or_else(|| std::env::var("ANTHROPIC_MODEL").ok())
        .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());

    let base_url = config.base_url.clone()
        .or_else(|| std::env::var("ANTHROPIC_BASE_URL").ok())
        .unwrap_or_else(|| "https://api.anthropic.com".to_string());
    
    let approve_mode = config.approve_mode.as_ref()
        .map(|m| matrixcode_core::approval::ApproveMode::parse(m))
        .unwrap_or(matrixcode_core::approval::ApproveMode::Ask);

    // Create tokio runtime
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    rt.block_on(async {
        match cmd {
            Commands::Chat { message } => {
                // Interactive or single-shot chat
                if let Some(msg) = message {
                    // Single-shot chat
                    println!("🤖 Processing: {}", msg);
                    
                    // Build system prompt with skills
                    let system_prompt = matrixcode_core::prompt::build_system_prompt(
                        &matrixcode_core::prompt::PromptProfile::Default,
                        skills,
                        None,
                        None,
                    );
                    
                    // Create provider
                    let provider = AnthropicProvider::new(api_key, model.clone(), base_url);
                    
                    // Build agent
                    let mut agent = AgentBuilder::new(Box::new(provider))
                        .system_prompt(system_prompt)
                        .model_name(model.clone())
                        .max_tokens(4096)
                        .tools(all_tools())
                        .approve_mode(approve_mode)
                        .build();
                    
                    // Run agent
                    match agent.run(msg).await {
                        Ok(_) => {
                            // Get last assistant message
                            let messages = agent.get_messages();
                            if let Some(last) = messages.last() {
                                let text = match &last.content {
                                    matrixcode_core::providers::MessageContent::Text(t) => t.clone(),
                                    matrixcode_core::providers::MessageContent::Blocks(blocks) => {
                                        blocks.iter().filter_map(|b| match b {
                                            matrixcode_core::ContentBlock::Text { text } => Some(text.as_str()),
                                            _ => None,
                                        }).collect::<Vec<_>>().join("\n")
                                    }
                                };
                                println!("\n{}", text);
                            }
                            
                            let (input, output) = agent.get_token_counts();
                            println!("\n📊 Tokens: {} in, {} out", input, output);
                        }
                        Err(e) => {
                            eprintln!("❌ Error: {}", e);
                        }
                    }
                } else {
                    // No message provided - start interactive mode
                    println!("Starting interactive chat session...");
                    println!("Note: For interactive chat, run 'matrixcode' without subcommand.");
                }
            }
            Commands::Status => {
                // Show system status (sync)
                println!("MatrixCode Status:\n");
                println!("  Version: {}", env!("CARGO_PKG_VERSION"));
                println!("  Mode: Ready");
                
                // Show configuration
                if config.api_key.is_some() || std::env::var("ANTHROPIC_AUTH_TOKEN").ok().is_some() {
                    println!("  API: ✓ configured");
                } else {
                    println!("  API: ❌ not configured");
                    println!("       Set ANTHROPIC_AUTH_TOKEN or configure in ~/.matrix/config.json");
                }
                
                if let Some(model) = &config.model {
                    println!("  Model: {}", model);
                } else if let Ok(model) = std::env::var("ANTHROPIC_MODEL") {
                    println!("  Model: {} (from env)", model);
                } else {
                    println!("  Model: claude-sonnet-4-20250514 (default)");
                }
                
                if let Some(base_url) = &config.base_url {
                    println!("  Base URL: {}", base_url);
                } else if let Ok(url) = std::env::var("ANTHROPIC_BASE_URL") {
                    println!("  Base URL: {} (from env)", url);
                }
                
                // Show approve mode
                if let Some(mode) = &config.approve_mode {
                    println!("  Approve Mode: {}", mode);
                } else {
                    println!("  Approve Mode: ask (default)");
                }
                
                // Show sessions
                if let Some(mgr) = SessionManager::new().ok() {
                    println!("  Sessions: {} (current: {})", 
                        mgr.list_sessions().len(),
                        if mgr.has_current() { "yes" } else { "no" }
                    );
                }
                
                // Show memory
                let project_path = std::env::current_dir().ok();
                if let Some(path) = &project_path {
                    if let Ok(storage) = MemoryStorage::new(Some(path.as_path())) {
                        if let Ok(mem) = storage.load_combined() {
                            println!("  Memory: {} entries", mem.entries.len());
                        }
                    }
                    
                    // Show project overview status
                    let overview_path = path.join(matrixcode_core::overview::OVERVIEW_FILENAME);
                    if overview_path.exists() {
                        if let Ok(metadata) = std::fs::metadata(&overview_path) {
                            let size = metadata.len();
                            if let Ok(modified) = metadata.modified() {
                                let modified_time: chrono::DateTime<chrono::Local> = modified.into();
                                println!("  Overview: ✓ MATRIX.md ({}, modified: {})", 
                                    if size > 1024 { format!("{} KB", size / 1024) } else { format!("{} bytes", size) },
                                    modified_time.format("%Y-%m-%d %H:%M")
                                );
                            } else {
                                println!("  Overview: ✓ MATRIX.md ({})", size);
                            }
                        }
                    } else {
                        println!("  Overview: ❌ not found (use /init to generate)");
                    }
                }
            }
            Commands::History => {
                // Show session history (sync)
                if let Some(mgr) = SessionManager::new().ok() {
                    let sessions = mgr.list_sessions();
                    if sessions.is_empty() {
                        println!("No session history found.");
                    } else {
                        println!("Session History:\n");
                        for session in sessions {
                            let project = session.project_path.as_deref().unwrap_or("unknown");
                            let is_current = mgr.has_current() && mgr.current_id() == Some(session.id.as_str());
                            
                            println!("Session: {} ({})", session.short_id(), session.id);
                            println!("  Project: {}", project);
                            println!("  Created: {}", session.created_at.format("%Y-%m-%d %H:%M"));
                            println!("  Current: {}", if is_current { "yes" } else { "no" });
                            println!("  Messages: {}", session.message_count);
                            println!("  Tokens: {} in, {} out", session.last_input_tokens, session.total_output_tokens);
                            println!();
                        }
                        println!("Total: {} sessions", sessions.len());
                        println!("\nResume: matrixcode --resume <id>");
                    }
                } else {
                    println!("Session manager not available.");
                }
            }
            Commands::NewSession => {
                // Create new session (sync)
                println!("Creating new session...");
                
                if let Some(mut mgr) = SessionManager::new().ok() {
                    let project_path = std::env::current_dir().ok();
                    if let Ok(_) = mgr.start_new(project_path.as_deref()) {
                        println!("✓ New session created");
                        
                        if let Some(id) = mgr.current_id() {
                            println!("  Session ID: {}", id);
                        }
                        
                        println!("\nStart chatting with: matrixcode");
                    } else {
                        println!("❌ Failed to create new session");
                    }
                } else {
                    println!("Session manager not available.");
                }
            }
            Commands::QuickAction { action, file } => {
                // Execute quick action
                println!("⚡ Quick Action: {}", action);
                if let Some(f) = &file {
                    println!("  Target: {}", f);
                }
                
                // Build prompt based on action type
                let prompt = match action.as_str() {
                    "explain" => {
                        if let Some(f) = file {
                            format!("Please explain the code in {} in detail, including its purpose, structure, and key concepts.", f)
                        } else {
                            "Please explain the code in detail.".to_string()
                        }
                    }
                    "fix" => {
                        if let Some(f) = file {
                            format!("Please analyze {} for bugs or issues and fix them.", f)
                        } else {
                            "Please analyze the code for bugs or issues and fix them.".to_string()
                        }
                    }
                    "refactor" => {
                        if let Some(f) = file {
                            format!("Please refactor {} to improve its structure, readability, and maintainability.", f)
                        } else {
                            "Please refactor the code to improve its structure.".to_string()
                        }
                    }
                    "test" => {
                        if let Some(f) = file {
                            format!("Please write unit tests for the code in {}.", f)
                        } else {
                            "Please write unit tests for the code.".to_string()
                        }
                    }
                    "doc" | "document" => {
                        if let Some(f) = file {
                            format!("Please add documentation and comments to {}.", f)
                        } else {
                            "Please add documentation and comments to the code.".to_string()
                        }
                    }
                    "optimize" => {
                        if let Some(f) = file {
                            format!("Please optimize {} for performance and efficiency.", f)
                        } else {
                            "Please optimize the code for performance.".to_string()
                        }
                    }
                    "review" => {
                        if let Some(f) = file {
                            format!("Please review {} and provide feedback on code quality, potential issues, and improvements.", f)
                        } else {
                            "Please review the code and provide feedback.".to_string()
                        }
                    }
                    other => {
                        if let Some(f) = file {
                            format!("{}: {}", other, f)
                        } else {
                            other.to_string()
                        }
                    }
                };
                
                println!("\n🤖 Processing...");
                
                // Build system prompt with skills for quick action
                let system_prompt = matrixcode_core::prompt::build_system_prompt(
                    &matrixcode_core::prompt::PromptProfile::Fast, // Fast profile for quick actions
                    skills,
                    None,
                    None,
                );
                
                // Create provider
                let provider = AnthropicProvider::new(api_key, model.clone(), base_url);
                
                // Build agent
                let mut agent = AgentBuilder::new(Box::new(provider))
                    .system_prompt(system_prompt)
                    .model_name(model.clone())
                    .max_tokens(4096)
                    .tools(all_tools())
                    .approve_mode(matrixcode_core::approval::ApproveMode::Auto)  // Auto mode for quick actions
                    .build();
                
                // Run agent
                match agent.run(prompt).await {
                    Ok(_) => {
                        // Get last assistant message
                        let messages = agent.get_messages();
                        if let Some(last) = messages.last() {
                            let text = match &last.content {
                                matrixcode_core::providers::MessageContent::Text(t) => t.clone(),
                                matrixcode_core::providers::MessageContent::Blocks(blocks) => {
                                    blocks.iter().filter_map(|b| match b {
                                        matrixcode_core::ContentBlock::Text { text } => Some(text.as_str()),
                                        _ => None,
                                    }).collect::<Vec<_>>().join("\n")
                                }
                            };
                            println!("\n{}", text);
                        }
                        
                        let (input, output) = agent.get_token_counts();
                        println!("\n📊 Tokens: {} in, {} out", input, output);
                        println!("✓ Action completed");
                    }
                    Err(e) => {
                        eprintln!("❌ Error: {}", e);
                    }
                }
            }
        }
    });
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