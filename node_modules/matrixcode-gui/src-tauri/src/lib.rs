use matrixcode_core::config::MatrixConfig;
use matrixcode_core::event::AgentEvent;
use matrixcode_core::providers::{create_provider_with_headers, Message, MessageContent};
use matrixcode_core::session::SessionManager;
use matrixcode_core::lsp::{LspManager, LspServerInfo, LspServerStatus};
use matrixcode_core::mcp::McpToolRegistry;
use matrixcode_core::skills::discover_skills;
use matrixcode_core::tools::codegraph::CodeGraphManager;
use matrixcode_core::AgentBuilder;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::Emitter;
use tokio::sync::{mpsc, Mutex, RwLock};

// ---------------------------------------------------------------------------
// AppState
// ---------------------------------------------------------------------------

/// Application state shared across all windows.
///
/// Holds core matrixcode components: session management, agent instances,
/// configuration, LSP manager, MCP registry, and CodeGraph manager.
/// Events from the agent are forwarded to the Tauri frontend via an mpsc channel.
#[allow(dead_code)]
pub struct AppState {
    config: Arc<Mutex<MatrixConfig>>,
    session_manager: Arc<Mutex<SessionManager>>,
    project_path: Arc<Mutex<Option<PathBuf>>>,
    /// LSP manager for language server connections (status tracking)
    lsp_manager: Arc<Mutex<LspManager>>,
    /// MCP registry for MCP server connections (lazy loading)
    mcp_registry: Arc<RwLock<McpToolRegistry>>,
    /// CodeGraph manager for index operations (lazy initialization)
    codegraph_manager: Arc<Mutex<Option<CodeGraphManager>>>,
}

impl AppState {
    pub fn new() -> Self {
        let config = MatrixConfig::load();
        let session_manager = Arc::new(Mutex::new(
            SessionManager::new().expect("Failed to initialize SessionManager"),
        ));
        let lsp_manager = Arc::new(Mutex::new(LspManager::new()));
        let mcp_registry = Arc::new(RwLock::new(McpToolRegistry::new()));

        Self {
            config: Arc::new(Mutex::new(config)),
            session_manager,
            project_path: Arc::new(Mutex::new(None)),
            lsp_manager,
            mcp_registry,
            codegraph_manager: Arc::new(Mutex::new(None)),
        }
    }
}

// ---------------------------------------------------------------------------
// Shared types for Tauri command responses
// ---------------------------------------------------------------------------

/// Lightweight session info for the frontend.
#[derive(serde::Serialize)]
struct SessionInfo {
    id: String,
    name: String,
    message_count: usize,
    created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    short_id: Option<String>,
}

/// Message info returned to the frontend.
#[derive(serde::Serialize)]
struct MessageInfo {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<String>,  // Thinking content from assistant messages
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp: Option<u64>,  // Message timestamp in milliseconds
}

/// Task info for the frontend task manager.
#[derive(Debug, Clone, serde::Serialize)]
struct TaskInfo {
    id: String,
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    progress: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<String>,
}

/// Sanitized config for frontend (API key masked).
#[derive(serde::Serialize)]
struct SafeConfig {
    provider: Option<String>,
    #[serde(rename = "api_key_set")]
    api_key_set: bool,
    base_url: Option<String>,
    model: Option<String>,
    think: bool,
    markdown: bool,
    max_tokens: u32,
    context_size: Option<u32>,
    multi_model: Option<bool>,
    plan_model: Option<String>,
    compress_model: Option<String>,
    fast_model: Option<String>,
    approve_mode: Option<String>,
    enable_lsp: bool,
    verify_strategy: Option<String>,
}

impl SafeConfig {
    fn from_config(config: &MatrixConfig) -> Self {
        Self {
            provider: config.provider.clone(),
            api_key_set: config.api_key.is_some(),
            base_url: config.base_url.clone(),
            model: config.model.clone(),
            think: config.think,
            markdown: config.markdown,
            max_tokens: config.max_tokens,
            context_size: config.context_size,
            multi_model: config.multi_model,
            plan_model: config.plan_model.clone(),
            compress_model: config.compress_model.clone(),
            fast_model: config.fast_model.clone(),
            approve_mode: config.approve_mode.clone(),
            enable_lsp: config.enable_lsp,
            verify_strategy: config.verify_strategy.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Session Commands
// ---------------------------------------------------------------------------

/// Create a new chat session.
#[tauri::command]
async fn create_session(
    name: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let mut mgr = state.session_manager.lock().await;
    let session = mgr
        .start_new(None::<&Path>)
        .map_err(|e| e.to_string())?;
    let session_id = session.metadata.id.clone();

    if let Some(n) = name {
        mgr.rename_current(&n).map_err(|e| e.to_string())?;
    }

    Ok(session_id)
}

/// List all sessions.
#[tauri::command]
async fn list_sessions(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SessionInfo>, String> {
    let mgr = state.session_manager.lock().await;
    let sessions = mgr.list_sessions();
    let _current_id = mgr.current_id();
    let result = sessions
        .iter()
        .map(|m| SessionInfo {
            id: m.id.clone(),
            name: m.name.clone().unwrap_or_default(),
            message_count: m.message_count,
            created_at: m.created_at.format("%Y-%m-%d %H:%M").to_string(),
            project_path: m.project_path.clone(),
            updated_at: Some(m.updated_at.format("%Y-%m-%d %H:%M").to_string()),
            short_id: Some(m.id[..8].to_string()),
        })
        .collect();
    Ok(result)
}

/// Get current session ID.
#[tauri::command]
async fn current_session(
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    let mgr = state.session_manager.lock().await;
    Ok(mgr.current_id().map(|s| s.to_string()))
}

/// Get current session metadata.
#[tauri::command]
async fn current_session_meta(
    state: tauri::State<'_, AppState>,
) -> Result<Option<SessionInfo>, String> {
    let mgr = state.session_manager.lock().await;
    Ok(mgr.current_metadata().map(|m| SessionInfo {
        id: m.id.clone(),
        name: m.name.clone().unwrap_or_default(),
        message_count: m.message_count,
        created_at: m.created_at.format("%Y-%m-%d %H:%M").to_string(),
        project_path: m.project_path.clone(),
        updated_at: Some(m.updated_at.format("%Y-%m-%d %H:%M").to_string()),
        short_id: Some(m.id[..8].to_string()),
    }))
}

/// Continue the last session.
#[tauri::command]
async fn continue_last_session(
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    let mut mgr = state.session_manager.lock().await;
    let session = mgr.continue_last().map_err(|e| e.to_string())?;
    Ok(session.map(|s| s.metadata.id.clone()))
}

/// Switch to a specific session by ID.
/// This loads the session on the backend so subsequent agent calls use its history.
#[tauri::command]
async fn switch_session(
    session_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut mgr = state.session_manager.lock().await;
    // Use resume with exact ID - the index.find will match exact ID first
    mgr.resume(&session_id)
        .map_err(|e| format!("Failed to load session {}: {}", session_id, e))?;
    Ok(())
}

/// Resume a session matching a query string.
#[tauri::command]
async fn resume_session(
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    let mut mgr = state.session_manager.lock().await;
    let session = mgr.resume(&query).map_err(|e| e.to_string())?;
    Ok(session.map(|s| s.metadata.id.clone()))
}

/// Rename the current session.
#[tauri::command]
async fn rename_session(
    new_name: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut mgr = state.session_manager.lock().await;
    mgr.rename_current(&new_name).map_err(|e| e.to_string())
}

/// Clear the current session (start fresh within the same session).
#[tauri::command]
async fn clear_session(
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut mgr = state.session_manager.lock().await;
    mgr.clear_current().map_err(|e| e.to_string())
}

/// Delete a specific session by ID.
#[tauri::command]
async fn delete_session(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut mgr = state.session_manager.lock().await;
    mgr.delete_session(&id).map_err(|e| e.to_string())
}

/// Batch delete multiple sessions by IDs.
/// Returns the number of successfully deleted sessions.
#[tauri::command]
async fn batch_delete_sessions(
    ids: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> Result<usize, String> {
    let mut mgr = state.session_manager.lock().await;
    let mut deleted_count = 0;
    let mut errors = Vec::new();

    for id in ids {
        match mgr.delete_session(&id) {
            Ok(_) => deleted_count += 1,
            Err(e) => errors.push(format!("{}: {}", id[..8].to_string(), e)),
        }
    }

    if errors.is_empty() {
        Ok(deleted_count)
    } else if deleted_count > 0 {
        // Partial success
        Err(format!("Deleted {} sessions, but failed: {}", deleted_count, errors.join(", ")))
    } else {
        // All failed
        Err(format!("Failed to delete all sessions: {}", errors.join(", ")))
    }
}

/// Get messages for the current session.
#[tauri::command]
async fn get_messages(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<MessageInfo>, String> {
    let mgr = state.session_manager.lock().await;
    let messages = mgr.display_messages();
    Ok(messages
        .map(|msgs| {
            msgs.iter()
                .map(|m| {
                    let role = format!("{:?}", m.role).to_lowercase();

                    // Extract content and thinking from blocks
                    let mut content_parts = Vec::new();
                    let mut thinking_content = None;

                    match &m.content {
                        MessageContent::Text(t) => content_parts.push(t.clone()),
                        MessageContent::Blocks(blocks) => {
                            for block in blocks {
                                match block {
                                    matrixcode_core::providers::ContentBlock::Text { text } => {
                                        content_parts.push(text.clone());
                                    }
                                    matrixcode_core::providers::ContentBlock::Thinking { thinking, .. } => {
                                        // Extract thinking content from assistant messages
                                        if role == "assistant" {
                                            thinking_content = Some(thinking.clone());
                                        }
                                    }
                                    // Skip other block types (ToolUse, ToolResult, etc.)
                                    _ => {}
                                }
                            }
                        }
                    }

                    let content = content_parts.join("\n");

                    MessageInfo {
                        role,
                        content,
                        thinking: thinking_content,
                        timestamp: None,
                    }
                })
                .collect()
        })
        .unwrap_or_default())
}

// ---------------------------------------------------------------------------
// Message / Agent Commands
// ---------------------------------------------------------------------------

/// Send a message to the agent and start streaming events.
///
/// Creates an Agent instance on-the-fly, loads conversation history from
/// the current session, runs it with the user message, and forwards all
/// AgentEvents to the Tauri frontend via `app.emit("agent-event", ...)`.
/// After completion, saves updated messages back to the session.
#[tauri::command]
async fn send_message(
    message: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    // Get config
    let config = state.config.lock().await.clone();
    let api_key = config
        .resolve_api_key()
        .ok_or("API key not configured")?;

    let model = config
        .model
        .clone()
        .or_else(|| std::env::var("MODEL").ok())
        .or_else(|| std::env::var("ANTHROPIC_MODEL").ok())
        .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());

    let provider_type = config.resolve_provider_type(&model);
    let base_url = config.resolve_base_url();
    let think = config.think;
    let max_tokens = config.max_tokens;

    let provider =
        create_provider_with_headers(provider_type, api_key, model.clone(), base_url, None)
            .map_err(|e| format!("Failed to create provider: {}", e))?;

    // Get project path
    let _project_path = state.project_path.lock().await.clone();  // Mark as unused intentionally

    // Get session messages for conversation history
    let session_manager_guard = state.session_manager.lock().await;
    let restored_messages: Vec<Message> = match session_manager_guard.api_messages() {
        Some(msgs) => msgs.to_vec(),
        None => Vec::new(),
    };
    drop(session_manager_guard); // Release lock before agent.run()

    // Drop config lock
    drop(config);

    // Create event channel
    let (agent_event_tx, mut agent_event_rx) = mpsc::channel::<AgentEvent>(256);

    // Build agent with tools
    let tools = matrixcode_core::tools::all_tools();
    let builder = AgentBuilder::new(provider)
        .model_name(&model)
        .event_tx(agent_event_tx)
        .think(think)
        .max_tokens(max_tokens)
        .tools(tools);  // ← Add all builtin tools

    // TODO: Add project_path support to AgentBuilder
    // if let Some(path) = project_path {
    //     builder = builder.project_path(path);
    // }

    let mut agent = builder.build();

    // Restore conversation history from session
    if !restored_messages.is_empty() {
        agent.set_messages(restored_messages);
    }

    // Forward agent events to Tauri frontend
    let app_handle = app.clone();
    tokio::spawn(async move {
        while let Some(event) = agent_event_rx.recv().await {
            if let Err(e) = app_handle.emit("agent-event", &event) {
                eprintln!("Failed to emit agent event: {}", e);
                break;
            }
        }
    });

    // Run the agent
    agent
        .run(message)
        .await
        .map_err(|e| format!("Agent error: {}", e))?;

    // Save updated messages back to session
    let updated_messages = agent.get_messages().to_vec();
    let mut session_manager_guard = state.session_manager.lock().await;
    session_manager_guard.set_messages(updated_messages);
    session_manager_guard.save_current().map_err(|e| {
        eprintln!("Failed to save session: {}", e);
        e.to_string()
    })?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Config Commands
// ---------------------------------------------------------------------------

/// Get the current configuration (sanitized - API key masked).
#[tauri::command]
async fn get_config(
    state: tauri::State<'_, AppState>,
) -> Result<SafeConfig, String> {
    let config = state.config.lock().await;
    Ok(SafeConfig::from_config(&config))
}

/// Update configuration fields.
#[tauri::command]
async fn update_config(
    updates: std::collections::HashMap<String, serde_json::Value>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut config = state.config.lock().await.clone();  // Clone to avoid holding lock

    // Validate and apply updates
    if let Some(serde_json::Value::String(v)) = updates.get("provider") {
        // Validate provider is a known type
        let valid_providers = ["anthropic", "openai", "openrouter", "ollama"];
        if !valid_providers.contains(&v.as_str()) {
            return Err(format!("Invalid provider: {}. Must be one of: {}", v, valid_providers.join(", ")));
        }
        config.provider = Some(v.clone());
    }
    if let Some(serde_json::Value::String(v)) = updates.get("api_key") {
        config.api_key = Some(v.clone());
    }
    if let Some(serde_json::Value::String(v)) = updates.get("base_url") {
        config.base_url = Some(v.clone());
    }
    if let Some(serde_json::Value::String(v)) = updates.get("model") {
        config.model = Some(v.clone());
    }
    if let Some(serde_json::Value::Bool(v)) = updates.get("think") {
        config.think = *v;
    }
    if let Some(serde_json::Value::Bool(v)) = updates.get("markdown") {
        config.markdown = *v;
    }
    if let Some(serde_json::Value::Number(v)) = updates.get("max_tokens") {
        let val = v.as_u64().unwrap_or(config.max_tokens as u64) as u32;
        if val < 1 || val > 128000 {
            return Err("max_tokens must be between 1 and 128000".to_string());
        }
        config.max_tokens = val;
    }
    if let Some(serde_json::Value::String(v)) = updates.get("plan_model") {
        config.plan_model = Some(v.clone());
    }
    if let Some(serde_json::Value::String(v)) = updates.get("compress_model") {
        config.compress_model = Some(v.clone());
    }
    if let Some(serde_json::Value::String(v)) = updates.get("fast_model") {
        config.fast_model = Some(v.clone());
    }
    if let Some(serde_json::Value::String(v)) = updates.get("approve_mode") {
        // Validate approve_mode
        let valid_modes = ["suggest", "auto-edit", "full-auto", "ask", "auto", "strict"];
        if !valid_modes.contains(&v.as_str()) {
            return Err(format!("Invalid approve_mode: {}. Must be one of: {}", v, valid_modes.join(", ")));
        }
        config.approve_mode = Some(v.clone());
    }
    if let Some(serde_json::Value::Bool(v)) = updates.get("enable_lsp") {
        config.enable_lsp = *v;
    }
    if let Some(serde_json::Value::String(v)) = updates.get("verify_strategy") {
        // Validate verify_strategy
        let valid_strategies = ["none", "post", "pre", "pre-quick"];
        if !valid_strategies.contains(&v.as_str()) {
            return Err(format!("Invalid verify_strategy: {}. Must be one of: {}", v, valid_strategies.join(", ")));
        }
        config.verify_strategy = Some(v.clone());
    }

    // Persist the updated config (use cloned config, don't hold lock)
    config.save().map_err(|e| format!("Failed to save config: {}", e))?;

    // Update the actual state config
    let mut state_config = state.config.lock().await;
    *state_config = config;

    Ok(())
}

/// Set the project path for the agent.
#[tauri::command]
async fn set_project_path(
    path: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let project_path = PathBuf::from(&path);
    if !project_path.exists() {
        return Err(format!("Path does not exist: {}", path));
    }
    let mut pp = state.project_path.lock().await;
    *pp = Some(project_path);
    Ok(())
}

// ---------------------------------------------------------------------------
// Task Commands
// ---------------------------------------------------------------------------

/// Get the list of tasks.
///
/// Currently returns an empty list as a placeholder.
/// Full task management will be implemented when workflow
/// integration is complete.
#[tauri::command]
async fn get_tasks() -> Result<Vec<TaskInfo>, String> {
    // Task management will be integrated with the workflow engine
    // For now, return an empty list
    Ok(vec![])
}

/// Cancel a running task.
#[tauri::command]
async fn cancel_task(_task_id: String) -> Result<(), String> {
    // Will be implemented with workflow engine integration
    Err("Task cancellation not yet implemented".to_string())
}

/// Pause a running task.
#[tauri::command]
async fn pause_task(_task_id: String) -> Result<(), String> {
    // Will be implemented with workflow engine integration
    Err("Task pausing not yet implemented".to_string())
}

/// Resume a paused task.
#[tauri::command]
async fn resume_task(_task_id: String) -> Result<(), String> {
    // Will be implemented with workflow engine integration
    Err("Task resuming not yet implemented".to_string())
}

/// Greet - simple health-check command.
#[tauri::command]
async fn greet(name: &str) -> Result<String, String> {
    Ok(format!("Hello, {}! Welcome to MatrixCode GUI.", name))
}

// ---------------------------------------------------------------------------
// Infrastructure Status Commands (LSP, CodeGraph)
// ---------------------------------------------------------------------------

/// Get LSP server status
#[tauri::command]
async fn get_lsp_status(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<LspServerInfo>, String> {
    let manager = state.lsp_manager.lock().await;
    Ok(manager.server_infos())
}

/// CodeGraph status for frontend display
#[derive(serde::Serialize)]
struct CodeGraphStatus {
    initialized: bool,
    indexing: bool,
    files_indexed: usize,
    symbols_indexed: usize,
    edges_indexed: usize,
    pending_files: Vec<String>,
    last_sync: String,
    error: Option<String>,
}

/// Get CodeGraph index status
#[tauri::command]
async fn get_codegraph_status(
    state: tauri::State<'_, AppState>,
) -> Result<Option<CodeGraphStatus>, String> {
    let project_path = state.project_path.lock().await.clone();

    if let Some(path) = project_path {
        let manager = CodeGraphManager::new(&path);

        if manager.is_initialized() {
            match manager.status() {
                Ok(status) => {
                    Ok(Some(CodeGraphStatus {
                        initialized: status.initialized,
                        indexing: false,
                        files_indexed: status.file_count as usize,
                        symbols_indexed: status.node_count as usize,
                        edges_indexed: status.edge_count as usize,
                        pending_files: vec![],
                        last_sync: "Recently".to_string(),
                        error: None,
                    }))
                }
                Err(e) => {
                    Ok(Some(CodeGraphStatus {
                        initialized: false,
                        indexing: false,
                        files_indexed: 0,
                        symbols_indexed: 0,
                        edges_indexed: 0,
                        pending_files: vec![],
                        last_sync: String::new(),
                        error: Some(e.to_string()),
                        
                    }))
                }
            }
        } else {
            Ok(Some(CodeGraphStatus {
                initialized: false,
                indexing: false,
                files_indexed: 0,
                symbols_indexed: 0,
                edges_indexed: 0,
                pending_files: vec![],
                last_sync: String::new(),
                error: None,
                
            }))
        }
    } else {
        Ok(None)
    }
}

/// Initialize CodeGraph index
#[tauri::command]
async fn initialize_codegraph(
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let project_path = state.project_path.lock().await.clone();

    if let Some(path) = project_path {
        let manager = CodeGraphManager::new(&path);
        manager.init().await.map_err(|e| e.to_string())?;
        log::info!("CodeGraph initialized for: {}", path.display());
        Ok(())
    } else {
        Err("No project path set".to_string())
    }
}

/// Reindex CodeGraph
#[tauri::command]
async fn reindex_codegraph(
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let project_path = state.project_path.lock().await.clone();

    if let Some(path) = project_path {
        let manager = CodeGraphManager::new(&path);
        manager.reinit().await.map_err(|e| e.to_string())?;
        log::info!("CodeGraph reindexed for: {}", path.display());
        Ok(())
    } else {
        Err("No project path set".to_string())
    }
}

// ---------------------------------------------------------------------------
// LSP Lifecycle Commands
// ---------------------------------------------------------------------------

/// Start an LSP server for a specific language
/// NOTE: LspManager doesn't have async start/stop methods yet
#[tauri::command]
async fn start_lsp_server(
    language: String,
) -> Result<(), String> {
    // TODO: Implement actual LSP server startup
    log::info!("LSP server start requested for: {}", language);
    Ok(())
}

/// Stop an LSP server for a specific language
#[tauri::command]
async fn stop_lsp_server(
    language: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let lsp_manager = state.lsp_manager.lock().await;
    lsp_manager.set_status(&language, LspServerStatus::NotStarted);
    log::info!("LSP server stop requested for: {}", language);
    Ok(())
}

/// Restart an LSP server for a specific language
#[tauri::command]
async fn restart_lsp_server(
    language: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let lsp_manager = state.lsp_manager.lock().await;
    lsp_manager.mark_starting(&language);
    log::info!("LSP server restart requested for: {}", language);
    Ok(())
}

// ---------------------------------------------------------------------------
// MCP Lifecycle Commands
// ---------------------------------------------------------------------------

/// MCP server info for frontend display
#[derive(serde::Serialize)]
struct McpServerInfo {
    name: String,
    status: String,
    transport: Option<String>,
    tools_count: Option<usize>,
    resources_count: Option<usize>,
    error: Option<String>,
}

/// Get MCP servers status
#[tauri::command]
async fn get_mcp_servers(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<McpServerInfo>, String> {
    let registry = state.mcp_registry.read().await;
    let statuses = registry.server_status().await;

    let infos = statuses.iter().map(|(name, status)| {
        McpServerInfo {
            name: name.clone(),
            status: if status.is_started { "started".to_string() } else { "stopped".to_string() },
            transport: None,
            tools_count: Some(status.tool_count),
            resources_count: None,
            error: None,
        }
    }).collect();

    Ok(infos)
}

/// Start an MCP server
#[tauri::command]
async fn start_mcp_server(
    server_name: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let registry = state.mcp_registry.read().await;

    // Get the placeholder for this server
    let placeholder = registry.get_server(&server_name)
        .ok_or_else(|| format!("MCP server '{}' not found in registry", server_name))?;

    // Try to start the server
    match placeholder.start().await {
        Ok(_) => {
            log::info!("MCP server '{}' started successfully", server_name);
            Ok(())
        }
        Err(e) => {
            log::error!("Failed to start MCP server '{}': {}", server_name, e);
            Err(format!("Failed to start MCP server '{}': {}", server_name, e))
        }
    }
}

/// Stop an MCP server
#[tauri::command]
async fn stop_mcp_server(
    server_name: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let registry = state.mcp_registry.read().await;

    // Get the placeholder for this server
    let placeholder = registry.get_server(&server_name)
        .ok_or_else(|| format!("MCP server '{}' not found in registry", server_name))?;

    match placeholder.shutdown().await {
        Ok(_) => {
            log::info!("MCP server '{}' stopped", server_name);
            Ok(())
        }
        Err(e) => {
            log::error!("Failed to stop MCP server '{}': {}", server_name, e);
            Err(format!("Failed to stop MCP server '{}': {}", server_name, e))
        }
    }
}

// ---------------------------------------------------------------------------
// CodeGraph Watcher Commands
// ---------------------------------------------------------------------------

/// Check if CodeGraph watcher daemon is running
#[tauri::command]
async fn is_watcher_running(
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    // For now, check if CodeGraph is initialized (simplified)
    let project_path = state.project_path.lock().await.clone();

    if let Some(path) = project_path {
        let manager = CodeGraphManager::new(&path);
        Ok(manager.is_initialized())
    } else {
        Ok(false)
    }
}

/// Start CodeGraph watcher daemon
#[tauri::command]
async fn start_watcher(
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let project_path = state.project_path.lock().await.clone();

    if let Some(path) = project_path {
        // Initialize CodeGraph if not already
        let manager = CodeGraphManager::new(&path);
        if !manager.is_initialized() {
            manager.init().await.map_err(|e| e.to_string())?;
        }
        log::info!("Watcher started (simplified) for: {}", path.display());
        Ok(())
    } else {
        Err("No project path set".to_string())
    }
}

/// Stop CodeGraph watcher daemon
#[tauri::command]
async fn stop_watcher() -> Result<(), String> {
    // Placeholder - would need watcher handle tracking
    log::info!("Watcher stop requested (placeholder)");
    Ok(())
}

// ---------------------------------------------------------------------------
// Tools & Skills Commands
// ---------------------------------------------------------------------------

/// Tool definition for frontend display
#[derive(serde::Serialize)]
struct ToolDefinition {
    name: String,
    description: String,
    parameters: Option<serde_json::Value>,
    is_priority: Option<bool>,
}

/// List available tools
#[tauri::command]
async fn list_tools() -> Result<Vec<ToolDefinition>, String> {
    // Get all builtin tools from matrixcode-core
    let tools = matrixcode_core::tools::all_tools();

    let definitions = tools.iter().map(|tool| {
        let def = tool.definition();
        ToolDefinition {
            name: def.name.clone(),
            description: def.description.clone(),
            parameters: Some(def.parameters.clone()),
            is_priority: None, // TODO: Add priority info
        }
    }).collect();

    Ok(definitions)
}

/// Skill info for frontend display
#[derive(serde::Serialize)]
struct SkillInfo {
    name: String,
    description: String,
    trigger: Option<String>,
    source_file: Option<String>,
}

/// List loaded skills
#[tauri::command]
async fn list_skills(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SkillInfo>, String> {
    // Get project path
    let project_path = state.project_path.lock().await.clone();

    // Discover skills from multiple roots
    let mut roots = Vec::new();

    // Project-local skills directory
    if let Some(path) = project_path {
        roots.push(path.join(".matrix").join("skills"));
    }

    // User-global skills directory
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join(".matrix").join("skills"));
    }

    // Load skills
    let skills = discover_skills(&roots);

    // Convert to SkillInfo
    let infos = skills.iter().map(|skill| {
        SkillInfo {
            name: skill.name.clone(),
            description: skill.description.clone(),
            trigger: skill.trigger.clone(),
            source_file: Some(skill.source_file.to_string_lossy().to_string()),
        }
    }).collect();

    Ok(infos)
}

// ---------------------------------------------------------------------------
// Application entry point
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            greet,
            // Session commands
            create_session,
            list_sessions,
            current_session,
            current_session_meta,
            continue_last_session,
            switch_session,
            resume_session,
            rename_session,
            clear_session,
            delete_session,
            batch_delete_sessions,
            get_messages,
            // Message / Agent commands
            send_message,
            // Config commands
            get_config,
            update_config,
            set_project_path,
            // Task commands
            get_tasks,
            cancel_task,
            pause_task,
            resume_task,
            // Infrastructure status commands
            get_lsp_status,
            get_codegraph_status,
            initialize_codegraph,
            reindex_codegraph,
            // LSP lifecycle commands
            start_lsp_server,
            stop_lsp_server,
            restart_lsp_server,
            // MCP lifecycle commands
            get_mcp_servers,
            start_mcp_server,
            stop_mcp_server,
            // CodeGraph watcher commands
            is_watcher_running,
            start_watcher,
            stop_watcher,
            // Tools & Skills commands
            list_tools,
            list_skills,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}