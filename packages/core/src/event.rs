//! MatrixCode Event Protocol

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::lsp::LspServerInfo;

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
    SessionRestored, // Session loaded from file with token stats
    NewSession,
    CompressionTriggered,
    CompressionCompleted,
    MemoryLoaded,
    MemoryDetected,    // Memory extracted from conversation
    KeywordsExtracted, // Keywords extracted from context (for debug)
    Error,
    Usage,
    Progress,
    ContextSize,       // Update context window size from provider
    AskQuestion,       // Ask tool: waiting for user input
    ProxyToolRequest,  // Proxy tool: request external execution
    ProxyToolResponse, // Proxy tool: external execution result
    DebugLog,          // Debug log entry for TUI debug panel
    SkillsLoaded,      // Skills loaded notification
    WorkflowsLoaded,   // Workflows loaded notification
    McpServerAdded,    // MCP server added
    QueueProcessed,    // Pending messages processed by Agent
    McpServerRemoved,  // MCP server removed
    McpServerStatus,   // MCP server status update
    LspServerAdded,    // LSP server added
    LspServerRemoved,  // LSP server removed
    LspServerStatus,   // LSP server status update
}

/// Event data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EventData {
    Text {
        delta: String,
    },
    Thinking {
        delta: String,
        signature: Option<String>,
    },
    ToolUse {
        id: String,
        name: String,
        input: Option<serde_json::Value>,
    },
    ToolUseInput {
        id: String,
        delta: String,
    },
    ToolResult {
        tool_use_id: String,
        name: String,
        detail: Option<String>,
        content: String,
        is_error: bool,
    },
    Error {
        message: String,
        code: Option<String>,
        source: Option<String>,
    },
    Usage {
        input_tokens: u64,
        output_tokens: u64,
        cache_creation_input_tokens: Option<u64>,
        cache_read_input_tokens: Option<u64>,
    },
    SessionRestore {
        input_tokens: u64,
        total_output_tokens: u64,
        message_count: usize,
    },
    Progress {
        message: String,
        percentage: Option<u8>,
    },
    ContextSize {
        context_size: u64,
    },
    Compression {
        original_tokens: u64,
        compressed_tokens: u64,
        ratio: f32,
    },
    Memory {
        summary: String,
        entries_count: usize,
    },
    Keywords {
        keywords: Vec<String>,
        source: String,
    }, // Extracted keywords
    AskQuestion {
        question: String,
        options: Option<serde_json::Value>,
    },
    /// Proxy tool request - needs external execution
    ProxyToolRequest {
        request_id: String,
        tool_name: String,
        tool_input: serde_json::Value,
        metadata: crate::tools::toolproxy::ProxyMetadata,
    },
    /// Proxy tool response - external execution result
    ProxyToolResponse {
        request_id: String,
        result: String,
        is_error: bool,
    },
    DebugLog {
        category: String,
        message: String,
    }, // Debug log entry
    SkillsLoaded {
        names: Vec<String>,
    },
    WorkflowsLoaded {
        names: Vec<String>,
    },
    McpServerAdded {
        name: String,
        tool_count: usize,
    },
    QueueProcessed {
        count: usize,
        messages: Vec<String>, // Messages that were processed
    }, // Pending messages processed by Agent
    McpServerRemoved {
        name: String,
    },
    McpServerStatus {
        servers: Vec<McpServerInfo>,
    },
    LspServerAdded {
        name: String,
        language: String,
    },
    LspServerRemoved {
        name: String,
    },
    LspServerStatus {
        servers: Vec<LspServerInfo>,
    },
}

impl AgentEvent {
    pub fn new(event_type: EventType) -> Self {
        Self {
            event_type,
            timestamp: current_timestamp(),
            data: None,
        }
    }

    pub fn with_data(event_type: EventType, data: EventData) -> Self {
        Self {
            event_type,
            timestamp: current_timestamp(),
            data: Some(data),
        }
    }

    pub fn text_delta(delta: impl Into<String>) -> Self {
        Self::with_data(
            EventType::TextDelta,
            EventData::Text {
                delta: delta.into(),
            },
        )
    }

    pub fn text_start() -> Self {
        Self::new(EventType::TextStart)
    }
    pub fn text_end() -> Self {
        Self::new(EventType::TextEnd)
    }
    pub fn thinking_start() -> Self {
        Self::new(EventType::ThinkingStart)
    }
    pub fn thinking_end() -> Self {
        Self::new(EventType::ThinkingEnd)
    }
    pub fn session_started() -> Self {
        Self::new(EventType::SessionStarted)
    }
    pub fn session_ended() -> Self {
        Self::new(EventType::SessionEnded)
    }
    pub fn session_restored(
        input_tokens: u64,
        total_output_tokens: u64,
        message_count: usize,
    ) -> Self {
        Self::with_data(
            EventType::SessionRestored,
            EventData::SessionRestore {
                input_tokens,
                total_output_tokens,
                message_count,
            },
        )
    }

    pub fn thinking_delta(delta: impl Into<String>, signature: Option<String>) -> Self {
        Self::with_data(
            EventType::ThinkingDelta,
            EventData::Thinking {
                delta: delta.into(),
                signature,
            },
        )
    }

    pub fn tool_use_start(
        id: impl Into<String>,
        name: impl Into<String>,
        input: Option<serde_json::Value>,
    ) -> Self {
        Self::with_data(
            EventType::ToolUseStart,
            EventData::ToolUse {
                id: id.into(),
                name: name.into(),
                input,
            },
        )
    }

    pub fn tool_result(
        tool_use_id: impl Into<String>,
        name: impl Into<String>,
        detail: Option<String>,
        content: impl Into<String>,
        is_error: bool,
    ) -> Self {
        Self::with_data(
            EventType::ToolResult,
            EventData::ToolResult {
                tool_use_id: tool_use_id.into(),
                name: name.into(),
                detail,
                content: content.into(),
                is_error,
            },
        )
    }

    pub fn error(message: impl Into<String>, code: Option<String>, source: Option<String>) -> Self {
        Self::with_data(
            EventType::Error,
            EventData::Error {
                message: message.into(),
                code,
                source,
            },
        )
    }

    pub fn progress(message: impl Into<String>, percentage: Option<u8>) -> Self {
        Self::with_data(
            EventType::Progress,
            EventData::Progress {
                message: message.into(),
                percentage,
            },
        )
    }

    pub fn queue_processed(count: usize, messages: Vec<String>) -> Self {
        Self::with_data(
            EventType::QueueProcessed,
            EventData::QueueProcessed { count, messages },
        )
    }

    pub fn usage(input_tokens: u64, output_tokens: u64) -> Self {
        Self::with_data(
            EventType::Usage,
            EventData::Usage {
                input_tokens,
                output_tokens,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            },
        )
    }

    pub fn usage_with_cache(
        input_tokens: u64,
        output_tokens: u64,
        cache_read: u64,
        cache_created: u64,
    ) -> Self {
        Self::with_data(
            EventType::Usage,
            EventData::Usage {
                input_tokens,
                output_tokens,
                cache_creation_input_tokens: if cache_created > 0 {
                    Some(cache_created)
                } else {
                    None
                },
                cache_read_input_tokens: if cache_read > 0 {
                    Some(cache_read)
                } else {
                    None
                },
            },
        )
    }

    pub fn debug_log(category: impl Into<String>, message: impl Into<String>) -> Self {
        Self::with_data(
            EventType::DebugLog,
            EventData::DebugLog {
                category: category.into(),
                message: message.into(),
            },
        )
    }

    pub fn skills_loaded(names: Vec<String>) -> Self {
        Self::with_data(EventType::SkillsLoaded, EventData::SkillsLoaded { names })
    }

    pub fn workflows_loaded(names: Vec<String>) -> Self {
        Self::with_data(
            EventType::WorkflowsLoaded,
            EventData::WorkflowsLoaded { names },
        )
    }

    /// MCP server added event
    pub fn mcp_server_added(name: impl Into<String>, tool_count: usize) -> Self {
        Self::with_data(
            EventType::McpServerAdded,
            EventData::McpServerAdded {
                name: name.into(),
                tool_count,
            },
        )
    }

    /// MCP server removed event
    pub fn mcp_server_removed(name: impl Into<String>) -> Self {
        Self::with_data(
            EventType::McpServerRemoved,
            EventData::McpServerRemoved { name: name.into() },
        )
    }

    /// MCP server status update event
    pub fn mcp_server_status(servers: Vec<McpServerInfo>) -> Self {
        Self::with_data(
            EventType::McpServerStatus,
            EventData::McpServerStatus { servers },
        )
    }

    /// LSP server added event
    pub fn lsp_server_added(name: impl Into<String>, language: impl Into<String>) -> Self {
        Self::with_data(
            EventType::LspServerAdded,
            EventData::LspServerAdded {
                name: name.into(),
                language: language.into(),
            },
        )
    }

    /// LSP server removed event
    pub fn lsp_server_removed(name: impl Into<String>) -> Self {
        Self::with_data(
            EventType::LspServerRemoved,
            EventData::LspServerRemoved { name: name.into() },
        )
    }

    /// LSP server status update event
    pub fn lsp_server_status(servers: Vec<LspServerInfo>) -> Self {
        Self::with_data(
            EventType::LspServerStatus,
            EventData::LspServerStatus { servers },
        )
    }

    /// 创建代理工具请求事件
    pub fn proxy_tool_request(
        request_id: impl Into<String>,
        tool_name: impl Into<String>,
        tool_input: serde_json::Value,
        metadata: crate::tools::toolproxy::ProxyMetadata,
    ) -> Self {
        Self::with_data(
            EventType::ProxyToolRequest,
            EventData::ProxyToolRequest {
                request_id: request_id.into(),
                tool_name: tool_name.into(),
                tool_input,
                metadata,
            },
        )
    }

    /// 创建代理工具响应事件
    pub fn proxy_tool_response(
        request_id: impl Into<String>,
        result: impl Into<String>,
        is_error: bool,
    ) -> Self {
        Self::with_data(
            EventType::ProxyToolResponse,
            EventData::ProxyToolResponse {
                request_id: request_id.into(),
                result: result.into(),
                is_error,
            },
        )
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// MCP server information for status display
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpServerInfo {
    pub name: String,
    pub is_started: bool,
    pub tool_count: usize,
}

impl McpServerInfo {
    pub fn new(name: impl Into<String>, is_started: bool, tool_count: usize) -> Self {
        Self {
            name: name.into(),
            is_started,
            tool_count,
        }
    }

    pub fn from_status(status: &crate::mcp::ServerStatus) -> Self {
        Self {
            name: status.name.clone(),
            is_started: status.is_started,
            tool_count: status.tool_count,
        }
    }
}

#[derive(Debug, Default)]
pub struct EventCollector {
    events: Vec<AgentEvent>,
}

impl EventCollector {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn push(&mut self, event: AgentEvent) {
        self.events.push(event);
    }
    pub fn events(&self) -> &[AgentEvent] {
        &self.events
    }
    pub fn len(&self) -> usize {
        self.events.len()
    }
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
    pub fn clear(&mut self) {
        self.events.clear();
    }
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
