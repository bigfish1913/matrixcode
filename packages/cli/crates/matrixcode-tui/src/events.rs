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
                if let Some(EventData::ToolResult { content, name, detail, is_error, .. }) = e.data {
                    self.messages.push(Message {
                        role: Role::Tool { name, detail, is_error },
                        content  // Keep full content, draw.rs will summarize
                    });
                    self.tool_calls += 1;
                    self.activity = Activity::Thinking;
                    self.activity_detail.clear();
                }
            }
            EventType::SessionEnded => {
                // Flush remaining content - thinking first, then assistant
                if !self.thinking.is_empty() {
                    self.messages.push(Message { role: Role::Thinking, content: self.thinking.clone() });
                    self.thinking.clear();
                }
                if !self.streaming.is_empty() {
                    self.messages.push(Message { role: Role::Assistant, content: self.streaming.clone() });
                    self.streaming.clear();
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
                    // Check if this is a cancellation error
                    let is_cancelled = message == "Operation cancelled";
                    
                    if is_cancelled {
                        // Flush partial content before showing cancel message
                        if !self.thinking.is_empty() {
                            self.messages.push(Message { role: Role::Thinking, content: self.thinking.clone() });
                            self.thinking.clear();
                        }
                        if !self.streaming.is_empty() {
                            self.messages.push(Message { role: Role::Assistant, content: self.streaming.clone() });
                            self.streaming.clear();
                        }
                        self.messages.push(Message { role: Role::System, content: "\u{26a1} Interrupted".into() });
                    } else {
                        self.messages.push(Message { role: Role::System, content: format!("\u{274c} Error: {}", message) });
                        self.streaming.clear();
                        self.thinking.clear();
                    }
                }
                self.activity_detail.clear();
                self.request_start = None;
                self.cancel.reset();  // Reset cancel state for next request
                
                // Process queue after cancellation or error
                if !self.pending_messages.is_empty() {
                    let next_msg = self.pending_messages.remove(0);
                    self.messages.push(Message { role: Role::User, content: next_msg.clone() });
                    self.tx.try_send(next_msg).ok();
                    self.activity = Activity::Thinking;
                    self.auto_scroll = true;
                } else {
                    self.activity = Activity::Idle;
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
                            content: format!("📦 Compressed: {} → {}tok ({:.0}% saved)",
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
                    // Format the question with clear styling
                    let mut content = String::new();

                    // Header line - prominent
                    content.push_str("╔══════════════════════════════════════╗\n");
                    content.push_str("║         ⚡ AWAITING INPUT ⚡        ║\n");
                    content.push_str("╚══════════════════════════════════════╝\n\n");

                    // Question content
                    content.push_str(&question);

                    // Parse options or create default y/n for approval
                    if let Some(ref opts) = options && let Some(arr) = opts.as_array() && !arr.is_empty() {
                        // Has explicit options from ask tool
                        self.ask_options = arr.iter().map(|opt| {
                            crate::types::AskOption {
                                id: opt["id"].as_str().unwrap_or("").to_string(),
                                label: opt["label"].as_str().unwrap_or("").to_string(),
                                description: opt["description"].as_str().map(|s| s.to_string()),
                            }
                        }).collect();
                        self.ask_selected_index = 0;
                        // Clear input so selection shows in input area
                        self.input.clear();
                        self.cursor_pos = 0;
                    } else if question.contains("(y/n)") || question.contains("Allow?") {
                        // Approval request: create y/n options
                        self.ask_options = vec![
                            crate::types::AskOption { id: "y".into(), label: "Yes".into(), description: Some("Allow this action".into()) },
                            crate::types::AskOption { id: "n".into(), label: "No".into(), description: Some("Reject this action".into()) },
                        ];
                        self.ask_selected_index = 0;
                        // Clear input so selection shows in input area
                        self.input.clear();
                        self.cursor_pos = 0;
                    } else {
                        // Free text input
                        self.ask_options.clear();
                        self.ask_selected_index = 0;
                    }

                    self.messages.push(Message { role: Role::Ask, content });
                    self.waiting_for_ask = true;
                    self.activity = Activity::Asking;
                    self.auto_scroll = true;
                }
            }
            _ => {}
        }
    }
}
