//! Compression functions and AI compressor implementation.

use crate::providers::{
    ChatRequest, ChatResponse, ContentBlock, Message, MessageContent, Provider, Role,
};
use crate::truncate::truncate_with_suffix;
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashSet;

use super::config::{CompressionBias, CompressionConfig};
use super::types::{CompressionStrategy, SummarizedSegment};

// ============================================================================
// Compressor Trait
// ============================================================================

/// Compressor trait for different implementations.
#[async_trait]
pub trait Compressor: Send + Sync {
    /// Compress messages using AI summarization.
    async fn summarize(
        &self,
        messages: &[Message],
        config: &CompressionConfig,
    ) -> Result<SummarizedSegment>;

    /// Get the model name used.
    fn model_name(&self) -> &str;
}

/// AI-based compressor using a Provider.
pub struct AiCompressor {
    provider: Box<dyn Provider>,
    model: String,
}

impl AiCompressor {
    pub fn new(provider: Box<dyn Provider>, model: String) -> Self {
        Self { provider, model }
    }
}

const SUMMARY_SYSTEM_PROMPT: &str = r#"你是一个对话历史压缩助手。将对话压缩为简洁摘要。

输出要求：
- 简洁：摘要控制在 200 字以内
- 关键：只保留重要操作和决策
- 敏感：必须保留用户的敏感指令
- 任务：必须保留未完成的待办事项
- 决策：必须保留关键方案选择和理由

输出格式：
【摘要】一句话概括主要工作
【已完成】列出已完成的操作
【未完成】列出待办任务（如有）
【关键决策】重要选择及理由（如有）

请直接输出内容。"#;

#[async_trait]
impl Compressor for AiCompressor {
    async fn summarize(
        &self,
        messages: &[Message],
        _config: &CompressionConfig,
    ) -> Result<SummarizedSegment> {
        let prompt = build_summary_prompt(messages);

        let request = ChatRequest {
            messages: vec![Message {
                role: Role::User,
                content: MessageContent::Text(prompt),
            }],
            tools: vec![],
            system: Some(SUMMARY_SYSTEM_PROMPT.to_string()),
            think: false,
            max_tokens: 1024,
            server_tools: vec![],
            enable_caching: false,
        };

        let response = self.provider.chat(request).await?;
        let summary_text = extract_text_from_response(&response);
        let (summary, key_points) = parse_summary_response(&summary_text);

        Ok(SummarizedSegment {
            time_range: (chrono::Utc::now(), chrono::Utc::now()),
            original_count: messages.len(),
            summary,
            key_points,
        })
    }

    fn model_name(&self) -> &str {
        &self.model
    }
}

fn extract_text_from_response(response: &ChatResponse) -> String {
    response
        .content
        .iter()
        .filter_map(|block| {
            if let ContentBlock::Text { text } = block {
                Some(text.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_summary_response(text: &str) -> (String, Vec<String>) {
    let mut summary = String::new();
    let mut key_points: Vec<String> = Vec::new();

    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("•") || line.starts_with("-") || line.starts_with("*") {
            let point = line.trim_start_matches(['•', '-', '*']).trim();
            if !point.is_empty() {
                key_points.push(point.to_string());
            }
        } else if !line.is_empty() && summary.is_empty() {
            summary = line.to_string();
        }
    }

    if summary.is_empty() && !text.is_empty() {
        summary = text.lines().take(3).collect::<Vec<_>>().join(" ");
        if summary.len() > 200 {
            summary = truncate_with_suffix(&summary, 200);
        }
    }

    (summary, key_points)
}

// ============================================================================
// Compression Functions
// ============================================================================

/// Compress messages synchronously.
pub fn compress_messages(
    messages: &[Message],
    strategy: CompressionStrategy,
    config: &CompressionConfig,
) -> Result<Vec<Message>> {
    match strategy {
        CompressionStrategy::Truncate => truncate_compress(messages, config),
        CompressionStrategy::SlidingWindow => sliding_window_compress(messages, config),
        CompressionStrategy::Summarize => sliding_window_compress(messages, config),
        CompressionStrategy::BiasBased => compress_with_bias(messages, config),
    }
}

/// Compress with bias-based scoring.
pub fn compress_with_bias(
    messages: &[Message],
    config: &CompressionConfig,
) -> Result<Vec<Message>> {
    if messages.len() <= config.min_preserve_messages {
        return Ok(messages.to_vec());
    }

    let scored: Vec<(usize, Message, f64)> = messages
        .iter()
        .enumerate()
        .map(|(idx, msg)| {
            (
                idx,
                msg.clone(),
                calculate_preservation_score(msg, idx, messages.len(), &config.bias),
            )
        })
        .collect();

    let mut scored_with_recency: Vec<(usize, Message, f64)> = scored
        .into_iter()
        .map(|(idx, msg, score)| {
            let recency_bonus = if idx >= messages.len() - config.min_preserve_messages {
                100.0
            } else {
                (idx as f64 / messages.len() as f64) * 20.0
            };
            (idx, msg, score + recency_bonus)
        })
        .collect();

    scored_with_recency.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    let target_count = if config.bias.aggressive {
        config.min_preserve_messages
    } else {
        let estimated = estimate_total_tokens(messages);
        let target_tokens = (estimated as f64 * config.target_ratio) as u32;
        let avg = estimated / messages.len() as u32;
        (target_tokens / avg.max(1)) as usize
    };

    let to_keep: HashSet<usize> = scored_with_recency
        .iter()
        .take(target_count)
        .map(|(idx, _, _)| *idx)
        .collect();

    let compressed: Vec<Message> = messages
        .iter()
        .enumerate()
        .filter(|(idx, _)| to_keep.contains(idx))
        .map(|(_, msg)| msg.clone())
        .collect();

    Ok(compressed)
}

fn calculate_preservation_score(
    message: &Message,
    index: usize,
    total: usize,
    bias: &CompressionBias,
) -> f64 {
    let mut score: f64 = 10.0;

    // First message (user's original request) gets highest priority
    if index == 0 {
        score += 100.0;
    }

    match message.role {
        Role::User => {
            if bias.preserve_user_questions {
                score += 30.0;
            }
        }
        Role::Assistant => {
            score += 5.0;
        }
        Role::Tool => {
            if bias.preserve_tools {
                score += 25.0;
            }
        }
        Role::System => {
            score += 40.0;
        }
    }

    match &message.content {
        MessageContent::Text(text) => {
            for keyword in &bias.preserve_keywords {
                if text.to_lowercase().contains(&keyword.to_lowercase()) {
                    score += 15.0;
                }
            }
            if contains_sensitive_instructions(text) {
                score += 50.0;
            }
        }
        MessageContent::Blocks(blocks) => {
            for block in blocks {
                match block {
                    ContentBlock::ToolUse { name, .. } => {
                        if bias.preserve_tools {
                            score += 20.0;
                        }
                        if name == "write" || name == "edit" || name == "bash" {
                            score += 10.0;
                        }
                        // todo_write gets high priority - preserve task tracking
                        if name == "todo_write" {
                            score += 60.0;
                        }
                        // ask tool contains key decisions
                        if name == "ask" {
                            score += 50.0;
                        }
                    }
                    ContentBlock::ToolResult { content, .. } => {
                        if bias.preserve_tools {
                            score += 20.0;
                        }
                        if contains_sensitive_instructions(content) {
                            score += 30.0;
                        }
                        // Preserve todo_write results (task status)
                        if content.contains("TodoWrite") || content.contains("todo") {
                            score += 40.0;
                        }
                        // Preserve ask responses (user decisions)
                        if content.contains("AskUserQuestion") || content.contains("answer") {
                            score += 30.0;
                        }
                    }
                    ContentBlock::Thinking { .. } => {
                        if bias.preserve_thinking {
                            score += 25.0;
                        } else {
                            score -= 5.0;
                        }
                    }
                    ContentBlock::Text { text } => {
                        if contains_sensitive_instructions(text) {
                            score += 50.0;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    score
}

fn contains_sensitive_instructions(text: &str) -> bool {
    let lower = text.to_lowercase();
    let patterns = [
        "不要",
        "禁止",
        "必须",
        "不允许",
        "never",
        "must not",
        "do not",
    ];
    patterns.iter().any(|p| lower.contains(p))
}

fn truncate_compress(messages: &[Message], config: &CompressionConfig) -> Result<Vec<Message>> {
    if messages.len() <= config.min_preserve_messages {
        return Ok(messages.to_vec());
    }
    Ok(messages[messages.len() - config.min_preserve_messages..].to_vec())
}

fn sliding_window_compress(
    messages: &[Message],
    config: &CompressionConfig,
) -> Result<Vec<Message>> {
    if messages.len() <= config.min_preserve_messages {
        return Ok(messages.to_vec());
    }

    let target_tokens = (estimate_total_tokens(messages) as f64 * config.target_ratio) as u32;

    for start_idx in config.min_preserve_messages..messages.len() {
        let candidate = &messages[start_idx..];
        if estimate_total_tokens(candidate) <= target_tokens {
            return Ok(candidate.to_vec());
        }
    }

    Ok(messages[messages.len() - config.min_preserve_messages..].to_vec())
}

// ============================================================================
// Token Estimation
// ============================================================================

/// Estimate token count for a message.
pub fn estimate_tokens(message: &Message) -> u32 {
    let (ascii, non_ascii) = match &message.content {
        MessageContent::Text(t) => count_chars(t),
        MessageContent::Blocks(blocks) => {
            let mut a = 0u32;
            let mut n = 0u32;
            for block in blocks {
                match block {
                    ContentBlock::Text { text } => {
                        let (ca, cn) = count_chars(text);
                        a += ca;
                        n += cn;
                    }
                    ContentBlock::ToolUse { name, input, .. } => {
                        let (ca, cn) = count_chars(name);
                        a += ca;
                        n += cn;
                        let (ja, jn) = count_chars(&input.to_string());
                        a += ja;
                        n += jn;
                    }
                    ContentBlock::ToolResult { content, .. } => {
                        let (ca, cn) = count_chars(content);
                        a += ca;
                        n += cn;
                    }
                    ContentBlock::Thinking { thinking, .. } => {
                        let (ca, cn) = count_chars(thinking);
                        a += ca;
                        n += cn;
                    }
                    _ => {}
                }
            }
            (a, n)
        }
    };

    let ascii_tokens = (ascii as f64 * 0.25).ceil() as u32;
    let non_ascii_tokens = (non_ascii as f64 * 0.67).ceil() as u32;
    (ascii_tokens + non_ascii_tokens + 10).max(1)
}

fn count_chars(s: &str) -> (u32, u32) {
    let mut ascii = 0u32;
    let mut non_ascii = 0u32;
    for ch in s.chars() {
        if ch.is_ascii() {
            ascii += 1;
        } else {
            non_ascii += 1;
        }
    }
    (ascii, non_ascii)
}

/// Estimate total tokens for a message list.
pub fn estimate_total_tokens(messages: &[Message]) -> u32 {
    messages.iter().map(estimate_tokens).sum()
}

/// Check if compression should be triggered.
pub fn should_compress(
    current_tokens: u32,
    context_size: Option<u32>,
    config: &CompressionConfig,
) -> bool {
    match context_size {
        Some(size) => (current_tokens as f64 / size as f64) >= config.threshold,
        None => false,
    }
}

/// Build a prompt for summarization.
pub fn build_summary_prompt(messages: &[Message]) -> String {
    let history = messages
        .iter()
        .map(|m| {
            let role = match m.role {
                Role::User => "用户",
                Role::Assistant => "助手",
                Role::Tool => "工具",
                Role::System => "系统",
            };
            let preview = match &m.content {
                MessageContent::Text(t) => truncate_with_suffix(t, 200),
                MessageContent::Blocks(blocks) => blocks
                    .iter()
                    .map(|b| match b {
                        ContentBlock::Text { text } => truncate_with_suffix(text, 100),
                        ContentBlock::ToolUse { name, .. } => format!("[工具: {}]", name),
                        ContentBlock::ToolResult { content, .. } => {
                            truncate_with_suffix(content, 100)
                        }
                        _ => "[...]".to_string(),
                    })
                    .collect::<Vec<_>>()
                    .join(" | "),
            };
            format!("{}: {}", role, preview)
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "请将以下对话压缩为简洁摘要（{} 条消息）：\n{}",
        messages.len(),
        history
    )
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens_simple() {
        let msg = Message {
            role: Role::User,
            content: MessageContent::Text("Hello world".to_string()),
        };
        assert!(estimate_tokens(&msg) >= 3);
    }

    #[test]
    fn test_should_compress() {
        let config = CompressionConfig::default();
        assert!(!should_compress(100_000, Some(200_000), &config));
        assert!(should_compress(160_000, Some(200_000), &config));
    }
}
