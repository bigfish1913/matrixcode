use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

use crate::constants::{
    DEFAULT_CONNECT_TIMEOUT_SECS, DEFAULT_READ_TIMEOUT_SECS, DEFAULT_REQUEST_TIMEOUT_SECS,
};
use crate::models::context_window_for;
use crate::tools::ToolDefinition;

use super::{
    ChatRequest, ChatResponse, ContentBlock, Message, MessageContent, Provider, Role, StopReason,
    Usage,
};

pub struct OpenAIProvider {
    api_key: String,
    model: String,
    base_url: String,
    client: reqwest::Client,
    /// Extra headers from config
    extra_headers: Vec<(String, String)>,
}

impl OpenAIProvider {
    pub fn new(api_key: String, model: String, base_url: String) -> Self {
        Self::with_headers(api_key, model, base_url, None)
    }

    pub fn with_headers(
        api_key: String,
        model: String,
        base_url: String,
        extra_headers: Option<std::collections::HashMap<String, String>>,
    ) -> Self {
        // Use longer timeout for streaming responses (like Anthropic provider)
        // - Total timeout: 300s (for long thinking/reasoning)
        // - Connect timeout: 10s
        // - Read timeout per chunk: 60s (for slow responses between chunks)
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS))
            .connect_timeout(Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS))
            .read_timeout(Duration::from_secs(DEFAULT_READ_TIMEOUT_SECS))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        let extra_headers: Vec<(String, String)> = extra_headers
            .map(|h| h.into_iter().collect())
            .unwrap_or_default();
        Self {
            api_key,
            model,
            base_url,
            client,
            extra_headers,
        }
    }

    fn convert_messages(&self, messages: &[Message], system: Option<&str>) -> Vec<Value> {
        let mut result = Vec::new();

        if let Some(sys) = system {
            result.push(json!({"role": "system", "content": sys}));
        }

        for msg in messages {
            match (&msg.role, &msg.content) {
                (Role::System, _) => {}
                (Role::User, MessageContent::Text(text)) => {
                    result.push(json!({"role": "user", "content": text}));
                }
                (Role::Assistant, MessageContent::Text(text)) => {
                    result.push(json!({"role": "assistant", "content": text}));
                }
                (Role::Assistant, MessageContent::Blocks(blocks)) => {
                    let mut tool_calls = Vec::new();
                    let mut text_parts = Vec::new();
                    // Note: We intentionally DO NOT include thinking/reasoning blocks from history
                    // to prevent the model from repeating similar thinking patterns.
                    // Thinking blocks are for user visibility only, not for API context.

                    for block in blocks {
                        match block {
                            // Skip thinking blocks - they should not be sent back to the API
                            ContentBlock::Thinking { .. } => {
                                continue;
                            }
                            ContentBlock::Text { text } => text_parts.push(text.clone()),
                            ContentBlock::ToolUse { id, name, input } => {
                                tool_calls.push(json!({
                                    "id": id,
                                    "type": "function",
                                    "function": {
                                        "name": name,
                                        "arguments": input.to_string(),
                                    }
                                }));
                            }
                            _ => {}
                        }
                    }

                    let mut msg_obj = json!({"role": "assistant"});
                    // Note: reasoning_content is NOT included from history to prevent repeated thinking
                    if !text_parts.is_empty() {
                        msg_obj["content"] = json!(text_parts.join("\n"));
                    }
                    if !tool_calls.is_empty() {
                        msg_obj["tool_calls"] = json!(tool_calls);
                    }
                    result.push(msg_obj);
                }
                (Role::Tool, MessageContent::Blocks(blocks)) => {
                    self.push_tool_results(blocks, &mut result);
                }
                (Role::User, MessageContent::Blocks(blocks)) => {
                    // Check if this is a tool result message (agent wraps tool results as User role)
                    if blocks
                        .iter()
                        .any(|b| matches!(b, ContentBlock::ToolResult { .. }))
                    {
                        // Emit as OpenAI tool messages
                        self.push_tool_results(blocks, &mut result);
                    } else {
                        // Regular user message with blocks
                        let text: String = blocks
                            .iter()
                            .filter_map(|b| match b {
                                ContentBlock::Text { text } => Some(text.as_str()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        result.push(json!({"role": "user", "content": text}));
                    }
                }
                _ => {}
            }
        }

        result
    }

    /// Push tool result blocks to message array
    fn push_tool_results(&self, blocks: &[ContentBlock], result: &mut Vec<Value>) {
        for block in blocks {
            if let ContentBlock::ToolResult {
                tool_use_id,
                content,
            } = block
            {
                result.push(json!({
                    "role": "tool",
                    "tool_call_id": tool_use_id,
                    "content": content,
                }));
            }
        }
    }

    fn convert_tools(&self, tools: &[ToolDefinition]) -> Vec<Value> {
        tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    }
                })
            })
            .collect()
    }
}

#[async_trait]
impl Provider for OpenAIProvider {
    fn context_size(&self) -> Option<u32> {
        context_window_for(&self.model)
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    fn clone_box(&self) -> Box<dyn Provider> {
        Box::new(Self {
            api_key: self.api_key.clone(),
            model: self.model.clone(),
            base_url: self.base_url.clone(),
            client: reqwest::Client::new(),
            extra_headers: self.extra_headers.clone(),
        })
    }

    fn clone_arc(&self) -> Arc<dyn Provider> {
        Arc::new(Self {
            api_key: self.api_key.clone(),
            model: self.model.clone(),
            base_url: self.base_url.clone(),
            client: reqwest::Client::new(),
            extra_headers: self.extra_headers.clone(),
        })
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        let messages = self.convert_messages(&request.messages, request.system.as_deref());

        let mut body = json!({
            "model": self.model,
            "messages": messages,
            "max_completion_tokens": request.max_tokens,
        });

        if !request.tools.is_empty() {
            body["tools"] = json!(self.convert_tools(&request.tools));
        }

        let url = format!("{}/chat/completions", self.base_url);

        // Debug: log request
        crate::debug::debug_log()
            .api_request(&url, &serde_json::to_string(&body).unwrap_or_default());

        let mut req = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body);

        // Add extra headers from config
        for (name, value) in &self.extra_headers {
            req = req.header(name, value);
        }

        let response = req
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("HTTP request failed: {} (url: {})", e, url))?;

        let status = response.status();
        let response_body: Value = response
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse response JSON: {}", e))?;

        // Debug: log response
        crate::debug::debug_log().api_response(
            status.as_u16(),
            &serde_json::to_string(&response_body).unwrap_or_default(),
        );

        if !status.is_success() {
            let err_msg = response_body["error"]["message"]
                .as_str()
                .unwrap_or_else(|| response_body["error"].as_str().unwrap_or("unknown error"));
            anyhow::bail!("API error ({}): {}", status, err_msg);
        }

        let choice = &response_body["choices"][0];
        let message = &choice["message"];
        let finish_reason = choice["finish_reason"].as_str().unwrap_or("stop");

        let stop_reason = match finish_reason {
            "tool_calls" => StopReason::ToolUse,
            "length" => StopReason::MaxTokens,
            _ => StopReason::EndTurn,
        };

        let mut content = Vec::new();

        let usage_blob = &response_body["usage"];
        let usage = Usage {
            input_tokens: usage_blob["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            output_tokens: usage_blob["completion_tokens"].as_u64().unwrap_or(0) as u32,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: usage_blob["prompt_tokens_details"]["cached_tokens"]
                .as_u64()
                .unwrap_or(0) as u32,
        };

        // DeepSeek thinking mode: reasoning_content must come before content
        if let Some(reasoning) = message["reasoning_content"].as_str()
            && !reasoning.is_empty()
        {
            content.push(ContentBlock::Thinking {
                thinking: reasoning.to_string(),
                signature: None,
            });
        }

        if let Some(text) = message["content"].as_str()
            && !text.is_empty()
        {
            content.push(ContentBlock::Text {
                text: text.to_string(),
            });
        }

        if let Some(tool_calls) = message["tool_calls"].as_array() {
            for tc in tool_calls {
                let id = tc["id"].as_str().unwrap_or_default().to_string();
                let name = tc["function"]["name"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let arguments = tc["function"]["arguments"].as_str().unwrap_or("{}");
                let input: Value = serde_json::from_str(arguments).unwrap_or(json!({}));

                content.push(ContentBlock::ToolUse { id, name, input });
            }

            if stop_reason == StopReason::EndTurn && !tool_calls.is_empty() {
                return Ok(ChatResponse {
                    content,
                    stop_reason: StopReason::ToolUse,
                    usage: usage.clone(),
                });
            }
        }

        Ok(ChatResponse {
            content,
            stop_reason,
            usage,
        })
    }
}
