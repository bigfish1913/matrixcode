//! Agent streaming implementation.

use anyhow::Result;
use tokio::time::{Duration, sleep, Instant};

use crate::constants::STREAM_DELTA_BUFFER_SIZE;
use crate::event::AgentEvent;
use crate::providers::{ChatRequest, ChatResponse, ContentBlock, StopReason, StreamEvent, Usage};

use super::types::Agent;

/// Buffered delta for efficient event emission
#[derive(Debug)]
struct BufferedDelta {
    text: String,
    thinking: String,
    last_emit: Instant,
}

impl Default for BufferedDelta {
    fn default() -> Self {
        Self::new()
    }
}

impl BufferedDelta {
    fn new() -> Self {
        Self {
            text: String::new(),
            thinking: String::new(),
            last_emit: Instant::now(),
        }
    }

    /// Add text delta to buffer, returns true if should flush
    fn add_text(&mut self, delta: &str) -> bool {
        self.text.push_str(delta);
        self.should_flush_text()
    }

    /// Add thinking delta to buffer, returns true if should flush
    fn add_thinking(&mut self, delta: &str) -> bool {
        self.thinking.push_str(delta);
        self.should_flush_thinking()
    }

    fn should_flush_text(&self) -> bool {
        self.text.len() >= STREAM_DELTA_BUFFER_SIZE
    }

    fn should_flush_thinking(&self) -> bool {
        self.thinking.len() >= STREAM_DELTA_BUFFER_SIZE
    }

    /// Check if buffer needs flush due to time interval
    fn should_flush_by_time(&self, interval_ms: u64) -> bool {
        self.last_emit.elapsed().as_millis() >= interval_ms as u128
            && (!self.text.is_empty() || !self.thinking.is_empty())
    }

    /// Flush text buffer, returns content if non-empty
    fn flush_text(&mut self) -> Option<String> {
        if self.text.is_empty() {
            return None;
        }
        let content = self.text.clone();
        self.text.clear();
        self.last_emit = Instant::now();
        Some(content)
    }

    /// Flush thinking buffer, returns content if non-empty
    fn flush_thinking(&mut self) -> Option<String> {
        if self.thinking.is_empty() {
            return None;
        }
        let content = self.thinking.clone();
        self.thinking.clear();
        self.last_emit = Instant::now();
        Some(content)
    }

    /// Flush all buffers
    fn flush_all(&mut self) -> (Option<String>, Option<String>) {
        let text = self.flush_text();
        let thinking = self.flush_thinking();
        (text, thinking)
    }
}

/// Wait for cancellation signal, checking periodically.
async fn wait_for_cancel_stream(token: &crate::cancel::CancellationToken) {
    while !token.is_cancelled() {
        sleep(Duration::from_millis(100)).await;
    }
}

impl Agent {
    /// Drain any pending input messages from the channel.
    /// Called during streaming to collect real-time appended messages.
    pub(crate) fn drain_pending_inputs(&mut self) {
        let inputs = self.session.drain_pending_inputs();
        for msg in inputs {
            log::info!(
                "Agent received pending input: {}",
                msg.chars().take(50).collect::<String>()
            );
            self.state.add_pending_input(msg);
        }
    }

    /// Check if there are pending inputs waiting to be processed.
    pub fn has_pending_inputs(&self) -> bool {
        self.state.has_pending_inputs()
    }

    /// Get and clear all pending inputs.
    pub fn take_pending_inputs(&mut self) -> Vec<String> {
        self.state.take_pending_inputs()
    }

    /// Call provider with streaming and emit events in real-time.
    /// Also monitors pending_input_rx for real-time message appending.
    /// Uses buffered delta emission to reduce event frequency.
    pub(crate) async fn call_streaming(&mut self, request: &ChatRequest) -> Result<ChatResponse> {
        const MAX_RETRIES: u32 = 5;
        const RETRY_DELAY_MS: u64 = 1000;
        const FLUSH_INTERVAL_MS: u64 = crate::constants::STREAM_DELTA_FLUSH_INTERVAL_MS;

        let mut attempt = 0;

        loop {
            attempt += 1;
            log::info!(
                "Agent: API call attempt {} with {} messages",
                attempt,
                request.messages.len()
            );

            if self.session.is_cancelled() {
                return Err(anyhow::anyhow!("Operation cancelled"));
            }

            log::info!("Agent: calling provider.chat_stream");
            let rx_result = self.provider.chat_stream(request.clone()).await;
            log::info!("Agent: provider.chat_stream returned");

            match rx_result {
                Ok(mut rx) => {
                    let mut response_content: Vec<ContentBlock> = Vec::new();
                    let mut current_text = String::new();
                    let mut current_thinking = String::new();
                    let mut usage = Usage {
                        input_tokens: 0,
                        output_tokens: 0,
                        cache_creation_input_tokens: 0,
                        cache_read_input_tokens: 0,
                    };
                    let mut should_retry = false;

                    // Buffered delta for efficient emission
                    let mut buffer = BufferedDelta::new();
                    let mut thinking_started = false;
                    let mut text_started = false;

                    loop {
                        // Use biased select! to prioritize stream events over pending input checks
                        // This prevents losing stream events when sleep completes first
                        let event = if let Some(token) = self.session.cancel_token() {
                            tokio::select! {
                                biased;

                                // Primary: receive stream event (highest priority)
                                event = rx.recv() => event,

                                // Cancellation signal (second priority)
                                _ = wait_for_cancel_stream(token) => {
                                    // Flush any pending buffers before cancelling
                                    let (text, thinking) = buffer.flush_all();
                                    if let Some(t) = thinking {
                                        self.emit(AgentEvent::thinking_delta(&t, None))?;
                                    }
                                    if let Some(t) = text {
                                        self.emit(AgentEvent::text_delta(&t))?;
                                    }
                                    return Err(anyhow::anyhow!("Operation cancelled"));
                                }

                                // Check for pending inputs periodically (lowest priority)
                                // Also check for buffer flush by time interval
                                _ = sleep(Duration::from_millis(FLUSH_INTERVAL_MS)) => {
                                    self.drain_pending_inputs();
                                    // Flush buffers if interval elapsed
                                    if buffer.should_flush_by_time(FLUSH_INTERVAL_MS) {
                                        if let Some(t) = buffer.flush_thinking() {
                                            self.emit(AgentEvent::thinking_delta(&t, None))?;
                                        }
                                        if let Some(t) = buffer.flush_text() {
                                            self.emit(AgentEvent::text_delta(&t))?;
                                        }
                                    }
                                    continue;
                                }
                            }
                        } else {
                            // No cancellation token, but still check pending inputs
                            tokio::select! {
                                biased;

                                // Primary: receive stream event (highest priority)
                                event = rx.recv() => event,

                                // Check for pending inputs periodically (lower priority)
                                // Also check for buffer flush by time interval
                                _ = sleep(Duration::from_millis(FLUSH_INTERVAL_MS)) => {
                                    self.drain_pending_inputs();
                                    // Flush buffers if interval elapsed
                                    if buffer.should_flush_by_time(FLUSH_INTERVAL_MS) {
                                        if let Some(t) = buffer.flush_thinking() {
                                            self.emit(AgentEvent::thinking_delta(&t, None))?;
                                        }
                                        if let Some(t) = buffer.flush_text() {
                                            self.emit(AgentEvent::text_delta(&t))?;
                                        }
                                    }
                                    continue;
                                }
                            }
                        };

                        match event {
                            None => break,
                            Some(StreamEvent::FirstByte) => {}
                            Some(StreamEvent::ThinkingDelta(delta)) => {
                                // Check cancellation before emitting
                                if self.session.is_cancelled() {
                                    return Err(anyhow::anyhow!("Operation cancelled"));
                                }
                                if !thinking_started {
                                    self.emit(AgentEvent::thinking_start())?;
                                    thinking_started = true;
                                }
                                current_thinking.push_str(&delta);
                                // Buffer the delta and emit if threshold reached
                                if buffer.add_thinking(&delta) {
                                    if let Some(t) = buffer.flush_thinking() {
                                        self.emit(AgentEvent::thinking_delta(&t, None))?;
                                    }
                                }
                            }
                            Some(StreamEvent::TextDelta(delta)) => {
                                // Check cancellation before emitting
                                if self.session.is_cancelled() {
                                    return Err(anyhow::anyhow!("Operation cancelled"));
                                }
                                if !text_started {
                                    self.emit(AgentEvent::text_start())?;
                                    text_started = true;
                                }
                                current_text.push_str(&delta);
                                // Buffer the delta and emit if threshold reached
                                if buffer.add_text(&delta) {
                                    if let Some(t) = buffer.flush_text() {
                                        self.emit(AgentEvent::text_delta(&t))?;
                                    }
                                }
                            }
                            Some(StreamEvent::ToolUseStart { id, name }) => {
                                // Flush any pending buffers before tool use
                                if let Some(t) = buffer.flush_thinking() {
                                    self.emit(AgentEvent::thinking_delta(&t, None))?;
                                }
                                if let Some(t) = buffer.flush_text() {
                                    self.emit(AgentEvent::text_delta(&t))?;
                                }
                                // Emit events for UI but don't push content blocks
                                // Content will be added from Done event's resp.content
                                if !current_thinking.is_empty() {
                                    self.emit(AgentEvent::thinking_end())?;
                                    current_thinking.clear();
                                }
                                if !current_text.is_empty() {
                                    self.emit(AgentEvent::text_end())?;
                                    current_text.clear();
                                }
                                thinking_started = false;
                                text_started = false;
                                self.emit(AgentEvent::tool_use_start(&id, &name, None))?;
                            }
                            Some(StreamEvent::ToolInputDelta { bytes_so_far: _ }) => {}
                            Some(StreamEvent::ToolInputComplete { id, name, input }) => {
                                self.state.mark_tool_input_previewed(id.clone());
                                self.emit(AgentEvent::tool_use_start(&id, &name, Some(input)))?;
                            }
                            Some(StreamEvent::Usage { output_tokens }) => {
                                self.emit(AgentEvent::usage_with_cache(
                                    0,
                                    output_tokens as u64,
                                    0,
                                    0,
                                ))?;
                                usage.output_tokens = output_tokens;
                            }
                            Some(StreamEvent::Done(resp)) => {
                                // Check cancellation before processing final response
                                if self.session.is_cancelled() {
                                    return Err(anyhow::anyhow!("Operation cancelled"));
                                }

                                // Final drain of pending inputs before completing
                                self.drain_pending_inputs();

                                // Flush any remaining buffered deltas
                                if let Some(t) = buffer.flush_thinking() {
                                    self.emit(AgentEvent::thinking_delta(&t, None))?;
                                }
                                if let Some(t) = buffer.flush_text() {
                                    self.emit(AgentEvent::text_delta(&t))?;
                                }

                                // IMPORTANT: Add current_thinking/current_text to response_content FIRST
                                // before checking for duplicates from resp.content
                                // This ensures all streamed content is preserved
                                if !current_thinking.is_empty() {
                                    self.emit(AgentEvent::thinking_end())?;
                                    // Add to response_content with signature from resp if available
                                    let signature = resp.content.iter()
                                        .find_map(|b| {
                                            if let ContentBlock::Thinking { thinking, signature } = b {
                                                if thinking == &current_thinking {
                                                    signature.clone()
                                                } else {
                                                    None
                                                }
                                            } else {
                                                None
                                            }
                                        });
                                    response_content.push(ContentBlock::Thinking {
                                        thinking: current_thinking.clone(),
                                        signature,
                                    });
                                    current_thinking.clear();
                                }
                                if !current_text.is_empty() {
                                    self.emit(AgentEvent::text_end())?;
                                    // Add to response_content
                                    response_content.push(ContentBlock::Text {
                                        text: current_text.clone(),
                                    });
                                    current_text.clear();
                                }

                                // Then add any additional blocks from final response that are NOT duplicates
                                for block in &resp.content {
                                    // Smart deduplication: compare content, not entire block
                                    let is_duplicate = response_content.iter().any(|b| {
                                        match (b, block) {
                                            // For Thinking blocks, compare thinking content only (signature may differ)
                                            (
                                                ContentBlock::Thinking { thinking: t1, .. },
                                                ContentBlock::Thinking { thinking: t2, .. },
                                            ) => t1 == t2,
                                            // For Text blocks, compare text content
                                            (
                                                ContentBlock::Text { text: t1 },
                                                ContentBlock::Text { text: t2 },
                                            ) => t1 == t2,
                                            // For ToolUse, compare id
                                            (
                                                ContentBlock::ToolUse { id: id1, .. },
                                                ContentBlock::ToolUse { id: id2, .. },
                                            ) => id1 == id2,
                                            // For ToolResult, compare tool_use_id
                                            (
                                                ContentBlock::ToolResult {
                                                    tool_use_id: id1, ..
                                                },
                                                ContentBlock::ToolResult {
                                                    tool_use_id: id2, ..
                                                },
                                            ) => id1 == id2,
                                            // Default: exact comparison
                                            _ => b == block,
                                        }
                                    });
                                    if !is_duplicate {
                                        response_content.push(block.clone());
                                    }
                                }
                                usage = resp.usage;
                            }
                            Some(StreamEvent::Error(msg)) => {
                                if attempt < MAX_RETRIES {
                                    self.emit(AgentEvent::progress(
                                        format!(
                                            "⚠️ Stream error, retrying ({}/{}): {}",
                                            attempt, MAX_RETRIES, &msg
                                        ),
                                        None,
                                    ))?;
                                    let delay = RETRY_DELAY_MS * (1 << (attempt - 1));
                                    tokio::time::sleep(tokio::time::Duration::from_millis(delay))
                                        .await;
                                    should_retry = true;
                                    break;
                                } else {
                                    self.emit(AgentEvent::error(msg.clone(), None, None))?;
                                    return Err(anyhow::anyhow!(
                                        "Stream error after {} retries: {}",
                                        MAX_RETRIES,
                                        msg
                                    ));
                                }
                            }
                        }
                    }

                    if should_retry {
                        continue;
                    }

                    return Ok(ChatResponse {
                        content: response_content,
                        stop_reason: StopReason::EndTurn,
                        usage,
                    });
                }
                Err(e) => {
                    if attempt < MAX_RETRIES {
                        let error_msg = e.to_string();
                        self.emit(AgentEvent::progress(
                            format!(
                                "⚠️ API error, retrying ({}/{}): {}",
                                attempt, MAX_RETRIES, &error_msg
                            ),
                            None,
                        ))?;
                        let delay = RETRY_DELAY_MS * (1 << (attempt - 1));
                        tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
                    } else {
                        return Err(anyhow::anyhow!(
                            "API error after {} retries: {}",
                            MAX_RETRIES,
                            e
                        ));
                    }
                }
            }
        }
    }
}
