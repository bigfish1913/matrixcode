use matrixcode_core::{AgentEvent, EventData, EventType};

use crate::types::{Activity, Message, Role};
use crate::utils::{truncate, extract_tool_detail, fmt_tokens};
use crate::app::TuiApp;

impl TuiApp {
    pub(crate) fn on_event(&mut self, e: AgentEvent) {
        match e.event_type {
            EventType::ThinkingStart => {
                self.activity = Activity::Thinking;
                self.thinking.clear();
                self.request_start = Some(std::time::Instant::now());
            }
            EventType::ThinkingDelta => {
                if let Some(EventData::Thinking { delta, .. }) = e.data {
                    self.thinking.push_str(&delta);
                    self.activity = Activity::Thinking;
                }
            }
            EventType::ThinkingEnd => {
                if !self.thinking.is_empty() {
                    self.messages.push(Message { role: Role::Thinking, content: self.thinking.clone() });
                    self.thinking.clear();
                }
            }
            EventType::TextStart => {
                self.streaming.clear();
                self.activity = Activity::Thinking;
                self.request_start = Some(std::time::Instant::now());
            }
            EventType::TextDelta => {
                if let Some(EventData::Text { delta }) = e.data {
                    self.streaming.push_str(&delta);
                    self.activity = Activity::Thinking;
                }
            }
            EventType::TextEnd => {
                if !self.streaming.is_empty() {
                    self.messages.push(Message { role: Role::Assistant, content: self.streaming.clone() });
                    self.streaming.clear();
                }
            }
            EventType::ToolUseStart => {
                if let Some(EventData::ToolUse { name, input, .. }) = e.data {
                    self.activity = Activity::from_tool(&name);
                    self.activity_detail = extract_tool_detail(&name, input.as_ref());
                    if self.request_start.is_none() {
                        self.request_start = Some(std::time::Instant::now());
                    }
                }
            }
            EventType::ToolResult => {
                if let Some(EventData::ToolResult { content, is_error, .. }) = e.data {
                    let tool_name = self.activity.label();
                    self.messages.push(Message {
                        role: Role::Tool { name: tool_name, is_error },
                        content  // Keep full content, draw.rs will summarize
                    });
                    self.tool_calls += 1;
                    self.activity = Activity::Thinking;
                    self.activity_detail.clear();
                }
            }
            EventType::SessionEnded => {
                // Flush remaining content
                if !self.streaming.is_empty() {
                    self.messages.push(Message { role: Role::Assistant, content: self.streaming.clone() });
                    self.streaming.clear();
                }
                if !self.thinking.is_empty() {
                    self.messages.push(Message { role: Role::Thinking, content: self.thinking.clone() });
                    self.thinking.clear();
                }

                // Clear current request tokens
                self.current_request_tokens = 0;

                // Process queue or go idle
                if !self.pending_messages.is_empty() {
                    let next_msg = self.pending_messages.remove(0);
                    self.messages.push(Message { role: Role::User, content: next_msg.clone() });
                    self.tx.try_send(next_msg).ok();
                    self.activity = Activity::Thinking;
                    self.auto_scroll = true;
                } else {
                    self.activity = Activity::Idle;
                    self.request_start = None;
                }
                self.activity_detail.clear();
                self.cancel.reset();  // Reset cancel state for next request
            }
            EventType::Error => {
                if let Some(EventData::Error { message, .. }) = e.data {
                    self.messages.push(Message { role: Role::System, content: format!("\u{274c} Error: {}", message) });
                    self.streaming.clear();
                    self.thinking.clear();
                }
                self.activity = Activity::Idle;
                self.activity_detail.clear();
                self.request_start = None;
                self.cancel.reset();  // Reset cancel state for next request
                
                // Check queue after error - user may want to retry
                if !self.pending_messages.is_empty() {
                    self.messages.push(Message {
                        role: Role::System,
                        content: format!("⚠️ Queue paused ({} messages). Send '/retry' to process.", self.pending_messages.len())
                    });
                }
            }
            EventType::Usage => {
                if let Some(EventData::Usage { input_tokens, output_tokens, cache_creation_input_tokens, cache_read_input_tokens }) = e.data {
                    // Only update tokens_in if it's non-zero (real-time updates may have 0)
                    if input_tokens > 0 {
                        self.tokens_in = input_tokens;
                        // Only count as a new API call when we have full usage info
                        self.api_calls += 1;
                    }
                    self.tokens_out = output_tokens;
                    self.session_total_out += output_tokens;
                    self.current_request_tokens = output_tokens;  // Real-time update
                    
                    // Update cache stats (only when actually reported by API)
                    let cache_read = cache_read_input_tokens.unwrap_or(0);
                    let cache_created = cache_creation_input_tokens.unwrap_or(0);
                    
                    // Only update cache if values are non-zero (final usage event)
                    if cache_read > 0 || cache_created > 0 {
                        self.cache_read += cache_read;
                        self.cache_created += cache_created;
                    }
                }
            }
            EventType::CompressionCompleted => {
                if let Some(EventData::Compression { original_tokens, compressed_tokens, ratio }) = e.data {
                    self.compressions += 1;
                    // Update token display to reflect compressed state
                    self.tokens_in = compressed_tokens;
                    if self.debug_mode {
                        self.messages.push(Message {
                            role: Role::System,
                            content: format!("📦 Compressed: {} → {} tokens ({:.0}% saved)",
                                fmt_tokens(original_tokens), fmt_tokens(compressed_tokens), (1.0 - ratio) * 100.0)
                        });
                        self.auto_scroll = true;
                    }
                }
            }
            EventType::CompressionTriggered => {
                if let Some(EventData::Progress { .. }) = e.data {
                    // Silent - usage bar already reflects compression state
                }
            }
            EventType::Progress => {
                if let Some(EventData::Progress { message, .. }) = e.data {
                    self.messages.push(Message {
                        role: Role::System,
                        content: message
                    });
                    self.auto_scroll = true;
                }
            }
            EventType::MemoryLoaded => {
                if let Some(EventData::Memory { entries_count, .. }) = e.data
                    && entries_count > 0 {
                    self.memory_saves += 1;
                    if self.debug_mode {
                        self.messages.push(Message {
                            role: Role::System,
                            content: format!("🧠 Memory: {} entries", entries_count)
                        });
                        self.auto_scroll = true;
                    }
                }
            }
            EventType::MemoryDetected => {
                if let Some(EventData::Memory { summary, entries_count }) = e.data {
                    self.memory_saves += 1;
                    if self.debug_mode {
                        self.messages.push(Message {
                            role: Role::System,
                            content: format!("🧠 Detected {} memories: {}", entries_count, summary)
                        });
                        self.auto_scroll = true;
                    }
                }
            }
            EventType::KeywordsExtracted => {
                // Keywords extraction for memory retrieval context
                // Show extraction info when keywords are found (informative, not debug-only)
                if let Some(EventData::Keywords { keywords, source }) = e.data {
                    if !keywords.is_empty() {
                        // Always show in status area (brief), full info in debug mode
                        let preview = truncate(&source, 30);
                        if self.debug_mode {
                            self.messages.push(Message {
                                role: Role::System,
                                content: format!("🔍 Keywords extracted: [{}] from '{}'", 
                                    keywords.iter().take(10).cloned().collect::<Vec<_>>().join(", "), preview)
                            });
                            self.auto_scroll = true;
                        }
                        // Update activity detail to show keywords briefly
                        self.activity_detail = format!("keywords: {}", 
                            keywords.iter().take(3).cloned().collect::<Vec<_>>().join(", "));
                    }
                }
            }
            EventType::AskQuestion => {
                if let Some(EventData::AskQuestion { question, options }) = e.data {
                    // Detect if this is an approval request
                    let is_approval = question.contains("requires approval") || question.contains("Allow?");
                    let has_options = options.is_some();
                    
                    // Format the display
                    let mut content = if is_approval {
                        // Extract tool name and risk level
                        let lines: Vec<&str> = question.lines().collect();
                        let header = lines.first().copied().unwrap_or("");
                        let detail = lines.get(1).copied().unwrap_or("");
                        format!("{}\n{}", header, detail)
                    } else if has_options {
                        format!("❓ {}", question)
                    } else {
                        question.clone()
                    };
                    
                    // Show options if available
                    if let Some(ref opts) = options && let Some(arr) = opts.as_array() {
                        content.push_str("\n\nOptions:");
                        for opt in arr {
                            let id = opt["id"].as_str().unwrap_or("?");
                            let label = opt["label"].as_str().unwrap_or("");
                            let desc = opt["description"].as_str().unwrap_or("");
                            let desc_text = if desc.is_empty() { String::new() } else { format!("({})", desc) };
                            content.push_str(&format!("\n  [{}] {} {}", id, label, desc_text));
                        }
                    }
                    
                    // Add hint for approval
                    if is_approval {
                        content.push_str("\n\n[y] Approve  [n] Reject  [a] Abort");
                    } else if has_options {
                        content.push_str("\n\nEnter option ID (e.g., 'A' or 'B')");
                    }
                    
                    self.messages.push(Message { role: Role::System, content });
                    self.waiting_for_ask = true;
                    self.activity = Activity::Asking;
                    self.auto_scroll = true;
                }
            }
            _ => {}
        }
    }
}
