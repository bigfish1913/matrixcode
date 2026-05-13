pub mod anthropic;
pub mod openai;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::tools::ToolDefinition;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: MessageContent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
    },
    /// Anthropic extended-thinking block. `signature` is required when sending
    /// the block back to the API in a follow-up turn.
    #[serde(rename = "thinking")]
    Thinking {
        thinking: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },
    /// Server-side tool use (e.g., web_search_tool). The server executes
    /// the tool and returns results directly without client intervention.
    #[serde(rename = "server_tool_use")]
    ServerToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// Result from a server-side web search tool.
    #[serde(rename = "web_search_tool_result")]
    WebSearchResult {
        tool_use_id: String,
        content: WebSearchContent,
    },
}

/// Content returned by the server-side web search tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchContent {
    pub results: Vec<WebSearchResultItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchResultItem {
    pub title: Option<String>,
    pub url: String,
    pub encrypted_content: Option<String>,
    pub snippet: Option<String>,
}

/// Server-side tool definition. These tools are executed by the API provider
/// rather than by the client. Currently only web_search_tool is supported.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerTool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_uses: Option<u32>,
}

impl ServerTool {
    /// Create a new web search server tool.
    pub fn web_search(max_uses: Option<u32>) -> Self {
        Self {
            tool_type: "web_search_tool".to_string(),
            name: "web_search".to_string(),
            max_uses,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDefinition>,
    pub system: Option<String>,
    pub think: bool,
    /// Server-side tools that are executed by the API provider.
    pub server_tools: Vec<ServerTool>,
}

#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub content: Vec<ContentBlock>,
    pub stop_reason: StopReason,
    pub usage: Usage,
}

/// Token accounting for one provider turn. `input_tokens` already includes
/// cached/uncached portions combined — when providers expose cache details
/// separately we capture them here so callers can report cache effectiveness.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_creation_input_tokens: u32,
    pub cache_read_input_tokens: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
}

/// Incremental events emitted during a streaming chat turn.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// First byte received from the server — agent should stop any waiting spinner.
    FirstByte,
    /// Extended-thinking text delta (Anthropic thinking block).
    ThinkingDelta(String),
    /// Visible assistant text delta.
    TextDelta(String),
    /// A new tool_use block started.
    ToolUseStart { id: String, name: String },
    /// Incremental progress for the current tool_use block's JSON input.
    /// `bytes_so_far` is the total accumulated size of the partial JSON
    /// received for this block — useful for driving progress indicators
    /// while the model streams large arguments (e.g. a full file body).
    ToolInputDelta { bytes_so_far: usize },
    /// Final turn result — includes the full assembled content blocks.
    Done(ChatResponse),
    /// Fatal error during streaming.
    Error(String),
}

#[async_trait]
pub trait Provider: Send + Sync {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse>;

    /// Best-effort context window size (in tokens) for the configured model.
    /// `None` if the provider cannot infer it; callers should treat that as
    /// "don't render a fullness bar".
    fn context_size(&self) -> Option<u32> {
        None
    }

    /// Stream a chat turn. Default impl wraps `chat` and emits one `Done` event,
    /// so providers without native streaming still work (no incremental thinking).
    async fn chat_stream(&self, request: ChatRequest) -> Result<mpsc::Receiver<StreamEvent>> {
        let (tx, rx) = mpsc::channel(32);
        let response = self.chat(request).await?;
        let _ = tx.send(StreamEvent::FirstByte).await;
        for block in &response.content {
            if let ContentBlock::Text { text } = block {
                let _ = tx.send(StreamEvent::TextDelta(text.clone())).await;
            }
        }
        let _ = tx.send(StreamEvent::Done(response)).await;
        Ok(rx)
    }
}
