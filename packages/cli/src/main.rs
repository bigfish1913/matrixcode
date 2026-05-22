//! MatrixCode CLI - Full Implementation with REPL

mod display;

use anyhow::Result;
use clap::{Parser, Subcommand};
use display::{print_response_border, print_thinking_border};
use matrixcode_core::{
    AgentEvent, Config, SessionManager, agent::AgentBuilder,
    cancel::CancellationToken, create_provider, infer_provider_type, memory::MemoryStorage,
    providers::Provider, tools::all_tools_with_skills,
};
use matrixcode_tui::{TuiApp, restore_terminal, setup_terminal};
use std::path::{Path, PathBuf};
use std::sync::Arc;

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
            let path = project_path
                .map(|p| p.to_path_buf())
                .or_else(|| std::env::current_dir().ok())
                .unwrap_or_default();

            let overview_path = path.join(matrixcode_core::overview::OVERVIEW_FILENAME);
            let matrix_dir = path.join(matrixcode_core::overview::MATRIXCODE_DIR);
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
                if has_session {
                    "✓ exists"
                } else {
                    "❌ none"
                }
            ))
        }
        "clear" | "reset" => {
            // Clear project overview
            let path = project_path
                .map(|p| p.to_path_buf())
                .or_else(|| std::env::current_dir().ok())
                .unwrap_or_default();

            let overview_path = path.join(matrixcode_core::overview::OVERVIEW_FILENAME);
            let matrix_dir = path.join(matrixcode_core::overview::MATRIXCODE_DIR);

            let mut reset_msg = String::new();
            if overview_path.exists() {
                match std::fs::remove_file(&overview_path) {
                    Ok(_) => reset_msg.push_str(&format!(
                        "✓ Removed overview: {}\n",
                        overview_path.display()
                    )),
                    Err(e) => reset_msg.push_str(&format!("❌ Failed to remove overview: {}\n", e)),
                }
            }
            if matrix_dir.exists() {
                match std::fs::remove_dir_all(&matrix_dir) {
                    Ok(_) => reset_msg
                        .push_str(&format!("✓ Removed config dir: {}\n", matrix_dir.display())),
                    Err(e) => {
                        reset_msg.push_str(&format!("❌ Failed to remove config dir: {}\n", e))
                    }
                }
            }

            if reset_msg.is_empty() {
                InitCommandResult::Message("⚠️ No project configuration found to reset.".into())
            } else {
                reset_msg.push_str("\nRun '/init' to regenerate project overview");
                InitCommandResult::Message(reset_msg)
            }
        }
        _ => InitCommandResult::Message(
            "Unknown init command. Use: /init, /init status, /init reset".into(),
        ),
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

    /// Resume session (interactive selection)
    #[arg(short = 'r', long)]
    resume: bool,

    /// Resume specific session by ID (non-interactive)
    #[arg(long)]
    resume_id: Option<String>,

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

/// Get default model name for anthropic provider.
fn default_model() -> String {
    "claude-sonnet-4-20250514".to_string()
}

/// Get default base URL for anthropic provider.
fn default_base_url() -> String {
    "https://api.anthropic.com".to_string()
}

/// Resolve model from config, env, or default.
fn resolve_model(config: &Config) -> String {
    config
        .model
        .clone()
        .or_else(|| std::env::var("ANTHROPIC_MODEL").ok())
        .unwrap_or_else(default_model)
}

/// Resolve base URL from config, env, or default.
fn resolve_base_url(config: &Config) -> String {
    config
        .base_url
        .clone()
        .or_else(|| std::env::var("ANTHROPIC_BASE_URL").ok())
        .unwrap_or_else(default_base_url)
}

/// Resolve model with optional override, then config, env, or default.
fn resolve_model_with_override(override_model: Option<String>, config: &Config) -> String {
    override_model
        .or(config.model.clone())
        .or_else(|| std::env::var("ANTHROPIC_MODEL").ok())
        .unwrap_or_else(default_model)
}

/// Get model name with source annotation for status display.
fn model_with_source(config: &Config) -> String {
    if let Some(model) = &config.model {
        format!("{} (config)", model)
    } else if let Ok(model) = std::env::var("ANTHROPIC_MODEL") {
        format!("{} (env)", model)
    } else {
        format!("{} (default)", default_model())
    }
}

fn main() -> Result<()> {
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

/// Interactive session resume - list sessions and let user select
fn interactive_resume() -> Result<()> {
    use std::io::{self, BufRead, Write};

    let mgr = SessionManager::new()?;
    let sessions = mgr.list_sessions();

    if sessions.is_empty() {
        println!("No sessions found.");
        println!("\nTip: Use 'matrixcode' to start a new session.");
        return Ok(());
    }

    println!("📚 Sessions:\n");
    for (i, session) in sessions.iter().enumerate() {
        let project = session
            .project_path
            .as_deref()
            .map(|p| p.split('/').next_back().unwrap_or(p))
            .unwrap_or("unknown");
        let is_current = mgr.has_current() && mgr.current_id() == Some(session.id.as_str());

        println!(
            "  {}. {} - {} ({} msgs, {} tokens) {}",
            i + 1,
            session.short_id(),
            project,
            session.message_count,
            session.total_output_tokens,
            if is_current { "[current]" } else { "" }
        );
    }

    println!(
        "\nSelect session to resume (1-{}), or 'q' to quit:",
        sessions.len()
    );
    print!("> ");
    io::stdout().flush()?;

    // Read input in a separate scope to release stdin lock before TUI starts
    let selection = {
        let stdin = io::stdin();
        let mut lines = stdin.lock().lines();
        lines.next().transpose()?.map(|l| l.trim().to_string())
    };

    if let Some(input) = selection {
        if matches!(input.as_str(), "q" | "quit" | "exit") {
            println!("Cancelled.");
            return Ok(());
        }

        // Try to parse as number
        if let Ok(num) = input.parse::<usize>()
            && num > 0
            && num <= sessions.len()
        {
            let session = &sessions[num - 1];
            println!("\n✓ Resuming session: {}", session.short_id());
            println!(
                "  Project: {}",
                session.project_path.as_deref().unwrap_or("unknown")
            );
            println!("  Messages: {}", session.message_count);
            println!("\nStarting matrixcode with resumed session...\n");

            // Run terminal mode with the selected session
            let cli = Cli {
                mode: "terminal".to_string(),
                continue_session: false,
                resume: false,
                resume_id: Some(session.id.clone()),
                list_sessions: false,
                skills_dir: None,
                think: true,
                max_tokens: 16384,
                command: None,
            };
            return run_terminal_mode(cli);
        }

        // Try to match by short_id or full id
        for session in sessions.iter() {
            if session.short_id() == input || session.id == input || session.id.starts_with(&input) {
                println!("\n✓ Resuming session: {}", session.short_id());
                println!(
                    "  Project: {}",
                    session.project_path.as_deref().unwrap_or("unknown")
                );
                println!("  Messages: {}", session.message_count);
                println!("\nStarting matrixcode with resumed session...\n");

                let cli = Cli {
                    mode: "terminal".to_string(),
                    continue_session: false,
                    resume: false,
                    resume_id: Some(session.id.clone()),
                    list_sessions: false,
                    skills_dir: None,
                    think: true,
                    max_tokens: 16384,
                    command: None,
                };
                return run_terminal_mode(cli);
            }
        }

        println!("Unknown session: {}", input);
    }

    Ok(())
}

/// Load skills from directories (MatrixCode only)
fn load_skills(extra_dirs: &[PathBuf]) -> Vec<matrixcode_core::skills::Skill> {
    use matrixcode_core::skills::discover_skills;
    use std::path::PathBuf;

    // Build list of skill directories to search (in priority order)
    // Multi-level search: global → project config → project root
    let mut roots: Vec<PathBuf> = Vec::new();

    // 1. User's global skills directory (~/.matrix/skills)
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join(".matrix").join("skills"));
    }

    // 2. Project-local skills directories (multiple locations)
    if let Ok(cwd) = std::env::current_dir() {
        // 2a. Project config directory (.matrix/skills)
        roots.push(cwd.join(".matrix").join("skills"));
        // 2b. Project root directory (skills/)
        roots.push(cwd.join("skills"));
    }

    // 3. Extra directories from CLI option (--skills-dir)
    roots.extend(extra_dirs.iter().cloned());

    // Discover and load skills

    discover_skills(&roots)
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
                println!(
                    "  {}. {} ({}){}",
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
    let api_key = config
        .api_key
        .clone()
        .or_else(|| std::env::var("ANTHROPIC_AUTH_TOKEN").ok())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No API key found. Set ANTHROPIC_AUTH_TOKEN or configure in ~/.matrix/config.json"
            )
        })?;

    let model = resolve_model(&config);
    let base_url = resolve_base_url(&config);

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
            if cli.continue_session || cli.resume_id.is_some() {
                let session = if let Some(ref query) = cli.resume_id {
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
    let agent_approve_mode = config
        .approve_mode
        .as_ref()
        .map(|m| matrixcode_core::approval::ApproveMode::parse(m))
        .unwrap_or(matrixcode_core::approval::ApproveMode::Ask);

    // Create shared approve mode atomic - accessible by both agent and TUI
    let shared_approve_mode =
        std::sync::Arc::new(std::sync::atomic::AtomicU8::new(agent_approve_mode.to_u8()));

    // Read fast_model config for keyword extraction
    let agent_fast_model = config
        .fast_model
        .clone()
        .or_else(|| std::env::var("ANTHROPIC_DEFAULT_HAIKU_MODEL").ok());

    // Clone skills for agent task
    let agent_skills = skills.clone();
    let agent_shared_approve_mode = shared_approve_mode.clone();

    // Spawn Agent task with real Agent
    let agent_task = rt.spawn(async move {
        // Create provider using factory
        let provider_type = infer_provider_type(&agent_model);
        let provider = match create_provider(
            provider_type,
            agent_api_key.clone(),
            agent_model.clone(),
            Some(agent_base_url.clone()),
        ) {
            Ok(p) => p,
            Err(e) => {
                let _ = agent_event_tx.send(matrixcode_core::AgentEvent::error(
                    format!("Failed to create provider: {}", e),
                    Some("provider_error".to_string()),
                    None,
                )).await;
                return;
            }
        };

        // Create fast provider for keyword extraction
        let fast_provider: Option<Box<dyn Provider>> = agent_fast_model.as_ref().and_then(|fast_model| {
            let fast_type = infer_provider_type(fast_model);
            create_provider(
                fast_type,
                agent_api_key.clone(),
                fast_model.clone(),
                Some(agent_base_url.clone()),
            ).ok()
        });

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

        // Initial memory summary (static, will be updated dynamically before each turn)
        let initial_memory_summary = memory.as_ref()
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
            if initial_memory_summary.is_empty() { None } else { Some(&initial_memory_summary) },
        );

        // Build agent with external event sender
        let mut agent = AgentBuilder::new(provider)
            .system_prompt(system_prompt)
            .model_name(agent_model.clone())
            .max_tokens(agent_max_tokens)
            .think(agent_think)
            .tools(all_tools_with_skills(Arc::new(agent_skills.clone())))
            .event_tx(agent_event_tx.clone())
            .approve_mode(agent_approve_mode)
            .build();

        // Use the shared approve mode so TUI can update it in real-time
        agent.set_approve_mode_shared(agent_shared_approve_mode);

        // Restore messages from pre-loaded session
        if !agent_restored_messages.is_empty() {
            agent.set_messages(agent_restored_messages);
        }

        // Re-open session manager inside the task for saving
        let mut session_mgr = session_mgr_state;

        // Set cancel token
        agent.set_cancel_token(agent_cancel.clone());
        agent.set_ask_channel(ask_rx);

        // Turn counter for periodic cleanup
        let mut turn_count: usize = 0;

        // Auto-analyze project structure on first run if no memories exist
        if let Some(ref project_path) = agent_project_path
            && let Some(ref mut ms) = memory_storage {
                let memory_file = project_path.join(".matrix/memory.json");
                if !memory_file.exists() {
                    // First time in this project - analyze structure
                    let count = matrixcode_core::memory::generate_project_structure_memories(
                        project_path.as_path(),
                        ms
                    );
                    if count > 0 {
                        let _ = agent_event_tx.send(matrixcode_core::AgentEvent::progress(
                            format!("🧠 自动分析项目结构，创建 {} 条记忆", count),
                            None,
                        )).await;
                    }
                }
            }

        while let Some(msg) = task_rx.recv().await {
            // Make msg mutable for skill activation transformation
            let mut msg = msg;

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
                            let overview_provider = match create_provider(
                                infer_provider_type(&agent_model),
                                agent_api_key.clone(),
                                agent_model.clone(),
                                Some(agent_base_url.clone()),
                            ) {
                                Ok(p) => p,
                                Err(e) => {
                                    let _ = agent_event_tx.send(matrixcode_core::AgentEvent::error(
                                        format!("Failed to create provider for overview: {}", e),
                                        Some("provider_error".to_string()),
                                        None,
                                    )).await;
                                    continue;
                                }
                            };

                            match matrixcode_core::overview::ProjectOverview::generate_with_ai(path.as_path(), overview_provider.as_ref()).await {
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
                        "📚 No skills loaded.\n\nSkills directories searched (in order):\n  1. ~/.matrix/skills (MatrixCode global)\n  2. .matrix/skills (Project local)\n  3. --skills-dir (CLI option)\n\nTo add a skill, create a .md file with frontmatter:\n---\nname: my-skill\ndescription: My skill description\n---\nSkill content here...".to_string()
                    } else {
                        let mut info = format!("📚 Loaded skills ({}):\n\n", agent_skills.len());
                        for skill in &agent_skills {
                            // Show skill name, description, and source
                            info.push_str(&format!("• {}: {}\n", skill.name, skill.description));
                        }
                        info.push_str("\nUsage: `/skills <name>` to view skill content.");
                        info.push_str("\n       `/skills reload` to re-scan directories.");
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
                        let files = matrixcode_core::skills::list_skill_files(&skill.dir);
                        let files_info = if files.len() > 1 {
                            format!("\n\n📁 Associated files:\n{}", files.iter().map(|f| format!("  - {}", f)).collect::<Vec<_>>().join("\n"))
                        } else {
                            String::new()
                        };

                        format!("📚 Skill: {}\n\n{}\n{}\n\nSource: {}",
                            skill.name,
                            skill.body,
                            files_info,
                            skill.source_file.display()
                        )
                    } else {
                        // Try to suggest similar skill names
                        let similar: Vec<_> = agent_skills.iter()
                            .filter(|s| s.name.contains(skill_name) || skill_name.contains(&s.name))
                            .map(|s| s.name.as_str())
                            .collect();

                        if similar.is_empty() {
                            format!("❌ Skill '{}' not found.\n\nUse `/skills` to see available skills.", skill_name)
                        } else {
                            format!("❌ Skill '{}' not found.\n\nSimilar skills: {}", skill_name, similar.join(", "))
                        }
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

            // Handle /skill_name form (direct skill invocation)
            if msg.starts_with("/") && !msg.starts_with("/skills")
               && !msg.starts_with("/compact") && !msg.starts_with("/compress")
               && !msg.starts_with("/help") && !msg.starts_with("/init")
               && !msg.starts_with("/memory") && !msg.starts_with("/overview")
               && !msg.starts_with("/save") && !msg.starts_with("/sessions")
               && !msg.starts_with("/resume") && !msg.starts_with("/loop")
               && !msg.starts_with("/exit") && !msg.starts_with("/quit")
               && !msg.starts_with("/clear") && !msg.starts_with("/debug")
               && !msg.starts_with("/status") && !msg.starts_with("/new")
               && !msg.starts_with("/load") && !msg.starts_with("/mode")
               && !msg.starts_with("/model") && !msg.starts_with("/retry")
               && !msg.starts_with("/history") && !msg.starts_with("/cron")
               && msg != "/"
            {
                // Try to match skill name
                let skill_name = msg.trim_start_matches('/');

                // Debug: log skill lookup
                matrixcode_core::debug::debug_log().log("skill",
                    &format!("Looking for skill '{}' in {} available skills", skill_name, agent_skills.len()));
                for sk in &agent_skills {
                    matrixcode_core::debug::debug_log().log("skill", &format!("  - available: {}", sk.name));
                }

                if let Some(skill) = agent_skills.iter().find(|s| s.name == skill_name) {
                    // Build skill activation message
                    let files = matrixcode_core::skills::list_skill_files(&skill.dir);
                    let files_info = if files.len() > 1 {
                        format!("\n\n📁 Associated files (use `read` tool to explore):\n{}",
                            files.iter().map(|f| format!("  - {}", f)).collect::<Vec<_>>().join("\n"))
                    } else {
                        String::new()
                    };

                    // Create user message that activates the skill
                    let skill_activation = format!(
                        "使用 skill '{}' 来处理当前任务。\n\n---\n{}\n---\n{}\n\n请按照上述 skill 指导开始执行。",
                        skill.name,
                        skill.body,
                        files_info
                    );

                    // Send to agent for execution (not just display)
                    msg = skill_activation;

                    // Log skill activation
                    matrixcode_core::debug::debug_log().log("skill", &format!("Activated skill: {}", skill.name));

                    let _ = agent_event_tx.send(matrixcode_core::AgentEvent::with_data(
                        matrixcode_core::EventType::Progress,
                        matrixcode_core::EventData::Progress {
                            message: format!("🎯 Activating skill: {}", skill.name),
                            percentage: None,
                        },
                    )).await;

                    // Continue to normal agent processing with modified message
                } else {
                    // Debug: skill not found
                    matrixcode_core::debug::debug_log().log("skill", &format!("Skill '{}' not found", skill_name));
                }
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
                let new_mode = match mode {
                    "ask" => matrixcode_core::approval::ApproveMode::Ask,
                    "auto" => matrixcode_core::approval::ApproveMode::Auto,
                    "strict" => matrixcode_core::approval::ApproveMode::Strict,
                    _ => continue,
                };
                agent.set_approve_mode(new_mode);
                continue;
            }

            // Handle /new command - create new session
            if msg == "/new" {
                if let Some(ref mut mgr) = session_mgr {
                    let project_path = std::env::current_dir().ok();
                    mgr.start_new(project_path.as_deref()).ok();
                    agent.clear_history();
                    let _ = agent_event_tx.send(matrixcode_core::AgentEvent::session_ended()).await;
                    let _ = agent_event_tx.send(matrixcode_core::AgentEvent::progress(
                        "✓ New session created",
                        None,
                    )).await;
                }
                continue;
            }

            // Handle /memory command
            if msg == "/memory" || msg.starts_with("/memory ") {
                let parts: Vec<&str> = msg.split_whitespace().collect();
                let subcmd = parts.get(1).copied().unwrap_or("");

                if let Some(ref mut ms) = memory_storage {
                    let response = match subcmd {
                        "" | "list" => {
                            // List all memories with better formatting
                            if let Ok(mem) = ms.load_combined() {
                                if mem.entries.is_empty() {
                                    "📝 No memories stored yet.\n\nMemories are auto-detected from AI responses.\nUse '/memory analyze' to scan project structure.".to_string()
                                } else {
                                    let stats = mem.generate_statistics();
                                    let mut info = stats.format_summary();
                                    info.push_str("\n\n📋 Recent entries:\n");
                                    for (i, entry) in mem.entries.iter().enumerate().take(10) {
                                        let content_preview: String = entry.content.chars().take(80).collect();
                                        let content_preview = content_preview.trim_end_matches('\n');
                                        let importance_marker = if entry.importance >= 80.0 { "⭐" } else { "" };
                                        let manual_marker = if entry.is_manual { "📝" } else { "" };
                                        info.push_str(&format!("{}. {}{}{} {} {}\n",
                                            i + 1,
                                            entry.category.icon(),
                                            importance_marker,
                                            manual_marker,
                                            content_preview,
                                            entry.category.display_name()));
                                    }
                                    if mem.entries.len() > 10 {
                                        info.push_str(&format!("\n... and {} more entries", mem.entries.len() - 10));
                                    }
                                    info.push_str("\n\nCommands: stats, search <query>, add <content>, forget <id>, analyze, merge");
                                    info
                                }
                            } else {
                                "❌ Failed to load memories".to_string()
                            }
                        }
                        "stats" => {
                            // Show detailed memory stats
                            if let Ok(mem) = ms.load_combined() {
                                let stats = mem.generate_statistics();
                                stats.format_summary()
                            } else {
                                "❌ Failed to get memory stats".to_string()
                            }
                        }
                        "search" => {
                            // Search memories by query
                            let query = parts.get(2..).map(|p| p.join(" ")).unwrap_or_default();
                            if query.is_empty() {
                                "Usage: /memory search <query>".to_string()
                            } else if let Ok(mem) = ms.load_combined() {
                                let results = mem.search_with_limit(&query, Some(10));
                                if results.is_empty() {
                                    format!("No memories found for '{}'", query)
                                } else {
                                    let mut info = format!("🔍 Search results for '{}':\n\n", query);
                                    for (i, entry) in results.iter().enumerate() {
                                        info.push_str(&format!("{}. {} {} (重要性: {:.0})\n   {}\n",
                                            i + 1,
                                            entry.category.icon(),
                                            entry.category.display_name(),
                                            entry.importance,
                                            entry.content.chars().take(100).collect::<String>().trim_end_matches('\n')));
                                    }
                                    info
                                }
                            } else {
                                "❌ Failed to search memories".to_string()
                            }
                        }
                        "add" => {
                            // Add manual memory
                            let content = parts.get(2..).map(|p| p.join(" ")).unwrap_or_default();
                            if content.is_empty() {
                                "Usage: /memory add <content>".to_string()
                            } else if let Ok(mut mem) = ms.load_global() {
                                // Infer category from content
                                let category = matrixcode_core::memory::infer_category_from_content(&content);
                                let entry = matrixcode_core::memory::MemoryEntry::manual(category, content.clone());
                                mem.add(entry);
                                if ms.save_global(&mem).is_ok() {
                                    format!("✓ Added memory: {} {}\n  {}", category.icon(), category.display_name(), content)
                                } else {
                                    "❌ Failed to save memory".to_string()
                                }
                            } else {
                                "❌ Failed to add memory".to_string()
                            }
                        }
                        "forget" | "delete" | "remove" => {
                            // Delete memory by index or ID
                            let target = parts.get(2).copied().unwrap_or("");
                            if target.is_empty() {
                                "Usage: /memory forget <index|id>".to_string()
                            } else if let Ok(mut mem) = ms.load_combined() {
                                // Try to parse as index first
                                let removed = if let Ok(idx) = target.parse::<usize>() {
                                    if idx > 0 && idx <= mem.entries.len() {
                                        let entry = mem.entries.remove(idx - 1);
                                        Some(entry.content)
                                    } else {
                                        None
                                    }
                                } else {
                                    // Try to remove by ID (partial match)
                                    mem.remove(target).then_some(target.to_string())
                                };

                                if let Some(content) = removed {
                                    // Save to appropriate storage
                                    if ms.save_global(&mem).is_err() {
                                        // Try project storage if global failed
                                        if let Err(e) = ms.save_project(&mem) {
                                            log::warn!("Failed to save project memory: {}", e);
                                        }
                                    }
                                    format!("✓ Removed memory: {}", content.chars().take(50).collect::<String>())
                                } else {
                                    format!("❌ Memory not found: {}", target)
                                }
                            } else {
                                "❌ Failed to delete memory".to_string()
                            }
                        }
                        "analyze" => {
                            // Analyze project structure and create memories
                            if let Some(ref project_path) = agent_project_path {
                                let count = matrixcode_core::memory::generate_project_structure_memories(
                                    project_path.as_path(),
                                    ms
                                );
                                if count > 0 {
                                    format!("✓ Generated {} structure memories from project analysis", count)
                                } else {
                                    "No new structure memories generated (may already exist)".to_string()
                                }
                            } else {
                                "❌ No project path available for analysis".to_string()
                            }
                        }
                        "merge" => {
                            // Execute smart merge
                            if let Ok(mut mem) = ms.load_combined() {
                                let count = mem.smart_merge();
                                if count > 0 {
                                    if let Err(e) = ms.save_global(&mem) {
                                        log::warn!("Failed to save merged memories: {}", e);
                                    }
                                    format!("✓ Merged {} similar memories", count)
                                } else {
                                    "No similar memories found to merge".to_string()
                                }
                            } else {
                                "❌ Failed to merge memories".to_string()
                            }
                        }
                        "clear" => {
                            // Clear all memories (with confirmation)
                            if let Ok(mut mem) = ms.load_global() {
                                let count = mem.entries.len();
                                mem.entries.clear();
                                if let Err(e) = ms.save_global(&mem) {
                                    log::warn!("Failed to clear memories: {}", e);
                                }
                                format!("✓ Cleared {} memories", count)
                            } else {
                                "❌ Failed to clear memories".to_string()
                            }
                        }
                        "help" => {
                            "📝 Memory commands:\n\
                            list     - Show all memories\n\
                            stats    - Show detailed statistics\n\
                            search   - Search memories by query\n\
                            add      - Add manual memory\n\
                            forget   - Delete memory by index\n\
                            analyze  - Scan project structure\n\
                            merge    - Merge similar memories\n\
                            clear    - Clear all memories".to_string()
                        }
                        _ => {
                            "Unknown memory command. Use '/memory help' for available commands.".to_string()
                        }
                    };

                    let _ = agent_event_tx.send(matrixcode_core::AgentEvent::progress(
                        response,
                        None,
                    )).await;
                } else {
                    let _ = agent_event_tx.send(matrixcode_core::AgentEvent::progress(
                        "❌ Memory storage not available",
                        None,
                    )).await;
                }
                continue;
            }

            // Handle /overview command
            if msg == "/overview" || msg.starts_with("/overview ") {
                let parts: Vec<&str> = msg.split_whitespace().collect();
                let subcmd = parts.get(1).copied().unwrap_or("");

                let cwd = std::env::current_dir().unwrap_or_default();
                let overview_path = cwd.join(matrixcode_core::overview::OVERVIEW_FILENAME);

                let response = match subcmd {
                    "" | "show" => {
                        // Show current overview
                        if overview_path.exists() {
                            let content = std::fs::read_to_string(&overview_path).unwrap_or_default();
                            let lines = content.lines().count();
                            format!("📄 Project Overview ({} lines):\n\n{}", lines,
                                content.chars().take(2000).collect::<String>())
                        } else {
                            "❌ No overview found. Run '/init' to generate one.".to_string()
                        }
                    }
                    "regenerate" | "gen" => {
                        // Trigger overview regeneration (handled by /init)
                        "Use '/init' to regenerate project overview".to_string()
                    }
                    "path" => {
                        format!("Overview path: {}", overview_path.display())
                    }
                    _ => {
                        "Unknown overview command. Use: show, regenerate, path".to_string()
                    }
                };

                let _ = agent_event_tx.send(matrixcode_core::AgentEvent::progress(
                    response,
                    None,
                )).await;
                continue;
            }

            // Handle /save command
            if msg == "/save" || msg.starts_with("/save ") {
                let parts: Vec<&str> = msg.split_whitespace().collect();
                let name = parts.get(1).copied();

                if let Some(ref mut mgr) = session_mgr {
                    let messages = agent.get_messages();
                    mgr.set_messages(messages.to_vec());

                    // Save with optional name
                    if let Some(n) = name {
                        // Rename then save
                        if let Err(e) = mgr.rename_current(n) {
                            let _ = agent_event_tx.send(matrixcode_core::AgentEvent::error(
                                format!("Failed to rename session: {}", e),
                                None,
                                None,
                            )).await;
                        }
                    }
                    if let Err(e) = mgr.save_current() {
                        let _ = agent_event_tx.send(matrixcode_core::AgentEvent::error(
                            format!("Failed to save session: {}", e),
                            None,
                            None,
                        )).await;
                    } else {
                        let _ = agent_event_tx.send(matrixcode_core::AgentEvent::progress(
                            if let Some(ref name) = name {
                                format!("✓ Session saved as '{}'", name)
                            } else {
                                "✓ Session saved".to_string()
                            },
                            None,
                        )).await;
                    }
                } else {
                    let _ = agent_event_tx.send(matrixcode_core::AgentEvent::progress(
                        "❌ Session manager not available",
                        None,
                    )).await;
                }
                continue;
            }

            // Handle /sessions command
            if msg == "/sessions" || msg == "/resume" {
                if let Some(ref mgr) = session_mgr {
                    let sessions = mgr.list_sessions();
                    if sessions.is_empty() {
                        let _ = agent_event_tx.send(matrixcode_core::AgentEvent::progress(
                            "No saved sessions found",
                            None,
                        )).await;
                    } else {
                        let mut info = format!("📚 Sessions ({}):\n\n", sessions.len());
                        for session in sessions.iter().take(10) {
                            let project = session.project_path.as_deref()
                                .map(|p| p.split('/').next_back().unwrap_or(p))
                                .unwrap_or("unknown");
                            info.push_str(&format!("• {} - {} ({} msgs, {} out)\n",
                                session.short_id(),
                                project,
                                session.message_count,
                                session.total_output_tokens));
                        }
                        if sessions.len() > 10 {
                            info.push_str(&format!("\n... and {} more sessions", sessions.len() - 10));
                        }
                        info.push_str("\n\nUse '/load <id>' to resume a session");
                        let _ = agent_event_tx.send(matrixcode_core::AgentEvent::progress(
                            info,
                            None,
                        )).await;
                    }
                } else {
                    let _ = agent_event_tx.send(matrixcode_core::AgentEvent::progress(
                        "❌ Session manager not available",
                        None,
                    )).await;
                }
                continue;
            }

            // Handle /load command
            if msg.starts_with("/load ") {
                let session_id = msg.strip_prefix("/load ").unwrap_or("");

                if let Some(ref mut mgr) = session_mgr {
                    // Use resume to load session
                    let project_path = std::env::current_dir().ok();
                    if mgr.resume(session_id, project_path.as_deref()).is_ok() {
                        if let Some(msgs) = mgr.messages() {
                            let messages = msgs.to_vec();
                            agent.set_messages(messages.clone());

                            let _ = agent_event_tx.send(matrixcode_core::AgentEvent::progress(
                                format!("✓ Session '{}' loaded ({} messages)", session_id, messages.len()),
                                None,
                            )).await;
                        }
                    } else {
                        let _ = agent_event_tx.send(matrixcode_core::AgentEvent::progress(
                            format!("❌ Session '{}' not found", session_id),
                            None,
                        )).await;
                    }
                } else {
                    let _ = agent_event_tx.send(matrixcode_core::AgentEvent::progress(
                        "❌ Session manager not available",
                        None,
                    )).await;
                }
                continue;
            }

            // Dynamic memory retrieval: update memory summary based on current context
            // This uses AI keyword extraction with fast_provider if available
            if let Some(ref mem) = memory {
                let context_keywords = if let Some(ref fp) = fast_provider {
                    // Use AI-enhanced keyword extraction
                    matrixcode_core::memory::extract_keywords_hybrid(&msg, Some(fp.as_ref())).await
                } else {
                    // Fallback to rule-based extraction
                    matrixcode_core::memory::extract_context_keywords(&msg)
                };

                // Generate context-aware summary using pre-extracted keywords (avoid double extraction)
                let contextual_summary = mem.generate_contextual_summary_with_keywords(&context_keywords, 15);

                // Update agent's memory summary (will rebuild system prompt internally)
                if !contextual_summary.is_empty() {
                    agent.update_memory_summary(Some(contextual_summary));

                    // Debug log: keywords extracted
                    matrixcode_core::debug::debug_log().keywords_extracted(&context_keywords, &msg);

                    // Send keywords event for TUI display (only in debug mode)
                    if !context_keywords.is_empty() {
                        let _ = agent_event_tx.send(matrixcode_core::AgentEvent::with_data(
                            matrixcode_core::EventType::KeywordsExtracted,
                            matrixcode_core::EventData::Keywords {
                                keywords: context_keywords,
                                source: msg.chars().take(50).collect(),
                            },
                        )).await;
                    }
                }
            }

            // Run agent - events are sent directly via event_tx during run()
            // Track turn count for periodic cleanup
            turn_count += 1;

            match agent.run(msg.clone()).await {
                Ok(_) => {
                    // Auto-save session after each turn
                    if let Some(ref mut mgr) = session_mgr {
                        let (input_tokens, output_tokens) = agent.get_token_counts();
                        let messages = agent.get_messages();
                        mgr.set_messages(messages.to_vec());
                        mgr.update_stats(input_tokens as u32, output_tokens);
                        if let Err(e) = mgr.save_current() {
                            let _ = agent_event_tx.send(matrixcode_core::AgentEvent::error(
                                format!("Session save failed: {}", e),
                                None,
                                None,
                            )).await;
                        }

                        // Debug log: session save
                        matrixcode_core::debug::debug_log().session_save(messages.len(), output_tokens);
                    }

                    // Auto-detect and save memories (enhanced)
                    if let Some(ref mut ms) = memory_storage {
                        let messages = agent.get_messages();

                        // 1. Check for user feedback/correction in the user message
                        let feedback_results = matrixcode_core::memory::detect_feedback_patterns(&msg);
                        if !feedback_results.is_empty()
                            && let Ok(mut mem) = ms.load_combined() {
                                let feedback_count = feedback_results.len();
                                for feedback in feedback_results {
                                    matrixcode_core::memory::apply_feedback_to_memory(&mut mem, &feedback);
                                }
                                // Save to appropriate storage
                                if mem.entries.iter().any(|e| e.tags.contains(&"project".to_string())) {
                                    if let Err(e) = ms.save_project(&mem) {
                                        log::warn!("Failed to save project memory: {}", e);
                                    }
                                } else {
                                    if let Err(e) = ms.save_global(&mem) {
                                        log::warn!("Failed to save global memory: {}", e);
                                    }
                                }

                                // Send feedback event
                                let _ = agent_event_tx.send(matrixcode_core::AgentEvent::progress(
                                    format!("🧠 Learned from feedback: {} corrections", feedback_count),
                                    None,
                                )).await;
                            }

                        // 2. Detect from last assistant message using AI (fast model)
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

                            // Use AI extraction with fast provider (smart detection)
                            // Falls back to rule-based if AI fails or unavailable
                            let detected = if let Some(ref fp) = fast_provider {
                                // AI extraction with fast model
                                let model_name = agent_fast_model.clone().unwrap_or_default();
                                let extractor = matrixcode_core::memory::AiMemoryExtractor::new(
                                    fp.clone_box(),
                                    model_name,
                                );
                                matrixcode_core::memory::detect_memories_smart(
                                    &text, None, Some(&extractor)
                                ).await
                            } else {
                                // Fallback to rule-based detection
                                matrixcode_core::memory::detect_memories_from_text(&text, None)
                            };

                            if !detected.is_empty() {
                                let detected_count = detected.len();

                                // Save each entry to appropriate storage
                                for entry in detected {
                                    // Determine if project-specific based on tags or context
                                    let is_project = entry.tags.contains(&"project".to_string())
                                        || agent_project_path.is_some();
                                    if let Err(e) = ms.add_entry(entry, is_project) {
                                        log::warn!("Failed to add memory entry: {}", e);
                                    }
                                }

                                // Debug log: memory save
                                matrixcode_core::debug_memory!(detected_count, text.len());

                                // Send event to TUI
                                let _ = agent_event_tx.send(matrixcode_core::AgentEvent::with_data(
                                    matrixcode_core::EventType::MemoryDetected,
                                    matrixcode_core::EventData::Memory {
                                        summary: format!("检测到 {} 条记忆", detected_count),
                                        entries_count: detected_count,
                                    },
                                )).await;
                            }

                            // 3. Infer preferences from behavior (every 5 turns)
                            if turn_count.is_multiple_of(5) && messages.len() >= 3
                                && let Ok(mut mem) = ms.load_combined() {
                                    let config = matrixcode_core::memory::BehaviorInferenceConfig::default();
                                    let inferred = matrixcode_core::memory::apply_behavior_inferences_to_memory(
                                        messages, &mut mem, Some(&config)
                                    );
                                    if inferred > 0 {
                                        if let Err(e) = ms.save_global(&mem) {
                                            log::warn!("Failed to save inferred preferences: {}", e);
                                        }
                                        let _ = agent_event_tx.send(matrixcode_core::AgentEvent::progress(
                                            format!("🧠 推断出 {} 个使用偏好", inferred),
                                            None,
                                        )).await;
                                    }
                                }
                        }

                        // 4. Periodic cleanup (every 10 turns)
                        if turn_count.is_multiple_of(10)
                            && let Ok(mut mem) = ms.load_combined() {
                                // Apply time decay
                                mem.apply_time_decay();
                                // Smart merge
                                let merged = mem.smart_merge();
                                // Prune low importance
                                mem.prune();
                                // Save
                                if let Err(e) = ms.save_global(&mem) {
                                    log::warn!("Failed to save memory after maintenance: {}", e);
                                }

                                if merged > 0 {
                                    let _ = agent_event_tx.send(matrixcode_core::AgentEvent::progress(
                                        format!("🧠 合并了 {} 条相似记忆", merged),
                                        None,
                                    )).await;
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

    // Check if debug mode should be enabled from environment
    let debug_mode = std::env::var("MATRIXCODE_DEBUG")
        .map(|v| v == "1" || v == "true" || v == "verbose")
        .unwrap_or(false);

    // Setup terminal for TUI
    let mut terminal = setup_terminal()?;

    // Create App and run it (TUI runs in sync context, but tokio channels are usable)
    let mut app = TuiApp::new(task_tx, event_rx, cancel_token.clone())
        .with_ask_channel(ask_tx)
        .with_shared_approve_mode(shared_approve_mode)
        .with_config(&model, cli.think, cli.max_tokens, None)
        .with_debug_mode(debug_mode);

    // Load restored messages if any
    if !restored_messages.is_empty() {
        app.load_messages(restored_messages);
    }
    let result = app.run(&mut terminal);

    // Restore terminal first (so user sees prompt immediately)
    restore_terminal()?;

    // Cleanup: cancel agent task and wait for completion
    cancel_token.cancel();
    // Give agent task a short grace period to finish
    let cleanup_result = rt.block_on(async {
        tokio::time::timeout(tokio::time::Duration::from_millis(500), async {
            // Just wait - the task will see cancel_token.cancelled() and exit
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }).await
    });

    if cleanup_result.is_err() {
        // Timeout - abort the task
        agent_task.abort();
    } else {
        // Task should have finished gracefully, drop the handle
        std::mem::drop(agent_task);
    }

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

    let model = resolve_model(&config);
    let base_url = resolve_base_url(&config);

    let approve_mode = config
        .approve_mode
        .as_ref()
        .map(|m| matrixcode_core::approval::ApproveMode::parse(m))
        .unwrap_or(matrixcode_core::approval::ApproveMode::Ask);

    // Create tokio runtime
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("Failed to create tokio runtime: {}", e);
            return;
        }
    };

    rt.block_on(async {
        match cmd {
            Commands::Chat { message } => {
                // Interactive or single-shot chat
                if let Some(msg) = message {
                    // Single-shot chat

                    // Build system prompt with skills
                    let system_prompt = matrixcode_core::prompt::build_system_prompt(
                        &matrixcode_core::prompt::PromptProfile::Default,
                        skills,
                        None,
                        None,
                    );

                    // Create provider using factory
                    let provider = match create_provider(
                        infer_provider_type(&model),
                        api_key,
                        model.clone(),
                        Some(base_url),
                    ) {
                        Ok(p) => p,
                        Err(e) => {
                            eprintln!("Failed to create provider: {}", e);
                            return;
                        }
                    };

                    // Build agent
                    let mut agent = AgentBuilder::new(provider)
                        .system_prompt(system_prompt)
                        .model_name(model.clone())
                        .max_tokens(4096)
                        .tools(all_tools_with_skills(Arc::new(skills.to_vec())))
                        .approve_mode(approve_mode)
                        .build();

                    // Run agent
                    match agent.run(msg).await {
                        Ok(_) => {
                            // Get all messages to show thinking first, then result
                            let messages = agent.get_messages();

                            // First, show thinking content if any
                            for msg in messages.iter() {
                                if msg.role == matrixcode_core::providers::Role::Assistant {
                                    // Check if this is thinking content
                                    let is_thinking = match &msg.content {
                                        matrixcode_core::providers::MessageContent::Text(t) => {
                                            t.contains("<thinking>") || t.starts_with("Let me") || t.starts_with("I need to")
                                        },
                                        matrixcode_core::providers::MessageContent::Blocks(blocks) => {
                                            blocks.iter().any(|b| match b {
                                                matrixcode_core::ContentBlock::Thinking { thinking, .. } => !thinking.is_empty(),
                                                _ => false,
                                            })
                                        },
                                    };

                                    if is_thinking {
                                        let text = match &msg.content {
                                            matrixcode_core::providers::MessageContent::Text(t) => t.clone(),
                                            matrixcode_core::providers::MessageContent::Blocks(blocks) => {
                                                blocks.iter().filter_map(|b| match b {
                                                    matrixcode_core::ContentBlock::Thinking { thinking, .. } => Some(thinking.as_str()),
                                                    matrixcode_core::ContentBlock::Text { text } => Some(text.as_str()),
                                                    _ => None,
                                                }).collect::<Vec<_>>().join("\n")
                                            },
                                        };
                                        print_thinking_border(&text);
                                    }
                                }
                            }

                            // Then show the final assistant message
                            if let Some(last) = messages.last()
                                && last.role == matrixcode_core::providers::Role::Assistant {
                                    let text = match &last.content {
                                        matrixcode_core::providers::MessageContent::Text(t) => t.clone(),
                                        matrixcode_core::providers::MessageContent::Blocks(blocks) => {
                                            blocks.iter().filter_map(|b| match b {
                                                matrixcode_core::ContentBlock::Text { text } => Some(text.as_str()),
                                                _ => None,
                                            }).collect::<Vec<_>>().join("\n")
                                        },
                                    };
                                    print_response_border("Response", &text);
                                }

                            let (input, output) = agent.get_token_counts();
                            println!();
                            println!("📊 Tokens: {} in, {} out", input, output);
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
                if config.is_api_configured() {
                    println!("  API: ✓ configured");
                } else {
                    println!("  API: ❌ not configured");
                    println!("       Set ANTHROPIC_AUTH_TOKEN or configure in ~/.matrix/config.json");
                }

                println!("  Model: {}", model_with_source(&config));

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
                if let Ok(mgr) = SessionManager::new() {
                    println!("  Sessions: {} (current: {})",
                        mgr.list_sessions().len(),
                        if mgr.has_current() { "yes" } else { "no" }
                    );
                }

                // Show memory
                let project_path = std::env::current_dir().ok();
                if let Some(path) = &project_path {
                    if let Ok(storage) = MemoryStorage::new(Some(path.as_path()))
                        && let Ok(mem) = storage.load_combined() {
                            println!("  Memory: {} entries", mem.entries.len());
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
                if let Ok(mgr) = SessionManager::new() {
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

                if let Ok(mut mgr) = SessionManager::new() {
                    let project_path = std::env::current_dir().ok();
                    if mgr.start_new(project_path.as_deref()).is_ok() {
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

                // Build system prompt with skills for quick action
                let system_prompt = matrixcode_core::prompt::build_system_prompt(
                    &matrixcode_core::prompt::PromptProfile::Fast, // Fast profile for quick actions
                    skills,
                    None,
                    None,
                );

                // Create provider using factory
                let provider = match create_provider(
                    infer_provider_type(&model),
                    api_key,
                    model.clone(),
                    Some(base_url),
                ) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("Failed to create provider: {}", e);
                        return;
                    }
                };

                // Build agent
                let mut agent = AgentBuilder::new(provider)
                    .system_prompt(system_prompt)
                    .model_name(model.clone())
                    .max_tokens(4096)
                    .tools(all_tools_with_skills(Arc::new(skills.to_vec())))
                    .approve_mode(matrixcode_core::approval::ApproveMode::Auto)  // Auto mode for quick actions
                    .build();

                // Run agent
                match agent.run(prompt).await {
                    Ok(_) => {
                        // Get all messages to show thinking first, then result
                        let messages = agent.get_messages();

                        // First, show thinking content if any
                        for msg in messages.iter() {
                            if msg.role == matrixcode_core::providers::Role::Assistant {
                                // Check if this is thinking content
                                let is_thinking = match &msg.content {
                                    matrixcode_core::providers::MessageContent::Text(t) => {
                                        t.contains("<thinking>") || t.starts_with("Let me") || t.starts_with("I need to")
                                    },
                                    matrixcode_core::providers::MessageContent::Blocks(blocks) => {
                                        blocks.iter().any(|b| match b {
                                            matrixcode_core::ContentBlock::Thinking { thinking, .. } => !thinking.is_empty(),
                                            _ => false,
                                        })
                                    },
                                };

                                if is_thinking {
                                    let text = match &msg.content {
                                        matrixcode_core::providers::MessageContent::Text(t) => t.clone(),
                                        matrixcode_core::providers::MessageContent::Blocks(blocks) => {
                                            blocks.iter().filter_map(|b| match b {
                                                matrixcode_core::ContentBlock::Thinking { thinking, .. } => Some(thinking.as_str()),
                                                matrixcode_core::ContentBlock::Text { text } => Some(text.as_str()),
                                                _ => None,
                                            }).collect::<Vec<_>>().join("\n")
                                        },
                                    };
                                    print_thinking_border(&text);
                                }
                            }
                        }

                        // Then show the final assistant message
                        if let Some(last) = messages.last()
                            && last.role == matrixcode_core::providers::Role::Assistant {
                                let text = match &last.content {
                                    matrixcode_core::providers::MessageContent::Text(t) => t.clone(),
                                    matrixcode_core::providers::MessageContent::Blocks(blocks) => {
                                        blocks.iter().filter_map(|b| match b {
                                            matrixcode_core::ContentBlock::Text { text } => Some(text.as_str()),
                                            _ => None,
                                        }).collect::<Vec<_>>().join("\n")
                                    },
                                };
                                // Skip if this was the thinking message we already showed
                                print_response_border("Result", &text);
                            }

                        let (input, output) = agent.get_token_counts();
                        println!();
                        println!("📊 Tokens: {} in, {} out", input, output);
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
    // Load config for all commands
    let config = Config::load();

    match cli.command {
        Some(Commands::Chat { message }) => {
            // For chat command, we run the actual agent
            let api_key = config
                .api_key
                .clone()
                .or_else(|| std::env::var("ANTHROPIC_AUTH_TOKEN").ok())
                .ok_or_else(|| anyhow::anyhow!("No API key found"))?;

            let model = resolve_model(&config);
            let base_url = resolve_base_url(&config);

            // Load skills
            let skills_dirs: Vec<PathBuf> = cli.skills_dir.iter().cloned().collect();
            let skills = load_skills(&skills_dirs);

            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                // Output session started event
                println!("{}", AgentEvent::session_started().to_json()?);

                let system_prompt = matrixcode_core::prompt::build_system_prompt(
                    &matrixcode_core::prompt::PromptProfile::Default,
                    &skills,
                    None,
                    None,
                );

                let provider = match create_provider(
                    infer_provider_type(&model),
                    api_key,
                    model.clone(),
                    Some(base_url),
                ) {
                    Ok(p) => p,
                    Err(e) => {
                        println!("{}", AgentEvent::error(
                            format!("Failed to create provider: {}", e),
                            None,
                            None,
                        ).to_json()?);
                        return Ok::<_, anyhow::Error>(());
                    }
                };
                let mut agent = AgentBuilder::new(provider)
                    .system_prompt(system_prompt)
                    .model_name(model)
                    .max_tokens(4096)
                    .tools(all_tools_with_skills(Arc::new(skills.clone())))
                    .approve_mode(matrixcode_core::approval::ApproveMode::Auto)
                    .build();

                match agent.run(message.unwrap_or_default()).await {
                    Ok(_) => {
                        let messages = agent.get_messages();
                        if let Some(last) = messages.last() {
                            let text = match &last.content {
                                matrixcode_core::providers::MessageContent::Text(t) => t.clone(),
                                matrixcode_core::providers::MessageContent::Blocks(blocks) => {
                                    blocks
                                        .iter()
                                        .filter_map(|b| match b {
                                            matrixcode_core::ContentBlock::Text { text } => {
                                                Some(text.as_str())
                                            }
                                            _ => None,
                                        })
                                        .collect::<Vec<_>>()
                                        .join("\n")
                                }
                            };
                            println!("{}", AgentEvent::text_delta(text).to_json()?);
                        }
                    }
                    Err(e) => {
                        println!(
                            "{}",
                            AgentEvent::error(format!("Agent error: {}", e), None, None)
                                .to_json()?
                        );
                    }
                }

                println!("{}", AgentEvent::session_ended().to_json()?);
                Ok::<_, anyhow::Error>(())
            })?;
        }
        Some(Commands::History) => {
            // Output session history as JSON events
            println!("{}", AgentEvent::session_started().to_json()?);

            if let Ok(mgr) = SessionManager::new() {
                let sessions = mgr.list_sessions();
                if sessions.is_empty() {
                    let data = serde_json::json!({
                        "type": "history",
                        "sessions": [],
                        "message": "No sessions found"
                    });
                    println!(
                        "{}",
                        AgentEvent::with_data(
                            matrixcode_core::EventType::Progress,
                            matrixcode_core::EventData::Progress {
                                message: serde_json::to_string(&data)?,
                                percentage: None,
                            },
                        )
                        .to_json()?
                    );
                } else {
                    let sessions_json: Vec<serde_json::Value> = sessions.iter().map(|s| {
                        serde_json::json!({
                            "id": s.id,
                            "short_id": s.short_id(),
                            "project_path": s.project_path,
                            "created_at": s.created_at.to_rfc3339(),
                            "message_count": s.message_count,
                            "input_tokens": s.last_input_tokens,
                            "output_tokens": s.total_output_tokens,
                            "is_current": mgr.has_current() && mgr.current_id() == Some(s.id.as_str())
                        })
                    }).collect();

                    let data = serde_json::json!({
                        "type": "history",
                        "sessions": sessions_json,
                        "total": sessions.len()
                    });
                    println!(
                        "{}",
                        AgentEvent::with_data(
                            matrixcode_core::EventType::Progress,
                            matrixcode_core::EventData::Progress {
                                message: serde_json::to_string(&data)?,
                                percentage: None,
                            },
                        )
                        .to_json()?
                    );
                }
            } else {
                println!(
                    "{}",
                    AgentEvent::error("Session manager not available".to_string(), None, None)
                        .to_json()?
                );
            }

            println!("{}", AgentEvent::session_ended().to_json()?);
        }
        Some(Commands::Status) => {
            // Output system status as JSON events
            println!("{}", AgentEvent::session_started().to_json()?);

            let mut status = serde_json::json!({
                "version": env!("CARGO_PKG_VERSION"),
                "mode": "service",
                "api_configured": config.is_api_configured(),
            });

            status["model"] = serde_json::json!(model_with_source(&config));

            if let Some(base_url) = &config.base_url {
                status["base_url"] = serde_json::json!(base_url);
            }

            if let Some(approve_mode) = &config.approve_mode {
                status["approve_mode"] = serde_json::json!(approve_mode);
            }

            // Add session info
            if let Ok(mgr) = SessionManager::new() {
                status["sessions_count"] = serde_json::json!(mgr.list_sessions().len());
                status["has_current_session"] = serde_json::json!(mgr.has_current());
            }

            // Add memory info
            let project_path = std::env::current_dir().ok();
            if let Some(path) = &project_path {
                if let Ok(storage) = MemoryStorage::new(Some(path.as_path()))
                    && let Ok(mem) = storage.load_combined()
                {
                    status["memory_entries"] = serde_json::json!(mem.entries.len());
                }

                // Add overview status
                let overview_path = path.join(matrixcode_core::overview::OVERVIEW_FILENAME);
                status["has_overview"] = serde_json::json!(overview_path.exists());
            }

            println!(
                "{}",
                AgentEvent::with_data(
                    matrixcode_core::EventType::Progress,
                    matrixcode_core::EventData::Progress {
                        message: serde_json::to_string(&status)?,
                        percentage: None,
                    },
                )
                .to_json()?
            );

            println!("{}", AgentEvent::session_ended().to_json()?);
        }
        Some(Commands::NewSession) => {
            // Create new session
            println!("{}", AgentEvent::session_started().to_json()?);

            if let Ok(mut mgr) = SessionManager::new() {
                let project_path = std::env::current_dir().ok();
                match mgr.start_new(project_path.as_deref()) {
                    Ok(_) => {
                        let data = serde_json::json!({
                            "success": true,
                            "session_id": mgr.current_id(),
                            "message": "New session created"
                        });
                        println!(
                            "{}",
                            AgentEvent::with_data(
                                matrixcode_core::EventType::Progress,
                                matrixcode_core::EventData::Progress {
                                    message: serde_json::to_string(&data)?,
                                    percentage: None,
                                },
                            )
                            .to_json()?
                        );
                    }
                    Err(e) => {
                        println!(
                            "{}",
                            AgentEvent::error(
                                format!("Failed to create session: {}", e),
                                None,
                                None
                            )
                            .to_json()?
                        );
                    }
                }
            } else {
                println!(
                    "{}",
                    AgentEvent::error("Session manager not available".to_string(), None, None)
                        .to_json()?
                );
            }

            println!("{}", AgentEvent::session_ended().to_json()?);
        }
        Some(Commands::QuickAction { action, file }) => {
            // Execute quick action
            let api_key = config
                .api_key
                .clone()
                .or_else(|| std::env::var("ANTHROPIC_AUTH_TOKEN").ok())
                .ok_or_else(|| anyhow::anyhow!("No API key found"))?;

            let model = resolve_model(&config);
            let base_url = resolve_base_url(&config);

            // Load skills
            let skills_dirs: Vec<PathBuf> = cli.skills_dir.iter().cloned().collect();
            let skills = load_skills(&skills_dirs);

            // Build prompt based on action type
            let prompt = match action.as_str() {
                "explain" => {
                    if let Some(f) = &file {
                        format!(
                            "Please explain the code in {} in detail, including its purpose, structure, and key concepts.",
                            f
                        )
                    } else {
                        "Please explain the code in detail.".to_string()
                    }
                }
                "fix" => {
                    if let Some(f) = &file {
                        format!("Please analyze {} for bugs or issues and fix them.", f)
                    } else {
                        "Please analyze the code for bugs or issues and fix them.".to_string()
                    }
                }
                "refactor" => {
                    if let Some(f) = &file {
                        format!(
                            "Please refactor {} to improve its structure, readability, and maintainability.",
                            f
                        )
                    } else {
                        "Please refactor the code to improve its structure.".to_string()
                    }
                }
                "test" => {
                    if let Some(f) = &file {
                        format!("Please write unit tests for the code in {}.", f)
                    } else {
                        "Please write unit tests for the code.".to_string()
                    }
                }
                "doc" | "document" => {
                    if let Some(f) = &file {
                        format!("Please add documentation and comments to {}.", f)
                    } else {
                        "Please add documentation and comments to the code.".to_string()
                    }
                }
                "optimize" => {
                    if let Some(f) = &file {
                        format!("Please optimize {} for performance and efficiency.", f)
                    } else {
                        "Please optimize the code for performance.".to_string()
                    }
                }
                "review" => {
                    if let Some(f) = &file {
                        format!(
                            "Please review {} and provide feedback on code quality, potential issues, and improvements.",
                            f
                        )
                    } else {
                        "Please review the code and provide feedback.".to_string()
                    }
                }
                other => {
                    if let Some(f) = &file {
                        format!("{}: {}", other, f)
                    } else {
                        other.to_string()
                    }
                }
            };

            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                println!("{}", AgentEvent::session_started().to_json()?);

                // Output action start event
                let action_data = serde_json::json!({
                    "action": action,
                    "file": file,
                    "status": "started"
                });
                println!(
                    "{}",
                    AgentEvent::with_data(
                        matrixcode_core::EventType::Progress,
                        matrixcode_core::EventData::Progress {
                            message: serde_json::to_string(&action_data)?,
                            percentage: Some(0),
                        },
                    )
                    .to_json()?
                );

                let system_prompt = matrixcode_core::prompt::build_system_prompt(
                    &matrixcode_core::prompt::PromptProfile::Fast,
                    &skills,
                    None,
                    None,
                );

                let provider = match create_provider(
                    infer_provider_type(&model),
                    api_key,
                    model.clone(),
                    Some(base_url),
                ) {
                    Ok(p) => p,
                    Err(e) => {
                        println!("{}", AgentEvent::error(
                            format!("Failed to create provider: {}", e),
                            None,
                            None,
                        ).to_json()?);
                        return Ok::<_, anyhow::Error>(());
                    }
                };
                let mut agent = AgentBuilder::new(provider)
                    .system_prompt(system_prompt)
                    .model_name(model)
                    .max_tokens(4096)
                    .tools(all_tools_with_skills(Arc::new(skills.clone())))
                    .approve_mode(matrixcode_core::approval::ApproveMode::Auto)
                    .build();

                match agent.run(prompt).await {
                    Ok(_) => {
                        let messages = agent.get_messages();
                        if let Some(last) = messages.last() {
                            let text = match &last.content {
                                matrixcode_core::providers::MessageContent::Text(t) => t.clone(),
                                matrixcode_core::providers::MessageContent::Blocks(blocks) => {
                                    blocks
                                        .iter()
                                        .filter_map(|b| match b {
                                            matrixcode_core::ContentBlock::Text { text } => {
                                                Some(text.as_str())
                                            }
                                            _ => None,
                                        })
                                        .collect::<Vec<_>>()
                                        .join("\n")
                                }
                            };
                            println!("{}", AgentEvent::text_delta(text).to_json()?);
                        }

                        let (input, output) = agent.get_token_counts();
                        let result_data = serde_json::json!({
                            "action": action,
                            "file": file,
                            "status": "completed",
                            "input_tokens": input,
                            "output_tokens": output
                        });
                        println!(
                            "{}",
                            AgentEvent::with_data(
                                matrixcode_core::EventType::Progress,
                                matrixcode_core::EventData::Progress {
                                    message: serde_json::to_string(&result_data)?,
                                    percentage: Some(100),
                                },
                            )
                            .to_json()?
                        );
                    }
                    Err(e) => {
                        println!(
                            "{}",
                            AgentEvent::error(format!("Quick action failed: {}", e), None, None)
                                .to_json()?
                        );
                    }
                }

                println!("{}", AgentEvent::session_ended().to_json()?);
                Ok::<_, anyhow::Error>(())
            })?;
        }
        None => {
            println!(
                "{}",
                AgentEvent::error("Please specify a command".to_string(), None, None).to_json()?
            );
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
    /// Content for chat messages
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    /// Action type for quick_action (explain, fix, refactor, test, doc, optimize, review)
    #[serde(skip_serializing_if = "Option::is_none")]
    action: Option<String>,
    /// Target file for quick_action
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    /// Session ID for load_session
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    /// Model override
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    /// Max tokens override
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

/// Handle daemon request
fn handle_daemon_request(request: DaemonRequest) -> Result<Vec<AgentEvent>> {
    let mut events = Vec::new();
    let config = Config::load();

    // Load skills for daemon mode
    let skills = load_skills(&[]);

    events.push(AgentEvent::session_started());

    match request.request_type.as_str() {
        "chat" => {
            // Execute actual chat with agent
            if let Some(content) = request.content {
                let api_key = config
                    .api_key
                    .clone()
                    .or_else(|| std::env::var("ANTHROPIC_AUTH_TOKEN").ok())
                    .ok_or_else(|| anyhow::anyhow!("No API key found"))?;

                let model = resolve_model_with_override(request.model.clone(), &config);
                let base_url = resolve_base_url(&config);

                let max_tokens = request.max_tokens.unwrap_or(4096);

                let rt = tokio::runtime::Runtime::new()?;
                let result = rt.block_on(async {
                    let provider = match create_provider(
                        infer_provider_type(&model),
                        api_key,
                        model.clone(),
                        Some(base_url),
                    ) {
                        Ok(p) => p,
                        Err(e) => return Err(e),
                    };
                    let mut agent = AgentBuilder::new(provider)
                        .model_name(model)
                        .max_tokens(max_tokens)
                        .tools(all_tools_with_skills(Arc::new(skills.clone())))
                        .approve_mode(matrixcode_core::approval::ApproveMode::Auto)
                        .build();

                    agent.run(content).await
                });

                match result {
                    Ok(_) => {
                        // For daemon mode, we can't easily capture all events,
                        // so we just return a completion event
                        events.push(AgentEvent::text_delta("Chat completed".to_string()));
                    }
                    Err(e) => {
                        events.push(AgentEvent::error(format!("Chat failed: {}", e), None, None));
                    }
                }
            } else {
                events.push(AgentEvent::error(
                    "No content provided for chat",
                    None,
                    None,
                ));
            }
        }
        "quick_action" => {
            // Execute quick action
            if let Some(action) = request.action.clone() {
                let prompt = build_quick_action_prompt(&action, request.file.as_ref());

                let api_key = config
                    .api_key
                    .clone()
                    .or_else(|| std::env::var("ANTHROPIC_AUTH_TOKEN").ok())
                    .ok_or_else(|| anyhow::anyhow!("No API key found"))?;

                let model = resolve_model_with_override(request.model.clone(), &config);
                let base_url = resolve_base_url(&config);

                events.push(AgentEvent::tool_use_start("action_1", action.clone(), None));

                let rt = tokio::runtime::Runtime::new()?;
                let result = rt.block_on(async {
                    let provider = match create_provider(
                        infer_provider_type(&model),
                        api_key,
                        model.clone(),
                        Some(base_url),
                    ) {
                        Ok(p) => p,
                        Err(e) => return Err(e),
                    };
                    let mut agent = AgentBuilder::new(provider)
                        .model_name(model)
                        .max_tokens(4096)
                        .tools(all_tools_with_skills(Arc::new(skills.clone())))
                        .approve_mode(matrixcode_core::approval::ApproveMode::Auto)
                        .build();

                    agent.run(prompt).await
                });

                match result {
                    Ok(_) => {
                        events.push(AgentEvent::tool_result(
                            "action_1",
                            "action",
                            None,
                            "Action completed",
                            false,
                        ));
                    }
                    Err(e) => {
                        events.push(AgentEvent::tool_result(
                            "action_1",
                            "action",
                            None,
                            format!("Error: {}", e),
                            true,
                        ));
                    }
                }
            } else {
                events.push(AgentEvent::error("No action specified", None, None));
            }
        }
        "status" => {
            // Return actual system status
            let status = serde_json::json!({
                "version": env!("CARGO_PKG_VERSION"),
                "mode": "daemon",
                "api_configured": config.is_api_configured(),
                "model": model_with_source(&config),
            });
            events.push(AgentEvent::with_data(
                matrixcode_core::EventType::Progress,
                matrixcode_core::EventData::Progress {
                    message: serde_json::to_string(&status)?,
                    percentage: None,
                },
            ));
        }
        "history" => {
            // Return session history
            if let Ok(mgr) = SessionManager::new() {
                let sessions = mgr.list_sessions();
                let sessions_json: Vec<serde_json::Value> = sessions
                    .iter()
                    .map(|s| {
                        serde_json::json!({
                            "id": s.id,
                            "short_id": s.short_id(),
                            "project_path": s.project_path,
                            "created_at": s.created_at.to_rfc3339(),
                            "message_count": s.message_count,
                        })
                    })
                    .collect();

                let data = serde_json::json!({
                    "type": "history",
                    "sessions": sessions_json,
                    "total": sessions.len()
                });
                events.push(AgentEvent::with_data(
                    matrixcode_core::EventType::Progress,
                    matrixcode_core::EventData::Progress {
                        message: serde_json::to_string(&data)?,
                        percentage: None,
                    },
                ));
            } else {
                events.push(AgentEvent::error(
                    "Session manager not available",
                    None,
                    None,
                ));
            }
        }
        "new_session" => {
            // Create new session
            if let Ok(mut mgr) = SessionManager::new() {
                let project_path = std::env::current_dir().ok();
                match mgr.start_new(project_path.as_deref()) {
                    Ok(_) => {
                        let data = serde_json::json!({
                            "success": true,
                            "session_id": mgr.current_id(),
                            "message": "New session created"
                        });
                        events.push(AgentEvent::with_data(
                            matrixcode_core::EventType::Progress,
                            matrixcode_core::EventData::Progress {
                                message: serde_json::to_string(&data)?,
                                percentage: None,
                            },
                        ));
                    }
                    Err(e) => {
                        events.push(AgentEvent::error(
                            format!("Failed to create session: {}", e),
                            None,
                            None,
                        ));
                    }
                }
            } else {
                events.push(AgentEvent::error(
                    "Session manager not available",
                    None,
                    None,
                ));
            }
        }
        "load_session" => {
            // Load/resume a session
            if let Some(session_id) = request.session_id.clone() {
                if let Ok(mut mgr) = SessionManager::new() {
                    let project_path = std::env::current_dir().ok();
                    match mgr.resume(&session_id, project_path.as_deref()) {
                        Ok(Some(session)) => {
                            let data = serde_json::json!({
                                "success": true,
                                "session_id": session.metadata.id,
                                "message_count": session.messages.len(),
                                "message": "Session loaded"
                            });
                            events.push(AgentEvent::with_data(
                                matrixcode_core::EventType::Progress,
                                matrixcode_core::EventData::Progress {
                                    message: serde_json::to_string(&data)?,
                                    percentage: None,
                                },
                            ));
                        }
                        Ok(None) => {
                            events.push(AgentEvent::error(
                                format!("Session '{}' not found", session_id),
                                None,
                                None,
                            ));
                        }
                        Err(e) => {
                            events.push(AgentEvent::error(
                                format!("Failed to load session: {}", e),
                                None,
                                None,
                            ));
                        }
                    }
                } else {
                    events.push(AgentEvent::error(
                        "Session manager not available",
                        None,
                        None,
                    ));
                }
            } else {
                events.push(AgentEvent::error("No session_id provided", None, None));
            }
        }
        "list_sessions" => {
            // List all sessions (alias for history)
            if let Ok(mgr) = SessionManager::new() {
                let sessions = mgr.list_sessions();
                let sessions_json: Vec<serde_json::Value> = sessions
                    .iter()
                    .map(|s| {
                        serde_json::json!({
                            "id": s.id,
                            "short_id": s.short_id(),
                            "project": s.project_path.as_deref().unwrap_or("unknown"),
                        })
                    })
                    .collect();

                events.push(AgentEvent::with_data(
                    matrixcode_core::EventType::Progress,
                    matrixcode_core::EventData::Progress {
                        message: serde_json::to_string(
                            &serde_json::json!({ "sessions": sessions_json }),
                        )?,
                        percentage: None,
                    },
                ));
            } else {
                events.push(AgentEvent::error(
                    "Session manager not available",
                    None,
                    None,
                ));
            }
        }
        "ping" => {
            // Simple ping/pong for health check
            events.push(AgentEvent::text_delta("pong".to_string()));
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

/// Build quick action prompt from action type and file
fn build_quick_action_prompt(action: &str, file: Option<&String>) -> String {
    match action {
        "explain" => {
            if let Some(f) = file {
                format!(
                    "Please explain the code in {} in detail, including its purpose, structure, and key concepts.",
                    f
                )
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
                format!(
                    "Please refactor {} to improve its structure, readability, and maintainability.",
                    f
                )
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
                format!(
                    "Please review {} and provide feedback on code quality, potential issues, and improvements.",
                    f
                )
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
    }
}
