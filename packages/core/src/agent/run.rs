//! Agent run loop and public methods.

use anyhow::Result;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use tokio::sync::mpsc;

use crate::approval::ApproveMode;
use crate::cancel::CancellationToken;
use crate::compress::{
    CompressionStrategy, compress_messages, estimate_total_tokens, should_compress,
};
use crate::event::{AgentEvent, EventData, EventType};
use crate::prompt;
use crate::providers::{ChatRequest, Message, MessageContent, Role};
use crate::tools::ToolDefinition;

use super::types::{Agent, AgentBuilder, MAX_ITERATIONS};

impl Agent {
    pub(crate) fn new(builder: AgentBuilder) -> Self {
        let event_tx = builder.event_tx.unwrap_or_else(|| {
            let (tx, _) = mpsc::channel(100);
            tx
        });

        Self {
            provider: builder.provider,
            model_name: builder.model_name,
            tools: builder.tools,
            messages: Vec::new(),
            system_prompt: builder.system_prompt,
            max_tokens: builder.max_tokens,
            think: builder.think,
            approve_mode: Arc::new(AtomicU8::new(builder.approve_mode.to_u8())),
            event_tx,
            skills: builder.skills,
            profile: builder.profile,
            project_overview: builder.project_overview,
            memory_summary: builder.memory_summary,
            total_input_tokens: std::sync::atomic::AtomicU64::new(0),
            total_output_tokens: std::sync::atomic::AtomicU64::new(0),
            last_input_tokens: std::sync::atomic::AtomicU64::new(0),
            cancel_token: None,
            compression_config: crate::compress::CompressionConfig::default(),
            ask_rx: None,
        }
    }

    /// Get event sender for streaming
    pub fn event_sender(&self) -> mpsc::Sender<AgentEvent> {
        self.event_tx.clone()
    }

    /// Set ask response channel (for TUI mode)
    pub fn set_ask_channel(&mut self, rx: mpsc::Receiver<String>) {
        self.ask_rx = Some(rx);
    }

    /// Set cancellation token
    pub fn set_cancel_token(&mut self, token: CancellationToken) {
        self.cancel_token = Some(token);
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
    pub fn update_memory_summary(&mut self, summary: Option<String>) {
        self.memory_summary = summary;
        self.system_prompt = prompt::build_system_prompt(
            &self.profile,
            &self.skills,
            self.project_overview.as_deref(),
            self.memory_summary.as_deref(),
        );
    }

    /// Run chat loop with tool execution (streaming version).
    pub async fn run(&mut self, user_input: String) -> Result<Vec<AgentEvent>> {
        self.emit(AgentEvent::session_started())?;

        self.messages.push(Message {
            role: Role::User,
            content: MessageContent::Text(user_input.clone()),
        });

        let mut iterations = 0;
        let mut should_continue = true;
        const ITERATION_WARNING_THRESHOLD: usize = MAX_ITERATIONS - 10;

        while should_continue && iterations < MAX_ITERATIONS {
            iterations += 1;

            if let Some(token) = &self.cancel_token
                && token.is_cancelled()
            {
                self.emit(AgentEvent::error(
                    "Operation cancelled".to_string(),
                    None,
                    None,
                ))?;
                break;
            }

            // Warn when approaching iteration limit
            if iterations == ITERATION_WARNING_THRESHOLD {
                self.messages.push(Message {
                    role: Role::User,
                    content: MessageContent::Text(
                        "⚠️ 接近最大迭代次数限制（当前 {iterations}/{MAX_ITERATIONS}）。\
                         请检查任务进度：\n\
                         1. 如果有未完成的子任务，优先完成最关键的项\n\
                         2. 使用 todo_write 查看和更新任务状态\n\
                         3. 确保在限制内完成或在最后输出剩余任务摘要".replace("{iterations}", &iterations.to_string()).replace("{MAX_ITERATIONS}", &MAX_ITERATIONS.to_string())
                    ),
                });
            }

            let tool_defs: Vec<ToolDefinition> =
                self.tools.iter().map(|t| t.definition()).collect();
            let request = ChatRequest {
                system: Some(self.system_prompt.clone()),
                messages: self.messages.clone(),
                max_tokens: self.max_tokens,
                tools: tool_defs,
                think: self.think,
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

            // If model wants to stop (no tool calls), check for pending todos
            if !should_continue && iterations < MAX_ITERATIONS - 1 {
                if self.has_pending_todos() {
                    self.messages.push(Message {
                        role: Role::User,
                        content: MessageContent::Text(
                            "📋 检测到未完成的待办任务。请继续执行剩余任务，或在 todo_write 中将已完成的任务标记为 completed。\n\
                             注意：只有所有任务都完成后才能结束。如果遇到阻塞，请说明原因。".to_string()
                        ),
                    });
                    should_continue = true;
                }
            }

            let context_size = self.provider.context_size();
            let api_tokens = self.last_input_tokens.load(Ordering::Relaxed) as u32;
            let estimated_tokens = estimate_total_tokens(&self.messages);

            let current_tokens = if api_tokens > 0 && api_tokens >= estimated_tokens / 2 {
                api_tokens
            } else {
                estimated_tokens
            };

            crate::debug::debug_log().log(
                "compression",
                &format!(
                    "check: api={}, estimated={}, using={}, context={}, threshold={}",
                    api_tokens,
                    estimated_tokens,
                    current_tokens,
                    context_size.unwrap_or(0),
                    self.compression_config.threshold
                ),
            );

            if should_compress(current_tokens, context_size, &self.compression_config) {
                self.emit(AgentEvent::progress("Compressing context...", None))?;

                let original_tokens = current_tokens;

                match compress_messages(
                    &self.messages,
                    CompressionStrategy::SlidingWindow,
                    &self.compression_config,
                ) {
                    Ok(compressed) => {
                        let compressed_tokens = estimate_total_tokens(&compressed);
                        self.messages = compressed;
                        self.total_input_tokens
                            .store(compressed_tokens as u64, Ordering::Relaxed);
                        self.last_input_tokens
                            .store(compressed_tokens as u64, Ordering::Relaxed);

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
                            format!("Compression failed: {}", e),
                            None,
                        ))?;
                    }
                }
            }
        }
        
        // Check if we stopped due to reaching MAX_ITERATIONS
        if iterations >= MAX_ITERATIONS && should_continue {
            self.emit(AgentEvent::error(
                format!(
                    "⚠️ Reached maximum iterations limit ({} iterations).\n\n\
                    **Task status**: The task may not be fully complete.\n\n\
                    **What happened**: Agent stopped after {} iterations to prevent infinite loops.\n\n\
                    **Next steps**:\n\
                    1. Check if the task is complete\n\
                    2. If incomplete, you can:\n\
                       - Continue with more specific instructions\n\
                       - Break down the task into smaller subtasks\n\
                       - Use '/resume' to continue from current state\n\n\
                    **Why this limit exists**: Prevents runaway operations and resource exhaustion.\n\
                    **Adjustable**: Future versions will allow custom iteration limits.",
                    MAX_ITERATIONS, iterations
                ),
                Some("MAX_ITERATIONS_REACHED".to_string()),
                Some("agent/run.rs".to_string()),
            ))?;
        }
        
        self.emit(AgentEvent::usage_with_cache(
            self.total_input_tokens.load(Ordering::Relaxed),
            self.total_output_tokens.load(Ordering::Relaxed),
            0,
            0,
        ))?;

        self.emit(AgentEvent::session_ended())?;

        Ok(Vec::new())
    }

    /// Restore message history (for session continue/resume)
    pub fn set_messages(&mut self, messages: Vec<Message>) {
        self.messages = messages;
    }

    /// Get current messages (for session saving)
    pub fn get_messages(&self) -> &[Message] {
        &self.messages
    }

    /// Get current token counts
    pub fn get_token_counts(&self) -> (u64, u64) {
        (
            self.total_input_tokens.load(Ordering::Relaxed),
            self.total_output_tokens.load(Ordering::Relaxed),
        )
    }

    /// Clear message history
    pub fn clear_history(&mut self) {
        self.messages.clear();
        self.total_input_tokens.store(0, Ordering::Relaxed);
        self.total_output_tokens.store(0, Ordering::Relaxed);
        self.last_input_tokens.store(0, Ordering::Relaxed);
    }

    /// Get message count
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }
}
