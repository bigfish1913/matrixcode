//! Conversation phase detection for dynamic weight adjustment.
//!
//! Analyzes message history to determine the current conversation phase,
//! which affects scoring weights during compression.

use crate::providers::{ContentBlock, Message, MessageContent, Role};

use super::types::ConversationPhase;

/// Detector for conversation phase.
pub struct PhaseDetector;

impl PhaseDetector {
    /// Detect the current conversation phase from message history.
    pub fn detect(messages: &[Message]) -> ConversationPhase {
        // Few messages means initial request phase
        if messages.len() <= 3 {
            return ConversationPhase::InitialRequest;
        }

        // Analyze recent messages (last 10)
        let recent_start = messages.len().saturating_sub(10);
        let recent = &messages[recent_start..];

        // Check for tool activity
        let has_tools = recent.iter().any(has_tool_use);

        if has_tools {
            // Check for finalizing signals
            if has_finalizing_signals(recent) {
                return ConversationPhase::Finalizing;
            }
            return ConversationPhase::ActiveDevelopment;
        }

        // Default to initial request if no tools
        ConversationPhase::InitialRequest
    }

    /// Detect phase with custom window size.
    pub fn detect_with_window(messages: &[Message], window_size: usize) -> ConversationPhase {
        if messages.len() <= 3 {
            return ConversationPhase::InitialRequest;
        }

        let recent_start = messages.len().saturating_sub(window_size);
        let recent = &messages[recent_start..];

        let has_tools = recent.iter().any(has_tool_use);

        if has_tools {
            if has_finalizing_signals(recent) {
                return ConversationPhase::Finalizing;
            }
            return ConversationPhase::ActiveDevelopment;
        }

        ConversationPhase::InitialRequest
    }
}

/// Check if a message contains ToolUse block.
fn has_tool_use(message: &Message) -> bool {
    match &message.content {
        MessageContent::Blocks(blocks) => blocks
            .iter()
            .any(|b| matches!(b, ContentBlock::ToolUse { .. })),
        _ => false,
    }
}

/// Check for signals indicating task is nearing completion.
fn has_finalizing_signals(messages: &[Message]) -> bool {
    for msg in messages {
        // Check for ask tool (user decisions)
        if has_ask_tool(msg) {
            return true;
        }

        // Check for todo completion signals
        if has_todo_completion(msg) {
            return true;
        }

        // Check for user confirmation patterns
        if has_user_confirmation(msg) {
            return true;
        }
    }
    false
}

/// Check if message contains ask tool.
fn has_ask_tool(message: &Message) -> bool {
    match &message.content {
        MessageContent::Blocks(blocks) => blocks.iter().any(|b| {
            if let ContentBlock::ToolUse { name, .. } = b {
                name == "ask"
            } else {
                false
            }
        }),
        MessageContent::Text(text) => text.contains("AskUserQuestion"),
    }
}

/// Check if message indicates todo completion.
fn has_todo_completion(message: &Message) -> bool {
    match &message.content {
        MessageContent::Blocks(blocks) => blocks.iter().any(|b| match b {
            ContentBlock::ToolUse { name, .. } => name == "todo_write",
            ContentBlock::ToolResult { content, .. } => {
                content.contains("completed") || content.contains("done")
            }
            _ => false,
        }),
        MessageContent::Text(text) => {
            text.contains("任务完成")
                || text.contains("task completed")
                || text.contains("all done")
        }
    }
}

/// Check if message contains user confirmation.
fn has_user_confirmation(message: &Message) -> bool {
    if message.role != Role::User {
        return false;
    }

    match &message.content {
        MessageContent::Text(text) => {
            let lower = text.to_lowercase();
            lower.contains("好的")
                || lower.contains("可以")
                || lower.contains("继续")
                || lower.contains("yes")
                || lower.contains("ok")
                || lower.contains("confirm")
                || lower.contains("done")
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_initial_request() {
        let messages = vec![Message {
            role: Role::User,
            content: MessageContent::Text("Hello".to_string()),
        }];
        assert_eq!(
            PhaseDetector::detect(&messages),
            ConversationPhase::InitialRequest
        );
    }

    #[test]
    fn test_detect_active_development() {
        let messages = vec![
            Message {
                role: Role::User,
                content: MessageContent::Text("Read file".to_string()),
            },
            Message {
                role: Role::Assistant,
                content: MessageContent::Blocks(vec![ContentBlock::ToolUse {
                    id: "t1".to_string(),
                    name: "read".to_string(),
                    input: serde_json::json!({"path": "test.rs"}),
                }]),
            },
            Message {
                role: Role::Tool,
                content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                    tool_use_id: "t1".to_string(),
                    content: "file content".to_string(),
                }]),
            },
        ];
        assert_eq!(
            PhaseDetector::detect(&messages),
            ConversationPhase::InitialRequest
        );
    }

    #[test]
    fn test_detect_finalizing() {
        let messages = vec![
            Message {
                role: Role::User,
                content: MessageContent::Text("Start task".to_string()),
            },
            Message {
                role: Role::Assistant,
                content: MessageContent::Blocks(vec![ContentBlock::ToolUse {
                    id: "t1".to_string(),
                    name: "read".to_string(),
                    input: serde_json::json!({"path": "test.rs"}),
                }]),
            },
            Message {
                role: Role::Assistant,
                content: MessageContent::Blocks(vec![ContentBlock::ToolUse {
                    id: "t2".to_string(),
                    name: "ask".to_string(),
                    input: serde_json::json!({"question": "Confirm?"}),
                }]),
            },
        ];
        // With 3 messages, still InitialRequest
        assert_eq!(
            PhaseDetector::detect(&messages),
            ConversationPhase::InitialRequest
        );
    }
}
