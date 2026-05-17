//! Event Bridge
//!
//! Converts AgentEvent to AppState updates.

use matrixcode_core::{AgentEvent, EventData, EventType};

use crate::app::{AppMode, AppState};

/// Event bridge for converting Agent events to UI state updates
pub struct EventBridge;

impl EventBridge {
    /// Create new event bridge
    pub fn new() -> Self {
        Self
    }

    /// Apply event to state
    pub fn apply(&mut self, event: AgentEvent, state: &mut AppState) {
        match event.event_type {
            EventType::SessionStarted => {
                state.mode = AppMode::Thinking;
                state.status_message = Some("Session started".to_string());
            }
            EventType::TextStart => {
                // Prepare for text output
            }
            EventType::TextDelta => {
                if let Some(EventData::Text { delta }) = &event.data {
                    state.append_output(delta);
                }
            }
            EventType::TextEnd => {
                // Text complete
            }
            EventType::ThinkingStart => {
                state.status_message = Some("Thinking...".to_string());
            }
            EventType::ThinkingDelta => {
                if let Some(EventData::Thinking { delta, .. }) = &event.data {
                    state.append_thinking(delta);
                }
            }
            EventType::ThinkingEnd => {
                state.status_message = None;
            }
            EventType::ToolUseStart => {
                if let Some(EventData::ToolUse { id, name, .. }) = &event.data {
                    state.mode = AppMode::ToolExecuting {
                        id: id.clone(),
                        name: name.clone(),
                    };
                    state.status_message = Some(format!("Executing: {}", name));
                }
            }
            EventType::ToolUseInputDelta => {
                // Tool input streaming - not handled for now
            }
            EventType::ToolUseInputEnd => {
                // Tool input complete
            }
            EventType::ToolResult => {
                if let Some(EventData::ToolResult { tool_use_id, content, is_error }) = &event.data {
                    state.append_tool_result(tool_use_id, "tool", content, *is_error);
                    // Reset to thinking mode after tool result
                    state.mode = AppMode::Thinking;
                }
            }
            EventType::SessionEnded => {
                state.mode = AppMode::Idle;
                state.status_message = Some("Session ended".to_string());
            }
            EventType::NewSession => {
                state.messages.clear();
                state.mode = AppMode::Idle;
                state.status_message = Some("New session".to_string());
            }
            EventType::CompressionTriggered => {
                state.status_message = Some("Compressing context...".to_string());
            }
            EventType::CompressionCompleted => {
                if let Some(EventData::Compression { compressed_tokens, .. }) = &event.data {
                    state.status_message = Some(format!("Compressed to {} tokens", compressed_tokens));
                }
            }
            EventType::MemoryLoaded => {
                if let Some(EventData::Memory { summary, .. }) = &event.data {
                    state.status_message = Some(format!("Memory loaded: {}", summary));
                }
            }
            EventType::Error => {
                if let Some(EventData::Error { message, .. }) = &event.data {
                    state.show_error(message);
                    state.mode = AppMode::Idle;
                }
            }
            EventType::Usage => {
                if let Some(EventData::Usage { input_tokens, output_tokens, .. }) = &event.data {
                    state.tokens_used = input_tokens + output_tokens;
                }
            }
            EventType::Progress => {
                if let Some(EventData::Progress { message, percentage }) = &event.data {
                    if let Some(p) = percentage {
                        state.status_message = Some(format!("{} ({}%)", message, p));
                    } else {
                        state.status_message = Some(message.clone());
                    }
                }
            }
        }
    }
}

impl Default for EventBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{OutputBlock, OutputMessage};

    // ===== EventBridge Creation Tests =====

    #[test]
    fn test_event_bridge_new() {
        let bridge = EventBridge::new();
        // Just verify it can be created
        assert!(true);
    }

    #[test]
    fn test_event_bridge_default() {
        let bridge = EventBridge::default();
        // Just verify it can be created
        assert!(true);
    }

    // ===== Text Event Tests =====

    #[test]
    fn test_apply_text_delta() {
        let mut bridge = EventBridge::new();
        let mut state = AppState::new();

        let event = AgentEvent::text_delta("Hello");
        bridge.apply(event, &mut state);

        assert!(!state.messages.is_empty());
    }

    #[test]
    fn test_apply_text_delta_multiple() {
        let mut bridge = EventBridge::new();
        let mut state = AppState::new();

        bridge.apply(AgentEvent::text_delta("Hello"), &mut state);
        bridge.apply(AgentEvent::text_delta(" "), &mut state);
        bridge.apply(AgentEvent::text_delta("World"), &mut state);

        assert_eq!(state.messages.len(), 1);
        if let OutputBlock::Text(text) = &state.messages[0].content[0] {
            assert_eq!(text, "Hello World");
        } else {
            panic!("Expected Text block");
        }
    }

    #[test]
    fn test_apply_text_start() {
        let mut bridge = EventBridge::new();
        let mut state = AppState::new();

        let event = AgentEvent::text_start();
        bridge.apply(event, &mut state);

        // TextStart doesn't modify state, just prepares for text output
        assert!(state.messages.is_empty());
    }

    #[test]
    fn test_apply_text_end() {
        let mut bridge = EventBridge::new();
        let mut state = AppState::new();

        let event = AgentEvent::text_end();
        bridge.apply(event, &mut state);

        // TextEnd doesn't modify state
        assert!(state.messages.is_empty());
    }

    // ===== Session Event Tests =====

    #[test]
    fn test_apply_session_started() {
        let mut bridge = EventBridge::new();
        let mut state = AppState::new();

        let event = AgentEvent::session_started();
        bridge.apply(event, &mut state);

        assert_eq!(state.mode, AppMode::Thinking);
        assert_eq!(state.status_message, Some("Session started".to_string()));
    }

    #[test]
    fn test_apply_session_ended() {
        let mut bridge = EventBridge::new();
        let mut state = AppState::new();
        state.mode = AppMode::Thinking;

        let event = AgentEvent::session_ended();
        bridge.apply(event, &mut state);

        assert_eq!(state.mode, AppMode::Idle);
        assert_eq!(state.status_message, Some("Session ended".to_string()));
    }

    #[test]
    fn test_apply_new_session() {
        let mut bridge = EventBridge::new();
        let mut state = AppState::new();
        state.messages.push(OutputMessage::user("Previous message".to_string()));

        let event = AgentEvent::new(EventType::NewSession);
        bridge.apply(event, &mut state);

        assert!(state.messages.is_empty());
        assert_eq!(state.mode, AppMode::Idle);
        assert_eq!(state.status_message, Some("New session".to_string()));
    }

    // ===== Thinking Event Tests =====

    #[test]
    fn test_apply_thinking_start() {
        let mut bridge = EventBridge::new();
        let mut state = AppState::new();

        let event = AgentEvent::thinking_start();
        bridge.apply(event, &mut state);

        assert_eq!(state.status_message, Some("Thinking...".to_string()));
    }

    #[test]
    fn test_apply_thinking_delta() {
        let mut bridge = EventBridge::new();
        let mut state = AppState::new();

        let event = AgentEvent::thinking_delta("Let me think...", None);
        bridge.apply(event, &mut state);

        assert_eq!(state.messages.len(), 1);
        if let OutputBlock::Thinking(text) = &state.messages[0].content[0] {
            assert_eq!(text, "Let me think...");
        } else {
            panic!("Expected Thinking block");
        }
    }

    #[test]
    fn test_apply_thinking_delta_with_signature() {
        let mut bridge = EventBridge::new();
        let mut state = AppState::new();

        let event = AgentEvent::thinking_delta("Analyzing...", Some("sig-123".to_string()));
        bridge.apply(event, &mut state);

        assert_eq!(state.messages.len(), 1);
        if let OutputBlock::Thinking(text) = &state.messages[0].content[0] {
            assert_eq!(text, "Analyzing...");
        } else {
            panic!("Expected Thinking block");
        }
    }

    #[test]
    fn test_apply_thinking_end() {
        let mut bridge = EventBridge::new();
        let mut state = AppState::new();
        state.status_message = Some("Thinking...".to_string());

        let event = AgentEvent::thinking_end();
        bridge.apply(event, &mut state);

        assert!(state.status_message.is_none());
    }

    // ===== Tool Event Tests =====

    #[test]
    fn test_apply_tool_use_start() {
        let mut bridge = EventBridge::new();
        let mut state = AppState::new();

        let event = AgentEvent::tool_use_start("tool-123", "ReadFile");
        bridge.apply(event, &mut state);

        assert_eq!(state.mode, AppMode::ToolExecuting {
            id: "tool-123".to_string(),
            name: "ReadFile".to_string(),
        });
        assert_eq!(state.status_message, Some("Executing: ReadFile".to_string()));
    }

    #[test]
    fn test_apply_tool_result() {
        let mut bridge = EventBridge::new();
        let mut state = AppState::new();
        state.mode = AppMode::ToolExecuting {
            id: "tool-123".to_string(),
            name: "ReadFile".to_string(),
        };

        let event = AgentEvent::tool_result("tool-123", "File contents here", false);
        bridge.apply(event, &mut state);

        assert_eq!(state.mode, AppMode::Thinking);
        assert_eq!(state.messages.len(), 1);
        if let OutputBlock::ToolUse { id, name, result, is_error } = &state.messages[0].content[0] {
            assert_eq!(id, "tool-123");
            assert_eq!(result, "File contents here");
            assert!(!is_error);
            // Note: name in tool result is hardcoded as "tool" in bridge.rs
            assert_eq!(name, "tool");
        } else {
            panic!("Expected ToolUse block");
        }
    }

    #[test]
    fn test_apply_tool_result_error() {
        let mut bridge = EventBridge::new();
        let mut state = AppState::new();

        let event = AgentEvent::tool_result("tool-456", "Permission denied", true);
        bridge.apply(event, &mut state);

        if let OutputBlock::ToolUse { is_error, .. } = &state.messages[0].content[0] {
            assert!(is_error);
        } else {
            panic!("Expected ToolUse block");
        }
    }

    // ===== Usage Event Tests =====

    #[test]
    fn test_apply_usage() {
        let mut bridge = EventBridge::new();
        let mut state = AppState::new();

        let event = AgentEvent::usage(100, 50);
        bridge.apply(event, &mut state);

        assert_eq!(state.tokens_used, 150);
    }

    #[test]
    fn test_apply_usage_multiple() {
        let mut bridge = EventBridge::new();
        let mut state = AppState::new();

        bridge.apply(AgentEvent::usage(100, 50), &mut state);
        assert_eq!(state.tokens_used, 150);

        bridge.apply(AgentEvent::usage(200, 100), &mut state);
        assert_eq!(state.tokens_used, 300); // Overwrites, not accumulates
    }

    // ===== Error Event Tests =====

    #[test]
    fn test_apply_error() {
        let mut bridge = EventBridge::new();
        let mut state = AppState::new();
        state.mode = AppMode::Thinking;

        let event = AgentEvent::error("Something went wrong", Some("E001".to_string()), None);
        bridge.apply(event, &mut state);

        assert_eq!(state.mode, AppMode::Idle);
        assert_eq!(state.status_message, Some("Error: Something went wrong".to_string()));
    }

    #[test]
    fn test_apply_error_with_source() {
        let mut bridge = EventBridge::new();
        let mut state = AppState::new();

        let event = AgentEvent::error("Network error", None, Some("NetworkLayer".to_string()));
        bridge.apply(event, &mut state);

        assert_eq!(state.status_message, Some("Error: Network error".to_string()));
    }

    // ===== Progress Event Tests =====

    #[test]
    fn test_apply_progress_with_percentage() {
        let mut bridge = EventBridge::new();
        let mut state = AppState::new();

        let event = AgentEvent::progress("Processing files", Some(75));
        bridge.apply(event, &mut state);

        assert_eq!(state.status_message, Some("Processing files (75%)".to_string()));
    }

    #[test]
    fn test_apply_progress_without_percentage() {
        let mut bridge = EventBridge::new();
        let mut state = AppState::new();

        let event = AgentEvent::progress("Starting process", None);
        bridge.apply(event, &mut state);

        assert_eq!(state.status_message, Some("Starting process".to_string()));
    }

    // ===== Compression Event Tests =====

    #[test]
    fn test_apply_compression_triggered() {
        let mut bridge = EventBridge::new();
        let mut state = AppState::new();

        let event = AgentEvent::new(EventType::CompressionTriggered);
        bridge.apply(event, &mut state);

        assert_eq!(state.status_message, Some("Compressing context...".to_string()));
    }

    #[test]
    fn test_apply_compression_completed() {
        let mut bridge = EventBridge::new();
        let mut state = AppState::new();

        let event = AgentEvent::with_data(
            EventType::CompressionCompleted,
            EventData::Compression {
                original_tokens: 10000,
                compressed_tokens: 3000,
                ratio: 0.3,
            },
        );
        bridge.apply(event, &mut state);

        assert_eq!(state.status_message, Some("Compressed to 3000 tokens".to_string()));
    }

    // ===== Memory Event Tests =====

    #[test]
    fn test_apply_memory_loaded() {
        let mut bridge = EventBridge::new();
        let mut state = AppState::new();

        let event = AgentEvent::with_data(
            EventType::MemoryLoaded,
            EventData::Memory {
                summary: "Previous context loaded".to_string(),
                entries_count: 5,
            },
        );
        bridge.apply(event, &mut state);

        assert_eq!(state.status_message, Some("Memory loaded: Previous context loaded".to_string()));
    }

    // ===== Event Sequence Tests =====

    #[test]
    fn test_event_sequence_text_generation() {
        let mut bridge = EventBridge::new();
        let mut state = AppState::new();

        // Simulate a text generation sequence
        bridge.apply(AgentEvent::session_started(), &mut state);
        assert_eq!(state.mode, AppMode::Thinking);

        bridge.apply(AgentEvent::text_start(), &mut state);
        bridge.apply(AgentEvent::text_delta("Hello"), &mut state);
        bridge.apply(AgentEvent::text_delta(" world"), &mut state);
        bridge.apply(AgentEvent::text_end(), &mut state);

        assert_eq!(state.messages.len(), 1);
        if let OutputBlock::Text(text) = &state.messages[0].content[0] {
            assert_eq!(text, "Hello world");
        } else {
            panic!("Expected Text block");
        }

        bridge.apply(AgentEvent::session_ended(), &mut state);
        assert_eq!(state.mode, AppMode::Idle);
    }

    #[test]
    fn test_event_sequence_tool_execution() {
        let mut bridge = EventBridge::new();
        let mut state = AppState::new();

        // Simulate a tool execution sequence
        bridge.apply(AgentEvent::session_started(), &mut state);
        bridge.apply(AgentEvent::thinking_start(), &mut state);
        assert_eq!(state.status_message, Some("Thinking...".to_string()));

        bridge.apply(AgentEvent::tool_use_start("tool-1", "ReadFile"), &mut state);
        assert_eq!(state.mode, AppMode::ToolExecuting {
            id: "tool-1".to_string(),
            name: "ReadFile".to_string(),
        });

        bridge.apply(AgentEvent::tool_result("tool-1", "file contents", false), &mut state);
        assert_eq!(state.mode, AppMode::Thinking);

        bridge.apply(AgentEvent::thinking_end(), &mut state);
        assert!(state.status_message.is_none());
    }
}