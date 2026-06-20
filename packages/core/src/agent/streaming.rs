//! Agent streaming implementation.

use anyhow::Result;
use tokio::time::{Duration, sleep};

use crate::event::AgentEvent;
use crate::providers::{ChatRequest, ChatResponse, ContentBlock, StopReason, StreamEvent, Usage};

use super::types::Agent;

/// Wait for cancellation signal, checking periodically.
async fn wait_for_cancel_stream(token: &crate::cancel::CancellationToken) {
    while !token.is_cancelled() {
        sleep(Duration::from_millis(100)).await;
    }
}

impl Agent {
    /// Call provider with streaming and emit events in real-time
    pub(crate) async fn call_streaming(&mut self, request: &ChatRequest) -> Result<ChatResponse> {
        const MAX_RETRIES: u32 = 5;
        const RETRY_DELAY_MS: u64 = 1000;

        let mut attempt = 0;

        loop {
            attempt += 1;
            log::info!("Agent: API call attempt {} with {} messages", attempt, request.messages.len());

            if let Some(token) = &self.cancel_token
                && token.is_cancelled()
            {
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

                    loop {
                        // Use select! with cancellation check integrated
                        // No need for busy-loop timeout - cancellation is checked directly
                        let event = if let Some(token) = &self.cancel_token {
                            tokio::select! {
                                event = rx.recv() => event,
                                _ = wait_for_cancel_stream(token) => {
                                    return Err(anyhow::anyhow!("Operation cancelled"));
                                }
                            }
                        } else {
                            rx.recv().await
                        };

                        match event {
                            None => break,
                            Some(StreamEvent::FirstByte) => {}
                            Some(StreamEvent::ThinkingDelta(delta)) => {
                                // Check cancellation before emitting
                                if let Some(token) = &self.cancel_token
                                    && token.is_cancelled()
                                {
                                    return Err(anyhow::anyhow!("Operation cancelled"));
                                }
                                if current_thinking.is_empty() {
                                    self.emit(AgentEvent::thinking_start())?;
                                }
                                current_thinking.push_str(&delta);
                                self.emit(AgentEvent::thinking_delta(delta, None))?;
                            }
                            Some(StreamEvent::TextDelta(delta)) => {
                                // Check cancellation before emitting
                                if let Some(token) = &self.cancel_token
                                    && token.is_cancelled()
                                {
                                    return Err(anyhow::anyhow!("Operation cancelled"));
                                }
                                if current_text.is_empty() {
                                    self.emit(AgentEvent::text_start())?;
                                }
                                current_text.push_str(&delta);
                                self.emit(AgentEvent::text_delta(delta))?;
                            }
                            Some(StreamEvent::ToolUseStart { id, name }) => {
                                // Emit events for UI but don't push content blocks
                                // Content will be added from Done event's resp.content
                                if !current_thinking.is_empty() {
                                    self.emit(AgentEvent::thinking_end())?;
                                    // Don't push - will be added from resp.content
                                }
                                if !current_text.is_empty() {
                                    self.emit(AgentEvent::text_end())?;
                                    // Don't push - will be added from resp.content
                                }
                                self.emit(AgentEvent::tool_use_start(&id, &name, None))?;
                            }
                            Some(StreamEvent::ToolInputDelta { bytes_so_far: _ }) => {}
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
                                if let Some(token) = &self.cancel_token
                                    && token.is_cancelled()
                                {
                                    return Err(anyhow::anyhow!("Operation cancelled"));
                                }

                                // Don't add current_thinking/current_text here - use resp.content directly
                                // This avoids duplicates since resp.content contains everything
                                // Just emit events for UI updates if we have pending content
                                if !current_thinking.is_empty() {
                                    self.emit(AgentEvent::thinking_end())?;
                                    // Don't push to response_content - will be added from resp.content
                                }
                                if !current_text.is_empty() {
                                    self.emit(AgentEvent::text_end())?;
                                    // Don't push to response_content - will be added from resp.content
                                }

                                // Add all blocks from final response with smart deduplication
                                for block in &resp.content {
                                    // Smart deduplication: compare content, not entire block
                                    let is_duplicate = response_content.iter().any(|b| {
                                        match (b, block) {
                                            // For Thinking blocks, compare thinking content only (signature may differ)
                                            (ContentBlock::Thinking { thinking: t1, .. }, ContentBlock::Thinking { thinking: t2, .. }) => {
                                                t1 == t2
                                            }
                                            // For Text blocks, compare text content
                                            (ContentBlock::Text { text: t1 }, ContentBlock::Text { text: t2 }) => {
                                                t1 == t2
                                            }
                                            // For ToolUse, compare id
                                            (ContentBlock::ToolUse { id: id1, .. }, ContentBlock::ToolUse { id: id2, .. }) => {
                                                id1 == id2
                                            }
                                            // For ToolResult, compare tool_use_id
                                            (ContentBlock::ToolResult { tool_use_id: id1, .. }, ContentBlock::ToolResult { tool_use_id: id2, .. }) => {
                                                id1 == id2
                                            }
                                            // Default: exact comparison
                                            _ => b == block
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
