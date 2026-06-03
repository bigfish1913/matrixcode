//! Agent helper functions and utilities.

use anyhow::Result;
use tokio::sync::mpsc;

use crate::event::{AgentEvent, EventType};
use crate::providers::{ContentBlock, MessageContent, Role, Usage};
use crate::truncate::truncate_chars;

use super::types::Agent;

/// Context information for display
pub struct ContextInfo {
    /// Message count
    pub message_count: usize,
    /// Estimated input tokens
    pub estimated_input_tokens: u64,
    /// Total input tokens (lifetime)
    pub total_input_tokens: u64,
    /// Total output tokens (lifetime)
    pub total_output_tokens: u64,
    /// System prompt preview
    pub system_prompt_preview: String,
    /// Memory summary
    pub memory_summary: Option<String>,
    /// Project overview preview
    pub project_overview_preview: Option<String>,
    /// Last few messages preview
    pub recent_messages_preview: Vec<String>,
    /// Model name
    pub model_name: String,
    /// Max tokens setting
    pub max_tokens: u32,
}

impl Agent {
    /// Get context information for display
    pub fn get_context_info(&self) -> ContextInfo {
        // Estimate tokens from messages
        let estimated_tokens = self.messages().iter()
            .map(|m| {
                let content = match &m.content {
                    MessageContent::Text(t) => t.len(),
                    MessageContent::Blocks(blocks) => {
                        blocks.iter()
                            .filter_map(|b| {
                                if let ContentBlock::Text { text } = b {
                                    Some(text.len())
                                } else {
                                    None
                                }
                            })
                            .sum::<usize>()
                    }
                };
                // Rough estimate: ~3 chars per token + 50 for metadata
                (content / 3 + 50) as u64
            })
            .sum();

        // System prompt preview (first 500 chars)
        let system_preview = truncate_chars(self.system_prompt(), 500);

        // Project overview preview
        let project_preview = self.project_overview()
            .map(|o| truncate_chars(o, 300));

        // Recent messages preview (last 5 messages)
        let recent_preview = self.messages().iter().rev().take(5).rev()
            .map(|m| {
                let role = match m.role {
                    Role::User => "User",
                    Role::Assistant => "Assistant",
                    Role::System => "System",
                    Role::Tool => "Tool",
                };
                let content_preview = match &m.content {
                    MessageContent::Text(t) => truncate_chars(t, 100),
                    MessageContent::Blocks(blocks) => {
                        let text = blocks.iter()
                            .filter_map(|b| {
                                if let ContentBlock::Text { text } = b {
                                    Some(text.clone())
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>()
                            .join(" ");
                        truncate_chars(&text, 100)
                    }
                };
                format!("{}: {}", role, content_preview)
            })
            .collect();

        ContextInfo {
            message_count: self.messages().len(),
            estimated_input_tokens: estimated_tokens,
            total_input_tokens: self.state.total_input_tokens(),
            total_output_tokens: self.state.total_output_tokens(),
            system_prompt_preview: system_preview,
            memory_summary: self.memory_summary().map(|s| s.to_string()),
            project_overview_preview: project_preview,
            recent_messages_preview: recent_preview,
            model_name: self.model_name.clone(),
            max_tokens: self.max_tokens(),
        }
    }

    /// Get full context preview (everything that will be sent to LLM)
    pub fn get_full_context_preview(&self) -> String {
        let mut preview = String::new();

        // System prompt
        preview.push_str("=== SYSTEM PROMPT ===\n");
        preview.push_str(self.system_prompt());
        preview.push_str("\n\n");

        // Memory summary
        if let Some(memory) = self.memory_summary() {
            preview.push_str("=== MEMORY SUMMARY ===\n");
            preview.push_str(memory);
            preview.push_str("\n\n");
        }

        // Project overview
        if let Some(overview) = self.project_overview() {
            preview.push_str("=== PROJECT OVERVIEW ===\n");
            preview.push_str(overview);
            preview.push_str("\n\n");
        }

        // Messages
        preview.push_str("=== MESSAGES ===\n");
        for (i, msg) in self.messages().iter().enumerate() {
            let role = match msg.role {
                Role::User => "User",
                Role::Assistant => "Assistant",
                Role::System => "System",
                Role::Tool => "Tool",
            };
            preview.push_str(&format!("\n[{}] {}:\n", i + 1, role));

            match &msg.content {
                MessageContent::Text(t) => {
                    preview.push_str(t);
                }
                MessageContent::Blocks(blocks) => {
                    for block in blocks {
                        match block {
                            ContentBlock::Text { text } => {
                                preview.push_str(text);
                                preview.push_str("\n");
                            }
                            ContentBlock::ToolUse { name, input, .. } => {
                                preview.push_str(&format!("[Tool: {}]\n", name));
                                preview.push_str(&serde_json::to_string_pretty(input).unwrap_or_default());
                                preview.push_str("\n");
                            }
                            ContentBlock::ToolResult { tool_use_id, content } => {
                                preview.push_str(&format!("[Tool Result: {}]\n", tool_use_id));
                                preview.push_str(content);
                                preview.push_str("\n");
                            }
                            // Thinking blocks are NOT sent to LLM, skip them
                            ContentBlock::Thinking { .. } => {
                                continue;
                            }
                            ContentBlock::ServerToolUse { name, .. } => {
                                preview.push_str(&format!("[Server Tool: {}]\n", name));
                            }
                            ContentBlock::ServerToolResult { tool_use_id, content, .. } => {
                                preview.push_str(&format!("[Server Tool Result: {}]\n", tool_use_id));
                                preview.push_str(content);
                                preview.push_str("\n");
                            }
                            _ => {
                                continue; // Skip other non-sendable content
                            }
                        }
                    }
                }
            }
        }

        preview
    }
    /// Track token usage
    pub(crate) fn track_usage(&self, usage: &Usage) {
        self.state.track_usage(usage);

        crate::debug::debug_log().log(
            "usage",
            &format!(
                "tracked: input_tokens={}, output_tokens={}, cache_read={}, cache_created={}",
                usage.input_tokens,
                usage.output_tokens,
                usage.cache_read_input_tokens,
                usage.cache_creation_input_tokens
            ),
        );

        let _ = self.event_tx.try_send(AgentEvent::usage_with_cache(
            self.state.total_input_tokens(),
            usage.output_tokens as u64,
            usage.cache_read_input_tokens as u64,
            usage.cache_creation_input_tokens as u64,
        ));
    }

    /// Emit event (with retry on full)
    ///
    /// This method tries to send an event and retries with backoff if the channel is full.
    /// This prevents event loss which could cause UI state inconsistency.
    pub(crate) fn emit(&self, event: AgentEvent) -> Result<()> {
        log::debug!("Agent emit: event_type={:?}", event.event_type);

        // First try non-blocking send for performance
        match self.event_tx.try_send(event) {
            Ok(_) => {
                log::debug!("Agent emit: sent successfully");
                Ok(())
            }
            Err(mpsc::error::TrySendError::Full(event)) => {
                // Channel full - for critical events, we must retry
                let is_critical = matches!(
                    event.event_type,
                    EventType::Error | EventType::SessionEnded | EventType::SessionStarted
                );

                if is_critical {
                    // Retry a few times with short delays (blocking approach)
                    let mut retries = 3;
                    let mut current_event = event;
                    while retries > 0 {
                        // Short spin wait
                        std::thread::sleep(std::time::Duration::from_millis(10));
                        match self.event_tx.try_send(current_event) {
                            Ok(_) => {
                                log::debug!("Agent emit: critical event sent after retry");
                                return Ok(());
                            }
                            Err(mpsc::error::TrySendError::Full(e)) => {
                                current_event = e;
                                retries -= 1;
                            }
                            Err(mpsc::error::TrySendError::Closed(_)) => {
                                log::error!("Agent emit: channel closed during retry");
                                return Err(anyhow::anyhow!("Event channel closed"));
                            }
                        }
                    }
                    log::warn!("Agent emit: critical event dropped after {} retries", 3);
                    Err(anyhow::anyhow!("Event channel full, critical event dropped"))
                } else {
                    log::warn!("Agent emit: channel full, skipping non-critical event");
                    Ok(())
                }
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                log::error!("Agent emit: channel closed");
                Err(anyhow::anyhow!("Event channel closed"))
            }
        }
    }

    /// Get pending (uncompleted) todos from the most recent todo_write.
    /// Returns list of (status, content) for non-completed tasks that haven't exceeded reminder limit.
    /// Note: todo_write replaces the entire list each time, so only the last one matters.
    /// 
    /// # Arguments
    /// * `todo_reminder_count` - Reference to the reminder counter map (immutable, will be cloned inside)
    /// * `max_reminders` - Maximum number of reminders allowed per todo (default: 2)
    /// 
    /// # Returns
    /// Tuple of (pending todos, whether all todos are at reminder limit)
    pub(crate) fn get_pending_todos_with_limit(
        &self,
        todo_reminder_count: &std::collections::HashMap<String, usize>,
        max_reminders: usize,
    ) -> (Vec<(String, String)>, bool) {
        // Find the most recent todo_write (current state)
        for msg in self.messages().iter().rev().take(10) {
            if let MessageContent::Blocks(blocks) = &msg.content {
                for block in blocks {
                    if let ContentBlock::ToolUse { name, input, .. } = block
                        && name == "todo_write"
                    {
                        // Extract non-completed todos from this todo_write
                        if let Some(todos) = input.get("todos").and_then(|t| t.as_array()) {
                            let pending: Vec<(String, String)> = todos
                                .iter()
                                .filter_map(|todo| {
                                    let status = todo.get("status").and_then(|s| s.as_str())?;
                                    let content = todo.get("content").and_then(|c| c.as_str())?;
                                    if status != "completed" {
                                        Some((status.to_string(), content.to_string()))
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                            
                            // Check which todos are at the reminder limit
                            let mut filtered_pending = Vec::new();
                            let mut all_at_limit = true;
                            
                            for (status, content) in pending {
                                let count = todo_reminder_count.get(&content).copied().unwrap_or(0);
                                if count < max_reminders {
                                    filtered_pending.push((status, content));
                                    all_at_limit = false;
                                }
                            }
                            
                            return (filtered_pending, all_at_limit); // Return immediately - this is the current state
                        }
                    }
                }
            }
        }
        (Vec::new(), true)
    }
    
    /// Check if the last user message was a todo reminder.
    /// This prevents adding duplicate reminders in consecutive iterations.
    pub(crate) fn last_message_was_todo_reminder(&self) -> bool {
        // Check the last few messages for a todo reminder
        for msg in self.messages().iter().rev().take(3) {
            if msg.role == Role::User {
                if let MessageContent::Text(text) = &msg.content {
                    if text.contains("任务尚未完成") && text.contains("待办项需要处理") {
                        return true;
                    }
                }
            }
        }
        false
    }
}

/// Extract tool detail for display
pub(crate) fn extract_tool_detail(tool_name: &str, input: &serde_json::Value) -> Option<String> {
    match tool_name.to_lowercase().as_str() {
        "read" => input
            .get("path")
            .and_then(|v| v.as_str())
            .map(|s| truncate_str(s, 50)),
        "write" => input
            .get("path")
            .and_then(|v| v.as_str())
            .map(|s| truncate_str(s, 50)),
        "edit" | "multi_edit" => {
            let path = input.get("path").and_then(|v| v.as_str());
            let old = input.get("old_string").and_then(|v| v.as_str());
            match (path, old) {
                (Some(p), Some(o)) => Some(format!(
                    "{}: \"{}\"",
                    truncate_str(p, 30),
                    truncate_str(o, 20)
                )),
                (Some(p), None) => Some(truncate_str(p, 50)),
                _ => None,
            }
        }
        "bash" => input
            .get("command")
            .and_then(|v| v.as_str())
            .map(|s| truncate_str(s, 60)),
        "search" | "grep" => input
            .get("pattern")
            .and_then(|v| v.as_str())
            .map(|s| format!("\"{}\"", truncate_str(s, 30))),
        "glob" => input
            .get("pattern")
            .and_then(|v| v.as_str())
            .map(|s| truncate_str(s, 40)),
        "ls" => input
            .get("path")
            .and_then(|v| v.as_str())
            .map(|s| truncate_str(s, 50)),
        "websearch" => input
            .get("query")
            .and_then(|v| v.as_str())
            .map(|s| truncate_str(s, 40)),
        "webfetch" => input
            .get("url")
            .and_then(|v| v.as_str())
            .map(|s| truncate_str(s, 50)),
        "task" => input
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| truncate_str(s, 40)),
        "task_create" => input
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| truncate_str(s, 40)),
        "task_get" | "task_stop" => input
            .get("task_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        _ => None,
    }
}

/// Truncate string at char boundary (using character count, not bytes)
pub(crate) fn truncate_str(s: &str, max: usize) -> String {
    truncate_chars(s, max)
}
