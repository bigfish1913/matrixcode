//! Agent task execution for terminal mode
//!
//! Handles the async agent loop with message processing.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use matrixcode_core::{
    AgentEvent, Config, SessionManager, agent::AgentBuilder, cancel::CancellationToken,
    create_provider_with_headers, infer_provider_type, providers::Provider,
    tools::all_tools_full, approval::ApproveMode, skills::Skill,
    memory::AutoMemory,
};
use crate::constants::{
    MEMORY_SUMMARY_SIZE, MEMORY_INITIAL_SUMMARY_SIZE,
    MEMORY_TURN_CLEANUP_INTERVAL, MEMORY_MIN_ENTRIES_FOR_AI_SELECTION,
};
use super::mcp_handler::McpManager;
use super::memory_handler::{load_memory, ai_select_memory, handle_feedback, periodic_cleanup, should_extract_memory, spawn_extraction_task};
use super::commands::{handle_command, is_backend_command, CommandContext};
use super::commands::skill_activation::activate_skill;
use super::session::save_after_turn;

/// Agent runtime context
pub struct AgentContext {
    pub cancel_token: CancellationToken,
    pub event_tx: tokio::sync::mpsc::Sender<AgentEvent>,
    pub task_rx: tokio::sync::mpsc::Receiver<String>,
    pub ask_rx: tokio::sync::mpsc::Receiver<String>,
    pub api_key: String,
    pub model: String,
    pub base_url: String,
    pub think: bool,
    pub max_tokens: u32,
    pub restored_messages: Vec<matrixcode_core::providers::Message>,
    pub project_path: Option<PathBuf>,
    pub approve_mode: ApproveMode,
    pub provider_type: matrixcode_core::providers::ProviderType,
    pub fast_model: Option<String>,
    pub extra_headers: Option<std::collections::HashMap<String, String>>,
    pub config: Config,
    pub skills: Vec<Skill>,
    pub shared_approve_mode: Arc<std::sync::atomic::AtomicU8>,
    pub session_mgr: Option<SessionManager>,
    pub watcher_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    pub mcp_servers: Vec<(String, matrixcode_core::mcp::McpServerConfig)>,
}

/// Run the agent task (async portion)
pub async fn run_agent_task(mut ctx: AgentContext) {
    log::info!("Agent task: starting");

    // Send skills loaded event
    let skill_names: Vec<String> = ctx.skills.iter().map(|s| s.name.clone()).collect();
    if !skill_names.is_empty() {
        let _ = ctx.event_tx.send(AgentEvent::skills_loaded(skill_names)).await;
    }

    // Send workflows loaded event
    use matrixcode_core::workflow::WorkflowRegistry;
    let registry = WorkflowRegistry::new(ctx.project_path.as_ref());
    let workflow_names: Vec<String> = registry.list().iter().map(|w| w.name.clone()).collect();
    if !workflow_names.is_empty() {
        let _ = ctx.event_tx.send(AgentEvent::workflows_loaded(workflow_names)).await;
    }

    // Create provider
    let provider = match create_provider_with_headers(
        ctx.provider_type,
        ctx.api_key.clone(),
        ctx.model.clone(),
        Some(ctx.base_url.clone()),
        ctx.extra_headers.clone(),
    ) {
        Ok(p) => p,
        Err(e) => {
            let _ = ctx.event_tx.send(AgentEvent::error(
                format!("Failed to create provider: {}", e),
                Some("provider_error".to_string()),
                None,
            )).await;
            return;
        }
    };

    // Create fast provider for keyword extraction
    let fast_provider: Option<Box<dyn Provider>> = ctx.fast_model.as_ref().and_then(|fm| {
        let fast_type = infer_provider_type(fm);
        create_provider_with_headers(
            fast_type,
            ctx.api_key.clone(),
            fm.clone(),
            Some(ctx.base_url.clone()),
            ctx.extra_headers.clone(),
        ).ok()
    });

    // Load memory
    let (mut memory_storage, memory) = load_memory(ctx.project_path.as_ref().map(|p| p.as_path()));

    // Send MemoryLoaded event
    if let Some(ref mem) = memory
        && !mem.entries.is_empty() {
        let _ = ctx.event_tx.send(AgentEvent::with_data(
            matrixcode_core::EventType::MemoryLoaded,
            matrixcode_core::EventData::Memory {
                summary: mem.generate_prompt_summary(MEMORY_INITIAL_SUMMARY_SIZE),
                entries_count: mem.entries.len(),
            },
        )).await;
    }

    let initial_memory_summary = memory.as_ref()
        .map(|mem: &AutoMemory| mem.generate_prompt_summary(MEMORY_SUMMARY_SIZE))
        .unwrap_or_default();

    // Load project overview
    let project_overview = ctx.project_path.as_deref()
        .and_then(|path| matrixcode_core::overview::ProjectOverview::load(path).ok().flatten());

    if let Some(ref overview) = project_overview {
        matrixcode_core::debug::debug_log().log("overview", &format!("Loaded project overview: {} chars", overview.content.len()));
    }

    // Build system prompt
    let system_prompt = matrixcode_core::prompt::build_system_prompt_with_workflows(
        &matrixcode_core::prompt::PromptProfile::Default,
        &ctx.skills,
        project_overview.as_ref().map(|o| o.content.as_str()),
        if initial_memory_summary.is_empty() { None } else { Some(&initial_memory_summary) },
        ctx.project_path.as_ref(),
    );

    // Create MCP manager and start servers
    let mcp_manager = McpManager::new();
    mcp_manager.add_servers(ctx.mcp_servers).await;
    let mcp_tools = mcp_manager.start_all(&ctx.event_tx).await;

    // Build agent with tools
    let project_path_for_tools = ctx.project_path.clone().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let mut base_tools = all_tools_full(
        Arc::new(ctx.skills.clone()),
        provider.clone_arc(),
        project_path_for_tools.clone(),
    );
    base_tools.extend(mcp_tools);

    let mut agent = AgentBuilder::new(provider.clone_box())
        .system_prompt(system_prompt)
        .model_name(ctx.model.clone())
        .max_tokens(ctx.max_tokens)
        .think(ctx.think)
        .tools(base_tools)
        .project_path(project_path_for_tools)
        .event_tx(ctx.event_tx.clone())
        .approve_mode(ctx.approve_mode)
        .proxy_executor(
            matrixcode_tui::image_search::create_default_executor(),
            matrixcode_tui::image_search::get_default_proxy_tools()
        )
        .mcp_registry(mcp_manager.registry())
        .build();

    agent.set_approve_mode_shared(ctx.shared_approve_mode.clone());

    // Restore messages
    if !ctx.restored_messages.is_empty() {
        log::info!("Agent task: restoring {} messages", ctx.restored_messages.len());
        agent.set_messages(ctx.restored_messages);
    }

    log::info!("Agent task: messages restored, entering receive loop");

    agent.set_cancel_token(ctx.cancel_token.clone());
    agent.set_ask_channel(ctx.ask_rx);

    let mut turn_count: usize = 0;

    // Auto-analyze project structure on first run
    if let Some(ref pp) = ctx.project_path
        && let Some(ref mut ms) = memory_storage {
        let memory_file = pp.join(".matrix/memory.json");
        if !memory_file.exists() {
            let count = matrixcode_core::memory::generate_project_structure_memories(
                pp.as_path(),
                ms
            );
            if count > 0 {
                let _ = ctx.event_tx.send(AgentEvent::progress(
                    format!("🧠 自动分析项目结构，创建 {} 条记忆", count),
                    None,
                )).await;
            }
        }
    }

    // Main receive loop
    log::info!("Agent task: entering receive loop");
    while let Some(msg) = ctx.task_rx.recv().await {
        log::info!("Agent task: received message (len={})", msg.len());

        let mut msg = msg;

        // Check cancellation
        if ctx.cancel_token.is_cancelled() {
            ctx.event_tx.send(AgentEvent::error(
                "Operation interrupted by user".to_string(),
                Some("interrupted".to_string()),
                None,
            )).await.ok();
            ctx.cancel_token.reset();
            continue;
        }

        // Handle backend commands
        if is_backend_command(&msg, &ctx.skills) {
            let mut cmd_ctx = CommandContext {
                event_tx: &ctx.event_tx,
                project_path: &ctx.project_path,
                skills: &ctx.skills,
                config: &ctx.config,
                model: &ctx.model,
                session_mgr: &mut ctx.session_mgr,
                memory_storage: &mut memory_storage,
            };
            
            let should_refresh = handle_command(
                &msg,
                &mut cmd_ctx,
                &mut agent,
                &ctx.watcher_handle,
                &ctx.cancel_token,
                provider.as_ref(),
            ).await;
            
            if should_refresh {
                agent.refresh_codegraph_tools();
            }
            continue;
        }

        // Handle skill activation
        if let Some((transformed_msg, notification)) = activate_skill(&msg, &ctx.skills) {
            msg = transformed_msg;
            let _ = ctx.event_tx.send(AgentEvent::progress(notification, None)).await;
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
                ai_select_memory(mem, &msg, fp.as_ref(), &ctx.event_tx, &mut agent).await;
            } else {
                let keywords = matrixcode_core::memory::extract_context_keywords(&msg);
                let contextual_summary = mem.generate_contextual_summary_with_keywords(&keywords, 10);
                if !contextual_summary.is_empty() {
                    agent.update_memory_summary(Some(contextual_summary));
                }
            }
        }

        // Run agent
        turn_count += 1;

        match agent.run(msg.clone()).await {
            Ok(_) => {
                // Auto-save session
                save_after_turn(&ctx.event_tx, &mut ctx.session_mgr, &mut agent).await;

                // Handle memory feedback
                handle_feedback(&ctx.event_tx, &mut memory_storage, &msg).await;

                // Periodic memory cleanup
                if turn_count.is_multiple_of(MEMORY_TURN_CLEANUP_INTERVAL) {
                    periodic_cleanup(&ctx.event_tx, &mut memory_storage).await;
                }

                // AI memory extraction
                if should_extract_memory(turn_count, fast_provider.is_some()) {
                    let messages = agent.get_messages();
                    if let Some(last_msg) = messages.last() {
                        spawn_extraction_task(
                            ctx.event_tx.clone(),
                            ctx.project_path.clone(),
                            ctx.fast_model.clone(),
                            last_msg,
                        );
                    }
                }
            }
            Err(e) => {
                ctx.event_tx.send(AgentEvent::error(
                    format!("Agent error: {}", e),
                    Some("agent_error".to_string()),
                    None,
                )).await.ok();
            }
        }
    }
}