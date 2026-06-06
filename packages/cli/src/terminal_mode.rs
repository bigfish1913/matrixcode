//! Terminal mode implementation for MatrixCode CLI
//!
//! Contains the TUI-based terminal mode with agent task handling.

use anyhow::Result;
use matrixcode_core::{
    AgentEvent, Config, SessionManager, agent::AgentBuilder, cancel::CancellationToken,
    create_provider_with_headers, infer_provider_type, providers::Provider,
    tools::all_tools_full, approval::ApproveMode, prompt::preprocess_with_skills, prompt::ProcessResult,
};
use matrixcode_tui::{TuiApp, restore_terminal, setup_terminal};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::commands::{handle_init_command, InitCommandResult};
use crate::constants::{
    DEFAULT_MAX_TOKENS, EVENT_CHANNEL_BUFFER, TASK_CHANNEL_BUFFER, ASK_CHANNEL_BUFFER,
    CLEANUP_TIMEOUT_MS, SESSION_CLEANUP_DAYS, DISPLAY_SESSIONS_LIMIT,
    MEMORY_MANIFEST_SIZE, MEMORY_SUMMARY_SIZE, MEMORY_INITIAL_SUMMARY_SIZE,
    MEMORY_TURN_CLEANUP_INTERVAL, MEMORY_EXTRACTION_INTERVAL, MEMORY_MIN_ENTRIES_FOR_AI_SELECTION,
    DISPLAY_OVERVIEW_CHARS_LIMIT, DISPLAY_MEMORY_SEARCH_LIMIT,
};
use crate::helpers::{resolve_provider, resolve_model, resolve_base_url, load_skills};
use crate::types::Cli;

/// Interactive session resume - list sessions and let user select
pub fn interactive_resume() -> Result<()> {
    use std::io::{self, Write};

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

    println!("\nSelect session to resume (1-{}), or 'q' to quit:", sessions.len());
    print!("> ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let selection = input.trim().to_string();

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
        println!("  Project: {}", session.project_path.as_deref().unwrap_or("unknown"));
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
            max_tokens: DEFAULT_MAX_TOKENS,
            mcp: Vec::new(),
            no_mcp: false,
            command: None,
        };
        return run_terminal_mode(cli);
    }

    // Try to match by short_id or full id
    for session in sessions.iter() {
        if session.short_id() == selection || session.id == selection || session.id.starts_with(&selection) {
            println!("\n✓ Resuming session: {}", session.short_id());
            println!("  Project: {}", session.project_path.as_deref().unwrap_or("unknown"));
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
                max_tokens: DEFAULT_MAX_TOKENS,
                mcp: Vec::new(),
                no_mcp: false,
                command: None,
            };
            return run_terminal_mode(cli);
        }
    }

    println!("Unknown session: {}", selection);
    Ok(())
}

/// List sessions
pub fn list_sessions() {
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
                println!("  {}. {} ({}){}", i + 1, session.short_id(), project, status);
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
pub fn run_terminal_mode(cli: Cli) -> Result<()> {
    // Set panic hook to restore terminal state before crashing
    std::panic::set_hook(Box::new(|info| {
        // Try to restore terminal state
        let _ = matrixcode_tui::crossterm::terminal::disable_raw_mode();
        let _ = matrixcode_tui::crossterm::execute!(
            std::io::stdout(),
            matrixcode_tui::crossterm::event::DisableBracketedPaste,
            matrixcode_tui::crossterm::cursor::Show
        );
        // Print panic message manually
        eprintln!("\n\n{}", info);
    }));

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
        crate::commands::handle_command(cmd, &skills);
        return Ok(());
    }

    // Setup tokio runtime
    let rt = tokio::runtime::Runtime::new()?;

    // Create channels for Agent communication
    let (event_tx, event_rx) = tokio::sync::mpsc::channel(EVENT_CHANNEL_BUFFER);
    let (task_tx, task_rx) = tokio::sync::mpsc::channel::<String>(TASK_CHANNEL_BUFFER);
    let (ask_tx, ask_rx) = tokio::sync::mpsc::channel::<String>(ASK_CHANNEL_BUFFER);
    // Channel for real-time appended messages during processing
    let (pending_input_tx, pending_input_rx) = tokio::sync::mpsc::channel::<String>(TASK_CHANNEL_BUFFER);

    // Set debug event sender for TUI debug panel
    matrixcode_core::set_debug_event_sender(event_tx.clone());

    // Create cancellation token
    let cancel_token = CancellationToken::new();

    // Load session BEFORE spawning agent task so TUI can also display restored messages
    let current_dir = std::env::current_dir().ok();
    let (full_messages, api_messages, session_mgr_state, session_metadata, effective_project_path) = {
        let mut mgr = SessionManager::new().ok();
        let mut full = Vec::new();
        let mut api = Vec::new();
        let mut metadata = None;
        // Start with current dir, then find project root
        let mut effective_path = current_dir.clone();

        if let Some(ref mut mgr) = mgr {
            if cli.continue_session || cli.resume_id.is_some() {
                let session = if let Some(ref query) = cli.resume_id {
                    mgr.resume(query).ok().flatten()
                } else {
                    mgr.continue_last().ok().flatten()
                };

                if let Some(s) = session {
                    log::info!(
                        "Session restored: full_messages={}, compressed_messages={}, display_messages={}",
                        s.full_messages.len(),
                        s.compressed_messages.len(),
                        s.display_messages().len()
                    );

                    full = s.full_messages.clone();
                    api = s.api_messages().to_vec();
                    metadata = Some(s.metadata.clone());

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

        // Find the true project root (git root or project marker files)
        if let Some(ref start_path) = effective_path {
            use matrixcode_core::tools::codegraph::find_project_root;
            let project_root = find_project_root(start_path);
            log::info!("Project root detected: {} (from start path: {})", project_root.display(), start_path.display());
            effective_path = Some(project_root);
        }

        (full, api, mgr, metadata, effective_path)
    };

    // Clone things needed in the agent task
    let agent_cancel = cancel_token.clone();
    let agent_event_tx = event_tx.clone();
    let agent_api_key = api_key.clone();
    let agent_model = model.clone();
    let agent_base_url = base_url.clone();
    let agent_think = cli.think.unwrap_or(config.think);
    let agent_max_tokens = cli.max_tokens;
    let agent_restored_messages = api_messages.clone();
    let agent_project_path = effective_project_path.clone();
    let agent_approve_mode = config
        .approve_mode
        .as_ref()
        .map(|m| ApproveMode::parse(m))
        .unwrap_or(ApproveMode::Auto);

    let agent_provider = resolve_provider(&config, &agent_model);

    let shared_approve_mode =
        std::sync::Arc::new(std::sync::atomic::AtomicU8::new(agent_approve_mode.to_u8()));

    let agent_fast_model = config
        .fast_model
        .clone()
        .or_else(|| std::env::var("ANTHROPIC_DEFAULT_HAIKU_MODEL").ok());

    let agent_extra_headers = config.extra_headers.clone();
    let agent_config = config.clone();
    let agent_skills = skills.clone();
    let agent_shared_approve_mode = shared_approve_mode.clone();

    // Prepare MCP servers configuration
    let agent_mcp_servers = crate::helpers::prepare_mcp_tools(
        &cli.mcp,
        cli.no_mcp,
        effective_project_path.as_ref(),
    );

    // Enter runtime context BEFORE spawning agent task
    let _guard = rt.enter();

    // Create shared watcher handle for dynamic watcher management
    let watcher_handle_arc = Arc::new(Mutex::new(None::<tokio::task::JoinHandle<()>>));

    // Start CodeGraph watcher for auto-sync (with hidden window on Windows)
    if let Some(path) = &effective_project_path {
        use matrixcode_core::tools::codegraph::CodeGraphWatcher;

        // Check if CodeGraph MCP daemon is already running
        // Multiple detection methods: daemon.pid, daemon.log active, or named pipe
        let daemon_running = {
            let mut running = false;

            // Method 1: Check daemon.pid file
            let daemon_pid_path = path.join(".codegraph").join("daemon.pid");
            if daemon_pid_path.exists() {
                running = std::fs::read_to_string(&daemon_pid_path)
                    .ok()
                    .and_then(|pid| pid.trim().parse::<u32>().ok())
                    .map(|pid| {
                        #[cfg(target_os = "windows")]
                        {
                            use std::os::windows::process::CommandExt;
                            const CREATE_NO_WINDOW: u32 = 0x08000000;
                            std::process::Command::new("tasklist")
                                .args(["/FI", &format!("PID eq {}", pid)])
                                .creation_flags(CREATE_NO_WINDOW)
                                .output()
                                .map(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()))
                                .unwrap_or(false)
                        }
                        #[cfg(not(target_os = "windows"))]
                        std::path::Path::new("/proc").join(pid.to_string()).exists()
                    })
                    .unwrap_or(false);
            }

            // Method 2: Check daemon.log for recent activity (last 30 seconds)
            if !running {
                let daemon_log_path = path.join(".codegraph").join("daemon.log");
                if daemon_log_path.exists() {
                    // Check if log was modified recently (daemon is active)
                    if let Ok(metadata) = std::fs::metadata(&daemon_log_path) {
                        if let Ok(modified) = metadata.modified() {
                            let now = std::time::SystemTime::now();
                            let elapsed = now.duration_since(modified).unwrap_or(std::time::Duration::MAX);
                            if elapsed < std::time::Duration::from_secs(60) {
                                matrixcode_core::debug::debug_log().log("codegraph", "daemon.log recently modified, daemon likely active");
                                running = true;
                            }
                        }
                    }
                }
            }

            running
        };

        if daemon_running {
            matrixcode_core::debug::debug_log().log("codegraph", "MCP daemon detected, skipping our watcher to avoid conflict");

            // Send initial status to TUI immediately (pending = 0, daemon handles sync)
            {
                use matrixcode_core::tools::codegraph::CodeGraphManager;
                use matrixcode_core::event::AgentEvent;
                use matrixcode_core::tools::codegraph::types::PendingChanges;
                let manager = CodeGraphManager::new(path);
                if manager.is_initialized() {
                    if let Ok(mut status) = manager.status() {
                        // Daemon handles sync automatically, so pending is always 0
                        status.pending_changes = PendingChanges { added: 0, modified: 0, removed: 0 };
                        matrixcode_core::debug::debug_log().log("codegraph", &format!(
                            "initial status: daemon running, nodes={}",
                            status.node_count
                        ));
                        let _ = agent_event_tx.send(AgentEvent::codegraph_status(status)).await;
                    }
                }
            }

            // When daemon is running, start a background task to poll status for TUI
            let status_event_tx = agent_event_tx.clone();
            let status_project_path = path.clone();
            let status_cancel = cancel_token.clone();
            tokio::spawn(async move {
                use matrixcode_core::tools::codegraph::CodeGraphManager;
                use matrixcode_core::event::AgentEvent;
                use matrixcode_core::tools::codegraph::types::PendingChanges;

                let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
                loop {
                    if status_cancel.is_cancelled() {
                        break;
                    }
                    interval.tick().await;

                    // Query status from CodeGraph index (pending = 0, daemon handles sync)
                    let manager = CodeGraphManager::new(&status_project_path);
                    if manager.is_initialized() {
                        if let Ok(mut status) = manager.status() {
                            status.pending_changes = PendingChanges { added: 0, modified: 0, removed: 0 };
                            matrixcode_core::debug::debug_log().log("codegraph", &format!(
                                "daemon status poll: nodes={}",
                                status.node_count
                            ));
                            let _ = status_event_tx.send(AgentEvent::codegraph_status(status)).await;
                        }
                    }
                }
            });
        } else {
            let watcher = CodeGraphWatcher::with_auto_detect(path.as_path());
            let handle = watcher.start_with_status_updates(cancel_token.clone(), agent_event_tx.clone());
            matrixcode_core::debug::debug_log().log("codegraph", "watcher started with status updates (no MCP daemon detected)");
            *watcher_handle_arc.lock().unwrap() = Some(handle);
        }
    }

    let watcher_handle_for_agent = watcher_handle_arc.clone();

    // Spawn Agent task (after entering runtime context and creating watcher)
    let agent_task = rt.spawn(async move {
        run_agent_task(
            agent_cancel,
            agent_event_tx,
            agent_api_key,
            agent_model,
            agent_base_url,
            agent_think,
            agent_max_tokens,
            agent_restored_messages,
            agent_project_path,
            agent_approve_mode,
            agent_provider,
            agent_fast_model,
            agent_extra_headers,
            agent_config,
            agent_skills,
            agent_shared_approve_mode,
            session_mgr_state,
            task_rx,
            ask_rx,
            watcher_handle_for_agent,
            agent_mcp_servers,
        ).await;
    });

    // Debug mode
    let debug_mode = std::env::var("MATRIXCODE_DEBUG")
        .map(|v| v == "1" || v == "true" || v == "verbose")
        .unwrap_or(cfg!(debug_assertions));

    // Enable debug logging if debug mode is on
    // Use session ID if available, otherwise generate one
    if debug_mode {
        let session_id = session_metadata.as_ref().map(|m| m.id.as_str());
        matrixcode_core::debug::enable_debug_logging(session_id);
        log::info!("Debug logging enabled for session: {:?}", session_id);
    }
    // Setup terminal for TUI
    let mut terminal = setup_terminal()?;

    // Create App and run it
    let mut app = TuiApp::new(task_tx, event_rx, cancel_token.clone())
        .with_ask_channel(ask_tx)
        .with_shared_approve_mode(shared_approve_mode)
        .with_pending_input_tx(pending_input_tx)
        .with_config(
            &model,
            cli.think.unwrap_or(config.think),
            cli.max_tokens,
            config.context_size.map(u64::from),
            config.approve_mode.clone(),
        )
        .with_debug_mode(debug_mode);

    // Load restored messages if any
    if !full_messages.is_empty() {
        app.load_messages(full_messages);
        if let Some(ref meta) = session_metadata {
            app.set_token_stats(
                meta.last_input_tokens,
                meta.total_output_tokens,
                meta.message_count,
            );
        }
    }
    let result = app.run(&mut terminal);

    // Restore terminal
    restore_terminal()?;

    // Cleanup: cancel agent task
    cancel_token.cancel();
    let cleanup_result = rt.block_on(async {
        tokio::time::timeout(tokio::time::Duration::from_millis(CLEANUP_TIMEOUT_MS), async {
            tokio::time::sleep(tokio::time::Duration::from_millis(CLEANUP_TIMEOUT_MS)).await;
        })
        .await
    });

    if cleanup_result.is_err() {
        agent_task.abort();
    } else {
        std::mem::drop(agent_task);
    }

    // Cleanup: abort CodeGraph watcher if still running
    {
        let handle = watcher_handle_arc.lock().unwrap();
        if let Some(ref h) = *handle
            && !h.is_finished() {
            log::info!("Aborting CodeGraph watcher...");
            h.abort();
        }
    }

    result
}

/// Run the agent task (async portion)
#[allow(clippy::too_many_arguments)]
async fn run_agent_task(
    cancel_token: CancellationToken,
    event_tx: tokio::sync::mpsc::Sender<AgentEvent>,
    api_key: String,
    model: String,
    base_url: String,
    think: bool,
    max_tokens: u32,
    restored_messages: Vec<matrixcode_core::providers::Message>,
    project_path: Option<PathBuf>,
    approve_mode: ApproveMode,
    provider_type: matrixcode_core::providers::ProviderType,
    fast_model: Option<String>,
    extra_headers: Option<std::collections::HashMap<String, String>>,
    config: Config,
    skills: Vec<matrixcode_core::skills::Skill>,
    shared_approve_mode: Arc<std::sync::atomic::AtomicU8>,
    mut session_mgr: Option<SessionManager>,
    mut task_rx: tokio::sync::mpsc::Receiver<String>,
    ask_rx: tokio::sync::mpsc::Receiver<String>,
    watcher_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    mcp_servers: Vec<(String, matrixcode_core::mcp::McpServerConfig)>,
) {
    log::info!("Agent task: starting");

    // Send skills loaded event
    let skill_names: Vec<String> = skills.iter().map(|s| s.name.clone()).collect();
    if !skill_names.is_empty() {
        let _ = event_tx.send(AgentEvent::skills_loaded(skill_names)).await;
    }

    // Send workflows loaded event
    use matrixcode_core::workflow::WorkflowRegistry;
    let registry = WorkflowRegistry::new(project_path.as_ref());
    let workflow_names: Vec<String> = registry.list().iter().map(|w| w.name.clone()).collect();
    if !workflow_names.is_empty() {
        let _ = event_tx.send(AgentEvent::workflows_loaded(workflow_names)).await;
    }

    // Create provider
    let provider = match create_provider_with_headers(
        provider_type,
        api_key.clone(),
        model.clone(),
        Some(base_url.clone()),
        extra_headers.clone(),
    ) {
        Ok(p) => p,
        Err(e) => {
            let _ = event_tx.send(AgentEvent::error(
                format!("Failed to create provider: {}", e),
                Some("provider_error".to_string()),
                None,
            )).await;
            return;
        }
    };

    // Create fast provider for keyword extraction
    let fast_provider: Option<Box<dyn Provider>> = fast_model.as_ref().and_then(|fm| {
        let fast_type = infer_provider_type(fm);
        create_provider_with_headers(
            fast_type,
            api_key.clone(),
            fm.clone(),
            Some(base_url.clone()),
            extra_headers.clone(),
        ).ok()
    });

    // Load memory
    let project_path_ref = project_path.as_deref();
    let mut memory_storage = matrixcode_core::memory::MemoryStorage::new(project_path_ref).ok();
    let memory = memory_storage.as_ref()
        .and_then(|ms| ms.load_combined().ok());

    // Send MemoryLoaded event
    if let Some(ref mem) = memory
        && !mem.entries.is_empty() {
        let _ = event_tx.send(AgentEvent::with_data(
            matrixcode_core::EventType::MemoryLoaded,
            matrixcode_core::EventData::Memory {
                summary: mem.generate_prompt_summary(MEMORY_INITIAL_SUMMARY_SIZE),
                entries_count: mem.entries.len(),
            },
        )).await;
    }

    let initial_memory_summary = memory.as_ref()
        .map(|mem| mem.generate_prompt_summary(MEMORY_SUMMARY_SIZE))
        .unwrap_or_default();

    // Load project overview
    let project_overview = project_path_ref
        .and_then(|path| matrixcode_core::overview::ProjectOverview::load(path).ok().flatten());

    if let Some(ref overview) = project_overview {
        matrixcode_core::debug::debug_log().log("overview", &format!("Loaded project overview: {} chars", overview.content.len()));
    }

    // Build system prompt
    let system_prompt = matrixcode_core::prompt::build_system_prompt_with_workflows(
        &matrixcode_core::prompt::PromptProfile::Default,
        &skills,
        project_overview.as_ref().map(|o| o.content.as_str()),
        if initial_memory_summary.is_empty() { None } else { Some(&initial_memory_summary) },
        project_path.as_ref(),
        None, // LSP servers will be injected dynamically when available
    );

    // Create MCP Tool Registry for unified management
    let mcp_registry = Arc::new(tokio::sync::RwLock::new(matrixcode_core::mcp::McpToolRegistry::new()));
    
    // Add MCP servers to registry
    {
        let mut registry = mcp_registry.write().await;
        for (name, server_config) in mcp_servers {
            registry.add_server(name.clone(), server_config);
            log::info!("MCP server '{}' added to registry", name);
        }
    }
    
    // Start all MCP servers and collect tools
    let mut mcp_tools: Vec<Box<dyn matrixcode_core::tools::Tool>> = Vec::new();
    {
        let registry = mcp_registry.read().await;
        match registry.start_all().await {
            Ok(server_tools) => {
                for (name, tools) in server_tools {
                    log::info!("Connected to '{}' with {} tools", name, tools.len());
                    
                    // Send MCP server added event
                    let _ = event_tx.send(AgentEvent::mcp_server_added(
                        name.clone(),
                        tools.len(),
                    )).await;
                    
                    // Convert Arc<McpToolWrapper> to Box<dyn Tool>
                    for tool in tools {
                        mcp_tools.push(Box::new((*tool).clone()));
                    }
                }
                
                // Send overall MCP status after all servers started
                let statuses = registry.server_status().await;
                let mcp_infos: Vec<matrixcode_core::event::McpServerInfo> = statuses
                    .iter()
                    .map(|(_, s)| matrixcode_core::event::McpServerInfo::from_status(s))
                    .collect();
                let _ = event_tx.send(AgentEvent::mcp_server_status(mcp_infos)).await;
            }
            Err(e) => {
                log::error!("Failed to start MCP servers: {}", e);
                let _ = event_tx.send(AgentEvent::error(
                    format!("MCP 服务器启动失败: {}", e),
                    Some("mcp_error".to_string()),
                    None,
                )).await;
            }
        }
    }

    // Build agent with CodeGraph tools
    let project_path_for_tools = project_path.clone().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let mut base_tools = all_tools_full(
        Arc::new(skills.clone()),
        provider.clone_arc(),
        project_path_for_tools.clone(),
    );
    // Add MCP tools
    base_tools.extend(mcp_tools);

    let mut agent = AgentBuilder::new(provider.clone_box())
        .system_prompt(system_prompt)
        .model_name(model.clone())
        .max_tokens(max_tokens)
        .context_size(config.context_size)
        .think(think)
        .tools(base_tools)
        .project_path(project_path_for_tools)
        .event_tx(event_tx.clone())
        .approve_mode(approve_mode)
        .proxy_executor(
            matrixcode_tui::image_search::create_default_executor(),
            matrixcode_tui::image_search::get_default_proxy_tools()
        )
        .mcp_registry(mcp_registry)
        .pending_input_rx(pending_input_rx)
        .build();

    agent.set_approve_mode_shared(shared_approve_mode);

    // Restore messages
    if !restored_messages.is_empty() {
        log::info!("Agent task: restoring {} messages", restored_messages.len());
        agent.set_messages(restored_messages);
    }

    log::info!("Agent task: messages restored, entering receive loop");

    agent.set_cancel_token(cancel_token.clone());
    agent.set_ask_channel(ask_rx);

    // Send CodeGraph status if initialized
    if let Some(ref pp) = project_path {
        use matrixcode_core::tools::codegraph::CodeGraphManager;
        let manager = CodeGraphManager::with_auto_detect(pp.as_path());
        if manager.is_initialized() {
            if let Ok(status) = manager.status() {
                let _ = event_tx.send(AgentEvent::codegraph_status(status)).await;
            }
        }
    }

    // CodeGraph watcher is started in main function, not here

    let mut turn_count: usize = 0;

    // Auto-analyze project structure on first run
    if let Some(ref pp) = project_path
        && let Some(ref mut ms) = memory_storage {
        let memory_file = pp.join(".matrix/memory.json");
        if !memory_file.exists() {
            let count = matrixcode_core::memory::generate_project_structure_memories(
                pp.as_path(),
                ms
            );
            if count > 0 {
                let _ = event_tx.send(AgentEvent::progress(
                    format!("🧠 自动分析项目结构，创建 {} 条记忆", count),
                    None,
                )).await;
            }
        }
    }

    log::info!("Agent task: entering receive loop");
    while let Some(msg) = task_rx.recv().await {
        log::info!("Agent task: received message (len={})", msg.len());

        let mut msg = msg;

        // Check cancellation
        if cancel_token.is_cancelled() {
            event_tx.send(AgentEvent::error(
                "Operation interrupted by user".to_string(),
                Some("interrupted".to_string()),
                None,
            )).await.ok();
            cancel_token.reset();
            continue;
        }

        // Handle /init - check for both "/init" and Git Bash path conversion ("C:/Program Files/Git/init")
        let is_init_cmd = msg.starts_with("/init")
            || msg.contains("/init") && (msg.contains("Program Files/Git") || msg.contains("Git/init"));
        if is_init_cmd {
            // Normalize the command if it was converted by Git Bash
            let normalized_msg = if msg.contains("Program Files/Git") || msg.contains("Git/init") {
                "/init".to_string()
            } else {
                msg.clone()
            };
            let should_refresh = handle_init_in_task(
                &event_tx,
                &normalized_msg,
                &project_path,
                provider.as_ref(),
                &watcher_handle,
                &cancel_token,
            ).await;
            if should_refresh {
                agent.refresh_codegraph_tools();
            }
            continue;
        }

        // Handle /skills
        if msg == "/skills" || msg.starts_with("/skills ") {
            handle_skills_in_task(&event_tx, &msg, &skills).await;
            continue;
        }

        // Handle /workflow
        if msg == "/workflow" || msg.starts_with("/workflow ") {
            handle_workflow_in_task(&event_tx, &msg, &project_path).await;
            continue;
        }

        // Handle skill activation (direct /skill_name)
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
           && !msg.starts_with("/config") && !msg.starts_with("/tools")
           && !msg.starts_with("/system")
           && msg != "/" {
            let skill_name = msg.trim_start_matches('/');
            if let Some(skill) = skills.iter().find(|s| s.name == skill_name) {
                let files = matrixcode_core::skills::list_skill_files(&skill.dir);
                let files_info = if files.len() > 1 {
                    format!("\n\n📁 Associated files:\n{}",
                        files.iter().map(|f| format!("  - {}", f)).collect::<Vec<_>>().join("\n"))
                } else {
                    String::new()
                };

                msg = format!(
                    "使用 skill '{}' 来处理当前任务。\n\n---\n{}\n---\n{}\n\n请按照上述 skill 指导开始执行。",
                    skill.name,
                    skill.body,
                    files_info
                );

                let _ = event_tx.send(AgentEvent::with_data(
                    matrixcode_core::EventType::Progress,
                    matrixcode_core::EventData::Progress {
                        message: format!("🎯 Activating skill: {}", skill.name),
                        percentage: None,
                    },
                )).await;
            }
        }

        // Handle /compact
        if msg == "/compact" || msg == "/compress" {
            handle_compact_in_task(&event_tx, &mut agent).await;
            continue;
        }

        // Handle /mode
        if let Some(mode_str) = msg.strip_prefix("/mode:") {
            let new_mode = match mode_str {
                "ask" => ApproveMode::Ask,
                "auto" => ApproveMode::Auto,
                "strict" => ApproveMode::Strict,
                _ => continue,
            };
            agent.set_approve_mode(new_mode);
            continue;
        }

        // Handle /new
        if msg == "/new" {
            if let Some(ref mut mgr) = session_mgr {
                let pp = std::env::current_dir().ok();
                mgr.start_new(pp.as_deref()).ok();
                agent.clear_history();
                let _ = event_tx.send(AgentEvent::session_ended()).await;
                let _ = event_tx.send(AgentEvent::progress("✓ New session created", None)).await;
            }
            continue;
        }

        // Handle /memory
        if msg == "/memory" || msg.starts_with("/memory ") {
            handle_memory_in_task(&event_tx, &msg, &mut memory_storage, &project_path).await;
            continue;
        }

        // Handle /overview
        if msg == "/overview" || msg.starts_with("/overview ") {
            handle_overview_in_task(&event_tx, &msg).await;
            continue;
        }

        // Handle /save
        if msg == "/save" || msg.starts_with("/save ") {
            handle_save_in_task(&event_tx, &msg, &mut session_mgr, agent.get_messages()).await;
            continue;
        }

        // Handle /sessions
        if msg == "/sessions" || msg == "/resume" || msg.starts_with("/sessions ") {
            handle_sessions_in_task(&event_tx, &msg, &mut session_mgr).await;
            continue;
        }

        // Handle /load
        if msg.starts_with("/load ") {
            handle_load_in_task(&event_tx, &msg, &mut session_mgr, &mut agent).await;
            continue;
        }

        // Handle /config
        if msg == "/config" {
            handle_config_in_task(&event_tx, &config, &model).await;
            continue;
        }

        // Handle /tools
        if msg == "/tools" {
            handle_tools_in_task(&event_tx, &agent).await;
            continue;
        }

        // Handle /system
        if msg == "/system" {
            handle_system_in_task(&event_tx, &agent, &config, &model).await;
            continue;
        }

        // Dynamic memory retrieval
        if let Some(ref mem) = memory {
            let is_first_turn = turn_count == 0;
            let is_simple_msg = matrixcode_core::memory::should_skip_simple_message(&msg);
            let has_few_memories = mem.entries.len() < MEMORY_MIN_ENTRIES_FOR_AI_SELECTION;

            if is_first_turn || is_simple_msg || has_few_memories {
                let static_summary = mem.generate_prompt_summary(MEMORY_INITIAL_SUMMARY_SIZE);
                if !static_summary.is_empty() {
                    agent.update_memory_summary(Some(static_summary));
                }
            } else if let Some(ref fp) = fast_provider {
                let manifest = mem.generate_manifest(MEMORY_MANIFEST_SIZE);
                if !manifest.is_empty() {
                    let selected_indices = matrixcode_core::memory::ai_select_memories(
                        &msg,
                        &manifest,
                        fp.as_ref(),
                    ).await;

                    let selected_entries = mem.get_entries_by_indices(&selected_indices);
                    let contextual_summary = if selected_entries.is_empty() {
                        mem.generate_prompt_summary(5)
                    } else {
                        let mut summary = String::from("【相关记忆】\n\n");
                        for entry in selected_entries.iter().take(5) {
                            summary.push_str(&format!("{} {}\n", entry.category.icon(), entry.content));
                        }
                        summary
                    };

                    if !contextual_summary.is_empty() {
                        agent.update_memory_summary(Some(contextual_summary));

                        if !selected_indices.is_empty() {
                            let _ = event_tx.send(AgentEvent::with_data(
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
                let keywords = matrixcode_core::memory::extract_context_keywords(&msg);
                let contextual_summary = mem.generate_contextual_summary_with_keywords(&keywords, 10);
                if !contextual_summary.is_empty() {
                    agent.update_memory_summary(Some(contextual_summary));
                }
            }
        }

        // Pre-process: detect skill/workflow triggers with skills
        let processed_msg = match preprocess_with_skills(&msg, &skills) {
            ProcessResult::SkillTriggered { skill_id, confidence: _, skill_body } => {
                log::info!("Skill triggered: {}", skill_id);
                // If skill body is auto-loaded, inject it directly
                if let Some(body) = skill_body {
                    format!(
                        "# Skill: {}\n\n{}\n\n---\n\n用户原始请求：{}",
                        skill_id, body, msg
                    )
                } else {
                    // Skill not auto-loaded, inject skill call prompt
                    format!(
                        "【系统检测到应使用技能: {}】\n\n请先调用 skill 工具加载此技能，然后立即执行其中的指令。\n\n用户原始请求：{}",
                        skill_id, msg
                    )
                }
            }
            ProcessResult::WorkflowTriggered { workflow_id, inputs } => {
                log::info!("Workflow triggered: {} (inputs: {:?})", workflow_id, inputs);
                let inputs_json = serde_json::to_string(&inputs).unwrap_or_default();
                format!(
                    "【系统检测到应使用工作流: {}】\n\n请先调用 workflow_run 工具执行此工作流，参数如下：{}\n\n用户原始请求：{}",
                    workflow_id, inputs_json, msg
                )
            }
            ProcessResult::Continue => msg.clone(),
        };

        // Run agent
        turn_count += 1;

        match agent.run(processed_msg).await {
            Ok(_) => {
                // Auto-save session
                save_session_after_turn(&event_tx, &mut session_mgr, &mut agent).await;

                // Handle memory feedback
                handle_memory_feedback(&event_tx, &mut memory_storage, &msg).await;

                // Periodic memory cleanup
                if turn_count.is_multiple_of(MEMORY_TURN_CLEANUP_INTERVAL) {
                    handle_memory_periodic_cleanup(&event_tx, &mut memory_storage).await;
                }

                // AI memory extraction
                let should_extract = turn_count.is_multiple_of(MEMORY_EXTRACTION_INTERVAL) && fast_provider.is_some();
                matrixcode_core::debug::debug_log().log(
                    "memory_extract",
                    &format!("turn={}, should_extract={}", turn_count, should_extract),
                );

                if should_extract {
                    let messages = agent.get_messages();
                    if let Some(last_msg) = messages.last() {
                        spawn_memory_extraction_task(
                            event_tx.clone(),
                            project_path.clone(),
                            fast_model.clone(),
                            last_msg,
                        );
                    }
                }
            }
            Err(e) => {
                event_tx.send(AgentEvent::error(
                    format!("Agent error: {}", e),
                    Some("agent_error".to_string()),
                    None,
                )).await.ok();
            }
        }
    }
}

// Helper functions for the agent task

/// Handle /init command. Returns true if overview was generated successfully
/// (indicating CodeGraph may need refresh if it was initialized during the process).
/// Also starts CodeGraph watcher if daemon is no longer running after init.
#[allow(clippy::too_many_arguments)]
async fn handle_init_in_task(
    event_tx: &tokio::sync::mpsc::Sender<AgentEvent>,
    msg: &str,
    project_path: &Option<PathBuf>,
    provider: &dyn matrixcode_core::providers::Provider,
    watcher_handle: &Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    cancel_token: &CancellationToken,
) -> bool {
    let result = handle_init_command(msg, project_path.as_deref());
    match result {
        InitCommandResult::Message(msg) => {
            let _ = event_tx.send(AgentEvent::with_data(
                matrixcode_core::EventType::Progress,
                matrixcode_core::EventData::Progress {
                    message: msg,
                    percentage: None,
                },
            )).await;
            false
        }
        InitCommandResult::GenerateOverview => {
            let _ = event_tx.send(AgentEvent::with_data(
                matrixcode_core::EventType::Progress,
                matrixcode_core::EventData::Progress {
                    message: "🔄 Generating project overview...".into(),
                    percentage: Some(10),
                },
            )).await;

            if let Some(path) = project_path {
                // Step 1: Generate project overview
                let overview_result = matrixcode_core::overview::ProjectOverview::generate_with_ai(path.as_path(), provider).await;

                match overview_result {
                    Ok(overview) => {
                        let _ = event_tx.send(AgentEvent::with_data(
                            matrixcode_core::EventType::Progress,
                            matrixcode_core::EventData::Progress {
                                message: format!("✓ Project overview generated: {}", overview.path.display()),
                                percentage: Some(50),
                            },
                        )).await;
                    }
                    Err(e) => {
                        let _ = event_tx.send(AgentEvent::error(
                            format!("Failed to generate overview: {}", e),
                            Some("overview_error".into()),
                            None,
                        )).await;
                        return false;
                    }
                }

                // Step 2: Initialize CodeGraph if CLI is installed and db doesn't exist
                use matrixcode_core::tools::codegraph::{
                    get_codegraph_path, should_inject_codegraph_tools, CodeGraphManager, CodeGraphWatcher
                };

                let cli_installed = get_codegraph_path().is_some();
                let db_exists = should_inject_codegraph_tools(path);

                if cli_installed && !db_exists {
                    let _ = event_tx.send(AgentEvent::with_data(
                        matrixcode_core::EventType::Progress,
                        matrixcode_core::EventData::Progress {
                            message: "🔄 Generating CodeGraph index...".into(),
                            percentage: Some(60),
                        },
                    )).await;

                    let manager = CodeGraphManager::new(path);
                    match manager.init().await {
                        Ok(_) => {
                            // Sync after init
                            if let Err(e) = manager.sync().await {
                                log::warn!("CodeGraph sync failed: {}", e);
                            }

                            // Step 3: Check daemon status and start watcher if no conflict
                            // Re-check after init - daemon might have stopped or we can start our watcher
                            {
                                let mut handle_guard = watcher_handle.lock().unwrap();
                                let watcher_running = handle_guard.is_some() &&
                                    handle_guard.as_ref().map(|h| !h.is_finished()).unwrap_or(false);

                                if !watcher_running {
                                    // Check if daemon is running
                                    let daemon_pid_path = path.join(".codegraph").join("daemon.pid");
                                    let daemon_running = if daemon_pid_path.exists() {
                                        std::fs::read_to_string(&daemon_pid_path)
                                            .ok()
                                            .and_then(|pid| pid.trim().parse::<u32>().ok())
                                            .map(|pid| {
                                                // On Windows, check if process exists
                                                #[cfg(target_os = "windows")]
                                                {
                                                    use std::os::windows::process::CommandExt;
                                                    const CREATE_NO_WINDOW: u32 = 0x08000000;
                                                    std::process::Command::new("tasklist")
                                                        .args(["/FI", &format!("PID eq {}", pid)])
                                                        .creation_flags(CREATE_NO_WINDOW)
                                                        .output()
                                                        .map(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()))
                                                        .unwrap_or(false)
                                                }
                                                #[cfg(not(target_os = "windows"))]
                                                {
                                                    std::path::Path::new("/proc").join(pid.to_string()).exists()
                                                }
                                            })
                                            .unwrap_or(false)
                                    } else {
                                        false
                                    };

                                    if !daemon_running {
                                        // No daemon, start our watcher
                                        let watcher = CodeGraphWatcher::with_auto_detect(path.as_path());
                                        let handle = watcher.start_with_status_updates(cancel_token.clone(), event_tx.clone());
                                        log::info!("CodeGraph watcher started after /init with status updates (no MCP daemon detected)");
                                        *handle_guard = Some(handle);
                                    } else {
                                        log::info!("CodeGraph MCP daemon still running after /init, skipping watcher");
                                    }
                                }
                            }

                            let _ = event_tx.send(AgentEvent::with_data(
                                matrixcode_core::EventType::Progress,
                                matrixcode_core::EventData::Progress {
                                    message: "✓ CodeGraph index generated (code analysis tools now available)".into(),
                                    percentage: Some(100),
                                },
                            )).await;

                            // Return true to refresh tools (state changed)
                            true
                        }
                        Err(e) => {
                            let _ = event_tx.send(AgentEvent::with_data(
                                matrixcode_core::EventType::Progress,
                                matrixcode_core::EventData::Progress {
                                    message: format!("⚠️ CodeGraph generation skipped: {}", e),
                                    percentage: Some(100),
                                },
                            )).await;
                            false
                        }
                    }
                } else if !cli_installed {
                    let _ = event_tx.send(AgentEvent::with_data(
                        matrixcode_core::EventType::Progress,
                        matrixcode_core::EventData::Progress {
                            message: "⚠️ CodeGraph CLI not installed. Run 'codegraph install' to enable code analysis tools.".into(),
                            percentage: Some(100),
                        },
                    )).await;
                    false
                } else {
                    // db already exists
                    let _ = event_tx.send(AgentEvent::with_data(
                        matrixcode_core::EventType::Progress,
                        matrixcode_core::EventData::Progress {
                            message: "✓ CodeGraph index already exists".into(),
                            percentage: Some(100),
                        },
                    )).await;
                    false
                }
            } else {
                let _ = event_tx.send(AgentEvent::error(
                    String::from("No project path set. Cannot generate overview."),
                    Some("no_project".into()),
                    None,
                )).await;
                false
            }
        }
    }
}

async fn handle_skills_in_task(
    event_tx: &tokio::sync::mpsc::Sender<AgentEvent>,
    msg: &str,
    skills: &[matrixcode_core::skills::Skill],
) {
    let parts: Vec<&str> = msg.split_whitespace().collect();
    let subcmd = parts.get(1).copied().unwrap_or("");

    let response = if subcmd.is_empty() || subcmd == "list" {
        if skills.is_empty() {
            "📚 No skills loaded.".to_string()
        } else {
            let mut info = format!("📚 Loaded skills ({}):\n\n", skills.len());
            for skill in skills {
                info.push_str(&format!("• {}: {}\n", skill.name, skill.description));
            }
            info
        }
    } else {
        let skill_name = subcmd;
        if let Some(skill) = skills.iter().find(|s| s.name == skill_name) {
            format!("📚 Skill: {}\n\n{}\n\nSource: {}", skill.name, skill.body, skill.source_file.display())
        } else {
            format!("❌ Skill '{}' not found.", skill_name)
        }
    };

    let _ = event_tx.send(AgentEvent::progress(response, None)).await;
}

async fn handle_workflow_in_task(
    event_tx: &tokio::sync::mpsc::Sender<AgentEvent>,
    msg: &str,
    project_path: &Option<PathBuf>,
) {
    use matrixcode_core::workflow::WorkflowRegistry;

    let parts: Vec<&str> = msg.split_whitespace().collect();
    let subcmd = parts.get(1).copied().unwrap_or("");

    let response = match subcmd {
        "" | "discover" | "list" => {
            let registry = WorkflowRegistry::new(project_path.as_ref());
            if registry.is_empty() {
                "📋 No workflows found.".to_string()
            } else {
                registry.generate_summary()
            }
        }
        "match" => {
            let query = parts.get(2..).map(|p| p.join(" ")).unwrap_or_default();
            if query.is_empty() {
                "Usage: /workflow match <query>".to_string()
            } else {
                let registry = WorkflowRegistry::new(project_path.as_ref());
                let matches = registry.match_workflows(&query);
                if matches.is_empty() {
                    format!("❌ No workflows match '{}'", query)
                } else {
                    let mut result = format!("🔍 Matching workflows for '{}':\n\n", query);
                    for info in matches.iter().take(5) {
                        result.push_str(&format!("• {} - {}\n", info.id, info.name));
                    }
                    result
                }
            }
        }
        "run" => {
            let workflow_id = parts.get(2).copied().unwrap_or("");
            if workflow_id.is_empty() {
                "Usage: /workflow run <workflow-id>".to_string()
            } else {
                format!("⏳ Workflow '{}' queued. Use CLI for full execution.", workflow_id)
            }
        }
        _ => format!("Unknown subcommand '{}'.", subcmd)
    };

    let _ = event_tx.send(AgentEvent::progress(response, None)).await;
}

async fn handle_compact_in_task(
    event_tx: &tokio::sync::mpsc::Sender<AgentEvent>,
    agent: &mut matrixcode_core::agent::Agent,
) {
    let original_tokens = matrixcode_core::compress::estimate_total_tokens(agent.get_messages());
    if original_tokens > 100 {
        let _ = event_tx.send(AgentEvent::with_data(
            matrixcode_core::EventType::CompressionTriggered,
            matrixcode_core::EventData::Progress {
                message: format!("Compressing {} tokens...", original_tokens),
                percentage: None,
            },
        )).await;

        match matrixcode_core::compress::compress_messages(
            agent.get_messages(),
            matrixcode_core::compress::CompressionStrategy::SlidingWindow,
            &matrixcode_core::compress::CompressionConfig::default(),
        ) {
            Ok(compressed) => {
                let compressed_tokens = matrixcode_core::compress::estimate_total_tokens(&compressed);
                agent.set_messages(compressed);
                let ratio = compressed_tokens as f32 / original_tokens as f32;

                let _ = event_tx.send(AgentEvent::with_data(
                    matrixcode_core::EventType::CompressionCompleted,
                    matrixcode_core::EventData::Compression {
                        original_tokens: original_tokens as u64,
                        compressed_tokens: compressed_tokens as u64,
                        ratio,
                    },
                )).await;
            }
            Err(e) => {
                let _ = event_tx.send(AgentEvent::error(
                    format!("Compression failed: {}", e),
                    None,
                    None,
                )).await;
            }
        }
    } else {
        let _ = event_tx.send(AgentEvent::progress("Context too small, no need to compress", None)).await;
    }
}

async fn handle_memory_in_task(
    event_tx: &tokio::sync::mpsc::Sender<AgentEvent>,
    msg: &str,
    memory_storage: &mut Option<matrixcode_core::memory::MemoryStorage>,
    project_path: &Option<PathBuf>,
) {
    let parts: Vec<&str> = msg.split_whitespace().collect();
    let subcmd = parts.get(1).copied().unwrap_or("");

    if let Some(ms) = memory_storage {
        let response = match subcmd {
            "" | "list" => {
                if let Ok(mem) = ms.load_combined() {
                    if mem.entries.is_empty() {
                        "📝 No memories stored.".to_string()
                    } else {
                        let stats = mem.generate_statistics();
                        stats.format_summary()
                    }
                } else {
                    "❌ Failed to load memories".to_string()
                }
            }
            "stats" => {
                if let Ok(mem) = ms.load_combined() {
                    mem.generate_statistics().format_summary()
                } else {
                    "❌ Failed to get stats".to_string()
                }
            }
            "search" => {
                let query = parts.get(2..).map(|p| p.join(" ")).unwrap_or_default();
                if query.is_empty() {
                    "Usage: /memory search <query>".to_string()
                } else if let Ok(mem) = ms.load_combined() {
                    let results = mem.search_with_limit(&query, Some(DISPLAY_MEMORY_SEARCH_LIMIT));
                    if results.is_empty() {
                        format!("No memories found for '{}'", query)
                    } else {
                        format!("🔍 Found {} memories for '{}'", results.len(), query)
                    }
                } else {
                    "❌ Failed to search".to_string()
                }
            }
            "analyze" => {
                if let Some(pp) = project_path {
                    let count = matrixcode_core::memory::generate_project_structure_memories(pp.as_path(), ms);
                    format!("✓ Generated {} structure memories", count)
                } else {
                    "❌ No project path".to_string()
                }
            }
            "merge" => {
                if let Ok(mut mem) = ms.load_combined() {
                    let count = mem.smart_merge();
                    if let Err(e) = ms.save_global(&mem) {
                        log::warn!("Failed to save: {}", e);
                    }
                    format!("✓ Merged {} similar memories", count)
                } else {
                    "❌ Failed to merge".to_string()
                }
            }
            "help" => "Commands: list, stats, search, analyze, merge".to_string(),
            _ => format!("Unknown command '{}'. Use '/memory help'", subcmd)
        };
        let _ = event_tx.send(AgentEvent::progress(response, None)).await;
    } else {
        let _ = event_tx.send(AgentEvent::progress("❌ Memory storage not available", None)).await;
    }
}

async fn handle_overview_in_task(
    event_tx: &tokio::sync::mpsc::Sender<AgentEvent>,
    msg: &str,
) {
    let parts: Vec<&str> = msg.split_whitespace().collect();
    let subcmd = parts.get(1).copied().unwrap_or("");

    let cwd = std::env::current_dir().unwrap_or_default();
    let overview_path = cwd.join(matrixcode_core::overview::OVERVIEW_FILENAME);

    let response = match subcmd {
        "" | "show" => {
            if overview_path.exists() {
                let content = std::fs::read_to_string(&overview_path).unwrap_or_default();
                format!("📄 Project Overview:\n\n{}", content.chars().take(DISPLAY_OVERVIEW_CHARS_LIMIT).collect::<String>())
            } else {
                "❌ No overview found. Run '/init' to generate.".to_string()
            }
        }
        "path" => format!("Overview path: {}", overview_path.display()),
        _ => "Unknown command. Use: show, path".to_string()
    };

    let _ = event_tx.send(AgentEvent::progress(response, None)).await;
}

async fn handle_save_in_task(
    event_tx: &tokio::sync::mpsc::Sender<AgentEvent>,
    msg: &str,
    session_mgr: &mut Option<SessionManager>,
    messages: &[matrixcode_core::providers::Message],
) {
    let parts: Vec<&str> = msg.split_whitespace().collect();
    let name = parts.get(1).copied();

    if let Some(mgr) = session_mgr {
        mgr.set_messages(messages.to_vec());
        if let Some(n) = name
            && let Err(e) = mgr.rename_current(n) {
                let _ = event_tx.send(AgentEvent::error(format!("Failed to rename: {}", e), None, None)).await;
            }
        if let Err(e) = mgr.save_current() {
            let _ = event_tx.send(AgentEvent::error(format!("Failed to save: {}", e), None, None)).await;
        } else {
            let _ = event_tx.send(AgentEvent::progress("✓ Session saved", None)).await;
        }
    } else {
        let _ = event_tx.send(AgentEvent::progress("❌ Session manager not available", None)).await;
    }
}

async fn handle_sessions_in_task(
    event_tx: &tokio::sync::mpsc::Sender<AgentEvent>,
    msg: &str,
    session_mgr: &mut Option<SessionManager>,
) {
    let subcmd = if msg.starts_with("/sessions ") {
        msg.strip_prefix("/sessions ").unwrap_or("")
    } else {
        ""
    };

    if let Some(mgr) = session_mgr {
        if subcmd == "cleanup" {
            let removed = mgr.cleanup_old_sessions(SESSION_CLEANUP_DAYS).unwrap_or(0);
            let _ = event_tx.send(AgentEvent::progress(format!("✓ Removed {} old sessions", removed), None)).await;
        } else if subcmd == "stats" {
            let sessions = mgr.list_sessions();
            let _ = event_tx.send(AgentEvent::progress(
                format!("📊 {} sessions total", sessions.len()),
                None,
            )).await;
        } else {
            let sessions = mgr.list_sessions();
            if sessions.is_empty() {
                let _ = event_tx.send(AgentEvent::progress("No saved sessions", None)).await;
            } else {
                let mut info = format!("📚 Sessions ({}):\n", sessions.len());
                for session in sessions.iter().take(DISPLAY_SESSIONS_LIMIT) {
                    info.push_str(&format!("• {} - {} msgs\n", session.short_id(), session.message_count));
                }
                let _ = event_tx.send(AgentEvent::progress(info, None)).await;
            }
        }
    } else {
        let _ = event_tx.send(AgentEvent::progress("❌ Session manager not available", None)).await;
    }
}

async fn handle_load_in_task(
    event_tx: &tokio::sync::mpsc::Sender<AgentEvent>,
    msg: &str,
    session_mgr: &mut Option<SessionManager>,
    agent: &mut matrixcode_core::agent::Agent,
) {
    let session_id = msg.strip_prefix("/load ").unwrap_or("");

    if let Some(mgr) = session_mgr {
        if mgr.resume(session_id).is_ok() {
            if let Some(msgs) = mgr.messages() {
                let messages = msgs.to_vec();
                agent.set_messages(messages.clone());
                let _ = event_tx.send(AgentEvent::progress(
                    format!("✓ Session '{}' loaded ({} messages)", session_id, messages.len()),
                    None,
                )).await;
            }
        } else {
            let _ = event_tx.send(AgentEvent::progress(format!("❌ Session '{}' not found", session_id), None)).await;
        }
    } else {
        let _ = event_tx.send(AgentEvent::progress("❌ Session manager not available", None)).await;
    }
}

async fn handle_config_in_task(
    event_tx: &tokio::sync::mpsc::Sender<AgentEvent>,
    config: &Config,
    model: &str,
) {
    let mut info = "⚙️ Current Configuration:\n\n".to_string();
    info.push_str(&format!("Provider: {}\n", config.provider.as_deref().unwrap_or("auto")));
    info.push_str(&format!("Model: {}\n", model));
    info.push_str(&format!("Think: {}\n", config.think));
    info.push_str(&format!("Max Tokens: {}\n", config.max_tokens));
    let _ = event_tx.send(AgentEvent::progress(info, None)).await;
}

async fn handle_tools_in_task(
    event_tx: &tokio::sync::mpsc::Sender<AgentEvent>,
    agent: &matrixcode_core::agent::Agent,
) {
    let tools = agent.get_tools();
    let mut info = format!("🔧 Available Tools: {}\n\n", tools.len());

    // Group tools dynamically by category
    // Use prefix matching for dynamic tools (MCP, proxy, etc.)
    let mut core_tools: Vec<_> = Vec::new();
    let mut file_tools: Vec<_> = Vec::new();
    let mut search_tools: Vec<_> = Vec::new();
    let mut web_tools: Vec<_> = Vec::new();
    let mut code_tools: Vec<_> = Vec::new();
    let mut mcp_tools: Vec<_> = Vec::new();
    let mut workflow_tools: Vec<_> = Vec::new();
    let mut other_tools: Vec<_> = Vec::new();

    for tool in tools.iter() {
        let def = tool.definition();
        let name = def.name.as_str();
        let desc = def.description.as_str();

        // Dynamic classification by name prefix or content
        if name.starts_with("mcp_") || name.starts_with("mcp__") {
            mcp_tools.push(tool);
        } else if name.starts_with("workflow_") || name.contains("workflow") {
            workflow_tools.push(tool);
        } else if name.starts_with("code_") || desc.contains("CodeGraph") {
            code_tools.push(tool);
        } else if name.starts_with("proxy_") || desc.contains("代理") {
            other_tools.push(tool);  // Proxy tools are special
        } else {
            // Static classification for built-in tools
            match name {
                "read" | "write" | "edit" | "multi_edit" | "ls" => file_tools.push(tool),
                "grep" | "glob" | "search" => search_tools.push(tool),
                "websearch" | "webfetch" => web_tools.push(tool),
                "bash" | "task" | "todo_write" | "notebook_edit"
                | "task_create" | "task_get" | "task_list" | "task_stop" => core_tools.push(tool),
                "ask" | "enter_plan_mode" | "exit_plan_mode" | "monitor" => core_tools.push(tool),
                _ => other_tools.push(tool),
            }
        }
    }

    if !core_tools.is_empty() {
        info.push_str("📁 Core:\n");
        for tool in core_tools.iter().take(12) {
            let def = tool.definition();
            info.push_str(&format!("  {} - {}\n", def.name,
                truncate_description(&def.description, 35)));
        }
    }

    if !file_tools.is_empty() {
        info.push_str("\n📄 File:\n");
        for tool in file_tools.iter() {
            let def = tool.definition();
            info.push_str(&format!("  {} - {}\n", def.name,
                truncate_description(&def.description, 35)));
        }
    }

    if !search_tools.is_empty() {
        info.push_str("\n🔍 Search:\n");
        for tool in search_tools.iter() {
            let def = tool.definition();
            info.push_str(&format!("  {} - {}\n", def.name,
                truncate_description(&def.description, 35)));
        }
    }

    if !code_tools.is_empty() {
        info.push_str("\n📊 CodeGraph:\n");
        for tool in code_tools.iter() {
            let def = tool.definition();
            info.push_str(&format!("  {} - {}\n", def.name,
                truncate_description(&def.description, 35)));
        }
    }

    if !web_tools.is_empty() {
        info.push_str("\n🌐 Web:\n");
        for tool in web_tools.iter() {
            let def = tool.definition();
            info.push_str(&format!("  {} - {}\n", def.name,
                truncate_description(&def.description, 35)));
        }
    }

    if !workflow_tools.is_empty() {
        info.push_str("\n🔄 Workflow:\n");
        for tool in workflow_tools.iter().take(10) {
            let def = tool.definition();
            info.push_str(&format!("  {} - {}\n", def.name,
                truncate_description(&def.description, 35)));
        }
    }

    if !mcp_tools.is_empty() {
        info.push_str("\n🔌 MCP:\n");
        for tool in mcp_tools.iter().take(15) {
            let def = tool.definition();
            info.push_str(&format!("  {} - {}\n", def.name,
                truncate_description(&def.description, 35)));
        }
        if mcp_tools.len() > 15 {
            info.push_str(&format!("  (+ {} more)\n", mcp_tools.len() - 15));
        }
    }

    if !other_tools.is_empty() {
        info.push_str("\n🔧 Other:\n");
        for tool in other_tools.iter().take(10) {
            let def = tool.definition();
            info.push_str(&format!("  {} - {}\n", def.name,
                truncate_description(&def.description, 35)));
        }
        if other_tools.len() > 10 {
            info.push_str(&format!("  (+ {} more)\n", other_tools.len() - 10));
        }
    }

    let _ = event_tx.send(AgentEvent::progress(info, None)).await;
}

async fn handle_system_in_task(
    event_tx: &tokio::sync::mpsc::Sender<AgentEvent>,
    agent: &matrixcode_core::agent::Agent,
    config: &Config,
    model: &str,
) {
    let mut info = "📋 System Information:\n\n".to_string();

    // Configuration
    info.push_str("⚙️ Configuration:\n");
    info.push_str(&format!("  Provider: {}\n", config.provider.as_deref().unwrap_or("auto")));
    info.push_str(&format!("  Model: {}\n", model));
    info.push_str(&format!("  Think: {}\n", config.think));
    info.push_str(&format!("  Max Tokens: {}\n", config.max_tokens));
    info.push_str(&format!("  Context Size: {}\n", config.context_size.unwrap_or(0)));
    info.push_str(&format!("  Approve Mode: {}\n", config.approve_mode.as_deref().unwrap_or("ask")));

    // System prompt summary (clean up markdown tables)
    let system_prompt = agent.get_system_prompt();
    let clean_prompt = clean_markdown_tables(system_prompt);
    let prompt_preview = if clean_prompt.len() > 500 {
        format!("{}... ({} chars total)",
            &clean_prompt[..500], clean_prompt.len())
    } else {
        clean_prompt
    };
    info.push_str(&format!("\n📝 System Prompt Preview:\n{}\n", prompt_preview));

    // Tools count
    let tools = agent.get_tools();
    info.push_str(&format!("\n🔧 Tools: {} available\n", tools.len()));

    // Message count
    let messages = agent.get_messages();
    info.push_str(&format!("💬 Messages: {} in history\n", messages.len()));

    // Token stats
    let (input_tokens, output_tokens) = agent.get_token_counts();
    info.push_str(&format!("📊 Tokens: {} in, {} out\n", input_tokens, output_tokens));

    let _ = event_tx.send(AgentEvent::progress(info, None)).await;
}

fn truncate_description(desc: &str, max_len: usize) -> String {
    // Take only the first line as short description (avoid markdown tables etc.)
    let first_line = desc.lines().next().unwrap_or(desc);

    // Truncate by characters (not bytes) to avoid UTF-8 boundary issues
    let chars: Vec<char> = first_line.chars().collect();
    if chars.len() > max_len {
        chars[..max_len.saturating_sub(3)].iter().collect::<String>() + "..."
    } else {
        first_line.to_string()
    }
}

fn clean_markdown_tables(text: &str) -> String {
    // Remove markdown table formatting to prevent display issues in TUI
    text.lines()
        .filter(|line| {
            // Skip table header separator lines (|---|)
            !line.trim().starts_with("|---")
            // Skip lines that are mostly table separators
            && line.trim().chars().filter(|c| *c == '|').count() <= 3
        })
        .map(|line| {
            // Remove table column separators but keep content
            line.replace("|", " ").trim().to_string()
        })
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

// Post-run handling functions

/// Save session after agent turn
async fn save_session_after_turn(
    event_tx: &tokio::sync::mpsc::Sender<AgentEvent>,
    session_mgr: &mut Option<SessionManager>,
    agent: &mut matrixcode_core::agent::Agent,
) {
    if let Some(mgr) = session_mgr {
        let (input_tokens, output_tokens) = agent.get_token_counts();
        let messages = agent.get_messages();
        mgr.set_messages(messages.to_vec());
        mgr.set_compressed_messages(messages.to_vec());
        mgr.update_stats(input_tokens as u32, output_tokens);
        if let Err(e) = mgr.save_current() {
            let _ = event_tx.send(AgentEvent::error(
                format!("Session save failed: {}", e),
                None,
                None,
            )).await;
        }
        matrixcode_core::debug::debug_log().session_save(messages.len(), output_tokens);
    }
}

/// Handle memory feedback detection
async fn handle_memory_feedback(
    event_tx: &tokio::sync::mpsc::Sender<AgentEvent>,
    memory_storage: &mut Option<matrixcode_core::memory::MemoryStorage>,
    msg: &str,
) {
    if let Some(ms) = memory_storage {
        let feedback_results = matrixcode_core::memory::detect_feedback_patterns(msg);
        if !feedback_results.is_empty()
            && let Ok(mut mem) = ms.load_combined() {
            let feedback_count = feedback_results.len();
            for feedback in feedback_results {
                matrixcode_core::memory::apply_feedback_to_memory(&mut mem, &feedback);
            }
            if mem.entries.iter().any(|e| e.tags.contains(&"project".to_string())) {
                if let Err(e) = ms.save_project(&mem) {
                    log::warn!("Failed to save project memory: {}", e);
                }
            } else {
                if let Err(e) = ms.save_global(&mem) {
                    log::warn!("Failed to save global memory: {}", e);
                }
            }
            let _ = event_tx.send(AgentEvent::progress(
                format!("🧠 Learned from feedback: {} corrections", feedback_count),
                None,
            )).await;
        }
    }
}

/// Handle periodic memory cleanup
async fn handle_memory_periodic_cleanup(
    event_tx: &tokio::sync::mpsc::Sender<AgentEvent>,
    memory_storage: &mut Option<matrixcode_core::memory::MemoryStorage>,
) {
    if let Some(ms) = memory_storage
        && let Ok(mut mem) = ms.load_combined() {
        mem.apply_time_decay();
        let merged = mem.smart_merge();
        mem.prune();
        if let Err(e) = ms.save_global(&mem) {
            log::warn!("Failed to save memory after maintenance: {}", e);
        }
        if merged > 0 {
            let _ = event_tx.send(AgentEvent::progress(
                format!("🧠 合并了 {} 条相似记忆", merged),
                None,
            )).await;
        }
    }
}

/// Spawn background task for AI memory extraction
fn spawn_memory_extraction_task(
    event_tx: tokio::sync::mpsc::Sender<AgentEvent>,
    project_path: Option<PathBuf>,
    fast_model: Option<String>,
    last_message: &matrixcode_core::providers::Message,
) {
    let text = match &last_message.content {
        matrixcode_core::providers::MessageContent::Text(t) => t.clone(),
        matrixcode_core::providers::MessageContent::Blocks(blocks) => {
            blocks.iter().filter_map(|b| match b {
                matrixcode_core::ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            }).collect::<Vec<_>>().join("\n")
        }
    };

    if text.is_empty() {
        return;
    }

    tokio::spawn(async move {
        let bg_ms = matrixcode_core::memory::MemoryStorage::new(project_path.as_deref()).ok();
        if bg_ms.is_none() {
            return;
        }
        let mut bg_ms = bg_ms.unwrap();

        let project_path_str = project_path.as_ref().map(|p| p.to_string_lossy().to_string());
        let detected = if let Some(model) = fast_model {
            matrixcode_core::debug::debug_log().log("memory_extract", &format!("Background: extracting with model={}", model));
            let extractor = matrixcode_core::memory::AiMemoryExtractor::new_minimal(model);
            matrixcode_core::memory::detect_memories_smart(
                &text, None, project_path_str.as_deref(), Some(&extractor)
            ).await
        } else {
            Vec::new()
        };

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
                        || project_path.is_some());

                if let Err(e) = bg_ms.add_entry(entry, is_project) {
                    log::warn!("Failed to add memory entry: {}", e);
                }
            }
            let _ = event_tx.send(AgentEvent::with_data(
                matrixcode_core::EventType::MemoryDetected,
                matrixcode_core::EventData::Memory {
                    summary: format!("检测到 {} 条记忆", detected_count),
                    entries_count: detected_count,
                },
            )).await;
        }
    });
}