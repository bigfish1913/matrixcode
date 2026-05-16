use anyhow::Result;
use clap::Parser;
use matrixcode::{
    agent,
    cancel::CancellationToken,
    compress::CompressionConfig,
    models::{MultiModelConfig, ModelConfig, ModelRole},
    overview::ProjectOverview,
    prompt,
    providers,
    session::SessionManager,
    skills,
    ui,
    workspace::Workspace,
};
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use rustyline::{Cmd, EventHandler, KeyCode, KeyEvent, Modifiers};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

// ============================================================================
// CLI Argument Definitions
// ============================================================================

#[derive(Parser)]
#[command(name = "matrixcode", about = "A simple code agent with tool use")]
struct Cli {
    #[arg(short, long, env = "PROVIDER", default_value = "anthropic")]
    provider: String,

    #[arg(short, long, env = "MODEL_NAME")]
    model: Option<String>,

    #[arg(long, env = "API_KEY")]
    api_key: Option<String>,

    #[arg(long, env = "BASE_URL")]
    base_url: Option<String>,

    /// Enable Anthropic extended thinking (default on). Pass --think false to disable.
    #[arg(long, env = "THINK", default_value_t = true, action = clap::ArgAction::Set)]
    think: bool,

    /// Auto-continue last session on startup (default: false).
    /// Set via AUTO_CONTINUE environment variable or --auto-continue flag.
    /// When true, behaves as if --continue was passed automatically.
    #[arg(long, env = "AUTO_CONTINUE", default_value_t = false)]
    auto_continue: bool,

    /// Render assistant output as Markdown in the terminal (default on).
    /// Pass --markdown false to disable, or set NO_COLOR / run in a non-TTY
    /// to auto-disable.
    #[arg(long, env = "MARKDOWN", default_value_t = true, action = clap::ArgAction::Set)]
    markdown: bool,

    /// Continue the last session (most common resume case).
    /// Equivalent to --resume with the most recently used session.
    #[arg(short = 'c', long)]
    continue_: bool,

    /// Resume a specific session by ID or name, or show interactive picker.
    /// Use --resume alone to pick from a list, or --resume <id> to resume directly.
    #[arg(short, long)]
    resume: Option<Option<String>>, // --resume, --resume <id>

    /// List all saved sessions.
    #[arg(long)]
    list_sessions: bool,

    /// Extra directory to scan for skills. May be passed multiple times.
    /// The defaults `./skills` and `~/.matrix/skills` are always
    /// scanned first unless `--no-default-skills` is set.
    #[arg(long = "skills-dir", env = "SKILLS_DIR", value_delimiter = ':')]
    skills_dir: Vec<PathBuf>,

    /// Skip the default skills roots (`./skills`, `~/.matrix/skills`).
    #[arg(long, default_value_t = false)]
    no_default_skills: bool,

    /// Prompt profile: default, safe, fast, review.
    #[arg(long, env = "PROMPT_PROFILE", default_value = "default")]
    profile: String,

    /// Enable server-side web search tool (default on for Anthropic provider).
    /// The model can perform web searches directly via the API provider.
    /// Pass --no-web-search to disable.
    #[arg(long, env = "NO_WEB_SEARCH", default_value_t = false, action = clap::ArgAction::SetTrue)]
    no_web_search: bool,

    /// Maximum number of web searches per turn when --web-search is enabled.
    #[arg(long, env = "WEB_SEARCH_MAX_USES", default_value = "5")]
    web_search_max_uses: u32,

    /// Maximum output tokens per response (default: 16384).
    #[arg(long, env = "MAX_TOKENS", default_value = "16384")]
    max_tokens: u32,

    /// One-shot prompt. If omitted, enters interactive REPL mode.
    prompt: Vec<String>,

    /// Skip loading project overview on startup.
    #[arg(long, default_value_t = false)]
    no_overview: bool,

    /// Generate project overview using AI and exit.
    #[arg(long, default_value_t = false)]
    init: bool,

    /// Context compression threshold (0.0-1.0). When context usage exceeds this ratio,
    /// old messages will be compressed. Default: 0.75.
    #[arg(long, env = "COMPRESSION_THRESHOLD", default_value = "0.75")]
    compression_threshold: f64,

    /// Disable automatic context compression.
    #[arg(long, env = "NO_COMPRESSION", default_value_t = false, action = clap::ArgAction::SetTrue)]
    no_compression: bool,

    /// Minimum messages to preserve when compressing context.
    #[arg(long, env = "MIN_PRESERVE_MESSAGES", default_value = "6")]
    min_preserve_messages: usize,

    /// Override context window size (in tokens). Useful for proxy endpoints.
    #[arg(long, env = "CONTEXT_SIZE")]
    context_size: Option<u32>,

    /// Enable prompt caching for Anthropic provider (default on).
    /// Pass --no-caching to disable.
    #[arg(long, env = "NO_CACHING", default_value_t = false, action = clap::ArgAction::SetTrue)]
    no_caching: bool,

    /// Model for planning tasks (defaults to same as main model).
    /// Use a capable model like claude-sonnet-4 for better planning.
    #[arg(long, env = "PLAN_MODEL")]
    plan_model: Option<String>,

    /// Model for compression/summarization (defaults to claude-3-5-haiku).
    /// Use a smaller, cheaper model for cost efficiency.
    #[arg(long, env = "COMPRESS_MODEL")]
    compress_model: Option<String>,

    /// Model for quick operations (defaults to claude-3-5-haiku).
    /// Use a fast model for classification and simple tasks.
    #[arg(long, env = "FAST_MODEL")]
    fast_model: Option<String>,

    /// Enable multi-model mode (use separate models for plan/compress/fast).
    #[arg(long, env = "MULTI_MODEL", default_value_t = false, action = clap::ArgAction::SetTrue)]
    multi_model: bool,

    /// Approval mode for tool execution:
    /// - ask: pause before mutating/dangerous operations (default)
    /// - auto: execute everything without asking
    /// - strict: ask before every tool call
    #[arg(long, env = "APPROVE_MODE", default_value = "ask")]
    approve_mode: String,
}

// ============================================================================
// Main Entry Point
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    env_logger::init();

    let cli = Cli::parse();

    let api_key = cli.api_key.unwrap_or_else(|| match cli.provider.as_str() {
        "openai" => std::env::var("OPENAI_API_KEY").expect("API_KEY or OPENAI_API_KEY required"),
        _ => std::env::var("ANTHROPIC_API_KEY").expect("API_KEY or ANTHROPIC_API_KEY required"),
    });

    let model = cli.model.unwrap_or_else(|| match cli.provider.as_str() {
        "openai" => "gpt-4o".to_string(),
        _ => "claude-sonnet-4-20250514".to_string(),
    });

    let base_url = cli.base_url.unwrap_or_else(|| match cli.provider.as_str() {
        "openai" => "https://api.openai.com/v1".to_string(),
        _ => "https://api.anthropic.com".to_string(),
    });

    // Set CONTEXT_SIZE env var if provided via CLI (must be before provider creation)
    if let Some(size) = cli.context_size {
        // SAFETY: set_var is unsafe because it can affect multi-threaded programs.
        // We're doing this during initialization before any threads are spawned.
        unsafe { std::env::set_var("CONTEXT_SIZE", size.to_string()); }
    }

    let provider: Box<dyn providers::Provider> = match cli.provider.as_str() {
        "openai" => Box::new(providers::openai::OpenAIProvider::new(
            api_key.clone(), model.clone(), base_url.clone(),
        )),
        "anthropic" => Box::new(providers::anthropic::AnthropicProvider::new(
            api_key.clone(), model.clone(), base_url.clone(),
        )),
        other => anyhow::bail!("Unknown provider: {other}. Use 'openai' or 'anthropic'"),
    };

    let profile = cli
        .profile
        .parse::<prompt::PromptProfile>()
        .map_err(anyhow::Error::msg)?;

    // Detect workspace root for overview loading
    let workspace = Workspace::detect(None).ok();
    let project_root = workspace.as_ref().map(|w| w.root().to_path_buf());

    // Load project overview if available
    let overview = if !cli.no_overview {
        if let Some(ref root) = project_root {
            match ProjectOverview::load(root) {
                Ok(Some(ov)) => {
                    println!("[loaded project overview from {}]", ov.path.display());
                    Some(ov)
                }
                Ok(None) => None,
                Err(e) => {
                    eprintln!("[warn] could not load overview: {e}");
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    // Load accumulated memory and generate summary for system prompt
    let memory_summary = load_memory_summary(project_root.as_deref());

    let mut agent = agent::Agent::with_memory_and_overview(
        provider,
        cli.think,
        cli.markdown,
        profile,
        load_skills(&cli.skills_dir, cli.no_default_skills),
        cli.max_tokens,
        overview.as_ref().map(|o| o.content.as_str()),
        memory_summary.as_deref(),
    );

    // Configure multi-model if enabled or specific models provided
    if cli.multi_model || cli.plan_model.is_some() || cli.compress_model.is_some() || cli.fast_model.is_some() {
        // Start with all roles using the main model
        let mut model_config = MultiModelConfig::with_main(model.clone());
        
        // Override plan model if specified
        if let Some(ref plan_model_name) = cli.plan_model {
            model_config.set(ModelRole::Plan, ModelConfig::new(plan_model_name.clone()));
            println!("[plan model: {}]", plan_model_name);
        } else if cli.multi_model {
            println!("[plan model: {} (using main model)]", model);
        }
        
        // Override compress model if specified
        if let Some(ref compress_model_name) = cli.compress_model {
            model_config.set(ModelRole::Compress, ModelConfig::new(compress_model_name.clone()));
            println!("[compress model: {}]", compress_model_name);
        } else if cli.multi_model {
            println!("[compress model: {} (using main model)]", model);
        }
        
        // Override fast model if specified
        if let Some(ref fast_model_name) = cli.fast_model {
            model_config.set(ModelRole::Fast, ModelConfig::new(fast_model_name.clone()));
            println!("[fast model: {}]", fast_model_name);
        } else if cli.multi_model {
            println!("[fast model: {} (using main model)]", model);
        }
        
        // Create providers for plan and compress if multi-model is enabled
        if cli.multi_model {
            // Plan provider (uses plan model config)
            let plan_model_name = model_config.plan.name.clone();
            let plan_provider: Box<dyn providers::Provider> = match cli.provider.as_str() {
                "openai" => Box::new(providers::openai::OpenAIProvider::new(
                    api_key.clone(), plan_model_name.clone(), base_url.clone(),
                )),
                _ => Box::new(providers::anthropic::AnthropicProvider::new(
                    api_key.clone(), plan_model_name.clone(), base_url.clone(),
                )),
            };
            agent = agent.with_plan_provider(plan_provider);
            
            // Compress provider (uses compress model config)
            let compress_model_name = model_config.compress.name.clone();
            let compress_provider: Box<dyn providers::Provider> = match cli.provider.as_str() {
                "openai" => Box::new(providers::openai::OpenAIProvider::new(
                    api_key.clone(), compress_model_name.clone(), base_url.clone(),
                )),
                _ => Box::new(providers::anthropic::AnthropicProvider::new(
                    api_key.clone(), compress_model_name.clone(), base_url.clone(),
                )),
            };
            agent = agent.with_compress_provider(compress_provider);
            
            println!("[multi-model enabled: all models default to main model]");
        } else if cli.compress_model.is_some() {
            // Only compress model specified, create compress provider
            let compress_model_name = model_config.compress.name.clone();
            let compress_provider: Box<dyn providers::Provider> = match cli.provider.as_str() {
                "openai" => Box::new(providers::openai::OpenAIProvider::new(
                    api_key.clone(), compress_model_name.clone(), base_url.clone(),
                )),
                _ => Box::new(providers::anthropic::AnthropicProvider::new(
                    api_key.clone(), compress_model_name.clone(), base_url.clone(),
                )),
            };
            agent = agent.with_compress_provider(compress_provider);
        }
        
        agent = agent.with_model_config(model_config);
    }

    // Enable server-side web search by default for Anthropic provider
    if cli.provider == "anthropic" && !cli.no_web_search {
        agent = agent.with_web_search(Some(cli.web_search_max_uses));
        println!("[server web search enabled, max {} uses per turn]", cli.web_search_max_uses);
    }

    // Configure context compression
    if !cli.no_compression {
        let compression_config = CompressionConfig {
            threshold: cli.compression_threshold,
            min_preserve_messages: cli.min_preserve_messages,
            use_summarization: true,
            target_ratio: 0.5,
            compressor_model: None,
            bias: matrixcode::compress::CompressionBias::balanced(),
        };
        agent.set_compression_config(compression_config);
    }

    // Configure prompt caching
    if cli.provider == "anthropic" && !cli.no_caching {
        agent.set_caching(true);
        println!("[prompt caching enabled for Anthropic]");
    } else if cli.no_caching {
        agent.set_caching(false);
    }

    // Configure approval mode
    let approve_mode = matrixcode::approval::ApproveMode::from_str(&cli.approve_mode);
    agent.set_approve_mode(approve_mode);
    if approve_mode != matrixcode::approval::ApproveMode::Ask {
        println!("[approve mode: {}]", approve_mode);
    }

    // Initialize session manager
    let mut session_manager = SessionManager::new()?;
    
    // Handle --list-sessions
    if cli.list_sessions {
        list_sessions(&session_manager);
        return Ok(());
    }

    // Handle session resumption
    let session_to_load = if cli.continue_ || cli.auto_continue {
        // --continue or AUTO_CONTINUE=true: load last session
        session_manager.continue_last(project_root.as_deref())?
    } else if let Some(ref resume_arg) = cli.resume {
        match resume_arg {
            Some(id_or_name) => {
                // --resume <id>: load specific session
                session_manager.resume(id_or_name, project_root.as_deref())?
            }
            None => {
                // --resume alone: show picker
                let picked = show_session_picker(&session_manager)?;
                if let Some(id) = picked {
                    session_manager.resume(&id, project_root.as_deref())?
                } else {
                    None
                }
            }
        }
    } else {
        // No resume flags: start new session
        None
    };

    // Load messages into agent if resuming
    if let Some(session) = session_to_load {
        let n = session.messages.len();
        agent.set_messages(session.messages.clone());
        println!("[resumed session '{}' with {} message(s)]", 
            session.metadata.display_name(), n);
    } else {
        // Start new session
        session_manager.start_new(project_root.as_deref())?;
        if let Some(meta) = session_manager.current_metadata() {
            println!("[new session '{}' started]", meta.display_name());
        } else {
            println!("[new session started]");
        }
    }

    if cli.init {
        // Generate project overview using AI and exit
        let root = project_root.as_ref().ok_or_else(|| {
            anyhow::anyhow!("no project root detected (not in a git repo?)")
        })?;
        
        println!("[generating project overview with AI...]");
        println!("[this may take 10-30 seconds, please wait]");
        
        match ProjectOverview::generate_with_ai(root, agent.provider()).await {
            Ok(overview) => {
                println!("[saved overview to {}]", overview.path.display());
                println!("[done]");
            }
            Err(e) => {
                eprintln!("[error] could not generate overview: {e}");
            }
        }
        return Ok(());
    }

    if !cli.prompt.is_empty() {
        agent.chat_once(&cli.prompt.join(" ")).await?;
        
        // Record compression result if any
        if let Some(result) = agent.last_compression_result() {
            use matrixcode::compress::CompressionHistoryEntry;
            session_manager.record_compression(CompressionHistoryEntry::from_result(result));
        }
        
        // Update session stats and save
        let stats = agent.token_stats();
        session_manager.set_messages(agent.messages().to_vec());
        session_manager.update_stats(stats.last_input_tokens, stats.total_output_tokens);
        session_manager.save_current()?;
        return Ok(());
    }

    run_repl(&mut agent, &mut session_manager, project_root.as_deref()).await
}

// ============================================================================
// REPL Loop (Interactive Mode)
// ============================================================================

async fn run_repl(agent: &mut agent::Agent, session_manager: &mut SessionManager, project_root: Option<&Path>) -> Result<()> {
    // Print welcome banner
    ui::print_welcome();

    let session_name = session_manager.current_metadata()
        .map(|m| m.display_name())
        .unwrap_or_else(|| "new".to_string());
    println!("  Session: '{}'\n", session_name);
    println!("  Approve mode: {} (Shift+Tab / Alt+M or /mode to toggle)\n", agent.approve_mode());

    let mut rl = DefaultEditor::new()?;
    let history_path = session_manager.history_path();
    if history_path.exists() {
        let _ = rl.load_history(&history_path);
    }

    // Shared flag: when the mode-toggle hotkey is pressed, we set this to 1.
    // The REPL loop checks it after readline returns.
    let mode_toggle_flag = Arc::new(AtomicU8::new(0));
    let flag_clone = mode_toggle_flag.clone();

    // Bind hotkey to toggle approve mode.
    // We use a ConditionalEventHandler to set the flag and accept the line.
    struct ModeToggleHandler(Arc<AtomicU8>);
    impl rustyline::ConditionalEventHandler for ModeToggleHandler {
        fn handle(
            &self,
            _evt: &rustyline::Event,
            _n: rustyline::RepeatCount,
            _positive: bool,
            _ctx: &rustyline::EventContext,
        ) -> Option<Cmd> {
            self.0.store(1, Ordering::SeqCst);
            Some(Cmd::AcceptLine)
        }
    }

    // Bind Shift+Tab (BackTab) - primary hotkey
    rl.bind_sequence(
        KeyEvent(KeyCode::BackTab, Modifiers::NONE),
        EventHandler::Conditional(Box::new(ModeToggleHandler(flag_clone.clone()))),
    );

    // Also bind Alt+M as alternative (some terminals don't send BackTab properly)
    rl.bind_sequence(
        KeyEvent(KeyCode::Char('m'), Modifiers::ALT),
        EventHandler::Conditional(Box::new(ModeToggleHandler(flag_clone))),
    );

    loop {
        // Build prompt showing current approve mode
        let mode_indicator = match agent.approve_mode() {
            matrixcode::approval::ApproveMode::Ask => "[ask]",
            matrixcode::approval::ApproveMode::Auto => "[auto]",
            matrixcode::approval::ApproveMode::Strict => "[strict]",
        };
        let prompt = format!("\n{} > ", mode_indicator);

        let line = match rl.readline(&prompt) {
            Ok(l) => l,
            Err(ReadlineError::Interrupted) => {
                // Ctrl+C at the prompt: cancel current input, stay in REPL.
                continue;
            }
            Err(ReadlineError::Eof) => break, // Ctrl+D
            Err(e) => {
                eprintln!("input error: {e}");
                break;
            }
        };

        // Check if hotkey was pressed (mode toggle)
        if mode_toggle_flag.load(Ordering::SeqCst) == 1 {
            mode_toggle_flag.store(0, Ordering::SeqCst);
            agent.toggle_approve_mode();
            println!("[approve mode: {}]", agent.approve_mode());
            continue;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if matches!(trimmed, "/exit" | "/quit" | ":q") {
            break;
        }
        if trimmed == "/clear" {
            agent.clear_messages();
            session_manager.clear_current()?;
            session_manager.start_new(project_root)?;
            if let Some(meta) = session_manager.current_metadata() {
                println!("[context cleared, new session '{}' started]", meta.display_name());
            } else {
                println!("[context cleared, new session started]");
            }
            continue;
        }
        if trimmed == "/help" {
            ui::print_help();
            continue;
        }
        if trimmed == "/mode" {
            agent.toggle_approve_mode();
            println!("[approve mode: {}]", agent.approve_mode());
            continue;
        }
        if trimmed == "/memory" {
            handle_memory(agent, project_root);
            continue;
        }
        if trimmed.starts_with("/memory ") {
            let args = trimmed.trim_start_matches("/memory ");
            handle_memory_command(args, agent, project_root);
            continue;
        }
        if trimmed == "/status" {
            print_status(agent, session_manager);
            continue;
        }
        if trimmed == "/history" {
            print_history(agent);
            continue;
        }
        if trimmed == "/sessions" {
            list_sessions(session_manager);
            continue;
        }
        if trimmed == "/resume" {
            let picked = show_session_picker(session_manager)?;
            if let Some(id) = picked {
                session_manager.resume(&id, project_root)?;
                if let Some(messages) = session_manager.messages() {
                    agent.set_messages(messages.to_vec());
                }
                let name = session_manager.current_name()
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| session_manager.current_id().unwrap_or("unknown").to_string());
                println!("[resumed session '{}']", name);
            }
            continue;
        }
        if trimmed.starts_with("/resume ") {
            let query = trimmed.strip_prefix("/resume ").unwrap().trim();
            match session_manager.resume(query, project_root)? {
                Some(session) => {
                    agent.set_messages(session.messages.clone());
                    println!("[resumed session '{}']", session.metadata.display_name());
                }
                None => {
                    println!("[session '{}' not found]", query);
                }
            }
            continue;
        }
        if trimmed.starts_with("/rename ") {
            let new_name = trimmed.strip_prefix("/rename ").unwrap().trim();
            session_manager.rename_current(new_name)?;
            println!("[session renamed to '{}']", new_name);
            continue;
        }
        if trimmed == "/init" {
            handle_init(project_root, agent).await;
            continue;
        }
        if trimmed == "/overview" {
            handle_overview(project_root);
            continue;
        }
        if trimmed == "/compress" {
            // Default compression with balanced bias
            handle_compress(agent, session_manager, None);
            continue;
        }
        if trimmed.starts_with("/compress ") {
            let bias_spec = trimmed.strip_prefix("/compress ").unwrap().trim();
            handle_compress(agent, session_manager, Some(bias_spec));
            continue;
        }
        if trimmed == "/plan" {
            handle_plan(agent).await;
            continue;
        }
        if trimmed.starts_with("/plan ") {
            let plan_request = trimmed.strip_prefix("/plan ").unwrap().trim();
            handle_plan_with_request(agent, plan_request).await;
            continue;
        }
        if trimmed == "/models" {
            handle_models(agent);
            continue;
        }
        if trimmed == "/model" {
            handle_model(agent);
            continue;
        }
        if trimmed == "/skills" {
            handle_skills(agent);
            continue;
        }

        let _ = rl.add_history_entry(trimmed);

        // Create a fresh cancellation token for each chat session.
        // This ensures that cancelling one operation doesn't affect subsequent ones.
        let cancel_token = CancellationToken::new();
        let cancel_token_for_signal = cancel_token.clone();

        // Spawn a short-lived task to handle ESC for this specific chat.
        // We use JoinHandle to ensure the thread exits properly when the chat ends.
        let esc_thread: std::thread::JoinHandle<()> = std::thread::spawn({
            let cancel_token = cancel_token_for_signal.clone();
            move || {
                use crossterm::event::{Event, KeyCode, poll, read};
                
                loop {
                    // Poll for keyboard events
                    if poll(std::time::Duration::from_millis(100)).ok() == Some(true) {
                        if let Ok(Event::Key(key)) = read() {
                            if key.code == KeyCode::Esc {
                                cancel_token.cancel();
                                break;
                            }
                        }
                    }
                    // Check if cancelled (chat ended)
                    if cancel_token.is_cancelled() {
                        break;
                    }
                }
            }
        });

        // Spawn async task for Ctrl+C
        tokio::spawn({
            let cancel_token = cancel_token_for_signal.clone();
            async move {
                tokio::signal::ctrl_c().await.ok();
                cancel_token.cancel();
            }
        });

        agent.set_cancel_token(cancel_token.clone());

        // Show spinner while preparing memory context (covers the gap before chat_once)
        let mut prep_spinner = Some(matrixcode::tools::spinner::ToolSpinner::new("preparing"));

        // Update memory context based on current user input
        // Use AI-enhanced keyword extraction for better memory matching
        let memory_summary = load_contextual_memory_summary_async(
            project_root,
            trimmed,
            Some(agent.provider()),
        ).await;
        if let Some(summary) = memory_summary {
            agent.set_memory_summary(&summary);
        }

        // Clear preparation spinner before chat_once (which creates its own spinner)
        if let Some(mut sp) = prep_spinner.take() {
            sp.finish_clear();
        }

        if let Err(e) = agent.chat_once(trimmed).await {
            eprintln!("\n[error] {e}");
        } else {
            // Show API call count after successful chat
            let api_calls = agent.api_call_count();
            println!("\n✓ [API calls: {}]", api_calls);
        }

        // Cancel the token to signal background threads to stop
        cancel_token.cancel();
        agent.clear_cancel_token();

        // Wait for the ESC listener thread to exit (with timeout to avoid hanging)
        // This ensures the crossterm event listener is properly cleaned up
        let _ = esc_thread.join();

        // Record compression result to session if any
        if let Some(result) = agent.last_compression_result() {
            use matrixcode::compress::CompressionHistoryEntry;
            session_manager.record_compression(CompressionHistoryEntry::from_result(result));
        }

        // Update session and save
        let stats = agent.token_stats();
        session_manager.set_messages(agent.messages().to_vec());
        session_manager.update_stats(stats.last_input_tokens, stats.total_output_tokens);
        if let Err(e) = session_manager.save_current() {
            eprintln!("[warn] could not save session: {e}");
        }

        // Detect and save memories from conversation
        save_detected_memories_async(
            agent.messages(),
            project_root,
            session_manager.current_id(),
            Some(agent.provider()),
        ).await;
    }

    let _ = rl.save_history(&history_path);
    Ok(())
}

// ============================================================================
// Skills Loading
// ============================================================================

fn load_skills(extra: &[PathBuf], skip_defaults: bool) -> Vec<skills::Skill> {
    let mut roots: Vec<PathBuf> = Vec::new();
    roots.extend(extra.iter().cloned());
    if !skip_defaults {
        roots.push(PathBuf::from("skills"));
        if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
            let mut p = PathBuf::from(home);
            p.push(".matrix");
            p.push("skills");
            roots.push(p);
        }
    }
    let found = skills::discover_skills(&roots);
    if !found.is_empty() {
        let names: Vec<&str> = found.iter().map(|s| s.name.as_str()).collect();
        println!("[loaded {} skill(s): {}]", found.len(), names.join(", "));
    }
    found
}

// ============================================================================
// Session Management UI
// ============================================================================

/// Show interactive session picker.
fn show_session_picker(session_manager: &SessionManager) -> Result<Option<String>> {
    let sessions = session_manager.list_sessions();
    if sessions.is_empty() {
        println!("[no saved sessions]");
        return Ok(None);
    }

    println!("Saved sessions:");
    println!("  (enter number to resume, or press Enter to cancel)");
    println!();
    
    let current_id = session_manager.current_id();
    for (i, meta) in sessions.iter().enumerate() {
        let is_current = current_id == Some(meta.id.as_str());
        println!("  {}. {}", i + 1, meta.format_line(is_current));
    }
    println!();

    // Simple text input for selection
    println!("Select session (1-{}): ", sessions.len());
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let input = input.trim();
    
    if input.is_empty() {
        println!("[cancelled]");
        return Ok(None);
    }

    match input.parse::<usize>() {
        Ok(n) if n > 0 && n <= sessions.len() => {
            Ok(Some(sessions[n - 1].id.clone()))
        }
        _ => {
            // Try to match by name or ID prefix
            if let Some(meta) = sessions.iter().find(|s| 
                s.name.as_deref() == Some(input) || 
                s.id.starts_with(input) ||
                s.id == input
            ) {
                Ok(Some(meta.id.clone()))
            } else {
                println!("[invalid selection: {}]", input);
                Ok(None)
            }
        }
    }
}

/// List all sessions.
fn list_sessions(session_manager: &SessionManager) {
    let sessions = session_manager.list_sessions();
    if sessions.is_empty() {
        println!("[no saved sessions]");
        return;
    }

    println!("Saved sessions ({} total):", sessions.len());
    let current_id = session_manager.current_id();
    for meta in sessions {
        let is_current = current_id == Some(meta.id.as_str());
        println!("  {}", meta.format_line(is_current));
    }
}

// ============================================================================
// UI Display Functions
// ============================================================================

// ============================================================================
// Command Handlers
// ============================================================================

/// Print current session status.
fn print_status(agent: &agent::Agent, session_manager: &SessionManager) {
    let stats = agent.token_stats();
    
    // Show session info
    if let Some(meta) = session_manager.current_metadata() {
        println!("Session: '{}'", meta.display_name());
        println!("  ID: {}", meta.id);
        if let Some(ref project) = meta.project_path {
            println!("  Project: {}", project);
        }
        println!("  Created: {}", meta.created_at.format("%Y-%m-%d %H:%M"));
    } else {
        println!("Session: (new/unsaved)");
    }
    
    println!();
    println!("Conversation:");
    println!("  Messages: {}", agent.message_count());
    println!("  API calls: {}", agent.api_call_count());
    println!(
        "  Last input tokens: {}",
        ui::format_tokens(stats.last_input_tokens as u64)
    );
    println!(
        "  Total output tokens: {}",
        ui::format_tokens(stats.total_output_tokens)
    );
    if let Some(ctx) = stats.context_size {
        let used = stats.last_input_tokens;
        let pct = (used as f64 / ctx as f64 * 100.0).min(100.0);
        println!(
            "  Context window: {} / {} ({:.1}%)",
            ui::format_tokens(used as u64),
            ui::format_tokens(ctx as u64),
            pct
        );
    }
}

/// Print conversation history summary.
fn print_history(agent: &agent::Agent) {
    let messages = agent.messages();
    if messages.is_empty() {
        println!("[no conversation history]");
        return;
    }
    println!("Conversation history ({} messages):", messages.len());
    for (i, msg) in messages.iter().enumerate() {
        let role = match msg.role {
            providers::Role::User => "User",
            providers::Role::Assistant => "Assistant",
            providers::Role::Tool => "Tool",
            providers::Role::System => "System",
        };
        let preview = match &msg.content {
            providers::MessageContent::Text(t) => {
                let s = t.trim();
                let first_line = s.lines().next().unwrap_or("");
                ui::truncate_str(first_line, 60)
            }
            providers::MessageContent::Blocks(blocks) => {
                format!("[{} blocks]", blocks.len())
            }
        };
        println!("  {}. {}: {}", i + 1, role, preview);
    }
}

/// Handle /init command: generate/update project overview using AI.
async fn handle_init(project_root: Option<&Path>, agent: &mut agent::Agent) {
    let root = match project_root {
        Some(r) => r,
        None => {
            println!("[error] no project root detected (not in a git repo?)");
            return;
        }
    };

    println!("[generating project overview with AI...]");
    println!("[this may take 10-30 seconds, please wait]");
    
    // Get provider from agent
    match ProjectOverview::generate_with_ai(root, agent.provider()).await {
        Ok(overview) => {
            println!("[saved overview to {}]", overview.path.display());
            // Update agent's system prompt with new overview
            agent.set_project_overview(&overview.content);
            println!("[overview injected into context]");
        }
        Err(e) => {
            eprintln!("[error] could not generate overview: {e}");
        }
    }
}

/// Handle /overview command: show current overview status.
fn handle_overview(project_root: Option<&Path>) {
    let root = match project_root {
        Some(r) => r,
        None => {
            println!("[no project root detected]");
            return;
        }
    };

    if ProjectOverview::exists(root) {
        let path = ProjectOverview::path(root);
        println!("[overview exists at {}]", path.display());
        match ProjectOverview::load(root) {
            Ok(Some(overview)) => {
                println!("[content preview:]");
                for line in overview.content.lines().take(20) {
                    println!("  {}", line);
                }
                if overview.content.lines().count() > 20 {
                    println!("  ... (more lines)");
                }
            }
            Ok(None) => println!("[unexpected: file exists but load returned None]"),
            Err(e) => eprintln!("[error loading overview: {e}]"),
        }
    } else {
        println!("[no overview found. use /init to generate one]");
    }
}

/// Handle /compress command: manually compress context with optional bias.
fn handle_compress(
    agent: &mut agent::Agent,
    session_manager: &mut SessionManager,
    bias_spec: Option<&str>,
) {
    match agent.compress_with_bias(bias_spec) {
        Ok(Some(result)) => {
            // Record compression to session
            use matrixcode::compress::CompressionHistoryEntry;
            session_manager.record_compression(CompressionHistoryEntry::from_result(&result));
            
            // Update session messages
            session_manager.set_messages(agent.messages().to_vec());
            
            // Save session
            if let Err(e) = session_manager.save_current() {
                eprintln!("[warn] could not save session: {e}");
            }
        }
        Ok(None) => {
            println!("[compression skipped]");
        }
        Err(e) => {
            eprintln!("[error] {e}");
        }
    }
}

/// Handle /plan command: show or generate task plan.
async fn handle_plan(agent: &mut agent::Agent) {
    // Show last plan if available
    if let Some(plan) = agent.last_plan() {
        println!("[last plan]:\n{}", plan.format());
    } else {
        println!("[no plan available. use /plan <task> to generate one]");
    }
}

/// Handle /plan <task> command: generate plan for specified task.
async fn handle_plan_with_request(agent: &mut agent::Agent, request: &str) {
    match agent.plan_task(request).await {
        Ok(Some(plan)) => {
            println!("\n{}", plan.format());
            
            // Optionally convert to todo items
            let todos = plan.to_todo_items();
            if !todos.is_empty() {
                println!("\n[todo items generated: {} steps]", todos.len());
            }
        }
        Ok(None) => {
            println!("[planning not available - no plan model configured]");
            println!("[use --multi-model or --plan-model to enable planning]");
        }
        Err(e) => {
            eprintln!("[error] planning failed: {e}");
        }
    }
}

/// Handle /models command: show current model configuration.
fn handle_models(agent: &agent::Agent) {
    let config = agent.model_config();
    println!("[multi-model configuration]");
    println!("  main:     {} (context: {:?})", config.main.display_name(), config.main.context_size);
    println!("  plan:     {} (context: {:?})", config.plan.display_name(), config.plan.context_size);
    println!("  compress: {} (context: {:?})", config.compress.display_name(), config.compress.context_size);
    println!("  fast:     {} (context: {:?})", config.fast.display_name(), config.fast.context_size);
}

/// Handle /model command: show current model (simple view).
fn handle_model(agent: &agent::Agent) {
    let config = agent.model_config();
    println!("Current model: {}", config.main.display_name());
    if let Some(ctx) = config.main.context_size {
        println!("Context window: {} tokens", ctx);
    }
}

/// Handle /skills command: show loaded skills list.
fn handle_skills(agent: &agent::Agent) {
    let skills = agent.skills();
    if skills.is_empty() {
        println!("[no skills loaded]");
        println!();
        println!("Skills are loaded from:");
        println!("  ./skills/              (project skills)");
        println!("  ~/.matrix/skills/      (user skills)");
        println!();
        println!("Use --skills-dir <path> to add custom skill directories.");
        return;
    }
    
    println!("Loaded skills ({} total):", skills.len());
    println!();
    for skill in skills {
        println!("  {}", skill.name);
        // Truncate description to ~60 chars for display
        let desc = &skill.description;
        let desc_preview = if desc.len() > 60 {
            let mut end = 60;
            while end > 0 && !desc.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}...", &desc[..end])
        } else {
            desc.clone()
        };
        println!("    {}", desc_preview);
    }
}

/// Load accumulated memory and generate summary for system prompt injection.
/// Returns None if no memories exist or loading fails.
fn load_memory_summary(project_root: Option<&Path>) -> Option<String> {
    use matrixcode::memory::MemoryStorage;
    
    let storage = MemoryStorage::new(project_root).ok()?;
    let memory = storage.load_combined().ok()?;
    
    if memory.entries.is_empty() {
        return None;
    }
    
    // Generate summary with top 15 most important entries
    let summary = memory.generate_prompt_summary(15);
    if summary.is_empty() {
        None
    } else {
        println!("[loaded {} accumulated memories]", memory.entries.len());
        Some(summary)
    }
}

/// Load memory summary with AI-enhanced keyword extraction (async version).
/// Uses hybrid keyword extraction: rule-based first, AI fallback when needed.
async fn load_contextual_memory_summary_async(
    project_root: Option<&Path>,
    context: &str,
    fast_provider: Option<&dyn matrixcode::providers::Provider>,
) -> Option<String> {
    use matrixcode::memory::MemoryStorage;
    
    let storage = MemoryStorage::new(project_root).ok()?;
    let memory = storage.load_combined().ok()?;
    
    if memory.entries.is_empty() {
        return None;
    }
    
    // Use contextual summary if context is available
    let summary = if context.is_empty() {
        memory.generate_prompt_summary(15)
    } else {
        memory.generate_contextual_summary_async(context, 15, fast_provider).await
    };
    
    if summary.is_empty() {
        None
    } else {
        Some(summary)
    }
}

/// Save detected memories from conversation to storage.
/// Called after each conversation turn to accumulate knowledge.
/// Uses AI extraction if provider is available, falls back to rule-based detection.
/// 
/// Note: This async version is currently unused but kept for future integration
/// into the REPL loop when async memory detection is desired.
#[allow(dead_code)]
async fn save_detected_memories_async(
    messages: &[matrixcode::providers::Message],
    project_root: Option<&Path>,
    session_id: Option<&str>,
    fast_provider: Option<&dyn matrixcode::providers::Provider>,
) {
    use matrixcode::memory::{MemoryStorage, detect_memories_from_text, detect_memories_with_ai, AiMemoryExtractor, MemoryExtractor};
    
    // Extract from USER messages (primary source of meaningful content)
    let user_text: String = messages
        .iter()
        .filter(|msg| msg.role == matrixcode::providers::Role::User)
        .filter_map(|msg| {
            match &msg.content {
                matrixcode::providers::MessageContent::Text(t) => Some(t.clone()),
                matrixcode::providers::MessageContent::Blocks(blocks) => {
                    Some(blocks
                        .iter()
                        .filter_map(|b| {
                            if let matrixcode::providers::ContentBlock::Text { text } = b {
                                Some(text.clone())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n"))
                }
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    
    // Extract from assistant messages, but clean formatting artifacts
    let assistant_text: String = messages
        .iter()
        .filter(|msg| msg.role == matrixcode::providers::Role::Assistant)
        .filter_map(|msg| {
            match &msg.content {
                matrixcode::providers::MessageContent::Text(t) => Some(t.clone()),
                matrixcode::providers::MessageContent::Blocks(blocks) => {
                    Some(blocks
                        .iter()
                        .filter_map(|b| {
                            if let matrixcode::providers::ContentBlock::Text { text } = b {
                                // Skip short outputs (likely tool results)
                                if text.len() > 50 {
                                    Some(text.clone())
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n"))
                }
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    
    // Clean formatting artifacts from assistant text
    let cleaned_assistant = clean_memory_text(&assistant_text);
    
    // Combine user text + cleaned assistant text
    let combined_text = format!("{}\n\n{}", user_text, cleaned_assistant);
    
    // Skip if too short
    if combined_text.len() < 100 {
        return;
    }
    
    // Detect memories from conversation
    let new_entries = if let Some(_provider) = fast_provider {
        // Use AI extraction with fast model
        // Try to get API key from correct environment variable
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .or_else(|_| std::env::var("API_KEY"))
            .unwrap_or_default();
        
        if api_key.is_empty() {
            // No API key available, fallback to rule-based detection
            log::debug!("No API key for memory extraction, using rule-based detection");
            detect_memories_from_text(&combined_text, session_id)
        } else {
            let extractor = AiMemoryExtractor::new(
                Box::new(matrixcode::providers::anthropic::AnthropicProvider::new(
                    api_key,
                    matrixcode::memory::DEFAULT_MEMORY_EXTRACTOR_MODEL.to_string(),
                    std::env::var("ANTHROPIC_BASE_URL").unwrap_or_default(),
                )),
                matrixcode::memory::DEFAULT_MEMORY_EXTRACTOR_MODEL.to_string(),
            );
            
            log::debug!("Using AI memory extraction with model: {}", extractor.model_name());
            match detect_memories_with_ai(&combined_text, session_id, Some(&extractor)).await {
                Ok(entries) if !entries.is_empty() => {
                    log::debug!("AI extracted {} memories", entries.len());
                    entries
                },
                Ok(_) => {
                    // AI returned empty, try fallback
                    log::debug!("AI extraction returned empty, trying rule-based detection");
                    detect_memories_from_text(&combined_text, session_id)
                },
                Err(e) => {
                    // AI extraction failed, fallback with warning
                    log::warn!("AI memory extraction failed: {}, falling back to rule-based", e);
                    detect_memories_from_text(&combined_text, session_id)
                }
            }
        }
    } else {
        // Use rule-based detection
        log::debug!("No fast provider available, using rule-based memory detection");
        detect_memories_from_text(&combined_text, session_id)
    };
    
    if new_entries.is_empty() {
        return;
    }
    
    // Save to storage and update references
    if let Ok(mut storage) = MemoryStorage::new(project_root) {
        // Load existing memory to update references
        if let Ok(mut existing_memory) = storage.load_combined() {
            // Update reference counts for existing entries
            existing_memory.update_references(messages);
            
            // Save updated memory
            if let Err(e) = storage.save_global(&existing_memory) {
                eprintln!("[warn] could not update memory references: {}", e);
            }
        }
        
        // Add new entries
        let new_count = new_entries.len();
        for entry in new_entries {
            // Try to save as global memory first (default)
            if let Err(e) = storage.add_entry(entry, false) {
                eprintln!("[warn] could not save memory: {}", e);
            }
        }
        
        println!("[saved {} new memories]", new_count);
    }
}

/// Clean formatting artifacts from text before memory detection.
/// Removes emoji, table borders, markdown decorations, etc.
fn clean_memory_text(text: &str) -> String {
    text.lines()
        .filter(|line| {
            let trimmed = line.trim();
            // Skip empty lines
            if trimmed.is_empty() {
                return false;
            }
            // Skip lines that are mostly formatting
            if is_formatting_line(trimmed) {
                return false;
            }
            true
        })
        .map(|line| strip_formatting(line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Check if a line is primarily formatting (not content).
fn is_formatting_line(line: &str) -> bool {
    // Table borders
    if line.starts_with("├") || line.starts_with("└") || line.starts_with("│") || line.starts_with("┌") || line.starts_with("┐") || line.starts_with("─") {
        return true;
    }
    // Lines that are mostly special characters
    let special_count = line.chars().filter(|c| {
        matches!(c, '│' | '├' | '└' | '┌' | '┐' | '─' | '═' | '║' | '╔' | '╗' | '╚' | '╝' | '┬' | '┴' | '┼')
    }).count();
    if special_count > line.chars().count() / 3 {
        return true;
    }
    // Lines that start with emoji markers (likely formatted output, not user intent)
    if line.starts_with("🎯") || line.starts_with("🔧") || line.starts_with("💡") || 
       line.starts_with("📚") || line.starts_with("🏗") || line.starts_with("👤") ||
       line.starts_with("⭐") || line.starts_with("📝") {
        return true;
    }
    // Section headers from memory summary
    if line.contains("【自动记忆摘要】") || line.contains("[ACCUMULATED MEMORY]") {
        return true;
    }
    // Code block markers
    if line.starts_with("```") {
        return true;
    }
    false
}

/// Strip formatting characters from a line.
fn strip_formatting(line: &str) -> &str {
    let trimmed = line.trim();
    // Remove leading tree characters
    let stripped = trimmed
        .trim_start_matches("│")
        .trim_start_matches("├──")
        .trim_start_matches("└──")
        .trim_start_matches("├─")
        .trim_start_matches("└─")
        .trim();
    stripped
}

/// Handle /memory command: show accumulated memories.
fn handle_memory(_agent: &agent::Agent, project_root: Option<&Path>) {
    use matrixcode::memory::{MemoryStorage, MemoryCategory};
    
    // Load combined memory
    let storage = match MemoryStorage::new(project_root) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[error] could not initialize memory storage: {}", e);
            return;
        }
    };
    
    let memory = match storage.load_combined() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("[error] could not load memory: {}", e);
            return;
        }
    };
    
    if memory.entries.is_empty() {
        println!("[no memories accumulated]");
        println!();
        println!("Memory automatically accumulates as you work with Claude:");
        println!("  - User preferences");
        println!("  - Project decisions");
        println!("  - Key findings and solutions");
        println!();
        println!("Memory storage:");
        println!("  Global:  ~/.matrix/memory.json");
        if project_root.is_some() {
            println!("  Project: .matrix/memory.json");
        }
        println!();
        println!("Commands:");
        println!("  /memory                 - Show all memories");
        println!("  /memory add <content>   - Add manual memory");
        println!("  /memory clear           - Clear memories");
        println!("  /memory search <query>  - Search memories");
        println!("  /memory stats           - Show statistics");
        println!("  /memory prune           - Prune old memories");
        println!("  /memory export [file]   - Export to JSON");
        println!("  /memory import <file>   - Import from JSON");
        return;
    }
    
    println!("Accumulated memories ({} entries):", memory.entries.len());
    println!();
    
    // Group by category
    let mut by_cat: std::collections::HashMap<MemoryCategory, Vec<&matrixcode::memory::MemoryEntry>> = std::collections::HashMap::new();
    for entry in &memory.entries {
        by_cat.entry(entry.category).or_default().push(entry);
    }
    
    // Display each category
    for (cat, entries) in by_cat {
        println!("{} {} ({} entries):", cat.icon(), cat.display_name(), entries.len());
        for entry in entries {
            let importance_marker = if entry.importance >= 80.0 { " ⭐" } else { "" };
            let manual_marker = if entry.is_manual { " 📝" } else { "" };
            println!("  {}{}{}", entry.format_for_prompt(), importance_marker, manual_marker);
        }
        println!();
    }
    
    println!("Storage:");
    println!("  Global:  ~/.matrix/memory.json");
    if project_root.is_some() {
        println!("  Project: .matrix/memory.json");
    }
}

/// Handle /memory subcommands.
fn handle_memory_command(args: &str, _agent: &mut agent::Agent, project_root: Option<&Path>) {
    use matrixcode::memory::{MemoryStorage, MemoryCategory, MemoryEntry, AutoMemory};
    
    let args = args.trim();
    
    if args.starts_with("add ") {
        // Add manual memory
        let content = args.trim_start_matches("add ").trim();
        if content.is_empty() {
            eprintln!("[error] provide content to add");
            println!("Usage: /memory add <content>");
            return;
        }
        
        // Detect category from content
        let entries = matrixcode::memory::detect_memories_from_text(content, None);
        let entry = if entries.is_empty() {
            MemoryEntry::manual(MemoryCategory::Technical, content.to_string())
        } else {
            let mut e = entries.into_iter().next().unwrap();
            e.is_manual = true;
            e.importance = 95.0;
            e
        };
        
        let mut storage = match MemoryStorage::new(project_root) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[error] {}", e);
                return;
            }
        };
        
        // Store in global memory by default (simplified UX)
        // Users can manually move to project later if needed
        let is_project = false;
        
        match storage.add_entry(entry, is_project) {
            Ok(_) => println!("[memory added: {}]", crate::ui::truncate_str(content, 60)),
            Err(e) => eprintln!("[error] {}", e),
        }
    } else if args == "clear" || args == "clear all" {
        // Clear all memories (simplified UX)
        let mut storage = match MemoryStorage::new(project_root) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[error] {}", e);
                return;
            }
        };
        
        let mut cleared = 0;
        
        // Clear global memory
        let mut global = storage.load_global().unwrap_or_default();
        cleared += global.entries.len();
        global.clear();
        if let Err(e) = storage.save_global(&global) {
            eprintln!("[error saving global memory: {}]", e);
        }
        
        // Clear project memory if exists
        if let Some(mut project) = storage.load_project().unwrap_or_default() {
            cleared += project.entries.len();
            project.clear();
            if let Err(e) = storage.save_project(&project) {
                eprintln!("[error saving project memory: {}]", e);
            }
        }
        
        println!("[{} memory entries cleared]", cleared);
    } else if args == "clear global" {
        // Clear only global memory
        let mut storage = match MemoryStorage::new(project_root) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[error] {}", e);
                return;
            }
        };
        
        let mut memory = storage.load_global().unwrap_or_default();
        let cleared = memory.entries.len();
        memory.clear();
        if let Err(e) = storage.save_global(&memory) {
            eprintln!("[error saving global memory: {}]", e);
        }
        
        println!("[{} global memory entries cleared]", cleared);
    } else if args == "clear project" {
        // Clear only project memory
        if project_root.is_none() {
            eprintln!("[error] no project root");
            return;
        }
        
        let mut storage = match MemoryStorage::new(project_root) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[error] {}", e);
                return;
            }
        };
        
        if let Some(mut memory) = storage.load_project().unwrap_or_default() {
            let cleared = memory.entries.len();
            memory.clear();
            if let Err(e) = storage.save_project(&memory) {
                eprintln!("[error saving project memory: {}]", e);
            }
            println!("[{} project memory entries cleared]", cleared);
        } else {
            println!("[no project memory to clear]");
        }
    } else if args.starts_with("search ") {
        // Search memory
        let query = args.trim_start_matches("search ").trim();
        let storage = match MemoryStorage::new(project_root) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[error] {}", e);
                return;
            }
        };
        
        let memory = match storage.load_combined() {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[error] {}", e);
                return;
            }
        };
        
        let results = memory.search(query);
        if results.is_empty() {
            println!("[no memories matching '{}']", query);
        } else {
            println!("Found {} matching memories:", results.len());
            for entry in results {
                println!("  {}", entry.format_line());
            }
        }
    } else if args == "stats" || args == "statistics" {
        // Show memory statistics
        let storage = match MemoryStorage::new(project_root) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[error] {}", e);
                return;
            }
        };
        
        let memory = match storage.load_combined() {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[error] {}", e);
                return;
            }
        };
        
        let stats = memory.generate_statistics();
        println!("{}", stats.format_summary());
        
        println!("Storage locations:");
        println!("  Global:  ~/.matrix/memory.json");
        if project_root.is_some() {
            println!("  Project: .matrix/memory.json");
        }
    } else if args == "prune" {
        // Manually trigger pruning
        let mut storage = match MemoryStorage::new(project_root) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[error] {}", e);
                return;
            }
        };
        
        let mut memory = match storage.load_combined() {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[error] {}", e);
                return;
            }
        };
        
        let before = memory.entries.len();
        memory.prune();
        memory.apply_time_decay();
        let after = memory.entries.len();
        
        if let Err(e) = storage.save_global(&memory) {
            eprintln!("[error saving memory: {}", e);
        }
        
        println!("[pruned {} entries, {} remaining]", before - after, after);
    } else if args.starts_with("export") {
        // Export memories to file
        let export_path = args.trim_start_matches("export").trim();
        let export_path = if export_path.is_empty() {
            PathBuf::from("memories_export.json")
        } else {
            PathBuf::from(export_path)
        };
        
        let storage = match MemoryStorage::new(project_root) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[error] {}", e);
                return;
            }
        };
        
        let memory = match storage.load_combined() {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[error] {}", e);
                return;
            }
        };
        
        if memory.entries.is_empty() {
            println!("[no memories to export]");
            return;
        }
        
        let json = serde_json::to_string_pretty(&memory).unwrap_or_default();
        if let Err(e) = std::fs::write(&export_path, json) {
            eprintln!("[error exporting: {}", e);
        } else {
            println!("[exported {} memories to {}]", memory.entries.len(), export_path.display());
        }
    } else if args.starts_with("import") {
        // Import memories from file
        let import_path = args.trim_start_matches("import").trim();
        if import_path.is_empty() {
            println!("Usage: /memory import <file.json>");
            return;
        }
        
        let import_path = PathBuf::from(import_path);
        if !import_path.exists() {
            eprintln!("[error: file not found: {}]", import_path.display());
            return;
        }
        
        let json = match std::fs::read_to_string(&import_path) {
            Ok(j) => j,
            Err(e) => {
                eprintln!("[error reading file: {}", e);
                return;
            }
        };
        
        let imported: AutoMemory = match serde_json::from_str(&json) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[error parsing JSON: {}", e);
                return;
            }
        };
        
        let mut storage = match MemoryStorage::new(project_root) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[error] {}", e);
                return;
            }
        };
        
        let mut existing = storage.load_global().unwrap_or_default();
        let imported_count = imported.entries.len();
        
        for entry in imported.entries {
            if !existing.has_similar(&entry.content) {
                existing.add(entry);
            }
        }
        
        if let Err(e) = storage.save_global(&existing) {
            eprintln!("[error saving memory: {}", e);
        }
        
        println!("[imported {} memories, {} duplicates skipped]", 
            imported_count,
            imported_count - (existing.entries.len() - imported_count));
    } else if args == "config" || args == "config show" {
        // Show current config
        let storage = match MemoryStorage::new(project_root) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[error] {}", e);
                return;
            }
        };
        
        let config = storage.load_config().unwrap_or_default();
        println!("记忆系统配置：");
        println!("  max_entries:       {} 条", config.max_entries);
        println!("  min_importance:    {:.1} 分", config.min_importance);
        println!("  enabled:           {}", if config.enabled { "是" } else { "否" });
        println!("  decay_start_days:  {} 天", config.decay_start_days);
        println!("  decay_rate:        {:.2}", config.decay_rate);
        println!("  reference_increment: {:.1}", config.reference_increment);
        println!("  max_importance:    {:.1}", config.max_importance_ceiling);
        println!();
        println!("修改配置：");
        println!("  /memory config set <key> <value>");
        println!("  /memory config reset");
        println!("  /memory config minimal");
        println!("  /memory config archival");
    } else if args == "config reset" {
        // Reset config to default
        let mut storage = match MemoryStorage::new(project_root) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[error] {}", e);
                return;
            }
        };
        
        let config = matrixcode::memory::MemoryConfig::default();
        if let Err(e) = storage.save_config(&config) {
            eprintln!("[error saving config: {}]", e);
        } else {
            println!("[config reset to default]");
        }
    } else if args == "config minimal" {
        // Set minimal config
        let mut storage = match MemoryStorage::new(project_root) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[error] {}", e);
                return;
            }
        };
        
        let config = matrixcode::memory::MemoryConfig::minimal();
        if let Err(e) = storage.save_config(&config) {
            eprintln!("[error saving config: {}]", e);
        } else {
            println!("[config set to minimal (50 entries, 14-day decay)]");
        }
    } else if args == "config archival" {
        // Set archival config
        let mut storage = match MemoryStorage::new(project_root) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[error] {}", e);
                return;
            }
        };
        
        let config = matrixcode::memory::MemoryConfig::archival();
        if let Err(e) = storage.save_config(&config) {
            eprintln!("[error saving config: {}]", e);
        } else {
            println!("[config set to archival (500 entries, 90-day decay)]");
        }
    } else if args.starts_with("config set ") {
        // Set a specific config value
        let parts: Vec<&str> = args.trim_start_matches("config set ").splitn(2, ' ').collect();
        if parts.len() != 2 {
            println!("Usage: /memory config set <key> <value>");
            println!("Keys: max_entries, min_importance, decay_start_days, decay_rate, reference_increment");
            return;
        }
        
        let key = parts[0];
        let value = parts[1];
        
        let mut storage = match MemoryStorage::new(project_root) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[error] {}", e);
                return;
            }
        };
        
        let mut config = storage.load_config().unwrap_or_default();
        
        match key {
            "max_entries" => {
                if let Ok(v) = value.parse::<usize>() {
                    config.max_entries = v;
                } else {
                    eprintln!("[error: invalid value for max_entries]");
                    return;
                }
            }
            "min_importance" => {
                if let Ok(v) = value.parse::<f64>() {
                    config.min_importance = v.clamp(0.0, 100.0);
                } else {
                    eprintln!("[error: invalid value for min_importance]");
                    return;
                }
            }
            "decay_start_days" => {
                if let Ok(v) = value.parse::<i64>() {
                    config.decay_start_days = v.max(1);
                } else {
                    eprintln!("[error: invalid value for decay_start_days]");
                    return;
                }
            }
            "decay_rate" => {
                if let Ok(v) = value.parse::<f64>() {
                    config.decay_rate = v.clamp(0.1, 0.9);
                } else {
                    eprintln!("[error: invalid value for decay_rate]");
                    return;
                }
            }
            "reference_increment" => {
                if let Ok(v) = value.parse::<f64>() {
                    config.reference_increment = v.clamp(0.1, 10.0);
                } else {
                    eprintln!("[error: invalid value for reference_increment]");
                    return;
                }
            }
            "enabled" => {
                config.enabled = value == "true" || value == "1" || value == "yes";
            }
            _ => {
                eprintln!("[error: unknown config key '{}']", key);
                println!("Valid keys: max_entries, min_importance, decay_start_days, decay_rate, reference_increment, enabled");
                return;
            }
        }
        
        if let Err(e) = storage.save_config(&config) {
            eprintln!("[error saving config: {}]", e);
        } else {
            println!("[config updated: {} = {}]", key, value);
        }
    } else if args.starts_with("semantic ") {
        // Semantic search using TF-IDF
        let query = args.trim_start_matches("semantic ").trim();
        if query.is_empty() {
            println!("Usage: /memory semantic <query>");
            return;
        }
        
        let storage = match MemoryStorage::new(project_root) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[error] {}", e);
                return;
            }
        };
        
        let memory = match storage.load_combined() {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[error] {}", e);
                return;
            }
        };
        
        if memory.entries.is_empty() {
            println!("[no memories to search]");
            return;
        }
        
        // Build TF-IDF index and search
        let mut tfidf = matrixcode::memory::TfIdfSearch::new();
        tfidf.index(&memory);
        
        let results = tfidf.search(query, Some(10));
        
        if results.is_empty() {
            println!("[no semantically similar memories found for '{}']", query);
        } else {
            println!("Semantic search results for '{}':", query);
            println!();
            for (i, (content, score)) in results.iter().enumerate() {
                let truncated = crate::ui::truncate_str(content, 70);
                println!("  {}. [{:.2}] {}", i + 1, score, truncated);
            }
        }
    } else {
        println!("Unknown memory command: {}", args);
        println!("Available commands:");
        println!("  /memory                 - Show all memories");
        println!("  /memory add <content>   - Add manual memory");
        println!("  /memory clear           - Clear memories");
        println!("  /memory search <query>  - Search memories");
        println!("  /memory stats           - Show statistics");
        println!("  /memory prune           - Prune old memories");
        println!("  /memory export [file]   - Export to JSON");
        println!("  /memory import <file>   - Import from JSON");
        println!("  /memory config          - Show/edit config");
        println!("  /memory semantic <query> - Semantic search (TF-IDF)");
    }
}
