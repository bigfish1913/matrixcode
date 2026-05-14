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
    workspace::Workspace,
};
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use std::path::{Path, PathBuf};

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

    /// Render assistant output as Markdown in the terminal (default on).
    /// Pass --markdown false to disable, or set NO_COLOR / run in a non-TTY
    /// to auto-disable.
    #[arg(long, env = "MARKDOWN", default_value_t = true, action = clap::ArgAction::Set)]
    markdown: bool,

    /// Continue the last session (most common resume case).
    /// Equivalent to --resume with the most recently used session.
    #[arg(short = 'C', long)]
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
}

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

    let mut agent = agent::Agent::with_profile_and_skills_and_max_tokens_and_overview(
        provider,
        cli.think,
        cli.markdown,
        profile,
        load_skills(&cli.skills_dir, cli.no_default_skills),
        cli.max_tokens,
        overview.as_ref().map(|o| o.content.as_str()),
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

    // Initialize session manager
    let mut session_manager = SessionManager::new()?;
    
    // Handle --list-sessions
    if cli.list_sessions {
        list_sessions(&session_manager);
        return Ok(());
    }

    // Handle session resumption
    let session_to_load = if cli.continue_ {
        // --continue: load last session
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
        println!("[new session started]");
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

async fn run_repl(agent: &mut agent::Agent, session_manager: &mut SessionManager, project_root: Option<&Path>) -> Result<()> {
    let session_name = session_manager.current_name()
        .map(|n| n.to_string())
        .unwrap_or_else(|| {
            session_manager.current_id()
                .map(|id| format!("session-{}", &id[..8]))
                .unwrap_or_else(|| "new".to_string())
        });
    println!("matrixcode — session: '{}' | /help for commands. | ESC to interrupt output.", session_name);

    let mut rl = DefaultEditor::new()?;
    let history_path = session_manager.history_path();
    if history_path.exists() {
        let _ = rl.load_history(&history_path);
    }

    // Create cancellation token for ESC key interrupt
    let cancel_token = CancellationToken::new();
    let cancel_token_clone = cancel_token.clone();
    
    // Start ESC key listener thread
    let _esc_thread = std::thread::spawn(move || {
        use std::io::{stdin, Read};
        let mut stdin = stdin();
        let mut buf = [0u8; 1];
        
        loop {
            // Read single byte
            if stdin.read_exact(&mut buf).is_ok() {
                // ESC key sends 27 (ASCII)
                if buf[0] == 27 {
                    cancel_token_clone.cancel();
                }
            }
        }
    });

    loop {
        let line = match rl.readline("\n> ") {
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
            println!("[context cleared, new session started]");
            continue;
        }
        if trimmed == "/help" {
            print_help();
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

        let _ = rl.add_history_entry(trimmed);

        // Set cancel token before chat
        agent.set_cancel_token(cancel_token.clone());
        
        if let Err(e) = agent.chat_once(trimmed).await {
            eprintln!("\n[error] {e}");
        }
        
        // Clear cancel token after chat
        agent.clear_cancel_token();

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
    }

    let _ = rl.save_history(&history_path);
    Ok(())
}

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

/// Print available commands and usage.
fn print_help() {
    println!("Available commands:");
    println!("  /help       - Show this help message");
    println!("  /status     - Show session status (messages, token usage)");
    println!("  /history    - Show conversation history summary");
    println!("  /sessions   - List all saved sessions");
    println!("  /resume     - Show session picker to resume a session");
    println!("  /resume <id> - Resume a specific session by ID or name");
    println!("  /rename <name> - Give the current session a name");
    println!("  /init       - Generate/update project overview");
    println!("  /overview   - Show current project overview status");
    println!("  /plan       - Plan the current task (show last plan or new plan)");
    println!("  /plan <task> - Generate a plan for the specified task");
    println!("  /models     - Show current multi-model configuration");
    println!("  /compress   - Manually compress context (balanced bias)");
    println!("  /compress <bias> - Compress with specific bias:");
    println!("      balanced     - Balanced preservation (default)");
    println!("      important    - Preserve tools, thinking, decisions");
    println!("      tools        - Focus on preserving tool operations");
    println!("      aggressive   - Remove as much as possible");
    println!("      preserve:tools,thinking keywords:决定,重要");
    println!("      preserve:tools,thinking,user keywords:决定,重要");
    println!("  /clear      - Clear context and start a new session");
    println!("  /exit       - Exit the REPL (also /quit or :q)");
    println!();
    println!("Keyboard shortcuts:");
    println!("  ESC         - Interrupt current output");
    println!("  Ctrl+C      - Cancel current input");
    println!("  Ctrl+D      - Exit the REPL");
}

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
    println!(
        "  Last input tokens: {}",
        format_tokens(stats.last_input_tokens as u64)
    );
    println!(
        "  Total output tokens: {}",
        format_tokens(stats.total_output_tokens)
    );
    if let Some(ctx) = stats.context_size {
        let used = stats.last_input_tokens;
        let pct = (used as f64 / ctx as f64 * 100.0).min(100.0);
        println!(
            "  Context window: {} / {} ({:.1}%)",
            format_tokens(used as u64),
            format_tokens(ctx as u64),
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
                truncate_str(first_line, 60)
            }
            providers::MessageContent::Blocks(blocks) => {
                format!("[{} blocks]", blocks.len())
            }
        };
        println!("  {}. {}: {}", i + 1, role, preview);
    }
}

/// Truncate a string for display.
fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let mut end = max;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &s[..end])
    }
}

/// Format a number of tokens for display.
fn format_tokens(n: u64) -> String {
    if n < 1_000 {
        n.to_string()
    } else if n < 1_000_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{:.2}M", n as f64 / 1_000_000.0)
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
