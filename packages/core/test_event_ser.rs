use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    TextStart,
    TextDelta,
    TextEnd,
    ThinkingStart,
    ToolUseStart,
    ToolResult,
    Error,
    Usage,
    SessionStarted,
    SessionEnded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    pub event_type: EventType,
    pub timestamp: u64,
}

fn main() {
    let event = AgentEvent {
        event_type: EventType::TextStart,
        timestamp: 12345,
    };
    let json = serde_json::to_string(&event).unwrap();
    println!("Serialized JSON: {}", json);
    
    // Check what event_type looks like
    let et_json = serde_json::to_string(&EventType::TextStart).unwrap();
    println!("EventType alone: {}", et_json);
    
    let et_json2 = serde_json::to_string(&EventType::ToolUseStart).unwrap();
    println!("ToolUseStart: {}", et_json2);
}
