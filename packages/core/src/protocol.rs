// ============================================================================
// IPC Protocol for VSCode Extension Integration
// ============================================================================
// 
// This module defines the message format for communication between
// the VSCode extension and the MatrixCode CLI daemon.
//
// Communication flow:
// 1. VSCode extension spawns CLI with --daemon --json flags
// 2. Extension sends JSON requests via stdin
// 3. CLI streams JSON events via stdout (JSON Lines format)
//
// Example request:
//   {"type":"chat","content":"帮我分析这个函数","context":{"file":"src/main.rs"}}
//
// Example response (streaming):
//   {"type":"text","content":"这是一个"}
//   {"type":"text","content":"简单的函数"}
//   {"type":"tool_use","id":"tool_1","name":"read","input":{"path":"src/main.rs"}}
//   {"type":"tool_result","tool_use_id":"tool_1","content":"..."}
//   {"type":"done","usage":{"input":1234,"output":567}}

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Client Requests (from VSCode extension)
// ============================================================================

/// Request from VSCode extension
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum ClientRequest {
    /// Chat message
    Chat {
        content: String,
        #[serde(default)]
        context: Option<RequestContext>,
    },
    
    /// Quick action on code
    QuickAction {
        action: QuickActionType,
        content: String,
        #[serde(default)]
        context: Option<RequestContext>,
        #[serde(default)]
        instructions: Option<String>,
    },
    
    /// Start a new session
    NewSession,
    
    /// Get current status
    Status,
    
    /// Memory operations
    Memory {
        operation: MemoryOperation,
    },
    
    /// Load a specific session
    LoadSession {
        session_id: String,
    },
    
    /// List sessions
    ListSessions,
}

/// Types of quick actions
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuickActionType {
    Explain,
    Fix,
    GenerateTests,
    Refactor,
    Optimize,
    Document,
    Translate,
}

/// Memory operations
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryOperation {
    List,
    Search { query: String },
    Add { content: String, category: Option<String> },
    Clear,
    Stats,
}

/// Context information from VSCode
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct RequestContext {
    /// Workspace root path
    #[serde(default)]
    pub workspace: Option<String>,
    
    /// Current file path
    #[serde(default)]
    pub file: Option<String>,
    
    /// File language ID
    #[serde(default)]
    pub language: Option<String>,
    
    /// Selected text range
    #[serde(default)]
    pub selection: Option<Selection>,
    
    /// Diagnostics (errors, warnings) in the selection
    #[serde(default)]
    pub diagnostics: Option<Vec<Diagnostic>>,
    
    /// Additional context (e.g., related files)
    #[serde(default)]
    pub extra_files: Option<Vec<String>>,
}

/// Text selection in editor
#[derive(Debug, Serialize, Deserialize)]
pub struct Selection {
    pub start: Position,
    pub end: Position,
}

/// Position in text
#[derive(Debug, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

/// Diagnostic information
#[derive(Debug, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: String,
    pub message: String,
    pub range: Selection,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub code: Option<String>,
}

// ============================================================================
// Server Events (streamed to VSCode extension)
// ============================================================================

/// Event streamed to VSCode extension
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum StreamEvent {
    /// Text content (streaming)
    Text { content: String },
    
    /// Thinking content (extended thinking mode)
    Thinking { content: String },
    
    /// Tool use request
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    
    /// Tool execution result
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(default)]
        success: bool,
    },
    
    /// Server-side web search result (Anthropic)
    WebSearchResult {
        tool_use_id: String,
        content: String,
    },
    
    /// Error occurred
    Error {
        message: String,
        #[serde(default)]
        code: Option<String>,
    },
    
    /// Request completed
    Done {
        #[serde(default)]
        usage: Option<Usage>,
    },
    
    /// Session started/loaded
    SessionStarted {
        session_id: String,
        #[serde(default)]
        memory_count: Option<usize>,
    },
    
    /// Status response
    StatusResponse {
        session_id: Option<String>,
        message_count: usize,
        total_tokens: u64,
        is_streaming: bool,
    },
    
    /// Memory list response
    MemoryList {
        memories: Vec<MemoryEntry>,
    },
    
    /// Memory stats response
    MemoryStats {
        total: usize,
        by_category: HashMap<String, usize>,
    },
    
    /// Session list response
    SessionList {
        sessions: Vec<SessionInfo>,
    },
    
    /// New memory added
    MemoryAdded {
        category: String,
        content: String,
    },
    
    /// Log message (for debugging)
    Log {
        level: String,
        message: String,
    },
}

/// Token usage information
#[derive(Debug, Serialize, Deserialize)]
pub struct Usage {
    pub input: u64,
    pub output: u64,
    #[serde(default)]
    pub cache_read: Option<u64>,
    #[serde(default)]
    pub cache_write: Option<u64>,
}

/// Memory entry
#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub category: String,
    pub content: String,
    pub created_at: String,
    #[serde(default)]
    pub project: Option<String>,
}

/// Session information
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    pub created_at: String,
    pub message_count: usize,
    #[serde(default)]
    pub last_used: Option<String>,
}

// ============================================================================
// Helper functions
// ============================================================================

impl StreamEvent {
    /// Create a text event
    pub fn text(content: impl Into<String>) -> Self {
        StreamEvent::Text { content: content.into() }
    }
    
    /// Create a thinking event
    pub fn thinking(content: impl Into<String>) -> Self {
        StreamEvent::Thinking { content: content.into() }
    }
    
    /// Create a tool use event
    pub fn tool_use(id: impl Into<String>, name: impl Into<String>, input: serde_json::Value) -> Self {
        StreamEvent::ToolUse {
            id: id.into(),
            name: name.into(),
            input,
        }
    }
    
    /// Create a tool result event
    pub fn tool_result(tool_use_id: impl Into<String>, content: impl Into<String>, success: bool) -> Self {
        StreamEvent::ToolResult {
            tool_use_id: tool_use_id.into(),
            content: content.into(),
            success,
        }
    }
    
    /// Create an error event
    pub fn error(message: impl Into<String>) -> Self {
        StreamEvent::Error { message: message.into(), code: None }
    }
    
    /// Create a done event
    pub fn done(usage: Option<Usage>) -> Self {
        StreamEvent::Done { usage }
    }
    
    /// Create a session started event
    pub fn session_started(session_id: impl Into<String>, memory_count: Option<usize>) -> Self {
        StreamEvent::SessionStarted {
            session_id: session_id.into(),
            memory_count,
        }
    }
    
    /// Serialize to JSON line
    pub fn to_json_line(&self) -> String {
        serde_json::to_string(self).unwrap_or_default() + "\n"
    }
}

impl Usage {
    /// Create usage from token counts
    pub fn new(input: u64, output: u64) -> Self {
        Usage { input, output, cache_read: None, cache_write: None }
    }
    
    /// Create usage with cache information
    pub fn with_cache(input: u64, output: u64, cache_read: u64, cache_write: u64) -> Self {
        Usage {
            input,
            output,
            cache_read: Some(cache_read),
            cache_write: Some(cache_write),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_serialize_chat_request() {
        let request = ClientRequest::Chat {
            content: "Hello".to_string(),
            context: None,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"type\":\"chat\""));
        assert!(json.contains("\"content\":\"Hello\""));
    }
    
    #[test]
    fn test_deserialize_chat_request() {
        let json = "{\"type\":\"chat\",\"content\":\"Hello\",\"context\":null}";
        let request: ClientRequest = serde_json::from_str(json).unwrap();
        match request {
            ClientRequest::Chat { content, .. } => {
                assert_eq!(content, "Hello");
            }
            _ => panic!("Expected Chat request"),
        }
    }
    
    #[test]
    fn test_serialize_stream_event() {
        let event = StreamEvent::text("Hello world");
        let json = event.to_json_line();
        assert!(json.contains("\"type\":\"text\""));
        assert!(json.contains("\"content\":\"Hello world\""));
        assert!(json.ends_with("\n"));
    }
    
    #[test]
    fn test_serialize_tool_use() {
        let event = StreamEvent::tool_use("tool_1", "read", serde_json::json!({"path": "src/main.rs"}));
        let json = event.to_json_line();
        assert!(json.contains("\"type\":\"tool_use\""));
        assert!(json.contains("\"id\":\"tool_1\""));
        assert!(json.contains("\"name\":\"read\""));
    }
    
    #[test]
    fn test_request_context_with_file() {
        let json = "{\"workspace\":\"/project\",\"file\":\"src/main.rs\",\"language\":\"rust\"}";
        let context: RequestContext = serde_json::from_str(json).unwrap();
        assert_eq!(context.workspace, Some("/project".to_string()));
        assert_eq!(context.file, Some("src/main.rs".to_string()));
        assert_eq!(context.language, Some("rust".to_string()));
    }
    
    #[test]
    fn test_quick_action_request() {
        let json = "{\"type\":\"quick_action\",\"action\":\"explain\",\"content\":\"fn main(){}\",\"context\":{\"language\":\"rust\"}}";
        let request: ClientRequest = serde_json::from_str(json).unwrap();
        match request {
            ClientRequest::QuickAction { action, content, .. } => {
                assert_eq!(action, QuickActionType::Explain);
                assert_eq!(content, "fn main(){}");
            }
            _ => panic!("Expected QuickAction request"),
        }
    }
}