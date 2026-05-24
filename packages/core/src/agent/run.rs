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
                    prompt::MSG_OPERATION_CANCELLED.to_string(),
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
                        prompt::MSG_ITERATION_WARNING
                            .replace("{iterations}", &iterations.to_string())
                            .replace("{max_iterations}", &MAX_ITERATIONS.to_string()),
                    ),
                });
            }

            // Proactive compression: check context size BEFORE API call
            // For long conversations, compress early to avoid timeout issues
            let context_size = self.provider.context_size();
            let estimated_tokens = estimate_total_tokens(&self.messages);

            if should_compress(estimated_tokens, context_size, &self.compression_config) {
                self.emit(AgentEvent::progress("⚠️ 上下文过大，正在预压缩...", None))?;

                match compress_messages(
                    &self.messages,
                    CompressionStrategy::SlidingWindow,
                    &self.compression_config,
                ) {
                    Ok(compressed) => {
                        let compressed_tokens = estimate_total_tokens(&compressed);
                        self.messages = compressed;
                        crate::debug::debug_log().compression(
                            estimated_tokens,
                            compressed_tokens,
                            compressed_tokens as f32 / estimated_tokens as f32,
                        );
                    }
                    Err(e) => {
                        self.emit(AgentEvent::progress(
                            format!("预压缩失败: {}", e),
                            None,
                        ))?;
                    }
                }
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
                        content: MessageContent::Text(prompt::MSG_PENDING_TODOS.to_string()),
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
                            self.compression_config.threshold * 100.0
                        ),
                    );
                }
            }

            if should_compress(current_tokens, context_size, &self.compression_config) {
                self.emit(AgentEvent::progress(prompt::MSG_COMPRESSING_CONTEXT, None))?;

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
                            format!("{}{}", prompt::MSG_COMPRESSION_FAILED, e),
                            None,
                        ))?;
                    }
                }
            }
        }
        
        // Check if we stopped due to reaching MAX_ITERATIONS
        if iterations >= MAX_ITERATIONS && should_continue {
            self.emit(AgentEvent::error(
                prompt::MSG_MAX_ITERATIONS_REACHED
                    .replace("{max_iterations}", &MAX_ITERATIONS.to_string())
                    .replace("{iterations}", &iterations.to_string()),
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
