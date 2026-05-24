pub mod anthropic;
pub mod openai;

#[cfg(test)]
mod tests;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WebSearchContent {
    pub results: Vec<WebSearchResultItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    /// Maximum output tokens for the response.
    pub max_tokens: u32,
    /// Server-side tools that are executed by the API provider.
    pub server_tools: Vec<ServerTool>,
    /// Enable prompt caching for Anthropic provider.
    pub enable_caching: bool,
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
    /// Real-time usage update (output tokens so far).
    Usage { output_tokens: u32 },
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

    /// Get the model name for this provider.
    fn model_name(&self) -> &str {
        "unknown"
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

    /// Clone the provider into a boxed type.
    fn clone_box(&self) -> Box<dyn Provider>;
}

impl Clone for Box<dyn Provider> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

// ============================================================================
// Provider Factory
// ============================================================================

/// Provider type enumeration for factory creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderType {
    Anthropic,
    OpenAI,
}

/// Create a provider instance based on type and configuration.
/// This is the recommended way to obtain a Provider instance.
pub fn create_provider(
    provider_type: ProviderType,
    api_key: String,
    model: String,
    base_url: Option<String>,
) -> Result<Box<dyn Provider>> {
    create_provider_with_headers(provider_type, api_key, model, base_url, None)
}

/// Create a provider with extra headers support.
pub fn create_provider_with_headers(
    provider_type: ProviderType,
    api_key: String,
    model: String,
    base_url: Option<String>,
    extra_headers: Option<std::collections::HashMap<String, String>>,
) -> Result<Box<dyn Provider>> {
    match provider_type {
        ProviderType::Anthropic => {
            let provider = anthropic::AnthropicProvider::with_headers(
                api_key,
                model,
                base_url.unwrap_or_else(|| "https://api.anthropic.com".to_string()),
                extra_headers,
            );
            Ok(Box::new(provider))
        }
        ProviderType::OpenAI => {
            let provider = openai::OpenAIProvider::with_headers(
                api_key,
                model,
                base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
                extra_headers,
            );
            Ok(Box::new(provider))
        }
    }
}

/// Create a minimal provider from environment variables (for background tasks).
/// Uses global config for API key and base URL, suitable for non-blocking background operations.
pub fn create_minimal_provider(model: &str) -> Box<dyn Provider> {
    // Try to load .env first (background tasks may not have .env loaded)
    let _ = dotenvy::dotenv();

    // Get API key from env (try multiple env vars)
    let api_key = std::env::var("API_KEY")
        .or_else(|_| std::env::var("ANTHROPIC_AUTH_TOKEN"))
        .or_else(|_| std::env::var("ANTHROPIC_API_KEY"))
        .or_else(|_| std::env::var("OPENAI_API_KEY"))
        .unwrap_or_default();

    // Get base URL from env
    let base_url = std::env::var("BASE_URL")
        .or_else(|_| std::env::var("ANTHROPIC_BASE_URL"))
        .ok();

    // Infer provider type from model name
    let provider_type = infer_provider_type(model);

    // Create provider (ignore errors, return a default)
    create_provider_with_headers(provider_type, api_key, model.to_string(), base_url, None)
        .unwrap_or_else(|_| {
            // Fallback: create a dummy provider that returns empty
            // This won't actually work, but prevents crashes
            panic!("Failed to create minimal provider for background task: no API key configured")
        })
}

/// Infer provider type from model name.
/// Returns Anthropic for Claude models, OpenAI for GPT models.
pub fn infer_provider_type(model: &str) -> ProviderType {
    let lower = model.to_lowercase();
    if lower.contains("claude")
        || lower.contains("opus")
        || lower.contains("sonnet")
        || lower.contains("haiku")
    {
        ProviderType::Anthropic
    } else if lower.contains("gpt")
        || lower.contains("o1")
        || lower.contains("o3")
        || lower.contains("o4")
    {
        ProviderType::OpenAI
    } else {
        // Default to Anthropic for unknown models
        ProviderType::Anthropic
    }
}
