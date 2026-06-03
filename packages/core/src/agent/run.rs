//! Agent run loop and public methods.

use anyhow::Result;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use tokio::sync::mpsc;

use crate::approval::ApproveMode;
use crate::cancel::CancellationToken;
use crate::compress::{CompressionConfig, CompressionStrategy, compress_messages, estimate_total_tokens, should_compress};
use crate::event::{AgentEvent, EventData, EventType};
use crate::prompt::{PromptProfile, preprocess::{preprocess_with_skills, ProcessResult}};
use crate::providers::{ChatRequest, Message, MessageContent, Role};
use crate::skills::Skill;
use crate::tools::Tool;
use crate::tools::ToolDefinition;
use crate::tools::toolproxy::{ProxyToolDef, ProxyToolExecutor};

use super::core::{AgentConfig, AgentState};
use super::context::AgentContext;
use super::session::SessionManager;
use super::types::{Agent, AgentBuilder, MAX_ITERATIONS};

#[allow(dead_code)]
impl Agent {
    pub(crate) fn new(builder: AgentBuilder) -> Self {
        // Create event channel if not provided
        let event_tx = builder.event_tx.unwrap_or_else(|| {
            let (tx, _) = mpsc::channel(100);
            tx
        });

        // Create modular components from builder
        let config = AgentConfig::new(
            builder.max_tokens,
            builder.context_size_override,
            builder.think,
            builder.compression_config,
        );

        let state = AgentState::new();

        // AgentContext builds system_prompt internally from profile, skills, overview, memory
        let context = AgentContext::with_context(
            builder.profile,
            builder.skills,
            builder.project_overview,
            builder.memory_summary,
            builder.project_path,
        );

        // SessionManager handles pending_input_rx and ask_rx
        let session = SessionManager::with_all_channels(
            event_tx.clone(),
            None, // ask_rx will be set later if needed
            builder.pending_input_rx,
        );

        Self {
            // Core components
            config,
            state,
            context,
            session,

            // Provider & Tools
            provider: builder.provider,
            model_name: builder.model_name,
            tools: builder.tools,

            // Event channel
            event_tx,

            // Approval
            approve_mode: Arc::new(AtomicU8::new(builder.approve_mode.to_u8())),

            // Proxy tools
            proxy_tool_defs: builder.proxy_tool_defs,
            proxy_executor: builder.proxy_executor,

            // External registries
            mcp_registry: builder.mcp_registry,
            lsp_registry: builder.lsp_registry,
        }
    }

    // === Field Accessors (delegating to components) ===
    // Some methods may be unused now but kept for future extensibility.

    /// Get messages (from state)
    pub(crate) fn messages(&self) -> &Vec<Message> {
        self.state.messages()
    }

    /// Get mutable messages (from state)
    pub(crate) fn messages_mut(&mut self) -> &mut Vec<Message> {
        self.state.messages_mut()
    }

    /// Get system prompt (from context)
    pub(crate) fn system_prompt(&self) -> &str {
        self.context.system_prompt()
    }

    /// Get max tokens (from config)
    pub(crate) fn max_tokens(&self) -> u32 {
        self.config.max_tokens()
    }

    /// Get context size override (from config)
    pub(crate) fn context_size_override(&self) -> Option<u32> {
        self.config.context_size_override()
    }

    /// Get think flag (from config)
    pub(crate) fn think(&self) -> bool {
        self.config.think()
    }

    /// Get compression config (from config)
    pub(crate) fn compression_config(&self) -> &CompressionConfig {
        self.config.compression_config()
    }

    /// Get mutable compression config (from config)
    pub(crate) fn compression_config_mut(&mut self) -> &mut CompressionConfig {
        self.config.compression_config_mut()
    }

    /// Get cancellation token (from session)
    pub(crate) fn cancel_token(&self) -> Option<&CancellationToken> {
        self.session.cancel_token()
    }

    /// Get event sender (direct field access)
    pub(crate) fn event_tx(&self) -> &mpsc::Sender<AgentEvent> {
        &self.event_tx
    }

    /// Get skills (from context)
    pub(crate) fn skills(&self) -> &[Skill] {
        self.context.skills()
    }

    /// Get profile (from context)
    pub(crate) fn profile(&self) -> &PromptProfile {
        self.context.profile()
    }

    /// Get project overview (from context)
    pub(crate) fn project_overview(&self) -> Option<&str> {
        self.context.project_overview()
    }

    /// Get memory summary (from context)
    pub(crate) fn memory_summary(&self) -> Option<&str> {
        self.context.memory_summary()
    }

    /// Get project path (from context)
    pub(crate) fn project_path(&self) -> Option<&std::path::PathBuf> {
        self.context.project_path()
    }

    /// Check if cancelled (from session)
    pub(crate) fn is_cancelled(&self) -> bool {
        self.session.is_cancelled()
    }

    /// Get total input tokens (from state)
    pub(crate) fn total_input_tokens(&self) -> u64 {
        self.state.total_input_tokens()
    }

    /// Get total output tokens (from state)
    pub(crate) fn total_output_tokens(&self) -> u64 {
        self.state.total_output_tokens()
    }

    /// Get last input tokens (from state)
    pub(crate) fn last_input_tokens(&self) -> u64 {
        self.state.last_input_tokens()
    }

    /// Get todo reminder count (from state)
    pub(crate) fn todo_reminder_count(&self) -> &std::collections::HashMap<String, usize> {
        self.state.todo_reminder_count_map()
    }

    /// Get mutable todo reminder count (from state)
    pub(crate) fn todo_reminder_count_mut(&mut self) -> &mut std::collections::HashMap<String, usize> {
        self.state.todo_reminder_count_map_mut()
    }

    /// Get pending inputs (from state)
    pub(crate) fn pending_inputs(&self) -> &Vec<String> {
        self.state.pending_inputs_vec()
    }

    /// Get mutable pending inputs (from state)
    pub(crate) fn pending_inputs_mut(&mut self) -> &mut Vec<String> {
        self.state.pending_inputs_vec_mut()
    }

    /// Get ask rx (from session)
    pub(crate) fn ask_rx(&mut self) -> Option<&mut mpsc::Receiver<String>> {
        self.session.ask_rx()
    }

    /// Effective context window size, preferring explicit configuration over model inference.
    pub(crate) fn effective_context_size(&self) -> Option<u32> {
        self.config.context_size_override()
            .or_else(|| self.provider.context_size())
    }

    /// Get event sender for streaming
    pub fn event_sender(&self) -> mpsc::Sender<AgentEvent> {
        self.event_tx.clone()
    }

    /// Set ask response channel (for TUI mode) - delegates to SessionManager
    pub fn set_ask_channel(&mut self, rx: mpsc::Receiver<String>) {
        self.session.set_ask_channel(rx);
    }

    /// Check if ask channel is available
    pub(crate) fn has_ask_channel(&self) -> bool {
        self.session.has_ask_channel()
    }

    /// Get ask channel receiver (for approval/ask tool)
    pub(crate) fn ask_channel(&mut self) -> Option<&mut mpsc::Receiver<String>> {
        self.session.ask_rx()
    }

    /// 设置代理工具执行器
    pub fn set_proxy_executor(
        &mut self,
        executor: Arc<dyn ProxyToolExecutor>,
        tool_defs: Vec<ProxyToolDef>,
    ) {
        self.proxy_executor = Some(executor);
        self.proxy_tool_defs = tool_defs;
    }

    /// Set cancellation token - delegates to SessionManager
    pub fn set_cancel_token(&mut self, token: CancellationToken) {
        self.session.set_cancel_token(token);
    }

    /// Get cancellation token reference
    pub(crate) fn get_cancel_token(&self) -> Option<&CancellationToken> {
        self.session.cancel_token()
    }

    /// Set approve mode at runtime
    pub fn set_approve_mode(&mut self, mode: ApproveMode) {
        let old = ApproveMode::from_u8(self.approve_mode.load(Ordering::Relaxed));
        log::info!("Agent approve mode changed: {} -> {}", old, mode);
        self.approve_mode.store(mode.to_u8(), Ordering::Relaxed);
    }

    /// Get a shared reference to the approve mode atomic.
    pub fn approve_mode_shared(&self) -> Arc<AtomicU8> {
        self.approve_mode.clone()
    }

    /// Replace the internal approve mode with an externally-created shared atomic.
    pub fn set_approve_mode_shared(&mut self, shared: Arc<AtomicU8>) {
        self.approve_mode = shared;
    }

    /// Update memory summary and rebuild system prompt.
    /// Note: Uses build_system_prompt (without project_path) to preserve cache.
    pub fn update_memory_summary(&mut self, summary: Option<String>) {
        self.context.update_memory(summary);
        // Context is now the source of truth for system_prompt
    }

    /// Refresh CodeGraph tools after /init or codegraph init.
    /// This rebuilds both tools and system prompt with project_path.
    /// Call this only when CodeGraph state changes (not every request) to preserve cache.
    pub fn refresh_codegraph_tools(&mut self) {
        if let Some(path) = self.context.project_path() {
            // Check if CodeGraph should be injected now
            let should_have_codegraph =
                crate::tools::codegraph::should_inject_codegraph_tools(path);

            // Check if we currently have CodeGraph tools
            let has_codegraph = self.tools.iter().any(|t| {
                let name = t.definition().name;
                name.starts_with("code_") && name != "code_review"
            });

            // Only update if state changed
            if should_have_codegraph != has_codegraph {
                // Add or remove CodeGraph tools
                if should_have_codegraph {
                    let codegraph_tools = crate::tools::codegraph::codegraph_tools(path);
                    for tool in codegraph_tools {
                        self.tools.push(Arc::from(tool));
                    }
                } else {
                    // Remove CodeGraph tools
                    self.tools.retain(|t| {
                        let name = t.definition().name;
                        !name.starts_with("code_") || name == "code_review"
                    });
                }
                // Update system prompt via context (includes/excludes CodeGraph rules)
                self.context.rebuild_system_prompt_with_workflows(Some(path.clone()));
            }
        }
    }

    /// Run chat loop with tool execution (streaming version).
    pub async fn run(&mut self, user_input: String) -> Result<Vec<AgentEvent>> {
        self.emit(AgentEvent::session_started())?;

        // Step 1: 预处理 - 检测技能/工作流触发
        let preprocess_result = self.preprocess_input(&user_input);

        // Step 2: 如果有阻塞触发的技能，先注入技能内容
        let processed_input = match preprocess_result {
            ProcessResult::SkillTriggered {
                skill_id,
                confidence,
                skill_body,
            } => {
                log::info!(
                    "Skill triggered: {} (confidence: {:.2})",
                    skill_id,
                    confidence
                );
                self.emit(AgentEvent::progress(
                    format!("🎯 触发技能: {}", skill_id),
                    None,
                ))?;

                // 注入技能内容作为系统提示上下文
                if let Some(body) = skill_body {
                    // 技能内容已自动加载，直接注入到用户输入前
                    let enhanced_input = format!(
                        "<command-name>{}</command-name>\n\n{}\n\n---\n\nUser request: {}",
                        skill_id,
                        body,
                        user_input
                    );
                    enhanced_input
                } else {
                    // 技能未自动加载，添加提示让模型调用 skill 工具
                    let enhanced_input = format!(
                        "User invoked skill '{}'. Use the `skill` tool with name '{}' to load its instructions before proceeding.\n\nUser request: {}",
                        skill_id,
                        skill_id,
                        user_input
                    );
                    enhanced_input
                }
            }
            ProcessResult::WorkflowTriggered {
                workflow_id,
                inputs,
            } => {
                log::info!("Workflow triggered: {} with inputs: {:?}", workflow_id, inputs);
                self.emit(AgentEvent::progress(
                    format!("🔄 触发工作流: {}", workflow_id),
                    None,
                ))?;
                // 工作流触发：注入提示让模型知道应该执行工作流
                let inputs_json = serde_json::to_string_pretty(&inputs).unwrap_or_default();
                let enhanced_input = format!(
                    "Workflow '{}' triggered with extracted inputs:\n{}\n\nUser request: {}",
                    workflow_id,
                    inputs_json,
                    user_input
                );
                enhanced_input
            }
            ProcessResult::Continue => {
                // 无触发，正常处理
                user_input
            }
        };

        // Step 3: 添加处理后的用户消息
        self.state.add_message(Message {
            role: Role::User,
            content: MessageContent::Text(processed_input),
        });

        let mut iterations = 0;
        let mut should_continue = true;
        const ITERATION_WARNING_THRESHOLD: usize = MAX_ITERATIONS - 10;

        while should_continue && iterations < MAX_ITERATIONS {
            iterations += 1;

            // Check for pending inputs BEFORE building request
            // This ensures appended messages are sent in this iteration's API call
            self.drain_pending_inputs();
            if self.has_pending_inputs() {
                let pending = self.take_pending_inputs();
                let count = pending.len();
                let merged = pending.join("\n\n---\n\n");
                log::info!("Adding {} pending input messages to request", count);

                // Send queue processed event to TUI with messages content
                self.emit(AgentEvent::queue_processed(count, pending.clone()))?;

                self.state.add_message(Message {
                    role: Role::User,
                    content: MessageContent::Text(merged),
                });
            }

            if self.session.is_cancelled() {
                self.emit(AgentEvent::error(
                    crate::prompt::MSG_OPERATION_CANCELLED.to_string(),
                    None,
                    None,
                ))?;
                break;
            }

            // Warn when approaching iteration limit (UI only, not in messages history)
            if iterations == ITERATION_WARNING_THRESHOLD {
                self.emit(AgentEvent::progress(
                    crate::prompt::MSG_ITERATION_WARNING_UI
                        .replace("{iterations}", &iterations.to_string())
                        .replace("{max_iterations}", &MAX_ITERATIONS.to_string()),
                    None,
                ))?;
            }

            // Proactive compression: check context size BEFORE API call
            // For long conversations, compress early to avoid timeout issues
            let context_size = self.effective_context_size();
            let estimated_tokens = estimate_total_tokens(self.state.messages());

            if should_compress(estimated_tokens, context_size, self.config.compression_config()) {
                self.emit(AgentEvent::progress("⚠️ 上下文过大，正在预压缩...", None))?;

                match compress_messages(
                    self.state.messages(),
                    CompressionStrategy::SlidingWindow,
                    self.config.compression_config(),
                ) {
                    Ok(compressed) => {
                        let compressed_tokens = estimate_total_tokens(&compressed);
                        self.state.set_messages(compressed);
                        crate::debug::debug_log().compression(
                            estimated_tokens,
                            compressed_tokens,
                            compressed_tokens as f32 / estimated_tokens as f32,
                        );
                    }
                    Err(e) => {
                        self.emit(AgentEvent::progress(format!("预压缩失败: {}", e), None))?;
                    }
                }
            }

            // Build request with current messages (including any pending inputs)
            let tool_defs: Vec<ToolDefinition> = {
                let mut defs: Vec<ToolDefinition> = self
                    .tools
                    .iter()
                    .map(|t| {
                        let def = t.definition();
                        let description = def.description_for_llm();
                        ToolDefinition {
                            name: def.name,
                            description,
                            parameters: def.parameters,
                            is_priority: def.is_priority,
                        }
                    })
                    .collect();
                // 添加代理工具定义
                defs.extend(self.proxy_tool_defs.iter().map(|t| {
                    let def = &t.definition;
                    let description = def.description_for_llm();
                    ToolDefinition {
                        name: def.name.clone(),
                        description,
                        parameters: def.parameters.clone(),
                        is_priority: def.is_priority,
                    }
                }));
                defs
            };
            let request = ChatRequest {
                system: Some(self.system_prompt().to_string()),
                messages: self.state.messages().clone(),
                max_tokens: self.max_tokens(),
                tools: tool_defs,
                think: self.think(),
                enable_caching: true,
                server_tools: Vec::new(),
            };

            let response = self.call_streaming(&request).await?;

            self.track_usage(&response.usage);

            crate::debug::debug_log().api_call(
                &self.model_name,
                response.usage.input_tokens,
                response.usage.cache_read_input_tokens > 0,
            );

            should_continue = self.process_response(&response).await?;

            // If model wants to stop, check for pending inputs first (higher priority than todos)
            // This ensures appended messages are processed before session ends
            if !should_continue && iterations < MAX_ITERATIONS - 1 {
                // Final drain of pending inputs before checking todos
                self.drain_pending_inputs();

                if self.has_pending_inputs() {
                    log::info!("Agent: found pending inputs at session end, continuing loop");
                    should_continue = true;
                    continue; // Will be processed at start of next iteration
                }

                // Then check for pending todos
                // First check if we just sent a reminder (prevent immediate duplicate)
                if self.last_message_was_todo_reminder() {
                    log::info!("Skipping todo check: reminder already sent in recent messages");
                } else {
                    const MAX_TODO_REMINDERS: usize = 2;

                    // Clone todo_reminder_count to avoid borrow conflict
                    let reminder_count_clone = self.state.todo_reminder_count_map().clone();
                    let (pending, all_at_limit) = self.get_pending_todos_with_limit(
                        &reminder_count_clone,
                        MAX_TODO_REMINDERS
                    );

                    if !pending.is_empty() {
                        // Update reminder counts for todos we're about to remind about
                        for (_, content) in &pending {
                            self.state.increment_todo_reminder(content.clone());
                        }

                        let pending_list = pending
                            .iter()
                            .map(|(status, content)| {
                                let marker = match status.as_str() {
                                    "in_progress" => "[~]",
                                    "pending" => "[ ]",
                                    _ => "[?]",
                                };
                                format!("  {} {}", marker, content)
                            })
                            .collect::<Vec<_>>()
                            .join("\n");

                        let reminder = format!(
                            "📋 任务尚未完成。以下待办项需要处理：\n{}\n\n请继续执行，或在 todo_write 中标记为 completed。如遇阻塞请说明原因。",
                            pending_list
                        );

                        self.state.add_message(Message {
                            role: Role::User,
                            content: MessageContent::Text(reminder),
                        });
                        should_continue = true;
                    } else if all_at_limit && !self.state.todo_reminder_count_map().is_empty() {
                        // All todos have reached reminder limit, allow session to end
                        // but inform user that todos remain incomplete
                        let remaining_count = self.state.todo_reminder_count_map().len();
                        self.emit(AgentEvent::progress(
                            format!(
                                "⚠️ 会话结束：{} 个待办项未完成（已提醒 {} 次，达到上限）",
                                remaining_count, MAX_TODO_REMINDERS
                            ),
                            None,
                        ))?;
                        log::warn!(
                            "Session ending with {} incomplete todos (reminder limit reached)",
                            remaining_count
                        );
                    }
                }
            }

            let context_size = self.effective_context_size();
            let api_tokens = self.state.last_input_tokens() as u32;
            let estimated_tokens = estimate_total_tokens(self.state.messages());

            let current_tokens = if api_tokens > 0 && api_tokens >= estimated_tokens / 2 {
                api_tokens
            } else {
                estimated_tokens
            };

            // Only log compression check when context is getting full (> 30%)
            // This avoids cluttering debug panel with meaningless checks
            if let Some(ctx_size) = context_size {
                // Send context size to TUI for accurate display
                self.emit(AgentEvent::with_data(
                    EventType::ContextSize,
                    EventData::ContextSize {
                        context_size: ctx_size as u64,
                    },
                ))?;

                let usage_ratio = current_tokens as f64 / ctx_size as f64;
                if usage_ratio >= 0.3 {
                    crate::debug::debug_log().log(
                        "checkcompress",
                        &format!(
                            "usage={:.1}%, tokens={}, context={}, threshold={}%",
                            usage_ratio * 100.0,
                            current_tokens,
                            ctx_size,
                            self.config.compression_config().threshold * 100.0
                        ),
                    );
                }
            }

            if should_compress(current_tokens, context_size, self.config.compression_config()) {
                self.emit(AgentEvent::progress(crate::prompt::MSG_COMPRESSING_CONTEXT, None))?;

                let original_tokens = current_tokens;

                match compress_messages(
                    self.state.messages(),
                    CompressionStrategy::SlidingWindow,
                    self.config.compression_config(),
                ) {
                    Ok(compressed) => {
                        let compressed_tokens = estimate_total_tokens(&compressed);
                        self.state.set_messages(compressed);
                        self.state.set_total_input_tokens(compressed_tokens as u64);
                        self.state.set_last_input_tokens(compressed_tokens as u64);

                        let ratio = compressed_tokens as f32 / original_tokens as f32;
                        crate::debug::debug_log().compression(
                            original_tokens,
                            compressed_tokens,
                            ratio,
                        );

                        self.emit(AgentEvent::with_data(
                            EventType::CompressionCompleted,
                            EventData::Compression {
                                original_tokens: original_tokens as u64,
                                compressed_tokens: compressed_tokens as u64,
                                ratio: compressed_tokens as f32 / original_tokens as f32,
                            },
                        ))?;
                    }
                    Err(e) => {
                        self.emit(AgentEvent::progress(
                            format!("{}{}", crate::prompt::MSG_COMPRESSION_FAILED, e),
                            None,
                        ))?;
                    }
                }
            }
        }

        // Check if we stopped due to reaching MAX_ITERATIONS
        if iterations >= MAX_ITERATIONS && should_continue {
            self.emit(AgentEvent::error(
                crate::prompt::MSG_MAX_ITERATIONS_REACHED
                    .replace("{max_iterations}", &MAX_ITERATIONS.to_string())
                    .replace("{iterations}", &iterations.to_string()),
                Some("MAX_ITERATIONS_REACHED".to_string()),
                Some("agent/run.rs".to_string()),
            ))?;
        }

        self.emit(AgentEvent::usage_with_cache(
            self.state.total_input_tokens(),
            self.state.total_output_tokens(),
            0,
            0,
        ))?;

        self.emit(AgentEvent::session_ended())?;

        Ok(Vec::new())
    }

    /// Restore message history (for session continue/resume)
    pub fn set_messages(&mut self, messages: Vec<Message>) {
        self.state.set_messages(messages);
    }

    /// Get current messages (for session saving)
    pub fn get_messages(&self) -> &[Message] {
        self.messages()
    }

    /// Get available tools
    pub fn get_tools(&self) -> &[Arc<dyn Tool>] {
        &self.tools
    }

    /// Get system prompt
    pub fn get_system_prompt(&self) -> &str {
        self.system_prompt()
    }

    /// Get current token counts
    pub fn get_token_counts(&self) -> (u64, u64) {
        (
            self.state.total_input_tokens(),
            self.state.total_output_tokens(),
        )
    }

    /// Clear message history
    pub fn clear_history(&mut self) {
        self.messages_mut().clear();
        self.state.set_total_input_tokens(0);
        self.state.set_total_output_tokens(0);
        self.state.set_last_input_tokens(0);
    }

    /// Get message count
    pub fn message_count(&self) -> usize {
        self.messages().len()
    }

    // ========================================================================
    // Skill/Workflow Trigger Detection
    // ========================================================================

    /// 预处理用户输入，检测技能/工作流触发
    ///
    /// # 触发类型处理
    /// - **slash_command** (/review, /debug): 阻塞调用，自动注入技能内容
    /// - **keyword** ("审查代码", "调试问题"): 阻塞调用，自动注入技能内容
    /// - **workflow**: 注入工作流上下文，让模型执行工作流
    ///
    /// # Returns
    /// - `SkillTriggered`: 技能被触发，包含技能ID、置信度和可选的技能内容
    /// - `WorkflowTriggered`: 工作流被触发，包含工作流ID和提取的输入
    /// - `Continue`: 无触发，继续正常处理
    pub fn preprocess_input(&self, user_input: &str) -> ProcessResult {
        // 使用动态触发加载：从已加载的技能中提取触发模式
        preprocess_with_skills(user_input, self.skills())
    }

    /// 强制执行触发的技能（注入技能内容到消息历史）
    ///
    /// 当技能触发时，此方法将技能内容作为系统上下文注入，
    /// 确保模型在处理用户请求之前已经加载了技能指令。
    ///
    /// # Arguments
    /// * `skill_id` - 技能标识符
    /// * `skill_body` - 技能内容（如果已自动加载）
    ///
    /// # Returns
    /// 注入后的增强消息内容
    pub fn inject_skill_context(&self, skill_id: &str, skill_body: Option<&str>) -> String {
        if let Some(body) = skill_body {
            format!(
                "<command-name>{}</command-name>\n\n{}\n\n**Important**: Follow the skill instructions above before responding to the user request below.",
                skill_id,
                body.trim_end()
            )
        } else {
            format!(
                "Skill '{}' was triggered but not auto-loaded. The model should call the `skill` tool with name '{}' to load its instructions.",
                skill_id,
                skill_id
            )
        }
    }

    // ========================================================================
    // MCP Runtime Management
    // ========================================================================

    /// 动态添加 MCP 服务器
    ///
    /// # Example
    /// ```ignore
    /// use matrixcode_core::mcp::McpServerConfig;
    ///
    /// let config = McpServerConfig::stdio("npx", vec!["-y", "@playwright/mcp@latest"]);
    /// agent.add_mcp_server("playwright", config).await?;
    /// ```
    pub async fn add_mcp_server(
        &mut self,
        name: &str,
        config: crate::mcp::McpServerConfig,
    ) -> Result<()> {
        if let Some(registry) = &self.mcp_registry {
            let mut reg = registry.write().await;
            reg.add_server(name.to_string(), config);
            log::info!("MCP server '{}' added to registry", name);
        } else {
            log::warn!("MCP registry not initialized, cannot add server '{}'", name);
        }
        Ok(())
    }

    /// 移除 MCP 服务器
    pub async fn remove_mcp_server(&mut self, name: &str) -> Result<()> {
        if let Some(registry) = &self.mcp_registry {
            let mut reg = registry.write().await;
            reg.remove_server(name).await?;
            log::info!("MCP server '{}' removed from registry", name);
        }
        Ok(())
    }

    /// 获取 MCP 服务器状态列表
    pub async fn mcp_server_status(&self) -> Vec<crate::mcp::ServerStatus> {
        if let Some(registry) = &self.mcp_registry {
            let reg = registry.read().await;
            reg.server_status().await.values().cloned().collect()
        } else {
            Vec::new()
        }
    }

    /// 启动指定的 MCP 服务器
    pub async fn start_mcp_server(
        &self,
        name: &str,
    ) -> Result<Vec<Arc<crate::mcp::McpToolWrapper>>> {
        if let Some(registry) = &self.mcp_registry {
            let reg = registry.read().await;
            if let Some(placeholder) = reg.get_server(name) {
                let tools = placeholder.start().await?;
                log::info!("MCP server '{}' started with {} tools", name, tools.len());
                Ok(tools)
            } else {
                Err(anyhow::anyhow!(
                    "MCP server '{}' not found in registry",
                    name
                ))
            }
        } else {
            Err(anyhow::anyhow!("MCP registry not initialized"))
        }
    }
}
