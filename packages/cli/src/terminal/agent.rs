//! Agent task execution for terminal mode
//!
//! Handles the async agent loop with message processing.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use matrixcode_core::{
    AgentEvent, Config, SessionManager, agent::AgentBuilder, cancel::CancellationToken,
    create_provider_with_headers, infer_provider_type, providers::Provider,
    tools::all_tools_full_with_lsp, approval::ApproveMode, skills::Skill,
    tools::code_quality_hook::VerificationStrategy,
    memory::AutoMemory, prompt::preprocess_with_skills, prompt::ProcessResult,
};
use crate::constants::{
    MEMORY_SUMMARY_SIZE, MEMORY_INITIAL_SUMMARY_SIZE,
    MEMORY_TURN_CLEANUP_INTERVAL, MEMORY_MIN_ENTRIES_FOR_AI_SELECTION,
};
use super::mcp_handler::McpManager;
use super::lsp_handler::LspHandler;
use super::memory_handler::{load_memory, ai_select_memory, handle_feedback, periodic_cleanup, should_extract_memory, spawn_extraction_task_with_context};
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
    pub lsp_servers: Vec<(String, matrixcode_core::lsp::LspServerConfig)>,
}

/// Run the agent task (async portion)
pub async fn run_agent_task(mut ctx: AgentContext) {
    log::info!("Agent task: starting");
    matrixcode_core::debug::debug_log().log("agent", "Agent task: starting");

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

    log::info!("Agent task: creating provider");
    matrixcode_core::debug::debug_log().log("agent", "Creating provider...");
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

    // Create MCP manager and start servers
    log::info!("Agent task: starting MCP servers");
    matrixcode_core::debug::debug_log().log("agent", "Starting MCP servers...");
    let mcp_manager = McpManager::new();
    mcp_manager.add_servers(ctx.mcp_servers).await;
    let mcp_tools = mcp_manager.start_all(&ctx.event_tx).await;
    log::info!("Agent task: MCP servers started, {} tools", mcp_tools.len());
    matrixcode_core::debug::debug_log().log("agent", &format!("MCP servers started, {} tools", mcp_tools.len()));

    // Create LSP handler and start servers (before building system prompt)
    log::info!("Agent task: starting LSP servers");
    matrixcode_core::debug::debug_log().log("agent", "Starting LSP servers...");
    let project_root = ctx.project_path.clone().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let lsp_handler = LspHandler::new();
    lsp_handler.add_servers(ctx.lsp_servers, project_root.clone(), ctx.event_tx.clone()).await;
    lsp_handler.start_all(&ctx.event_tx).await;
    log::info!("Agent task: LSP servers started");
    matrixcode_core::debug::debug_log().log("agent", "LSP servers started");

    // Get LSP registry for tool injection
    let lsp_registry = lsp_handler.registry();

    // Get LSP server status for prompt injection
    let lsp_servers = lsp_handler.get_status().await;

    // Build system prompt with LSP integration
    let system_prompt = matrixcode_core::prompt::build_system_prompt_with_workflows_and_lsp(
        &matrixcode_core::prompt::PromptProfile::Default,
        &ctx.skills,
        project_overview.as_ref().map(|o| o.content.as_str()),
        if initial_memory_summary.is_empty() { None } else { Some(&initial_memory_summary) },
        ctx.project_path.as_ref(),
        Some(&lsp_servers),
        Some(lsp_registry.clone()),
    );

    // Build agent with tools
    let project_path_for_tools = project_root.clone();
    let mut base_tools = all_tools_full_with_lsp(
        Arc::new(ctx.skills.clone()),
        provider.clone_arc(),
        project_path_for_tools.clone(),
        Some(lsp_registry),
    );
    base_tools.extend(mcp_tools);

    let mut agent = AgentBuilder::new(provider.clone_box())
        .system_prompt(system_prompt)
        .model_name(ctx.model.clone())
        .max_tokens(ctx.max_tokens)
        .context_size(ctx.config.context_size)
        .think(ctx.think)
        .verify_strategy(VerificationStrategy::from_str(
            ctx.config.verify_strategy.as_deref().unwrap_or("post")
        ))
        .tools(base_tools)
        .project_path(project_path_for_tools)
        .event_tx(ctx.event_tx.clone())
        .approve_mode(ctx.approve_mode)
        .proxy_executor(
            matrixcode_tui::image_search::create_default_executor(),
            matrixcode_tui::image_search::get_default_proxy_tools()
        )
        .mcp_registry(mcp_manager.registry())
        .initial_messages(ctx.restored_messages.clone())
        .build();

    agent.set_approve_mode_shared(ctx.shared_approve_mode.clone());

    // Messages are already restored via initial_messages
    if !ctx.restored_messages.is_empty() {
        log::info!("Agent task: restored {} messages via builder", ctx.restored_messages.len());
    }

    log::info!("Agent task: messages restored, entering receive loop");
    matrixcode_core::debug::debug_log().log("agent", "Entering receive loop...");

    agent.set_cancel_token(ctx.cancel_token.clone());
    agent.set_ask_channel(ctx.ask_rx);

    // Send CodeGraph status if initialized
    if let Some(ref pp) = ctx.project_path {
        use matrixcode_core::tools::codegraph::CodeGraphManager;
        let manager = CodeGraphManager::with_auto_detect(pp.as_path());
        if manager.is_initialized() {
            if let Ok(status) = manager.status() {
                let _ = ctx.event_tx.send(AgentEvent::codegraph_status(status)).await;
            }
        }
    }

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
    matrixcode_core::debug::debug_log().log("agent", "Ready to receive messages");
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

        // Pre-process: detect skill/workflow triggers with skills
        let processed_msg = match preprocess_with_skills(&msg, &ctx.skills) {
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
                // Inject workflow call prompt into the message
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
                    // Extract from last 2-3 messages (user question + assistant response)
                    // This provides context for better memory extraction
                    let recent_messages: Vec<_> = messages.iter().rev().take(3).collect();
                    if !recent_messages.is_empty() {
                        // Combine recent messages for context
                        let combined_text = recent_messages.iter().rev().map(|m| {
                            match &m.content {
                                matrixcode_core::providers::MessageContent::Text(t) => t.clone(),
                                matrixcode_core::providers::MessageContent::Blocks(blocks) => {
                                    blocks.iter().filter_map(|b| match b {
                                        matrixcode_core::ContentBlock::Text { text } => Some(text.clone()),
                                        matrixcode_core::ContentBlock::Thinking { thinking, .. } => Some(thinking.clone()),
                                        _ => None,
                                    }).collect::<Vec<_>>().join("\n")
                                }
                            }
                        }).collect::<Vec<_>>().join("\n\n---\n\n");

                        spawn_extraction_task_with_context(
                            ctx.event_tx.clone(),
                            ctx.project_path.clone(),
                            ctx.fast_model.clone(),
                            &combined_text,
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