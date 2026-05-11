use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};

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
}

impl OpenAIProvider {
    pub fn new(api_key: String, model: String, base_url: String) -> Self {
        Self {
            api_key,
            model,
            base_url,
            client: reqwest::Client::new(),
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

                    for block in blocks {
                        match block {
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
                            ContentBlock::Thinking { .. } => {}
                            _ => {}
                        }
                    }

                    let mut msg_obj = json!({"role": "assistant"});
                    if !text_parts.is_empty() {
                        msg_obj["content"] = json!(text_parts.join("\n"));
                    }
                    if !tool_calls.is_empty() {
                        msg_obj["tool_calls"] = json!(tool_calls);
                    }
                    result.push(msg_obj);
                }
                (Role::Tool, MessageContent::Blocks(blocks)) => {
                    for block in blocks {
                        if let ContentBlock::ToolResult { tool_use_id, content } = block {
                            result.push(json!({
                                "role": "tool",
                                "tool_call_id": tool_use_id,
                                "content": content,
                            }));
                        }
                    }
                }
                (Role::User, MessageContent::Blocks(blocks)) => {
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
                _ => {}
            }
        }

        result
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
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        let messages = self.convert_messages(&request.messages, request.system.as_deref());

        let mut body = json!({
            "model": self.model,
            "messages": messages,
        });

        if !request.tools.is_empty() {
            body["tools"] = json!(self.convert_tools(&request.tools));
        }

        let url = format!("{}/chat/completions", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        let response_body: Value = response.json().await?;

        if !status.is_success() {
            let err_msg = response_body["error"]["message"]
                .as_str()
                .unwrap_or("unknown error");
            anyhow::bail!("OpenAI API error ({}): {}", status, err_msg);
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

        if let Some(text) = message["content"].as_str() {
            if !text.is_empty() {
                content.push(ContentBlock::Text { text: text.to_string() });
            }
        }

        if let Some(tool_calls) = message["tool_calls"].as_array() {
            for tc in tool_calls {
                let id = tc["id"].as_str().unwrap_or_default().to_string();
                let name = tc["function"]["name"].as_str().unwrap_or_default().to_string();
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
