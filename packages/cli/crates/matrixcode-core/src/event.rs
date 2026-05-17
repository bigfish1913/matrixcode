//! MatrixCode Event Protocol

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Agent event
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentEvent {
    pub event_type: EventType,
    pub timestamp: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<EventData>,
}

/// Event types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    TextStart,
    TextDelta,
    TextEnd,
    ThinkingStart,
    ThinkingDelta,
    ThinkingEnd,
    ToolUseStart,
    ToolUseInputDelta,
    ToolUseInputEnd,
    ToolResult,
    SessionStarted,
    SessionEnded,
    NewSession,
    CompressionTriggered,
    CompressionCompleted,
    MemoryLoaded,
    Error,
    Usage,
    Progress,
}

/// Event data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EventData {
    Text { delta: String },
    Thinking { delta: String, signature: Option<String> },
    ToolUse { id: String, name: String, input: Option<serde_json::Value> },
    ToolUseInput { id: String, delta: String },
    ToolResult { tool_use_id: String, content: String, is_error: bool },
    Error { message: String, code: Option<String>, source: Option<String> },
    Usage { input_tokens: u64, output_tokens: u64, cache_creation_input_tokens: Option<u64>, cache_read_input_tokens: Option<u64> },
    Progress { message: String, percentage: Option<u8> },
    Compression { original_tokens: u64, compressed_tokens: u64, ratio: f32 },
    Memory { summary: String, entries_count: usize },
}

impl AgentEvent {
    pub fn new(event_type: EventType) -> Self {
        Self { event_type, timestamp: current_timestamp(), data: None }
    }

    pub fn with_data(event_type: EventType, data: EventData) -> Self {
        Self { event_type, timestamp: current_timestamp(), data: Some(data) }
    }

    pub fn text_delta(delta: impl Into<String>) -> Self {
        Self::with_data(EventType::TextDelta, EventData::Text { delta: delta.into() })
    }

    pub fn text_start() -> Self { Self::new(EventType::TextStart) }
    pub fn text_end() -> Self { Self::new(EventType::TextEnd) }
    pub fn thinking_start() -> Self { Self::new(EventType::ThinkingStart) }
    pub fn thinking_end() -> Self { Self::new(EventType::ThinkingEnd) }
    pub fn session_started() -> Self { Self::new(EventType::SessionStarted) }
    pub fn session_ended() -> Self { Self::new(EventType::SessionEnded) }

    pub fn thinking_delta(delta: impl Into<String>, signature: Option<String>) -> Self {
        Self::with_data(EventType::ThinkingDelta, EventData::Thinking { delta: delta.into(), signature })
    }

    pub fn tool_use_start(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self::with_data(EventType::ToolUseStart, EventData::ToolUse { id: id.into(), name: name.into(), input: None })
    }

    pub fn tool_result(tool_use_id: impl Into<String>, content: impl Into<String>, is_error: bool) -> Self {
        Self::with_data(EventType::ToolResult, EventData::ToolResult {
            tool_use_id: tool_use_id.into(), content: content.into(), is_error
        })
    }

    pub fn error(message: impl Into<String>, code: Option<String>, source: Option<String>) -> Self {
        Self::with_data(EventType::Error, EventData::Error { message: message.into(), code, source })
    }

    pub fn progress(message: impl Into<String>, percentage: Option<u8>) -> Self {
        Self::with_data(EventType::Progress, EventData::Progress { message: message.into(), percentage })
    }

    pub fn usage(input_tokens: u64, output_tokens: u64) -> Self {
        Self::with_data(EventType::Usage, EventData::Usage {
            input_tokens, output_tokens,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        })
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> { serde_json::to_string(self) }
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> { serde_json::from_str(json) }
}

fn current_timestamp() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

#[derive(Debug, Default)]
pub struct EventCollector { events: Vec<AgentEvent> }

impl EventCollector {
    pub fn new() -> Self { Self::default() }
    pub fn push(&mut self, event: AgentEvent) { self.events.push(event); }
    pub fn events(&self) -> &[AgentEvent] { &self.events }
    pub fn len(&self) -> usize { self.events.len() }
    pub fn is_empty(&self) -> bool { self.events.is_empty() }
    pub fn clear(&mut self) { self.events.clear(); }
    pub fn to_json_lines(&self) -> Result<Vec<String>, serde_json::Error> {
        self.events.iter().map(|e| e.to_json()).collect()
    }
    pub fn output_json_lines(&self) -> Result<String, serde_json::Error> {
        Ok(self.to_json_lines()?.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_event() {
        let e = AgentEvent::text_delta("Hello");
        assert!(e.to_json().unwrap().contains("Hello"));
    }
}