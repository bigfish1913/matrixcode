use anyhow::Result;
use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::{Value, json};
use tokio::sync::mpsc;

use crate::tools::ToolDefinition;

use super::{
    ChatRequest, ChatResponse, ContentBlock, Message, MessageContent, Provider, Role, StopReason,
    StreamEvent,
};

pub struct AnthropicProvider {
    api_key: String,
    model: String,
    base_url: String,
    client: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(api_key: String, model: String, base_url: String) -> Self {
        Self {
            api_key,
            model,
            base_url,
            client: reqwest::Client::new(),
        }
    }

    fn convert_messages(&self, messages: &[Message]) -> Vec<Value> {
        messages
            .iter()
            .filter(|m| m.role != Role::System)
            .map(|m| {
                let role = match m.role {
                    Role::User | Role::Tool => "user",
                    Role::Assistant => "assistant",
                    Role::System => unreachable!(),
                };

                let content = match &m.content {
                    MessageContent::Text(text) => json!(text),
                    MessageContent::Blocks(blocks) => {
                        let converted: Vec<Value> = blocks
                            .iter()
                            .map(|b| match b {
                                ContentBlock::Text { text } => json!({"type": "text", "text": text}),
                                ContentBlock::ToolUse { id, name, input } => {
                                    json!({"type": "tool_use", "id": id, "name": name, "input": input})
                                }
                                ContentBlock::ToolResult { tool_use_id, content } => {
                                    json!({"type": "tool_result", "tool_use_id": tool_use_id, "content": content})
                                }
                                ContentBlock::Thinking { thinking, signature } => {
                                    let mut obj = json!({"type": "thinking", "thinking": thinking});
                                    if let Some(sig) = signature {
                                        obj["signature"] = json!(sig);
                                    }
                                    obj
                                }
                            })
                            .collect();
                        json!(converted)
                    }
                };

                json!({"role": role, "content": content})
            })
            .collect()
    }

    fn convert_tools(&self, tools: &[ToolDefinition]) -> Vec<Value> {
        tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.parameters,
                })
            })
            .collect()
    }

    /// Build the base JSON body shared by streaming and non-streaming requests.
    fn build_body(&self, request: &ChatRequest) -> Value {
        let mut body = json!({
            "model": self.model,
            "max_tokens": 8192,
            "messages": self.convert_messages(&request.messages),
        });

        if let Some(system) = &request.system {
            body["system"] = json!(system);
        }

        if !request.tools.is_empty() {
            body["tools"] = json!(self.convert_tools(&request.tools));
        }

        if request.think {
            body["thinking"] = thinking_config(&self.model);
        }

        body
    }
}

/// Models that require the new `adaptive` thinking mode instead of the
/// legacy `enabled`+`budget_tokens` form. Conservative allow-list: if we
/// don't recognize the name we default to the legacy shape (which older
/// models and most third-party gateways understand).
fn thinking_config(model: &str) -> Value {
    let adaptive = model.contains("opus-4-7") || model.contains("opus-4.7");
    if adaptive {
        json!({"type": "adaptive"})
    } else {
        json!({"type": "enabled", "budget_tokens": 2048})
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        let body = self.build_body(&request);

        let url = format!("{}/v1/messages", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        let response_body: Value = response.json().await?;

        if !status.is_success() {
            let err_msg = response_body["error"]["message"]
                .as_str()
                .unwrap_or("unknown error");
            anyhow::bail!("Anthropic API error ({}): {}", status, err_msg);
        }

        let stop_reason = match response_body["stop_reason"].as_str() {
            Some("tool_use") => StopReason::ToolUse,
            Some("max_tokens") => StopReason::MaxTokens,
            _ => StopReason::EndTurn,
        };

        let content = response_body["content"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|block| match block["type"].as_str()? {
                "text" => Some(ContentBlock::Text {
                    text: block["text"].as_str()?.to_string(),
                }),
                "tool_use" => Some(ContentBlock::ToolUse {
                    id: block["id"].as_str()?.to_string(),
                    name: block["name"].as_str()?.to_string(),
                    input: block["input"].clone(),
                }),
                "thinking" => Some(ContentBlock::Thinking {
                    thinking: block["thinking"].as_str()?.to_string(),
                    signature: block["signature"].as_str().map(String::from),
                }),
                _ => None,
            })
            .collect();

        Ok(ChatResponse {
            content,
            stop_reason,
        })
    }

    async fn chat_stream(&self, request: ChatRequest) -> Result<mpsc::Receiver<StreamEvent>> {
        let mut body = self.build_body(&request);
        body["stream"] = json!(true);

        let url = format!("{}/v1/messages", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic API error ({}): {}", status, text);
        }

        let (tx, rx) = mpsc::channel(64);
        tokio::spawn(async move {
            let send = |ev: StreamEvent| {
                let tx = tx.clone();
                async move {
                    let _ = tx.send(ev).await;
                }
            };

            let mut stream = response.bytes_stream();
            let mut buffer = String::new();
            let mut sent_first_byte = false;

            // In-flight block assembly: index → partial data
            let mut blocks: Vec<AssembledBlock> = Vec::new();
            let mut stop_reason = StopReason::EndTurn;

            while let Some(chunk) = stream.next().await {
                let chunk = match chunk {
                    Ok(c) => c,
                    Err(e) => {
                        send(StreamEvent::Error(format!("stream read error: {}", e))).await;
                        return;
                    }
                };

                if !sent_first_byte {
                    sent_first_byte = true;
                    send(StreamEvent::FirstByte).await;
                }

                buffer.push_str(&String::from_utf8_lossy(&chunk));

                while let Some(pos) = buffer.find("\n\n") {
                    let event_text = buffer[..pos].to_string();
                    buffer.drain(..pos + 2);

                    let mut data_line = String::new();
                    for line in event_text.lines() {
                        if let Some(rest) = line.strip_prefix("data: ") {
                            data_line = rest.to_string();
                            break;
                        }
                    }
                    if data_line.is_empty() {
                        continue;
                    }

                    let evt: Value = match serde_json::from_str(&data_line) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    match evt["type"].as_str().unwrap_or("") {
                        "content_block_start" => {
                            let idx = evt["index"].as_u64().unwrap_or(0) as usize;
                            let block = &evt["content_block"];
                            let kind = block["type"].as_str().unwrap_or("");
                            while blocks.len() <= idx {
                                blocks.push(AssembledBlock::default());
                            }
                            match kind {
                                "text" => {
                                    blocks[idx] = AssembledBlock::Text(String::new());
                                }
                                "thinking" => {
                                    blocks[idx] = AssembledBlock::Thinking {
                                        text: String::new(),
                                        signature: None,
                                    };
                                }
                                "tool_use" => {
                                    let id = block["id"].as_str().unwrap_or("").to_string();
                                    let name = block["name"].as_str().unwrap_or("").to_string();
                                    blocks[idx] = AssembledBlock::ToolUse {
                                        id: id.clone(),
                                        name: name.clone(),
                                        input_json: String::new(),
                                    };
                                    send(StreamEvent::ToolUseStart { id, name }).await;
                                }
                                _ => {}
                            }
                        }
                        "content_block_delta" => {
                            let idx = evt["index"].as_u64().unwrap_or(0) as usize;
                            let delta = &evt["delta"];
                            let dt = delta["type"].as_str().unwrap_or("");
                            if idx >= blocks.len() {
                                continue;
                            }
                            match (dt, &mut blocks[idx]) {
                                ("text_delta", AssembledBlock::Text(buf)) => {
                                    if let Some(t) = delta["text"].as_str() {
                                        buf.push_str(t);
                                        send(StreamEvent::TextDelta(t.to_string())).await;
                                    }
                                }
                                ("thinking_delta", AssembledBlock::Thinking { text, .. }) => {
                                    if let Some(t) = delta["thinking"].as_str() {
                                        text.push_str(t);
                                        send(StreamEvent::ThinkingDelta(t.to_string())).await;
                                    }
                                }
                                (
                                    "signature_delta",
                                    AssembledBlock::Thinking { signature, .. },
                                ) => {
                                    if let Some(s) = delta["signature"].as_str() {
                                        signature.get_or_insert_with(String::new).push_str(s);
                                    }
                                }
                                (
                                    "input_json_delta",
                                    AssembledBlock::ToolUse { input_json, .. },
                                ) => {
                                    if let Some(p) = delta["partial_json"].as_str() {
                                        input_json.push_str(p);
                                        send(StreamEvent::ToolInputDelta {
                                            bytes_so_far: input_json.len(),
                                        })
                                        .await;
                                    }
                                }
                                _ => {}
                            }
                        }
                        "message_delta" => {
                            if let Some(sr) = evt["delta"]["stop_reason"].as_str() {
                                stop_reason = match sr {
                                    "tool_use" => StopReason::ToolUse,
                                    "max_tokens" => StopReason::MaxTokens,
                                    _ => StopReason::EndTurn,
                                };
                            }
                        }
                        "message_stop" => {
                            let content: Vec<ContentBlock> = blocks
                                .into_iter()
                                .filter_map(|b| b.finish())
                                .collect();
                            send(StreamEvent::Done(ChatResponse {
                                content,
                                stop_reason,
                            }))
                            .await;
                            return;
                        }
                        "error" => {
                            let msg = evt["error"]["message"]
                                .as_str()
                                .unwrap_or("unknown stream error")
                                .to_string();
                            send(StreamEvent::Error(msg)).await;
                            return;
                        }
                        _ => {}
                    }
                }
            }
        });

        Ok(rx)
    }
}

#[derive(Default)]
enum AssembledBlock {
    #[default]
    Empty,
    Text(String),
    Thinking {
        text: String,
        signature: Option<String>,
    },
    ToolUse {
        id: String,
        name: String,
        input_json: String,
    },
}

impl AssembledBlock {
    fn finish(self) -> Option<ContentBlock> {
        match self {
            AssembledBlock::Empty => None,
            AssembledBlock::Text(text) => Some(ContentBlock::Text { text }),
            AssembledBlock::Thinking { text, signature } => Some(ContentBlock::Thinking {
                thinking: text,
                signature,
            }),
            AssembledBlock::ToolUse {
                id,
                name,
                input_json,
            } => {
                let input: Value = if input_json.is_empty() {
                    json!({})
                } else {
                    serde_json::from_str(&input_json).unwrap_or(json!({}))
                };
                Some(ContentBlock::ToolUse { id, name, input })
            }
        }
    }
}
