//! MatrixCode CLI - Full Implementation with REPL

mod display;

use anyhow::Result;
use clap::{Parser, Subcommand};
use display::{print_response_border, print_thinking_border};
use matrixcode_core::{
    AgentEvent, Config, SessionManager, agent::AgentBuilder, cancel::CancellationToken,
    create_provider_with_headers, infer_provider_type, memory::MemoryStorage, providers::Provider,
    tools::all_tools_with_skills,
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

    /// Think mode (optional, uses config default if not specified)
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    think: Option<bool>,

    /// Max tokens
    #[arg(long, default_value = "16384")]
    max_tokens: u32,

    #[command(subcommand)]
    command: Option<Commands>,
}

/// Workflow subcommands
#[derive(Subcommand)]
enum WorkflowCommands {
    /// Run a workflow from YAML file
    Run {
        /// YAML workflow file path
        #[arg(short, long)]
        file: String,

        /// Input parameters (JSON format)
        #[arg(short, long)]
        inputs: Option<String>,
    },

    /// Discover available workflows
    Discover {
        /// Match query to find relevant workflows
        #[arg(short, long)]
        query: Option<String>,
    },

    /// List workflow history
    List {
        /// Filter by status (running, paused, completed, failed)
        #[arg(short, long)]
        status: Option<String>,
    },

    /// Show workflow status
    Status {
        /// Workflow instance ID
        #[arg(short, long)]
        id: String,
    },

    /// Resume a paused workflow
    Resume {
        /// Workflow instance ID
        #[arg(short, long)]
        id: String,
    },

    /// Abort a running workflow
    Abort {
        /// Workflow instance ID
        #[arg(short, long)]
        id: String,
    },
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

    /// Workflow management commands
    Workflow {
        #[command(subcommand)]
        command: WorkflowCommands,
    },
}

/// Get default model name for anthropic provider.
fn default_model() -> String {
    "claude-sonnet-4-20250514".to_string()
}

/// Get default base URL for anthropic provider.
fn default_base_url() -> String {
    "https://api.anthropic.com".to_string()
}

/// Resolve provider type from config, env, or model name.
fn resolve_provider(config: &Config, model: &str) -> matrixcode_core::providers::ProviderType {
    // Try config first, then env var, then infer from model
    let provider_str = config
        .provider
        .as_ref()
        .cloned()
        .or_else(|| std::env::var("PROVIDER").ok());

    provider_str
        .map(|p| match p.to_lowercase().as_str() {
            "openai" => matrixcode_core::providers::ProviderType::OpenAI,
            _ => matrixcode_core::providers::ProviderType::Anthropic,
        })
        .unwrap_or_else(|| infer_provider_type(model))
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
    // Load .env file with multiple paths (silently ignore if not found)
    // Priority: current_dir/.env > parent dirs/.env (up to 4 levels)
    let current_dir = std::env::current_dir().unwrap_or_default();

    // Build search paths - go up multiple levels for nested project structures
    let mut env_paths: Vec<std::path::PathBuf> = vec![current_dir.join(".env")];
    let mut dir = current_dir.clone();
    // Search up to 4 parent levels (handles packages/cli -> packages -> matrixcode)
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
        if path.exists() {
            if dotenvy::from_path(path).is_ok() {
                println!("[env: loaded from {}]", path.display());
                loaded_env = true;
                break;
            }
        }
    }

    if !loaded_env {
        println!("[env: no .env file found, searched: {}]",
            env_paths.iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", "));
    }

    // Debug: show key env vars after loading
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
    use std::io::{self, Write};

    // Ensure terminal is in normal mode (after possible TUI exit)
    // Try to disable raw mode again (safe to call multiple times)
    let _ = matrixcode_tui::crossterm::terminal::disable_raw_mode();

    let mgr = SessionManager::new()?;
    let sessions = mgr.list_sessions();

    if sessions.is_empty() {
        println!("No sessions found.");
        println!("\nTip: Use 'matrixcode' to start a new session.");
        return Ok(());
    }

    println!("📚 Sessions:\n");
    for (i, session) in sessions.iter().enumerate() {
        let is_current = mgr.has_current() && mgr.current_id() == Some(session.id.as_str());
        println!("  {}. {}", i + 1, session.format_line(is_current));
    }

    println!(
        "\nSelect session to resume (1-{}), or 'q' to quit:",
        sessions.len()
    );
    print!("> ");
    io::stdout().flush()?;

    // Simple stdin read
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let selection = input.trim().to_string();

    // Debug output
    eprintln!("DEBUG: selection = '{}'", selection);

    if matches!(selection.as_str(), "q" | "quit" | "exit" | "") {
        println!("Cancelled.");
        return Ok(());
    }

    // Try to parse as number
    if let Ok(num) = selection.parse::<usize>()
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
            think: Some(true),
            max_tokens: 16384,
            command: None,
        };
        return run_terminal_mode(cli);
    }

    // Try to match by short_id or full id
    for session in sessions.iter() {
        if session.short_id() == selection || session.id == selection || session.id.starts_with(&selection)
        {
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
                think: Some(true),
                max_tokens: 16384,
                command: None,
            };
            return run_terminal_mode(cli);
        }
    }

    println!("Unknown session: {}", selection);

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
    
    // 创建代理工具响应 channel（TUI 发送响应，Agent 接收）
    let (proxy_response_tx, proxy_response_rx) = tokio::sync::mpsc::channel::<matrixcode_core::tools::ProxyToolResponse>(10);

    // Set debug event sender for TUI debug panel
    matrixcode_core::set_debug_event_sender(event_tx.clone());

    // Create cancellation token
    let cancel_token = CancellationToken::new();

    // Load session BEFORE spawning agent task so TUI can also display restored messages
    // Get current directory as fallback for new sessions
    let current_dir = std::env::current_dir().ok();
    let (full_messages, api_messages, session_mgr_state, session_metadata, effective_project_path) = {
        let mut mgr = SessionManager::new().ok();
        let mut full = Vec::new();
        let mut api = Vec::new();
        let mut metadata = None;
        let mut effective_path = current_dir.clone();

        if let Some(ref mut mgr) = mgr {
            if cli.continue_session || cli.resume_id.is_some() {
                let session = if let Some(ref query) = cli.resume_id {
                    mgr.resume(query).ok().flatten()
                } else {
                    mgr.continue_last().ok().flatten()
                };
                if let Some(s) = session {
                    // Log session restore details
                    log::info!(
                        "Session restored: full_messages={}, compressed_messages={}, display_messages={}",
                        s.full_messages.len(),
                        s.compressed_messages.len(),
                        s.display_messages().len()
                    );

                    // Full messages for TUI display
                    full = s.full_messages.clone();
                    // API messages (compressed if available) for Agent
                    api = s.api_messages().to_vec();
                    metadata = Some(s.metadata.clone());

                    // Use session's project_path as effective path (fallback to current_dir if not exists)
                    if let Some(ref session_path) = s.metadata.project_path {
                        let path = std::path::PathBuf::from(session_path);
                        if path.exists() {
                            effective_path = Some(path);
                            log::info!("Using session project_path: {}", session_path);
                        } else {
                            log::warn!(
                                "Session project_path '{}' no longer exists, falling back to current_dir",
                                session_path
                            );
                        }
                    }

                    log::info!("After clone: full={}, api={}", full.len(), api.len());
                }
            } else {
                let _ = mgr.start_new(current_dir.as_deref());
            }
        }
        (full, api, mgr, metadata, effective_path)
    };

    // Clone things needed in the agent task
    let agent_cancel = cancel_token.clone();
    let agent_event_tx = event_tx.clone();
    let agent_api_key = api_key.clone();
    let agent_model = model.clone();
    let agent_base_url = base_url.clone();
    let agent_think = cli.think.unwrap_or(config.think);  // Use config default if CLI not specified
    let agent_max_tokens = cli.max_tokens;
    let agent_restored_messages = api_messages.clone();  // Agent uses compressed messages
    // Use effective project path (session's saved path if available, otherwise current_dir)
    let agent_project_path = effective_project_path.clone();
    let agent_approve_mode = config
        .approve_mode
        .as_ref()
        .map(|m| matrixcode_core::approval::ApproveMode::parse(m))
        .unwrap_or(matrixcode_core::approval::ApproveMode::Ask);

    // Provider from config, env, or infer from model name
    let agent_provider = resolve_provider(&config, &agent_model);

    // Create shared approve mode atomic - accessible by both agent and TUI
    let shared_approve_mode =
        std::sync::Arc::new(std::sync::atomic::AtomicU8::new(agent_approve_mode.to_u8()));

    // Read fast_model config for keyword extraction
    let agent_fast_model = config
        .fast_model
        .clone()
        .or_else(|| std::env::var("ANTHROPIC_DEFAULT_HAIKU_MODEL").ok());

    // Extra headers from config
    let agent_extra_headers = config.extra_headers.clone();

    // Clone full config for /config command display
    let agent_config = config.clone();

    // Clone skills for agent task
    let agent_skills = skills.clone();
    let agent_shared_approve_mode = shared_approve_mode.clone();

    // Spawn Agent task with real Agent
    let agent_task = rt.spawn(async move {
        log::info!("Agent task: starting");

        // Create provider using factory
        let provider = match create_provider_with_headers(
            agent_provider,
            agent_api_key.clone(),
            agent_model.clone(),
            Some(agent_base_url.clone()),
            agent_extra_headers.clone(),
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
            create_provider_with_headers(
                fast_type,
                agent_api_key.clone(),
                fast_model.clone(),
                Some(agent_base_url.clone()),
                agent_extra_headers.clone(),
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
        let system_prompt = matrixcode_core::prompt::build_system_prompt_with_workflows(
            &matrixcode_core::prompt::PromptProfile::Default,
            &agent_skills,
            project_overview.as_ref().map(|o| o.content.as_str()),
            if initial_memory_summary.is_empty() { None } else { Some(&initial_memory_summary) },
            agent_project_path.as_ref(),
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
            .proxy_tool({
                // 创建图片搜索代理工具（优先工具，LLM 会优先选择）
                use matrixcode_core::tools::{ToolDefinition, proxy::{ProxyTool, ProxyMetadata}};
                ProxyTool::new(
                    ToolDefinition {
                        name: "image_search".to_string(),
                        description: "搜索网络图片。当用户需要查找图片、照片、图像资源时使用此工具。返回图片URL列表，包含来源平台(Unsplash/Pexels/Pixabay)、摄影师信息、尺寸等。适用于：查找壁纸、素材、插图、风景照片等视觉内容。参数：query（搜索关键词，必需）、max_results（最大结果数，可选，默认5）".to_string(),
                        parameters: serde_json::json!({
                            "type": "object",
                            "properties": {
                                "query": {
                                    "type": "string",
                                    "description": "搜索关键词，建议使用英文获得更多结果"
                                },
                                "max_results": {
                                    "type": "integer",
                                    "description": "每个平台返回的最大结果数，默认5，最大10",
                                    "default": 5
                                }
                            },
                            "required": ["query"]
                        }),
                        is_priority: true, // 优先工具，描述会自动添加 "[优先]" 标记
                    },
                    ProxyMetadata {
                        tool_type: "image_search".to_string(),
                        endpoint: None,
                        timeout_ms: 30000,
                        custom: None,
                    }
                )
            })  // 添加图片搜索代理工具
            .build();
        
        // 设置代理工具响应 channel
        agent.set_proxy_response_channel(proxy_response_rx);

        // Use the shared approve mode so TUI can update it in real-time
        agent.set_approve_mode_shared(agent_shared_approve_mode);

        // Restore messages from pre-loaded session
        if !agent_restored_messages.is_empty() {
            log::info!("Agent task: restoring {} messages", agent_restored_messages.len());
            agent.set_messages(agent_restored_messages);
        }

        log::info!("Agent task: messages restored, entering receive loop");

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

        log::info!("Agent task: entering receive loop");
        while let Some(msg) = task_rx.recv().await {
            log::info!("Agent task: received message (len={})", msg.len());
            
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
                            let overview_provider = match create_provider_with_headers(
                                agent_provider,
                                agent_api_key.clone(),
                                agent_model.clone(),
                                Some(agent_base_url.clone()),
                                agent_extra_headers.clone(),
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

            // Handle /workflow commands
            if msg == "/workflow" || msg.starts_with("/workflow ") {
                use matrixcode_core::workflow::WorkflowRegistry;

                let parts: Vec<&str> = msg.split_whitespace().collect();
                let subcmd = parts.get(1).copied().unwrap_or("");

                let project_path = agent_project_path.clone();

                // Quick response for discover/list (no async needed)
                let response = match subcmd {
                    "" | "discover" | "list" => {
                        let registry = WorkflowRegistry::new(project_path.as_ref());
                        if registry.is_empty() {
                            "📋 No workflows found.\n\nCreate workflow YAML files in:\n  - .matrix/workflows/ (project)\n  - ~/.matrix/workflows/ (global)".to_string()
                        } else {
                            registry.generate_summary()
                        }
                    }
                    "match" => {
                        let query = parts.get(2..).map(|p| p.join(" ")).unwrap_or_default();
                        if query.is_empty() {
                            "Usage: /workflow match <query>\nExample: /workflow match process text".to_string()
                        } else {
                            let registry = WorkflowRegistry::new(project_path.as_ref());
                            let matches = registry.match_workflows(&query);
                            if matches.is_empty() {
                                format!("❌ No workflows match '{}'\n\nUse '/workflow discover' to see all available.", query)
                            } else {
                                let mut result = format!("🔍 Matching workflows for '{}':\n\n", query);
                                for info in matches.iter().take(5) {
                                    result.push_str(&format!("• {} - {}\n", info.id, info.name));
                                }
                                result.push_str("\nTo run: /workflow run <id>");
                                result
                            }
                        }
                    }
                    "run" => {
                        let workflow_id = parts.get(2).copied().unwrap_or("");
                        if workflow_id.is_empty() {
                            "Usage: /workflow run <workflow-id> [inputs]\nExample: /workflow run hello-world".to_string()
                        } else {
                            // Queue workflow execution (complex operation)
                            format!("⏳ Workflow '{}' queued for execution.\n\nUse CLI command for full execution:\n  matrixcode workflow run --file .matrix/workflows/{workflow_id}.yaml", workflow_id)
                        }
                    }
                    "help" => {
                        "📋 /workflow commands:\n\n  discover - List available workflows\n  match <query> - Find matching workflows\n  run <id> - Execute workflow (use CLI for full run)".to_string()
                    }
                    _ => {
                        format!("Unknown subcommand '{}'. Use '/workflow help' for available commands.", subcmd)
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
               && !msg.starts_with("/workflow")
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
               && !msg.starts_with("/config")
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
                                let entry = matrixcode_core::memory::MemoryEntry::manual_global(category, content.clone());
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
            if msg == "/sessions" || msg == "/resume" || msg.starts_with("/sessions ") {
                let subcmd = if msg.starts_with("/sessions ") {
                    msg.strip_prefix("/sessions ").unwrap_or("")
                } else {
                    ""
                };

                if let Some(ref mut mgr) = session_mgr {
                    if subcmd == "cleanup" || subcmd == "prune" {
                        // Clean up old sessions (older than 30 days)
                        let old_removed = mgr.cleanup_old_sessions(30).unwrap_or(0);
                        // Prune to keep only 50 most recent sessions
                        let pruned = mgr.prune_sessions(50).unwrap_or(0);
                        let total = old_removed + pruned;

                        let _ = agent_event_tx.send(matrixcode_core::AgentEvent::progress(
                            format!("✓ Session cleanup: removed {} old sessions ({} by age, {} by count)",
                                total, old_removed, pruned),
                            None,
                        )).await;
                    } else if subcmd == "stats" {
                        let sessions = mgr.list_sessions();
                        let total = sessions.len();
                        let total_msgs: usize = sessions.iter().map(|s| s.message_count).sum();
                        let total_tokens: u64 = sessions.iter().map(|s| s.total_output_tokens).sum();

                        let _ = agent_event_tx.send(matrixcode_core::AgentEvent::progress(
                            format!("📊 Session stats:\n  Total sessions: {}\n  Total messages: {}\n  Total output tokens: {}",
                                total, total_msgs, total_tokens),
                            None,
                        )).await;
                    } else {
                        // List sessions
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
                            info.push_str("\n\nCommands: /sessions cleanup, /sessions stats\nUse '/load <id>' to resume");
                            let _ = agent_event_tx.send(matrixcode_core::AgentEvent::progress(
                                info,
                                None,
                            )).await;
                        }
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
                    // Use resume to load session (no need to pass project_path - session keeps its own)
                    if mgr.resume(session_id).is_ok() {
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

            // Handle /config command - display current configuration
            if msg == "/config" {
                let mut info = "⚙️ Current Configuration:\n\n".to_string();

                // Provider
                info.push_str(&format!("Provider: {}\n",
                    agent_config.provider.as_deref().unwrap_or("auto-detected")));

                // API Key (masked)
                let key_masked = agent_config.api_key.as_ref()
                    .map(|k| if k.len() > 8 {
                        format!("{}...{}",
                            &k[..4],
                            &k[k.len()-4..])
                    } else {
                        "***".to_string()
                    })
                    .unwrap_or_else(|| "not set".to_string());
                info.push_str(&format!("API Key: {}\n", key_masked));

                // Base URL
                info.push_str(&format!("Base URL: {}\n",
                    agent_config.base_url.as_deref().unwrap_or("default")));

                // Models
                info.push_str(&format!("Model: {}\n", agent_model));
                if let Some(ref pm) = agent_config.plan_model {
                    info.push_str(&format!("Plan Model: {}\n", pm));
                }
                if let Some(ref cm) = agent_config.compress_model {
                    info.push_str(&format!("Compress Model: {}\n", cm));
                }
                if let Some(ref fm) = agent_config.fast_model {
                    info.push_str(&format!("Fast Model: {}\n", fm));
                }

                // Other settings
                info.push_str(&format!("Think: {}\n", agent_config.think));
                info.push_str(&format!("Markdown: {}\n", agent_config.markdown));
                info.push_str(&format!("Max Tokens: {}\n", agent_config.max_tokens));
                if let Some(cs) = agent_config.context_size {
                    info.push_str(&format!("Context Size: {}\n", cs));
                }
                info.push_str(&format!("Approve Mode: {}\n",
                    agent_config.approve_mode.as_deref().unwrap_or("ask")));

                // Extra headers
                if let Some(ref headers) = agent_config.extra_headers {
                    if !headers.is_empty() {
                        info.push_str(&format!("Extra Headers: {} header(s)\n", headers.len()));
                        for (k, v) in headers.iter().take(3) {
                            info.push_str(&format!("  {}: {}\n", k, v));
                        }
                    }
                }

                info.push_str("\n📝 Config sources (priority order):\n");
                info.push_str("  1. Environment variables (highest)\n");
                info.push_str("  2. ~/.matrix/config.json\n");
                info.push_str("  3. ~/.claude/settings.json (fallback)\n");

                let _ = agent_event_tx.send(matrixcode_core::AgentEvent::progress(
                    info,
                    None,
                )).await;
                continue;
            }

            // Dynamic memory retrieval: AI selects relevant memories (Claude Code style)
            // Skip for first turn or simple messages to avoid unnecessary API calls
            if let Some(ref mem) = memory {
                // Check skip conditions
                let is_first_turn = turn_count == 0;
                let is_simple_msg = matrixcode_core::memory::should_skip_simple_message(&msg);
                let has_few_memories = mem.entries.len() < 5;

                if is_first_turn || is_simple_msg {
                    // Skip AI selection for first turn or simple messages
                    // Just use static summary if available
                    let static_summary = mem.generate_prompt_summary(10);
                    if !static_summary.is_empty() {
                        agent.update_memory_summary(Some(static_summary));
                    }
                } else if has_few_memories {
                    // Few memories: just use all of them without AI selection
                    let static_summary = mem.generate_prompt_summary(10);
                    if !static_summary.is_empty() {
                        agent.update_memory_summary(Some(static_summary));
                    }
                } else if let Some(ref fp) = fast_provider {
                    // Normal case: AI selects relevant memories
                    // Generate manifest (descriptions list)
                    let manifest = mem.generate_manifest(50);  // Top 50 by importance

                    if !manifest.is_empty() {
                        // AI selects relevant memories
                        let selected_indices = matrixcode_core::memory::ai_select_memories(
                            &msg,
                            &manifest,
                            fp.as_ref(),
                        ).await;

                        // Get selected entries and generate summary
                        let selected_entries = mem.get_entries_by_indices(&selected_indices);
                        let contextual_summary = if selected_entries.is_empty() {
                            // AI didn't select any, use top entries
                            mem.generate_prompt_summary(5)
                        } else {
                            // Generate summary from selected entries
                            let mut summary = String::from("【相关记忆】\n\n");
                            for entry in selected_entries.iter().take(5) {
                                summary.push_str(&format!("{} {}\n", entry.category.icon(), entry.content));
                            }
                            summary
                        };

                        if !contextual_summary.is_empty() {
                            agent.update_memory_summary(Some(contextual_summary));

                            // Debug log
                            matrixcode_core::debug::debug_log().log("memory_selection",
                                &format!("AI selected {} memories from {} candidates",
                                    selected_indices.len(), mem.entries.len()));

                            // Send event for TUI
                            if !selected_indices.is_empty() {
                                let _ = agent_event_tx.send(matrixcode_core::AgentEvent::with_data(
                                    matrixcode_core::EventType::MemoryLoaded,
                                    matrixcode_core::EventData::Memory {
                                        summary: format!("AI 选择了 {} 条相关记忆", selected_indices.len()),
                                        entries_count: selected_indices.len(),
                                    },
                                )).await;
                            }
                        }
                    }
                } else {
                    // No fast provider: use rule-based keyword search
                    let keywords = matrixcode_core::memory::extract_context_keywords(&msg);
                    let contextual_summary = mem.generate_contextual_summary_with_keywords(&keywords, 10);
                    if !contextual_summary.is_empty() {
                        agent.update_memory_summary(Some(contextual_summary));
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
                        // Update full messages first (for display/TUI restore)
                        mgr.set_messages(messages.to_vec());
                        // Then update compressed messages (for API efficiency)
                        mgr.set_compressed_messages(messages.to_vec());
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

                    // Auto-detect and save memories (background task for AI extraction)
                    // Feedback detection (rule-based, fast) stays in main thread
                    if let Some(ref mut ms) = memory_storage {
                        // 1. Check for user feedback/correction (rule-based, fast)
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

                        // 2. Periodic cleanup (every 10 turns) - rule-based, fast
                        if turn_count.is_multiple_of(10)
                            && let Ok(mut mem) = ms.load_combined() {
                                mem.apply_time_decay();
                                let merged = mem.smart_merge();
                                mem.prune();
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

                    // 3. AI memory extraction - spawn background task (non-blocking)
                    // Only run every N turns to reduce API calls
                    let should_extract = turn_count.is_multiple_of(3) && fast_provider.is_some();
                    matrixcode_core::debug::debug_log().log(
                        "memory_extract",
                        &format!(
                            "turn={}, should_extract={}, fast_model={}, project_path={}",
                            turn_count,
                            should_extract,
                            agent_fast_model.as_deref().unwrap_or("none"),
                            agent_project_path.as_deref().map(|p| p.display().to_string()).unwrap_or_else(|| "none".to_string())
                        ),
                    );

                    if should_extract {
                        let messages = agent.get_messages();
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

                            // Clone necessary data for background task
                            let bg_project_path = agent_project_path.clone();
                            let bg_fast_model = agent_fast_model.clone();
                            let bg_event_tx = agent_event_tx.clone();

                            // Spawn background extraction task
                            tokio::spawn(async move {
                                // Create new memory storage for background task
                                // Use cloned path (not borrowed reference)
                                let bg_ms = matrixcode_core::memory::MemoryStorage::new(bg_project_path.as_deref()).ok();

                                if bg_ms.is_none() {
                                    matrixcode_core::debug::debug_log().log("memory_extract", "Background task: failed to create memory storage");
                                    return;
                                }
                                if text.is_empty() {
                                    matrixcode_core::debug::debug_log().log("memory_extract", "Background task: text is empty");
                                    return;
                                }

                                let mut bg_ms = bg_ms.unwrap();

                                // Use minimal prompt for extraction (efficient, focused task)
                                let project_path_str = bg_project_path.as_ref().map(|p| p.to_string_lossy().to_string());
                                let detected = if let Some(model) = bg_fast_model {
                                    // Use simple extraction prompt (not full system prompt)
                                    // This is a focused task, so we use minimal context
                                    matrixcode_core::debug::debug_log().log("memory_extract", &format!("Background task: extracting with model={}, text_len={}", model, text.len()));
                                    let extractor = matrixcode_core::memory::AiMemoryExtractor::new_minimal(model);
                                    matrixcode_core::memory::detect_memories_smart(
                                        &text, None, project_path_str.as_deref(), Some(&extractor)
                                    ).await
                                } else {
                                    matrixcode_core::debug::debug_log().log("memory_extract", "Background task: no fast_model, skipping");
                                    Vec::new()
                                };

                                matrixcode_core::debug::debug_log().log("memory_extract", &format!("Background task: detected {} entries", detected.len()));

                                if !detected.is_empty() {
                                    let detected_count = detected.len();

                                    for entry in detected {
                                        let is_global_category = matches!(
                                            entry.category,
                                            matrixcode_core::memory::MemoryCategory::Preference
                                                | matrixcode_core::memory::MemoryCategory::UserIntentPattern
                                                | matrixcode_core::memory::MemoryCategory::TaskPattern
                                        );

                                        let is_project = !is_global_category
                                            && (entry.tags.contains(&"project".to_string())
                                                || entry.project_path.is_some()
                                                || bg_project_path.is_some());

                                        if let Err(e) = bg_ms.add_entry(entry, is_project) {
                                            log::warn!("Failed to add memory entry: {}", e);
                                        }
                                    }

                                    // Send event to TUI (non-blocking)
                                    let _ = bg_event_tx.send(matrixcode_core::AgentEvent::with_data(
                                        matrixcode_core::EventType::MemoryDetected,
                                        matrixcode_core::EventData::Memory {
                                            summary: format!("检测到 {} 条记忆", detected_count),
                                            entries_count: detected_count,
                                        },
                                    )).await;
                                }
                            });
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
    // Default: true for debug builds, false for release builds
    let debug_mode = std::env::var("MATRIXCODE_DEBUG")
        .map(|v| v == "1" || v == "true" || v == "verbose")
        .unwrap_or(cfg!(debug_assertions)); // Default to true in debug builds

    // Setup terminal for TUI
    let mut terminal = setup_terminal()?;

    // Create App and run it (TUI runs in sync context, but tokio channels are usable)
    let mut app = TuiApp::new(task_tx, event_rx, cancel_token.clone())
        .with_ask_channel(ask_tx)
        .with_shared_approve_mode(shared_approve_mode)
        .with_proxy_response_tx(proxy_response_tx)
        .with_config(&model, cli.think.unwrap_or(config.think), cli.max_tokens, None)
        .with_debug_mode(debug_mode);

    // Load restored messages if any (full messages for TUI display)
    if !full_messages.is_empty() {
        app.load_messages(full_messages);
        // Restore token stats from session metadata
        if let Some(ref meta) = session_metadata {
            app.set_token_stats(
                meta.last_input_tokens,
                meta.total_output_tokens,
                meta.message_count,
            );
        }
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
        })
        .await
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

/// Handle workflow subcommands
fn handle_workflow_command(command: WorkflowCommands) {
    use matrixcode_core::workflow::{
        parse_workflow_from_file, WorkflowEngine, WorkflowPersistence,
        WorkflowStatus,
    };

    match command {
        WorkflowCommands::Run { file, inputs } => {
            println!("🔄 Running workflow from: {}", file);

            // Parse workflow definition
            let workflow_def = match parse_workflow_from_file(&file) {
                Ok(def) => def,
                Err(e) => {
                    eprintln!("❌ Failed to parse workflow: {}", e);
                    return;
                }
            };

            println!("  Workflow: {}", workflow_def.id);
            println!("  Name: {}", workflow_def.name);
            println!("  Nodes: {}", workflow_def.nodes.len());

            // Parse inputs if provided
            let inputs_map: std::collections::HashMap<String, serde_json::Value> = inputs
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();

            // Create engine
            let engine = match WorkflowEngine::new(workflow_def) {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("❌ Failed to create workflow engine: {}", e);
                    return;
                }
            };

            // Run workflow - use block_in_place to allow blocking inside async context
            // or create new runtime if not in async context
            let context = if tokio::runtime::Handle::try_current().is_ok() {
                // Already in a runtime, use block_in_place
                tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(engine.run(inputs_map))
                })
            } else {
                // Not in a runtime, create one
                let rt = match tokio::runtime::Runtime::new() {
                    Ok(rt) => rt,
                    Err(e) => {
                        eprintln!("Failed to create tokio runtime: {}", e);
                        return;
                    }
                };
                rt.block_on(engine.run(inputs_map))
            };

            match context {
                Ok(context) => {
                    println!();
                    println!("📊 Workflow completed:");
                    println!("  Instance ID: {}", context.instance_id);
                    println!("  Status: {:?}", context.status);
                    println!("  Nodes executed: {}", context.execution_path.len());

                    if context.status == WorkflowStatus::Completed {
                        println!("✓ Workflow completed successfully");
                    } else if context.status == WorkflowStatus::Failed {
                        println!("❌ Workflow failed: {}", context.error.as_ref().unwrap_or(&String::new()));
                    }

                    // Save context
                    let project_path = std::env::current_dir().ok();
                    let persistence = WorkflowPersistence::new(project_path.as_ref());
                    if let Err(e) = persistence.save(&context) {
                        eprintln!("Warning: Failed to save workflow context: {}", e);
                    }
                }
                Err(e) => {
                    eprintln!("❌ Workflow execution failed: {}", e);
                }
            }
        }

        WorkflowCommands::Discover { query } => {
            use matrixcode_core::workflow::WorkflowRegistry;

            let project_path = std::env::current_dir().ok();
            let registry = WorkflowRegistry::new(project_path.as_ref());

            if registry.is_empty() {
                println!("No workflows found in:");
                println!("  - Project: .matrix/workflows/");
                println!("  - User: ~/.matrix/workflows/");
                println!("\nCreate workflow YAML files in these directories.");
                return;
            }

            if let Some(q) = query {
                // Match workflows by query
                let matches = registry.match_workflows(&q);
                if matches.is_empty() {
                    println!("No workflows match query: '{}'", q);
                    println!("\nAvailable workflows:");
                    for info in registry.list() {
                        let source = if info.source == matrixcode_core::workflow::WorkflowSource::Project { "project" } else { "global" };
                        println!("  - {} ({})", info.id, source);
                    }
                } else {
                    println!("🔍 Matching workflows for '{}':\n", q);
                    for info in matches {
                        let source = if info.source == matrixcode_core::workflow::WorkflowSource::Project { "project" } else { "global" };
                        println!("  {} - {} [{}]", info.id, info.name, source);
                        if let Some(ref desc) = info.description {
                            let desc_short = desc.chars().take(60).collect::<String>();
                            println!("    {}", desc_short);
                        }
                        if !info.required_inputs.is_empty() {
                            println!("    Required: {}", info.required_inputs.join(", "));
                        }
                        println!("    File: {}", info.path.display());
                        println!();
                    }
                }
            } else {
                // List all discovered workflows
                println!("🔍 Discovered workflows ({}):\n", registry.count());
                let summary = registry.generate_summary();
                println!("{}", summary);
            }
        }

        WorkflowCommands::List { status } => {
            let project_path = std::env::current_dir().ok();
            let persistence = WorkflowPersistence::new(project_path.as_ref());

            let workflows = if let Some(filter) = status {
                // Parse status filter
                let filter_status = match filter.to_lowercase().as_str() {
                    "running" => WorkflowStatus::Running,
                    "paused" => WorkflowStatus::Paused,
                    "completed" => WorkflowStatus::Completed,
                    "failed" => WorkflowStatus::Failed,
                    "cancelled" => WorkflowStatus::Cancelled,
                    _ => {
                        eprintln!("Unknown status: {}. Use: running, paused, completed, failed, cancelled", filter);
                        return;
                    }
                };
                persistence.list_by_status(filter_status).unwrap_or_default()
            } else {
                persistence.list().unwrap_or_default()
            };

            if workflows.is_empty() {
                println!("No workflows found.");
                println!("\nWorkflows are stored in:");
                println!("  - Project: .matrix/workflows/ (if in project)");
                println!("  - User: ~/.matrix/workflows/ (global)");
            } else {
                println!("📚 Workflow History:\n");
                for ctx in &workflows {
                    println!("  {} - {} ({:?})",
                        ctx.instance_id,
                        ctx.workflow_id,
                        ctx.status
                    );
                    println!("    Nodes: {} | Created: {}",
                        ctx.execution_path.len(),
                        ctx.created_at.format("%Y-%m-%d %H:%M")
                    );
                    if let Some(err) = &ctx.error {
                        println!("    Error: {}", err.chars().take(50).collect::<String>());
                    }
                    println!();
                }
                println!("Total: {} workflows", workflows.len());
            }
        }

        WorkflowCommands::Status { id } => {
            let project_path = std::env::current_dir().ok();
            let persistence = WorkflowPersistence::new(project_path.as_ref());

            match persistence.load(&id) {
                Ok(Some(ctx)) => {
                    println!("📊 Workflow Status:\n");
                    println!("  Instance ID: {}", ctx.instance_id);
                    println!("  Workflow: {}", ctx.workflow_id);
                    println!("  Status: {:?}", ctx.status);
                    println!("  Current Node: {}", ctx.current_node_id.as_ref().unwrap_or(&"none".to_string()));
                    println!("  Created: {}", ctx.created_at.format("%Y-%m-%d %H:%M"));
                    if let Some(started) = ctx.started_at {
                        println!("  Started: {}", started.format("%Y-%m-%d %H:%M"));
                    }
                    if let Some(finished) = ctx.finished_at {
                        println!("  Finished: {}", finished.format("%Y-%m-%d %H:%M"));
                        if let Some(duration) = ctx.total_duration_ms() {
                            println!("  Duration: {} ms", duration);
                        }
                    }
                    println!();
                    println!("  Execution Path:");
                    for node_id in &ctx.execution_path {
                        if let Some(exec) = ctx.get_node_execution(node_id) {
                            println!("    - {} ({:?})", node_id, exec.status);
                        }
                    }
                    if let Some(err) = &ctx.error {
                        println!();
                        println!("  ❌ Error: {}", err);
                    }
                }
                Ok(None) => {
                    println!("❌ Workflow '{}' not found", id);
                }
                Err(e) => {
                    eprintln!("❌ Failed to load workflow: {}", e);
                }
            }
        }

        WorkflowCommands::Resume { id } => {
            println!("🔄 Resuming workflow: {}", id);

            let project_path = std::env::current_dir().ok();
            let persistence = WorkflowPersistence::new(project_path.as_ref());

            match persistence.load(&id) {
                Ok(Some(ctx)) => {
                    if ctx.status != WorkflowStatus::Paused {
                        println!("❌ Workflow is not paused (status: {:?})", ctx.status);
                        println!("Only paused workflows can be resumed.");
                        return;
                    }

                    println!("  Workflow: {}", ctx.workflow_id);
                    println!("  Current Node: {}", ctx.current_node_id.as_ref().unwrap_or(&"unknown".to_string()));

                    // TODO: Implement actual resume logic with engine
                    // For now, just update status
                    let mut ctx = ctx;
                    ctx.resume();

                    if let Err(e) = persistence.save(&ctx) {
                        eprintln!("❌ Failed to save resumed workflow: {}", e);
                    } else {
                        println!("✓ Workflow resumed (status: Running)");
                        println!();
                        println!("Note: Full resume execution requires re-running the workflow engine.");
                        println!("Use: matrixcode workflow run --file <yaml> --inputs '{}'",
                            serde_json::to_string(&ctx.inputs).unwrap_or_default());
                    }
                }
                Ok(None) => {
                    println!("❌ Workflow '{}' not found", id);
                }
                Err(e) => {
                    eprintln!("❌ Failed to load workflow: {}", e);
                }
            }
        }

        WorkflowCommands::Abort { id } => {
            println!("⏹️ Aborting workflow: {}", id);

            let project_path = std::env::current_dir().ok();
            let persistence = WorkflowPersistence::new(project_path.as_ref());

            match persistence.load(&id) {
                Ok(Some(ctx)) => {
                    if ctx.status != WorkflowStatus::Running {
                        println!("❌ Workflow is not running (status: {:?})", ctx.status);
                        return;
                    }

                    let mut ctx = ctx;
                    ctx.cancel();

                    if let Err(e) = persistence.save(&ctx) {
                        eprintln!("❌ Failed to save aborted workflow: {}", e);
                    } else {
                        println!("✓ Workflow aborted");
                    }
                }
                Ok(None) => {
                    println!("❌ Workflow '{}' not found", id);
                }
                Err(e) => {
                    eprintln!("❌ Failed to load workflow: {}", e);
                }
            }
        }
    }
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
                    let provider = match create_provider_with_headers(
                        resolve_provider(&config, &model),
                        api_key,
                        model.clone(),
                        Some(base_url),
                        config.extra_headers.clone(),
                    ) {
                        Ok(p) => p,
                        Err(e) => {
                            eprintln!("Failed to create provider: {}", e);
                            return;
                        }
                    };

                    // Build agent with event channel
                    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(100);
                    let mut agent = AgentBuilder::new(provider)
                        .system_prompt(system_prompt)
                        .model_name(model.clone())
                        .max_tokens(4096)
                        .tools(all_tools_with_skills(Arc::new(skills.to_vec())))
                        .approve_mode(approve_mode)
                        .event_tx(event_tx)
                        .build();

                    // Run agent
                    let run_future = agent.run(msg);

                    // Process events while running - use spawn to avoid race condition
                    let event_task = tokio::spawn(async move {
                        while let Some(event) = event_rx.recv().await {
                            // Log events for debug
                            if event.event_type == matrixcode_core::EventType::Error {
                                if let Some(data) = &event.data {
                                    eprintln!("⚠️ Error event: {:?}", data);
                                }
                            }
                        }
                    });

                    // Wait for agent to complete
                    let result = run_future.await;

                    // Wait for event processing to complete (with timeout)
                    let _ = tokio::time::timeout(
                        tokio::time::Duration::from_millis(100),
                        event_task
                    ).await;

                    match result {
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
                let provider = match create_provider_with_headers(
                    resolve_provider(&config, &model),
                    api_key,
                    model.clone(),
                    Some(base_url),
                    config.extra_headers.clone(),
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
            Commands::Workflow { command } => {
                // Handle workflow subcommands
                handle_workflow_command(command);
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

                // Create event channel for agent
                let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(100);

                let system_prompt = matrixcode_core::prompt::build_system_prompt(
                    &matrixcode_core::prompt::PromptProfile::Default,
                    &skills,
                    None,
                    None,
                );

                let provider = match create_provider_with_headers(
                    resolve_provider(&config, &model),
                    api_key,
                    model.clone(),
                    Some(base_url),
                    config.extra_headers.clone(),
                ) {
                    Ok(p) => p,
                    Err(e) => {
                        println!(
                            "{}",
                            AgentEvent::error(
                                format!("Failed to create provider: {}", e),
                                None,
                                None,
                            )
                            .to_json()?
                        );
                        return Ok::<_, anyhow::Error>(());
                    }
                };
                let mut agent = AgentBuilder::new(provider)
                    .system_prompt(system_prompt)
                    .model_name(model)
                    .max_tokens(4096)
                    .tools(all_tools_with_skills(Arc::new(skills.clone())))
                    .approve_mode(matrixcode_core::approval::ApproveMode::Auto)
                    .event_tx(event_tx)
                    .build();

                // Run agent and collect events
                let run_result = agent.run(message.unwrap_or_default()).await;

                // Process events
                while let Some(event) = event_rx.recv().await {
                    match event.event_type {
                        matrixcode_core::EventType::TextDelta => {
                            if let Some(_data) = &event.data {
                                println!("{}", event.to_json()?);
                            }
                        }
                        matrixcode_core::EventType::Error => {
                            println!("{}", event.to_json()?);
                        }
                        matrixcode_core::EventType::SessionEnded => {
                            break;
                        }
                        _ => {}
                    }
                }

                match run_result {
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

                let provider = match create_provider_with_headers(
                    resolve_provider(&config, &model),
                    api_key,
                    model.clone(),
                    Some(base_url),
                    config.extra_headers.clone(),
                ) {
                    Ok(p) => p,
                    Err(e) => {
                        println!(
                            "{}",
                            AgentEvent::error(
                                format!("Failed to create provider: {}", e),
                                None,
                                None,
                            )
                            .to_json()?
                        );
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
        Some(Commands::Workflow { command }) => {
            // Workflow commands don't use JSON output, just handle directly
            handle_workflow_command(command);
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
                    let provider = match create_provider_with_headers(
                        resolve_provider(&config, &model),
                        api_key,
                        model.clone(),
                        Some(base_url),
                        config.extra_headers.clone(),
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
                    let provider = match create_provider_with_headers(
                        resolve_provider(&config, &model),
                        api_key,
                        model.clone(),
                        Some(base_url),
                        config.extra_headers.clone(),
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
            // Load/resume a session (no need to pass project_path - session keeps its own)
            if let Some(session_id) = request.session_id.clone() {
                if let Ok(mut mgr) = SessionManager::new() {
                    match mgr.resume(&session_id) {
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
