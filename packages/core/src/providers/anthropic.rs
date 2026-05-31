use anyhow::Result;
use async_trait::async_trait;
use futures_util::StreamExt;
use log::debug;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::constants::{
    ANTHROPIC_API_VERSION, DEFAULT_CONNECT_TIMEOUT_SECS, DEFAULT_CONTENT_TIMEOUT_SECS,
    DEFAULT_READ_TIMEOUT_SECS, DEFAULT_REQUEST_TIMEOUT_SECS, THINKING_BUDGET_NEW_MODELS,
    THINKING_BUDGET_OLD_MODELS,
};
use crate::models::context_window_for;
use crate::tools::ToolDefinition;

use super::{
    ChatRequest, ChatResponse, ContentBlock, Message, MessageContent, Provider, Role, StopReason,
    StreamEvent, Usage,
};

pub struct AnthropicProvider {
    api_key: String,
    model: String,
    base_url: String,
    client: reqwest::Client,
    /// Extra headers from config
    extra_headers: Vec<(String, String)>,
}

impl AnthropicProvider {
    pub fn new(api_key: String, model: String, base_url: String) -> Self {
        Self::with_headers(api_key, model, base_url, None)
    }

    /// Load proxy from environment variables
    fn load_proxy_from_env() -> Option<String> {
        std::env::var("HTTPS_PROXY")
            .ok()
            .or_else(|| std::env::var("https_proxy").ok())
            .or_else(|| std::env::var("HTTP_PROXY").ok())
            .or_else(|| std::env::var("http_proxy").ok())
    }

    pub fn with_headers(
        api_key: String,
        model: String,
        base_url: String,
        extra_headers: Option<std::collections::HashMap<String, String>>,
    ) -> Self {
        // Create client with better timeout handling for streaming
        // - No total timeout (streaming responses can take a long time)
        // - Connect timeout: DEFAULT_CONNECT_TIMEOUT_SECS seconds
        // - Read timeout per chunk: DEFAULT_READ_TIMEOUT_SECS seconds (for slow responses between chunks)
        let mut client_builder = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS))
            .read_timeout(std::time::Duration::from_secs(DEFAULT_READ_TIMEOUT_SECS))
            .timeout(std::time::Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS)); // Total timeout for non-streaming

        // Add proxy from environment if available
        if let Some(proxy_url) = Self::load_proxy_from_env()
            && let Ok(proxy) = reqwest::Proxy::all(&proxy_url)
        {
            log::info!("AnthropicProvider using proxy: {}", proxy_url);
            client_builder = client_builder.proxy(proxy);
        }

        let client = client_builder
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

    /// Check if this is the official Anthropic API endpoint.
    /// Non-official endpoints typically use Bearer auth (OpenAI-compatible).
    fn is_official_anthropic(&self) -> bool {
        self.base_url.contains("api.anthropic.com")
    }

    fn convert_messages(&self, messages: &[Message]) -> Vec<Value> {
        // For non-official APIs (like glm-5/DashScope), filter out thinking blocks
        // to prevent API from entering extended thinking mode and hanging
        // This is necessary because some APIs don't properly support thinking blocks
        let filter_thinking = !self.is_official_anthropic();
        log::debug!(
            "convert_messages: filter_thinking={}, base_url={}",
            filter_thinking,
            self.base_url
        );

        // Count thinking blocks in original messages
        let mut thinking_count = 0;
        for m in messages {
            if let MessageContent::Blocks(blocks) = &m.content {
                for b in blocks {
                    if matches!(b, ContentBlock::Thinking { .. }) {
                        thinking_count += 1;
                    }
                }
            }
        }
        if thinking_count > 0 {
            log::debug!(
                "convert_messages: Found {} thinking blocks in {} messages, filter_thinking={}",
                thinking_count,
                messages.len(),
                filter_thinking
            );
        }

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
                            .filter(|b| {
                                // Skip thinking blocks for non-official APIs
                                if filter_thinking && matches!(b, ContentBlock::Thinking { .. }) {
                                    return false;
                                }
                                true
                            })
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
                                    // Only send non-empty signature
                                    if let Some(sig) = signature
                                        && !sig.is_empty() {
                                            obj["signature"] = json!(sig);
                                        }
                                    obj
                                }
                                ContentBlock::ServerToolUse { id, name, input } => {
                                    json!({"type": "server_tool_use", "id": id, "name": name, "input": input})
                                }
                                ContentBlock::ServerToolResult { tool_use_id, content } => {
                                    json!({"type": "server_tool_result", "tool_use_id": tool_use_id, "content": content})
                                }
                                ContentBlock::WebSearchResult { tool_use_id, content } => {
                                    json!({"type": "web_search_tool_result", "tool_use_id": tool_use_id, "content": content})
                                }
                            })
                            .collect();

                        if converted.is_empty() {
                            json!([])
                        } else {
                            json!(converted)
                        }
                    }
                };

                // Skip messages with empty content
                if content.is_array() && content.as_array().map(|a| a.is_empty()).unwrap_or(false) {
                    return json!(null);
                }

                json!({"role": role, "content": content})
            })
            .filter(|v| !v.is_null())
            .collect()
    }

    /// Convert tools with caching control for Anthropic prompt caching.
    fn convert_tools_with_caching(
        &self,
        tools: &[ToolDefinition],
        enable_caching: bool,
    ) -> Vec<Value> {
        let mut converted: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.parameters,
                })
            })
            .collect();

        // Add cache_control to the last tool definition for tools caching
        if enable_caching && !converted.is_empty() {
            let last_idx = converted.len() - 1;
            if let Some(obj) = converted[last_idx].as_object_mut() {
                obj.insert("cache_control".to_string(), json!({"type": "ephemeral"}));
            }
        }

        converted
    }

    /// Build the base JSON body shared by streaming and non-streaming requests.
    fn build_body(&self, request: &ChatRequest) -> Value {
        let mut body = json!({
            "model": self.model,
            "max_tokens": request.max_tokens,
            "messages": self.convert_messages(&request.messages),
        });

        // Add prompt caching for system prompt (Anthropic-specific)
        if request.enable_caching {
            if let Some(system) = &request.system {
                // System prompt caching: add cache_control to enable caching
                body["system"] = json!([
                    {
                        "type": "text",
                        "text": system,
                        "cache_control": {"type": "ephemeral"}
                    }
                ]);
            }
        } else if let Some(system) = &request.system {
            body["system"] = json!(system);
        }

        if !request.tools.is_empty() {
            let tools = self.convert_tools_with_caching(&request.tools, request.enable_caching);
            body["tools"] = json!(tools);
        }

        if !request.server_tools.is_empty() {
            body["tools"] = json!(
                body["tools"]
                    .as_array()
                    .map(|t| {
                        let mut tools = t.clone();
                        for st in &request.server_tools {
                            tools.push(serde_json::to_value(st).unwrap_or_default());
                        }
                        tools
                    })
                    .unwrap_or_else(|| request
                        .server_tools
                        .iter()
                        .map(|st| serde_json::to_value(st).unwrap_or_default())
                        .collect())
            );
        }

        // Extended thinking (Anthropic-specific)
        // Only enable thinking for official Anthropic API - third-party APIs like DashScope
        // forcibly enable thinking mode which causes hanging issues
        if request.think && self.is_official_anthropic() {
            let config = thinking_config(&self.model);
            log::debug!(
                "Adding thinking config for model {}: {:?}",
                self.model,
                config
            );
            body["thinking"] = config;
        } else if request.think {
            log::debug!(
                "Skipping thinking config for non-official API (model: {}, base_url: {})",
                self.model,
                self.base_url
            );
        }

        body
    }
}

/// Models that require the new `adaptive` thinking mode instead of the
/// legacy `enabled`+`budget_tokens` form. Conservative allow-list: if we
/// don't recognize the name we default to the legacy shape (which older
/// models and most third-party gateways understand).
fn thinking_config(model: &str) -> Value {
    let m = model.to_lowercase();
    // New models (2025+) use adaptive thinking
    let adaptive = m.contains("opus-4")
        || m.contains("sonnet-4")
        || m.contains("claude-4")
        || m.contains("20250")
        || m.contains("2025");
    if adaptive {
        json!({"type": "enabled", "budget_tokens": THINKING_BUDGET_NEW_MODELS})
    } else {
        // to prevent hanging on long histories
        json!({"type": "enabled", "budget_tokens": THINKING_BUDGET_OLD_MODELS})
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
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
        let body = self.build_body(&request);

        let url = format!("{}/v1/messages", self.base_url);

        // Debug: log request
        crate::debug::debug_log()
            .api_request(&url, &serde_json::to_string(&body).unwrap_or_default());

        let mut req = self
            .client
            .post(&url)
            .header("User-Agent", "curl/8.0")
            .json(&body);

        // Auth: official Anthropic API uses x-api-key, others use Bearer (OpenAI-compatible)
        if self.is_official_anthropic() {
            req = req
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", ANTHROPIC_API_VERSION)
                .header("anthropic-beta", "prompt-caching-2024-07-31");
        } else {
            req = req
                .header("Authorization", format!("Bearer {}", self.api_key))
                // Add anthropic-version for third-party APIs to ensure correct protocol behavior
                .header("anthropic-version", "2023-06-01")
                // Try enabling prompt caching for third-party APIs (DashScope may support this)
                .header("anthropic-beta", "prompt-caching-2024-07-31");
        }

        // Add extra headers from config (all custom headers go here)
        for (name, value) in &self.extra_headers {
            req = req.header(name, value);
        }

        let response = req.send().await?;

        let status = response.status();
        let response_body: Value = response.json().await?;

        // Debug: log response
        crate::debug::debug_log().api_response(
            status.as_u16(),
            &serde_json::to_string(&response_body).unwrap_or_default(),
        );

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
                "server_tool_use" => Some(ContentBlock::ServerToolUse {
                    id: block["id"].as_str()?.to_string(),
                    name: block["name"].as_str()?.to_string(),
                    input: block["input"].clone(),
                }),
                "web_search_tool_result" => {
                    let tool_use_id = block["tool_use_id"].as_str()?.to_string();
                    let content = parse_web_search_content(&block["content"]);
                    Some(ContentBlock::WebSearchResult {
                        tool_use_id,
                        content,
                    })
                }
                _ => None,
            })
            .collect();

        Ok(ChatResponse {
            content,
            stop_reason,
            usage: parse_usage(&response_body["usage"]),
        })
    }

    async fn chat_stream(&self, request: ChatRequest) -> Result<mpsc::Receiver<StreamEvent>> {
        let mut body = self.build_body(&request);
        body["stream"] = json!(true);

        let url = format!("{}/v1/messages", self.base_url);

        // Debug: log streaming request
        crate::debug::debug_log()
            .api_request(&url, &serde_json::to_string(&body).unwrap_or_default());

        let mut req = self
            .client
            .post(&url)
            .header("User-Agent", "curl/8.0")
            .json(&body);

        // Auth: official Anthropic API uses x-api-key, others use Bearer (OpenAI-compatible)
        if self.is_official_anthropic() {
            req = req
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", ANTHROPIC_API_VERSION)
                .header("anthropic-beta", "prompt-caching-2024-07-31");
        } else {
            req = req
                .header("Authorization", format!("Bearer {}", self.api_key))
                // Add anthropic-version for third-party APIs to ensure correct protocol behavior
                .header("anthropic-version", "2023-06-01")
                // Try enabling prompt caching for third-party APIs (DashScope may support this)
                .header("anthropic-beta", "prompt-caching-2024-07-31");
        }

        // Add extra headers from config (all custom headers go here)
        for (name, value) in &self.extra_headers {
            req = req.header(name, value);
        }

        let response = req.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic API error ({}): {}", status, text);
        }

        let (tx, rx) = mpsc::channel(64);
        tokio::spawn(async move {
            let mut stream = response.bytes_stream();
            let mut buffer = String::new();
            let mut sent_first_byte = false;

            // In-flight block assembly: index → partial data
            let mut blocks: Vec<AssembledBlock> = Vec::new();
            let mut stop_reason = StopReason::EndTurn;
            let mut usage = Usage::default();

            // Timeout detection: track last meaningful event (non-ping)
            let mut last_content_time = std::time::Instant::now();
            const CONTENT_TIMEOUT_SECS: u64 = DEFAULT_CONTENT_TIMEOUT_SECS; // 5 minutes without content = timeout (for slow APIs like DashScope/glm-5)

            while let Some(chunk) = stream.next().await {
                let chunk = match chunk {
                    Ok(c) => c,
                    Err(e) => {
                        // Log detailed error for debugging
                        let error_msg = e.to_string();
                        let is_timeout =
                            error_msg.contains("timeout") || error_msg.contains("timed out");
                        let is_decode =
                            error_msg.contains("decode") || error_msg.contains("decoding");

                        let msg = if is_timeout {
                            format!(
                                "Stream timeout - the API took too long to respond: {}",
                                error_msg
                            )
                        } else if is_decode {
                            format!(
                                "Stream decode error - possibly interrupted or corrupted response: {}",
                                error_msg
                            )
                        } else {
                            format!("Stream read error: {}", error_msg)
                        };

                        // Try to finalize any partial content we have
                        if sent_first_byte && !blocks.is_empty() {
                            debug!("finalizing partial stream due to error");
                            let _ = tx
                                .send(StreamEvent::Done(finalize_incomplete_stream(
                                    std::mem::take(&mut blocks),
                                    stop_reason,
                                    usage,
                                )))
                                .await;
                        } else {
                            let _ = tx.send(StreamEvent::Error(msg)).await;
                        }
                        return;
                    }
                };

                if !sent_first_byte {
                    sent_first_byte = true;
                    let _ = tx.send(StreamEvent::FirstByte).await;
                }

                buffer.push_str(&String::from_utf8_lossy(&chunk));

                // Check for timeout: if only ping events for too long, force finalize
                let elapsed = last_content_time.elapsed().as_secs();
                if elapsed > CONTENT_TIMEOUT_SECS && !blocks.is_empty() {
                    crate::debug::debug_log().stream_chunk(
                        "TIMEOUT_FORCE_FINALIZE",
                        &format!("elapsed={}s, blocks={}", elapsed, blocks.len()),
                    );
                    let _ = tx
                        .send(StreamEvent::Done(finalize_incomplete_stream(
                            std::mem::take(&mut blocks),
                            stop_reason,
                            usage,
                        )))
                        .await;
                    return;
                }

                while let Some(frame) = take_next_sse_frame(&mut buffer) {
                    if handle_sse_frame(
                        &frame,
                        &mut blocks,
                        &mut stop_reason,
                        &mut usage,
                        &tx,
                        &mut last_content_time,
                    )
                    .await
                    {
                        return;
                    }
                }
            }

            if let Some(frame) = take_trailing_sse_frame(&mut buffer)
                && handle_sse_frame(
                    &frame,
                    &mut blocks,
                    &mut stop_reason,
                    &mut usage,
                    &tx,
                    &mut last_content_time,
                )
                .await
            {
                return;
            }

            if sent_first_byte {
                debug!("stream ended without explicit message_stop; finalizing best-effort");
                let _ = tx
                    .send(StreamEvent::Done(finalize_incomplete_stream(
                        std::mem::take(&mut blocks),
                        stop_reason,
                        usage,
                    )))
                    .await;
            } else {
                let _ = tx
                    .send(StreamEvent::Error(
                        "stream ended before any events were received".to_string(),
                    ))
                    .await;
            }
        });

        Ok(rx)
    }
}

fn take_next_sse_frame(buffer: &mut String) -> Option<String> {
    let lf = buffer.find("\n\n").map(|pos| (pos, 2usize));
    let crlf = buffer.find("\r\n\r\n").map(|pos| (pos, 4usize));
    let (pos, delim_len) = match (lf, crlf) {
        (Some(a), Some(b)) => {
            if a.0 <= b.0 {
                a
            } else {
                b
            }
        }
        (Some(a), None) => a,
        (None, Some(b)) => b,
        (None, None) => return None,
    };

    let frame = buffer[..pos].to_string();
    buffer.drain(..pos + delim_len);
    Some(frame)
}

fn take_trailing_sse_frame(buffer: &mut String) -> Option<String> {
    let frame = buffer.trim().trim_end_matches('\r').to_string();
    buffer.clear();
    if frame.is_empty() { None } else { Some(frame) }
}

fn extract_sse_data_line(frame: &str) -> Option<String> {
    for line in frame.lines() {
        let line = line.trim_end_matches('\r');
        // Support both "data: " (Anthropic) and "data:" (DashScope)
        if let Some(rest) = line.strip_prefix("data: ") {
            return Some(rest.to_string());
        }
        if let Some(rest) = line.strip_prefix("data:") {
            return Some(rest.to_string());
        }
    }
    None
}

async fn handle_sse_frame(
    frame: &str,
    blocks: &mut Vec<AssembledBlock>,
    stop_reason: &mut StopReason,
    usage: &mut Usage,
    tx: &mpsc::Sender<StreamEvent>,
    last_content_time: &mut std::time::Instant,
) -> bool {
    let Some(data_line) = extract_sse_data_line(frame) else {
        return false;
    };

    let evt: Value = match serde_json::from_str(&data_line) {
        Ok(v) => v,
        Err(_) => return false,
    };

    handle_sse_event(evt, blocks, stop_reason, usage, tx, last_content_time).await
}

async fn handle_sse_event(
    evt: Value,
    blocks: &mut Vec<AssembledBlock>,
    stop_reason: &mut StopReason,
    usage: &mut Usage,
    tx: &mpsc::Sender<StreamEvent>,
    last_content_time: &mut std::time::Instant,
) -> bool {
    let evt_type = evt["type"].as_str().unwrap_or("");

    // Debug: log all SSE events for diagnosis (with full content for debugging)
    let evt_json = serde_json::to_string(&evt).unwrap_or_default();
    crate::debug::debug_log().stream_chunk(evt_type, &evt_json);

    // Log event handling for thinking_delta specifically
    if evt_type == "content_block_delta" {
        let delta_type = evt["delta"]["type"].as_str().unwrap_or("");
        let idx = evt["index"].as_u64().unwrap_or(0) as usize;
        log::debug!(
            "content_block_delta: type={}, idx={}, blocks_len={}, has_block={}",
            delta_type,
            idx,
            blocks.len(),
            idx < blocks.len()
        );
    }

    // Update last_content_time for non-ping events
    if evt_type != "ping" {
        *last_content_time = std::time::Instant::now();
    }

    match evt_type {
        "message_start" => {
            // Initial usage payload — `input_tokens` is final
            // (they don't grow during streaming) but
            // `output_tokens` starts near 0 and is updated by
            // subsequent `message_delta` events.
            *usage = merge_usage(usage.clone(), &evt["message"]["usage"]);
            debug!(
                "message_start: usage_json={}",
                serde_json::to_string(&evt["message"]["usage"]).unwrap_or_default()
            );
            debug!(
                "message_start parsed: input={}, output={}, cache_read={}, cache_created={}",
                usage.input_tokens,
                usage.output_tokens,
                usage.cache_read_input_tokens,
                usage.cache_creation_input_tokens
            );
            // Send real-time usage update
            let _ = tx
                .send(StreamEvent::Usage {
                    output_tokens: usage.output_tokens,
                })
                .await;
        }
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
                "tool_use" | "server_tool_use" => {
                    let id = block["id"].as_str().unwrap_or_default();
                    let name = block["name"].as_str().unwrap_or_default();
                    let is_server = kind == "server_tool_use";
                    blocks[idx] = if is_server {
                        AssembledBlock::ServerToolUse {
                            id: id.into(),
                            name: name.into(),
                            input_json: String::new(),
                        }
                    } else {
                        AssembledBlock::ToolUse {
                            id: id.into(),
                            name: name.into(),
                            input_json: String::new(),
                        }
                    };
                    let _ = tx
                        .send(StreamEvent::ToolUseStart {
                            id: id.into(),
                            name: name.into(),
                        })
                        .await;
                }
                "web_search_tool_result" => {
                    let tool_use_id = block["tool_use_id"].as_str().unwrap_or("").to_string();
                    blocks[idx] = AssembledBlock::WebSearchResult {
                        tool_use_id,
                        content_json: String::new(),
                    };
                }
                _ => {}
            }
        }
        "content_block_delta" => {
            let idx = evt["index"].as_u64().unwrap_or(0) as usize;
            let delta = &evt["delta"];
            let dt = delta["type"].as_str().unwrap_or("");
            if idx >= blocks.len() {
                return false;
            }
            match (dt, &mut blocks[idx]) {
                ("text_delta", AssembledBlock::Text(buf)) => {
                    if let Some(t) = delta["text"].as_str() {
                        buf.push_str(t);
                        let _ = tx.send(StreamEvent::TextDelta(t.to_string())).await;
                    }
                }
                ("thinking_delta", AssembledBlock::Thinking { text, .. }) => {
                    if let Some(t) = delta["thinking"].as_str() {
                        text.push_str(t);
                        log::debug!("Received thinking_delta: {} chars", t.len());
                        let _ = tx.send(StreamEvent::ThinkingDelta(t.to_string())).await;
                    }
                }
                ("signature_delta", AssembledBlock::Thinking { signature, .. }) => {
                    if let Some(s) = delta["signature"].as_str() {
                        signature.get_or_insert_with(String::new).push_str(s);
                    }
                }
                ("input_json_delta", AssembledBlock::ToolUse { input_json, .. }) => {
                    if let Some(p) = delta["partial_json"].as_str() {
                        input_json.push_str(p);
                        let _ = tx
                            .send(StreamEvent::ToolInputDelta {
                                bytes_so_far: input_json.len(),
                            })
                            .await;
                    }
                }
                ("input_json_delta", AssembledBlock::ServerToolUse { input_json, .. }) => {
                    if let Some(p) = delta["partial_json"].as_str() {
                        input_json.push_str(p);
                        let _ = tx
                            .send(StreamEvent::ToolInputDelta {
                                bytes_so_far: input_json.len(),
                            })
                            .await;
                    }
                }
                _ => {}
            }
        }
        "content_block_stop" => {
            let idx = evt["index"].as_u64().unwrap_or(0) as usize;
            if let Some(AssembledBlock::ToolUse {
                id,
                name,
                input_json,
            }) = blocks.get(idx)
            {
                let input: Value = if input_json.is_empty() {
                    json!({})
                } else {
                    serde_json::from_str(input_json).unwrap_or(json!({}))
                };
                let _ = tx
                    .send(StreamEvent::ToolInputComplete {
                        id: id.clone(),
                        name: name.clone(),
                        input,
                    })
                    .await;
            }
        }
        "message_delta" => {
            if let Some(sr) = evt["delta"]["stop_reason"].as_str() {
                *stop_reason = match sr {
                    "tool_use" => StopReason::ToolUse,
                    "max_tokens" => StopReason::MaxTokens,
                    _ => StopReason::EndTurn,
                };
            }
            // `usage` on message_delta reflects cumulative
            // counts for the current message — most notably
            // the final `output_tokens`.
            *usage = merge_usage(usage.clone(), &evt["usage"]);
            debug!(
                "message_delta: input={}, output={}, cache_read={}, cache_created={}",
                usage.input_tokens,
                usage.output_tokens,
                usage.cache_read_input_tokens,
                usage.cache_creation_input_tokens
            );
            // Send real-time usage update
            let _ = tx
                .send(StreamEvent::Usage {
                    output_tokens: usage.output_tokens,
                })
                .await;
        }
        "message_stop" => {
            debug!(
                "Message completed: stop_reason={}, usage={}",
                match *stop_reason {
                    StopReason::EndTurn => "end_turn",
                    StopReason::ToolUse => "tool_use",
                    StopReason::MaxTokens => "max_tokens",
                },
                usage.output_tokens
            );
            debug!(
                "message_stop final usage: cache_read={}, cache_created={}",
                usage.cache_read_input_tokens, usage.cache_creation_input_tokens
            );
            let _ = tx
                .send(StreamEvent::Done(finalize_incomplete_stream(
                    std::mem::take(blocks),
                    stop_reason.clone(),
                    usage.clone(),
                )))
                .await;
            return true;
        }
        "error" => {
            let msg = evt["error"]["message"]
                .as_str()
                .unwrap_or("unknown stream error")
                .to_string();
            let _ = tx.send(StreamEvent::Error(msg)).await;
            return true;
        }
        _ => {}
    }

    false
}

fn build_chat_response(
    blocks: Vec<AssembledBlock>,
    stop_reason: StopReason,
    usage: Usage,
) -> ChatResponse {
    let content: Vec<ContentBlock> = blocks.into_iter().filter_map(|b| b.finish()).collect();
    ChatResponse {
        content,
        stop_reason,
        usage,
    }
}

fn finalize_incomplete_stream(
    blocks: Vec<AssembledBlock>,
    stop_reason: StopReason,
    usage: Usage,
) -> ChatResponse {
    build_chat_response(blocks, stop_reason, usage)
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
    ServerToolUse {
        id: String,
        name: String,
        input_json: String,
    },
    WebSearchResult {
        tool_use_id: String,
        content_json: String,
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
            AssembledBlock::ServerToolUse {
                id,
                name,
                input_json,
            } => {
                let input: Value = if input_json.is_empty() {
                    json!({})
                } else {
                    serde_json::from_str(&input_json).unwrap_or(json!({}))
                };
                Some(ContentBlock::ServerToolUse { id, name, input })
            }
            AssembledBlock::WebSearchResult {
                tool_use_id,
                content_json,
            } => {
                let content: Value = if content_json.is_empty() {
                    json!({"results": []})
                } else {
                    serde_json::from_str(&content_json).unwrap_or(json!({"results": []}))
                };
                Some(ContentBlock::WebSearchResult {
                    tool_use_id,
                    content: parse_web_search_content(&content),
                })
            }
        }
    }
}

/// Parse the provider's `usage` blob (non-streaming response) into our
/// internal `Usage` struct. Missing fields default to 0.
fn parse_usage(u: &Value) -> Usage {
    Usage {
        input_tokens: u["input_tokens"].as_u64().unwrap_or(0) as u32,
        output_tokens: u["output_tokens"].as_u64().unwrap_or(0) as u32,
        cache_creation_input_tokens: u["cache_creation_input_tokens"].as_u64().unwrap_or(0) as u32,
        cache_read_input_tokens: u["cache_read_input_tokens"].as_u64().unwrap_or(0) as u32,
    }
}

/// Merge a fresh usage payload into the accumulated one. Non-zero new values
/// override prior ones — this matches the streaming protocol where
/// `message_start` gives input counts and `message_delta` updates output.
fn merge_usage(mut acc: Usage, u: &Value) -> Usage {
    let fresh = parse_usage(u);
    if fresh.input_tokens > 0 {
        acc.input_tokens = fresh.input_tokens;
    }
    if fresh.output_tokens > 0 {
        acc.output_tokens = fresh.output_tokens;
    }
    if fresh.cache_creation_input_tokens > 0 {
        acc.cache_creation_input_tokens = fresh.cache_creation_input_tokens;
    }
    if fresh.cache_read_input_tokens > 0 {
        acc.cache_read_input_tokens = fresh.cache_read_input_tokens;
    }
    acc
}

/// Parse web search content from the API response.
fn parse_web_search_content(value: &serde_json::Value) -> crate::providers::WebSearchContent {
    let results = value["results"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    Some(crate::providers::WebSearchResultItem {
                        title: item["title"].as_str().map(String::from),
                        url: item["url"].as_str()?.to_string(),
                        encrypted_content: item["encrypted_content"].as_str().map(String::from),
                        snippet: item["snippet"].as_str().map(String::from),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    crate::providers::WebSearchContent { results }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn take_next_sse_frame_supports_crlf_delimiter() {
        let mut buffer = concat!(
            "event: message_start\r\n",
            "data: {\"type\":\"message_start\"}\r\n\r\n",
            "data: {\"type\":\"message_stop\"}\r\n\r\n"
        )
        .to_string();

        let first = take_next_sse_frame(&mut buffer).expect("first frame");
        assert!(first.contains("message_start"));

        let second = take_next_sse_frame(&mut buffer).expect("second frame");
        assert!(second.contains("message_stop"));
        assert!(buffer.is_empty());
    }

    #[test]
    fn take_trailing_sse_frame_returns_unterminated_event() {
        let mut buffer = "data: {\"type\":\"message_stop\"}\r\n".to_string();
        let frame = take_trailing_sse_frame(&mut buffer).expect("trailing frame");
        assert_eq!(frame, "data: {\"type\":\"message_stop\"}");
        assert!(buffer.is_empty());
    }

    #[test]
    fn extract_sse_data_line_supports_optional_space() {
        assert_eq!(
            extract_sse_data_line("event: x\r\ndata: {\"k\":1}\r"),
            Some("{\"k\":1}".to_string())
        );
        assert_eq!(
            extract_sse_data_line("event: x\r\ndata:{\"k\":2}\r"),
            Some("{\"k\":2}".to_string())
        );
    }

    #[test]
    fn finalize_incomplete_stream_keeps_accumulated_content() {
        let response = finalize_incomplete_stream(
            vec![AssembledBlock::Text("partial reply".to_string())],
            StopReason::EndTurn,
            Usage::default(),
        );

        assert_eq!(response.stop_reason, StopReason::EndTurn);
        assert_eq!(response.content.len(), 1);
        match &response.content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "partial reply"),
            other => panic!("unexpected block: {other:?}"),
        }
    }
}
