//! Agent streaming implementation.

use anyhow::Result;
use tokio::time::{sleep, Duration};

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

            if let Some(token) = &self.cancel_token
                && token.is_cancelled()
            {
                return Err(anyhow::anyhow!("Operation cancelled"));
            }

            let rx_result = self.provider.chat_stream(request.clone()).await;

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
                                if current_thinking.is_empty() {
                                    self.emit(AgentEvent::thinking_start())?;
                                }
                                current_thinking.push_str(&delta);
                                self.emit(AgentEvent::thinking_delta(delta, None))?;
                            }
                            Some(StreamEvent::TextDelta(delta)) => {
                                if current_text.is_empty() {
                                    self.emit(AgentEvent::text_start())?;
                                }
                                current_text.push_str(&delta);
                                self.emit(AgentEvent::text_delta(delta))?;
                            }
                            Some(StreamEvent::ToolUseStart { id, name }) => {
                                if !current_thinking.is_empty() {
                                    self.emit(AgentEvent::thinking_end())?;
                                    response_content.push(ContentBlock::Thinking {
                                        thinking: current_thinking.clone(),
                                        signature: None,
                                    });
                                    current_thinking.clear();
                                }
                                if !current_text.is_empty() {
                                    self.emit(AgentEvent::text_end())?;
                                    response_content.push(ContentBlock::Text {
                                        text: current_text.clone(),
                                    });
                                    current_text.clear();
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
                                if !current_thinking.is_empty() {
                                    self.emit(AgentEvent::thinking_end())?;
                                    response_content.push(ContentBlock::Thinking {
                                        thinking: current_thinking.clone(),
                                        signature: None,
                                    });
                                }
                                if !current_text.is_empty() {
                                    self.emit(AgentEvent::text_end())?;
                                    response_content.push(ContentBlock::Text {
                                        text: current_text.clone(),
                                    });
                                }
                                for block in &resp.content {
                                    if !response_content.iter().any(|b| b == block) {
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
