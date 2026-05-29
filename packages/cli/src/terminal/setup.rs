//! Terminal mode setup and main entry point
//!
//! Coordinates all terminal mode components.

use anyhow::Result;
use matrixcode_core::{
    Config, SessionManager, cancel::CancellationToken,
    approval::ApproveMode,
};
use matrixcode_tui::{TuiApp, restore_terminal, setup_terminal};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::constants::{
    EVENT_CHANNEL_BUFFER, TASK_CHANNEL_BUFFER, ASK_CHANNEL_BUFFER,
    CLEANUP_TIMEOUT_MS,
};
use crate::helpers::{resolve_provider, resolve_model, resolve_base_url, load_skills, prepare_mcp_tools, prepare_lsp_servers};
use crate::types::Cli;

use super::watcher::{start_watcher_if_needed, cleanup_watcher};
use super::agent::{run_agent_task, AgentContext};

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

    // Set debug event sender for TUI debug panel
    matrixcode_core::set_debug_event_sender(event_tx.clone());

    // Create cancellation token
    let cancel_token = CancellationToken::new();

    // Load session BEFORE spawning agent task so TUI can also display restored messages
    let current_dir = std::env::current_dir().ok();
    let (full_messages, api_messages, session_mgr_state, session_metadata, effective_project_path) = 
        load_session_state(&cli, current_dir.clone());

    // Prepare agent context
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
        .unwrap_or(ApproveMode::Ask);

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
    let agent_mcp_servers = prepare_mcp_tools(
        &cli.mcp,
        cli.no_mcp,
        effective_project_path.as_ref().map(|v| v.as_path()),
    );

    // Prepare LSP servers configuration
    let agent_lsp_servers = prepare_lsp_servers(&config, effective_project_path.as_ref().map(|v| v.as_path()));

    // Enter runtime context BEFORE spawning agent task
    let _guard = rt.enter();

    // Create shared watcher handle for dynamic watcher management
    let watcher_handle_arc = Arc::new(Mutex::new(None::<tokio::task::JoinHandle<()>>));

    // Start CodeGraph watcher for auto-sync (with hidden window on Windows)
    start_watcher_if_needed(
        effective_project_path.as_ref().map(|v| v.as_path()),
        cancel_token.clone(),
        watcher_handle_arc.clone(),
    );

    let watcher_handle_for_agent = watcher_handle_arc.clone();

    // Spawn Agent task (after entering runtime context and creating watcher)
    let agent_task = rt.spawn(async move {
        let ctx = AgentContext {
            cancel_token: agent_cancel,
            event_tx: agent_event_tx,
            task_rx,
            ask_rx,
            api_key: agent_api_key,
            model: agent_model,
            base_url: agent_base_url,
            think: agent_think,
            max_tokens: agent_max_tokens,
            restored_messages: agent_restored_messages,
            project_path: agent_project_path,
            approve_mode: agent_approve_mode,
            provider_type: agent_provider,
            fast_model: agent_fast_model,
            extra_headers: agent_extra_headers,
            config: agent_config,
            skills: agent_skills,
            shared_approve_mode: agent_shared_approve_mode,
            session_mgr: session_mgr_state,
            watcher_handle: watcher_handle_for_agent,
            mcp_servers: agent_mcp_servers,
            lsp_servers: agent_lsp_servers,
        };
        run_agent_task(ctx).await;
    });

    // Debug mode
    let debug_mode = std::env::var("MATRIXCODE_DEBUG")
        .map(|v| v == "1" || v == "true" || v == "verbose")
        .unwrap_or(cfg!(debug_assertions));

    // Enable debug logging if debug mode is on
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
        .with_config(&model, cli.think.unwrap_or(config.think), cli.max_tokens, None)
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
    cleanup_watcher(&watcher_handle_arc);

    result
}

/// Load session state from previous session or create new
fn load_session_state(
    cli: &Cli,
    current_dir: Option<PathBuf>,
) -> (
    Vec<matrixcode_core::Message>,  // full_messages
    Vec<matrixcode_core::providers::Message>,  // api_messages
    Option<SessionManager>,  // session_mgr_state
    Option<matrixcode_core::session::SessionMetadata>,  // session_metadata
    Option<PathBuf>,  // effective_project_path
) {
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
}