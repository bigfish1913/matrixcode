//! Agent helper functions and utilities.

use anyhow::Result;
use std::sync::atomic::Ordering;
use tokio::sync::mpsc;

use crate::event::AgentEvent;
use crate::providers::{ContentBlock, MessageContent, Usage};
use crate::truncate::truncate_chars;

use super::types::Agent;

impl Agent {
    /// Track token usage
    pub(crate) fn track_usage(&self, usage: &Usage) {
        self.total_input_tokens
            .fetch_add(usage.input_tokens as u64, Ordering::Relaxed);
        self.total_output_tokens
            .fetch_add(usage.output_tokens as u64, Ordering::Relaxed);
        self.last_input_tokens
            .store(usage.input_tokens as u64, Ordering::Relaxed);

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
            self.total_input_tokens.load(Ordering::Relaxed),
            usage.output_tokens as u64,
            usage.cache_read_input_tokens as u64,
            usage.cache_creation_input_tokens as u64,
        ));
    }

    /// Emit event (non-blocking)
    pub(crate) fn emit(&self, event: AgentEvent) -> Result<()> {
        match self.event_tx.try_send(event) {
            Ok(_) => Ok(()),
            Err(mpsc::error::TrySendError::Full(_)) => Ok(()),
            Err(mpsc::error::TrySendError::Closed(_)) => {
                Err(anyhow::anyhow!("Event channel closed"))
            }
        }
    }

    /// Check if there are pending (uncompleted) todos in recent messages
    pub(crate) fn has_pending_todos(&self) -> bool {
        // Look for todo_write tool_use in recent messages (last 10)
        for msg in self.messages.iter().rev().take(10) {
            if let MessageContent::Blocks(blocks) = &msg.content {
                for block in blocks {
                    if let ContentBlock::ToolUse { name, input, .. } = block {
                        if name == "todo_write" {
                            // Check if there are non-completed todos
                            if let Some(todos) = input.get("todos").and_then(|t| t.as_array()) {
                                for todo in todos {
                                    if let Some(status) = todo.get("status").and_then(|s| s.as_str()) {
                                        if status != "completed" {
                                            return true;
                                        }
                                    }
                                }
                            }
                        }
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
