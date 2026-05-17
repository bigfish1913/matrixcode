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

    #[test]
    fn test_apply_text_delta() {
        let mut bridge = EventBridge::new();
        let mut state = AppState::new();

        let event = AgentEvent::text_delta("Hello");
        bridge.apply(event, &mut state);

        assert!(!state.messages.is_empty());
    }

    #[test]
    fn test_apply_session_started() {
        let mut bridge = EventBridge::new();
        let mut state = AppState::new();

        let event = AgentEvent::session_started();
        bridge.apply(event, &mut state);

        assert_eq!(state.mode, AppMode::Thinking);
    }

    #[test]
    fn test_apply_session_ended() {
        let mut bridge = EventBridge::new();
        let mut state = AppState::new();
        state.mode = AppMode::Thinking;

        let event = AgentEvent::session_ended();
        bridge.apply(event, &mut state);

        assert_eq!(state.mode, AppMode::Idle);
    }

    #[test]
    fn test_apply_usage() {
        let mut bridge = EventBridge::new();
        let mut state = AppState::new();

        let event = AgentEvent::usage(100, 50);
        bridge.apply(event, &mut state);

        assert_eq!(state.tokens_used, 150);
    }
}