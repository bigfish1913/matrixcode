//! Agent state management.
//!
//! This module manages the runtime state of the Agent, including:
//! - Message history
//! - Token usage tracking
//! - Pending inputs
//! - Todo reminders
//! - Read history
//!
//! By extracting state into a dedicated struct, we enable:
//! - Clear separation between state and configuration
//! - Easier testing of state transitions
//! - Better encapsulation of mutable state

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::providers::{ContentBlock, Message, MessageContent, Role, Usage};
use crate::tools::ReadHistoryTracker;

/// Agent runtime state.
///
/// Manages all mutable state during agent execution.
/// All fields are private to enforce encapsulation.
pub struct AgentState {
    /// Message history (conversation with LLM).
    messages: Vec<Message>,

    /// Total input tokens consumed (lifetime).
    total_input_tokens: AtomicU64,

    /// Total output tokens generated (lifetime).
    total_output_tokens: AtomicU64,

    /// Last input tokens (for compression tracking).
    last_input_tokens: AtomicU64,

    /// Tool input IDs that were previewed during streaming.
    /// Prevents duplicate emission of ToolUseStart events.
    previewed_tool_inputs: HashSet<String>,

    /// Todo reminder counts per todo content hash.
    /// Prevents infinite reminder loops.
    todo_reminder_count: HashMap<String, usize>,

    /// Files read in this session.
    /// Enforces "read before edit/write" rule.
    read_history: ReadHistoryTracker,

    /// Pending user inputs queued for next iteration.
    pending_inputs: Vec<String>,
}

impl AgentState {
    /// Create a new empty state.
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            total_input_tokens: AtomicU64::new(0),
            total_output_tokens: AtomicU64::new(0),
            last_input_tokens: AtomicU64::new(0),
            previewed_tool_inputs: HashSet::new(),
            todo_reminder_count: HashMap::new(),
            read_history: ReadHistoryTracker::new(),
            pending_inputs: Vec::new(),
        }
    }

    /// Add a message to history.
    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
    }

    /// Get reference to message history.
    pub fn messages(&self) -> &Vec<Message> {
        &self.messages
    }

    /// Get mutable reference to message history.
    pub fn messages_mut(&mut self) -> &mut Vec<Message> {
        &mut self.messages
    }

    /// Replace message history (used in compression).
    ///
    /// This method validates and cleans orphaned tool results/tool uses
    /// before setting the message history to prevent API errors.
    pub fn set_messages(&mut self, messages: Vec<Message>) {
        let cleaned = Self::clean_orphaned_messages(messages);
        self.messages = cleaned;
    }

    /// Clean orphaned tool results and tool uses from messages.
    ///
    /// Orphaned tool result: a Tool message whose tool_use_id has no corresponding ToolUse block.
    /// Orphaned tool use: a ToolUse block whose id has no corresponding ToolResult.
    fn clean_orphaned_messages(messages: Vec<Message>) -> Vec<Message> {
        if messages.is_empty() {
            return messages;
        }

        // Collect all tool_use_ids from ToolUse blocks
        let mut tool_use_ids: HashSet<String> = HashSet::new();
        for msg in &messages {
            if let MessageContent::Blocks(blocks) = &msg.content {
                for block in blocks {
                    if let ContentBlock::ToolUse { id, .. } = block {
                        tool_use_ids.insert(id.clone());
                    }
                }
            }
        }

        // Collect all tool_use_ids from ToolResult blocks
        let mut tool_result_ids: HashSet<String> = HashSet::new();
        for msg in &messages {
            if msg.role == Role::Tool {
                if let MessageContent::Blocks(blocks) = &msg.content {
                    for block in blocks {
                        if let ContentBlock::ToolResult { tool_use_id, .. } = block {
                            tool_result_ids.insert(tool_use_id.clone());
                        }
                    }
                }
            }
        }

        // Find orphaned ids (no matching pair)
        let orphaned_tool_use_ids: HashSet<&str> = tool_use_ids
            .iter()
            .filter(|id| !tool_result_ids.contains(*id))
            .map(|s| s.as_str())
            .collect();

        let orphaned_tool_result_ids: HashSet<&str> = tool_result_ids
            .iter()
            .filter(|id| !tool_use_ids.contains(*id))
            .map(|s| s.as_str())
            .collect();

        // If no orphans, return as-is
        if orphaned_tool_use_ids.is_empty() && orphaned_tool_result_ids.is_empty() {
            return messages;
        }

        log::warn!(
            "Cleaning orphaned messages: {} tool_uses without results, {} tool_results without uses",
            orphaned_tool_use_ids.len(),
            orphaned_tool_result_ids.len()
        );

        // Clean messages
        let original_len = messages.len();
        let mut cleaned = Vec::with_capacity(messages.len());
        for msg in messages {
            // Skip entire Tool messages that are orphaned tool results
            if msg.role == Role::Tool {
                if let MessageContent::Blocks(blocks) = &msg.content {
                    let has_orphaned_result = blocks.iter().any(|b| {
                        if let ContentBlock::ToolResult { tool_use_id, .. } = b {
                            orphaned_tool_result_ids.contains(tool_use_id.as_str())
                        } else {
                            false
                        }
                    });
                    if has_orphaned_result {
                        log::info!("Removing orphaned tool result message");
                        continue;
                    }
                }
            }

            // For assistant messages, filter out orphaned tool_use blocks
            if let MessageContent::Blocks(blocks) = msg.content {
                let filtered_blocks: Vec<ContentBlock> = blocks
                    .into_iter()
                    .filter(|b| {
                        if let ContentBlock::ToolUse { id, .. } = b {
                            if orphaned_tool_use_ids.contains(id.as_str()) {
                                log::info!("Removing orphaned tool_use block: {}", id);
                                return false;
                            }
                        }
                        true
                    })
                    .collect();

                // Only add message if it has remaining content
                if !filtered_blocks.is_empty() {
                    cleaned.push(Message {
                        role: msg.role,
                        content: MessageContent::Blocks(filtered_blocks),
                    });
                }
            } else {
                cleaned.push(msg);
            }
        }

        log::info!(
            "Message cleaning complete: {} messages -> {} messages",
            original_len,
            cleaned.len()
        );

        cleaned
    }

    /// Track token usage from API response.
    pub fn track_usage(&self, usage: &Usage) {
        self.total_input_tokens.fetch_add(usage.input_tokens as u64, Ordering::Relaxed);
        self.total_output_tokens.fetch_add(usage.output_tokens as u64, Ordering::Relaxed);
        self.last_input_tokens.store(usage.input_tokens as u64, Ordering::Relaxed);
    }

    /// Get total input tokens consumed.
    pub fn total_input_tokens(&self) -> u64 {
        self.total_input_tokens.load(Ordering::Relaxed)
    }

    /// Get total output tokens generated.
    pub fn total_output_tokens(&self) -> u64 {
        self.total_output_tokens.load(Ordering::Relaxed)
    }

    /// Get last input tokens (for compression decisions).
    pub fn last_input_tokens(&self) -> u64 {
        self.last_input_tokens.load(Ordering::Relaxed)
    }

    /// Set total input tokens (used after compression).
    pub fn set_total_input_tokens(&self, value: u64) {
        self.total_input_tokens.store(value, Ordering::Relaxed);
    }

    /// Set total output tokens.
    pub fn set_total_output_tokens(&self, value: u64) {
        self.total_output_tokens.store(value, Ordering::Relaxed);
    }

    /// Set last input tokens (used after compression).
    pub fn set_last_input_tokens(&self, value: u64) {
        self.last_input_tokens.store(value, Ordering::Relaxed);
    }

    /// Mark a tool input as previewed during streaming.
    pub fn mark_tool_input_previewed(&mut self, tool_id: String) {
        self.previewed_tool_inputs.insert(tool_id);
    }

    /// Check if a tool input was already previewed.
    pub fn was_tool_input_previewed(&self, tool_id: &str) -> bool {
        self.previewed_tool_inputs.contains(tool_id)
    }

    /// Remove a tool input from previewed set (after processing).
    pub fn remove_previewed_tool_input(&mut self, tool_id: &str) -> bool {
        self.previewed_tool_inputs.remove(tool_id)
    }

    /// Increment todo reminder count for a todo item.
    /// Returns the new count.
    pub fn increment_todo_reminder(&mut self, todo_hash: String) -> usize {
        let count = self.todo_reminder_count.get(&todo_hash).copied().unwrap_or(0) + 1;
        self.todo_reminder_count.insert(todo_hash, count);
        count
    }

    /// Get todo reminder count for a todo item.
    pub fn todo_reminder_count(&self, todo_hash: &str) -> usize {
        self.todo_reminder_count.get(todo_hash).copied().unwrap_or(0)
    }

    /// Get reference to the entire todo reminder count map.
    pub fn todo_reminder_count_map(&self) -> &std::collections::HashMap<String, usize> {
        &self.todo_reminder_count
    }

    /// Get mutable reference to the entire todo reminder count map.
    pub fn todo_reminder_count_map_mut(&mut self) -> &mut std::collections::HashMap<String, usize> {
        &mut self.todo_reminder_count
    }

    /// Check if todo reminder limit reached.
    pub fn is_todo_reminder_limit_reached(&self, todo_hash: &str, max_reminders: usize) -> bool {
        self.todo_reminder_count(todo_hash) >= max_reminders
    }

    /// Get reference to read history tracker.
    pub fn read_history(&self) -> &ReadHistoryTracker {
        &self.read_history
    }

    /// Get mutable reference to read history tracker.
    pub fn read_history_mut(&mut self) -> &mut ReadHistoryTracker {
        &mut self.read_history
    }

    /// Add a pending input to queue.
    pub fn add_pending_input(&mut self, input: String) {
        self.pending_inputs.push(input);
    }

    /// Check if there are pending inputs.
    pub fn has_pending_inputs(&self) -> bool {
        !self.pending_inputs.is_empty()
    }

    /// Get reference to pending inputs vector.
    pub fn pending_inputs_vec(&self) -> &Vec<String> {
        &self.pending_inputs
    }

    /// Get mutable reference to pending inputs vector.
    pub fn pending_inputs_vec_mut(&mut self) -> &mut Vec<String> {
        &mut self.pending_inputs
    }

    /// Take all pending inputs (drains the queue).
    pub fn take_pending_inputs(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending_inputs)
    }

    /// Get count of pending inputs.
    pub fn pending_input_count(&self) -> usize {
        self.pending_inputs.len()
    }

    /// Get message count.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Clear all state (reset to initial state).
    pub fn clear(&mut self) {
        self.messages.clear();
        self.total_input_tokens.store(0, Ordering::Relaxed);
        self.total_output_tokens.store(0, Ordering::Relaxed);
        self.last_input_tokens.store(0, Ordering::Relaxed);
        self.previewed_tool_inputs.clear();
        self.todo_reminder_count.clear();
        self.read_history = ReadHistoryTracker::new();
        self.pending_inputs.clear();
    }
}

impl Default for AgentState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::{MessageContent, Role};

    fn create_test_message(text: &str) -> Message {
        Message {
            role: Role::User,
            content: MessageContent::Text(text.to_string()),
        }
    }

    #[test]
    fn test_state_new_is_empty() {
        let state = AgentState::new();

        assert_eq!(state.message_count(), 0);
        assert_eq!(state.total_input_tokens(), 0);
        assert_eq!(state.total_output_tokens(), 0);
        assert_eq!(state.last_input_tokens(), 0);
        assert!(!state.has_pending_inputs());
        assert_eq!(state.pending_input_count(), 0);
    }

    #[test]
    fn test_state_add_message() {
        let mut state = AgentState::new();

        state.add_message(create_test_message("Hello"));
        state.add_message(create_test_message("World"));

        assert_eq!(state.message_count(), 2);
        assert_eq!(state.messages().len(), 2);
    }

    #[test]
    fn test_state_track_usage() {
        let state = AgentState::new();
        let usage = Usage {
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        };

        state.track_usage(&usage);

        assert_eq!(state.total_input_tokens(), 100);
        assert_eq!(state.total_output_tokens(), 50);
        assert_eq!(state.last_input_tokens(), 100);

        // Track again (should accumulate)
        state.track_usage(&usage);
        assert_eq!(state.total_input_tokens(), 200);
        assert_eq!(state.total_output_tokens(), 100);
        assert_eq!(state.last_input_tokens(), 100);
    }

    #[test]
    fn test_state_previewed_tool_inputs() {
        let mut state = AgentState::new();

        // Initially not previewed
        assert!(!state.was_tool_input_previewed("tool_1"));

        // Mark as previewed
        state.mark_tool_input_previewed("tool_1".to_string());
        assert!(state.was_tool_input_previewed("tool_1"));
        assert!(!state.was_tool_input_previewed("tool_2"));

        // Remove previewed
        let removed = state.remove_previewed_tool_input("tool_1");
        assert!(removed, "should return true when removing existing item");
        assert!(!state.was_tool_input_previewed("tool_1"));

        // Remove non-existent
        let removed = state.remove_previewed_tool_input("tool_2");
        assert!(!removed, "should return false when removing non-existent item");
    }

    #[test]
    fn test_state_todo_reminders() {
        let mut state = AgentState::new();
        let todo_hash = "hash_123".to_string();

        // Initially 0
        assert_eq!(state.todo_reminder_count(&todo_hash), 0);
        assert!(!state.is_todo_reminder_limit_reached(&todo_hash, 2));

        // Increment
        let count = state.increment_todo_reminder(todo_hash.clone());
        assert_eq!(count, 1);
        assert_eq!(state.todo_reminder_count(&todo_hash), 1);
        assert!(!state.is_todo_reminder_limit_reached(&todo_hash, 2));

        // Increment again
        let count = state.increment_todo_reminder(todo_hash.clone());
        assert_eq!(count, 2);
        assert!(state.is_todo_reminder_limit_reached(&todo_hash, 2));

        // Increment beyond limit
        let count = state.increment_todo_reminder(todo_hash.clone());
        assert_eq!(count, 3);
        assert!(state.is_todo_reminder_limit_reached(&todo_hash, 2));
    }

    #[test]
    fn test_state_pending_inputs() {
        let mut state = AgentState::new();

        // Initially empty
        assert!(!state.has_pending_inputs());
        assert_eq!(state.pending_input_count(), 0);

        // Add inputs
        state.add_pending_input("input 1".to_string());
        state.add_pending_input("input 2".to_string());

        assert!(state.has_pending_inputs());
        assert_eq!(state.pending_input_count(), 2);

        // Take inputs
        let inputs = state.take_pending_inputs();
        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[0], "input 1");
        assert_eq!(inputs[1], "input 2");

        // Queue drained
        assert!(!state.has_pending_inputs());
        assert_eq!(state.pending_input_count(), 0);
    }

    #[test]
    fn test_state_set_messages() {
        let mut state = AgentState::new();
        state.add_message(create_test_message("Old message"));

        // Replace messages
        let new_messages = vec![
            create_test_message("New 1"),
            create_test_message("New 2"),
        ];
        state.set_messages(new_messages);

        assert_eq!(state.message_count(), 2);
        assert_eq!(state.messages()[0].content, MessageContent::Text("New 1".to_string()));
    }

    #[test]
    fn test_state_clear() {
        let mut state = AgentState::new();

        // Add some state
        state.add_message(create_test_message("Test"));
        state.track_usage(&Usage {
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        });
        state.add_pending_input("pending".to_string());
        state.mark_tool_input_previewed("tool_1".to_string());

        // Clear
        state.clear();

        // Verify all cleared
        assert_eq!(state.message_count(), 0);
        assert_eq!(state.total_input_tokens(), 0);
        assert_eq!(state.total_output_tokens(), 0);
        assert!(!state.has_pending_inputs());
        assert!(!state.was_tool_input_previewed("tool_1"));
    }
}