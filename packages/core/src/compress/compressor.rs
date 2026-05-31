//! Compression functions and AI compressor implementation.

use crate::providers::{
    ChatRequest, ChatResponse, ContentBlock, Message, MessageContent, Provider, Role,
};
use crate::tokenizer::{count_tokens, message_overhead};
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

const SUMMARY_SYSTEM_PROMPT: &str = r#"CRITICAL: 仅用文本响应。不要调用任何工具。

- 不要使用 read、bash、grep、glob、edit、write 或任何其他工具
- 你已在上方对话中获得所需的所有上下文
- 工具调用将被拒绝并浪费你唯一的 turn — 你将失败任务
- 你的整个响应必须是纯文本摘要

---

你是一个对话历史压缩助手。将对话压缩为结构化摘要。

在提供最终摘要前，将你的分析包裹在 <analysis> 标签中以组织思路：
<analysis>
1. 按时间顺序分析每条消息
2. 识别：用户请求、助手行动、关键决策、错误及修复
3. 特别注意用户的敏感指令（禁止/必须）
4. 双重检查技术准确性和完整性
</analysis>

输出要求：
- 结构化：使用9个章节格式
- 关键：只保留重要信息，忽略无关细节
- 敏感：必须保留用户的敏感指令（禁止、必须等）— 原样保留
- 任务：必须保留未完成的待办事项
- 决策：必须保留关键方案选择和理由

9章节输出格式：
【摘要】一句话概括主要工作（50字以内）
【已完成】列出已完成的操作（工具调用、文件变更）
【未完成】列出待办任务和阻塞项 — 最关键，压缩后恢复需要
【关键决策】重要选择及理由（技术选型、方案决策）
【敏感指令】用户的禁止/必须指令（必须原样保留）
【技术栈】使用的语言、框架、库、工具
【文件变更】读取、修改、创建的文件路径
【问题记录】遇到的问题及解决方案
【下一步】建议的下一步操作（直接引用最近对话展示任务中断点）

每章节控制在100字以内，空章节可省略。
输出摘要后立即停止，不要添加任何解释或后续建议。

REMINDER: 不要调用任何工具。仅用纯文本响应。"#;

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

    // Parse 9-section structured format
    let sections = [
        "【摘要】",
        "【已完成】",
        "【未完成】",
        "【关键决策】",
        "【敏感指令】",
        "【技术栈】",
        "【文件变更】",
        "【问题记录】",
        "【下一步】",
    ];

    for line in text.lines() {
        let line = line.trim();

        // Check if this is a section header
        let is_header = sections.iter().any(|s| line.starts_with(s));

        if is_header {
            // Extract content after the header
            for section in &sections {
                if line.starts_with(section) {
                    let replaced = line.replace(section, "");
                    let content = replaced.trim();
                    if !content.is_empty() {
                        if *section == "【摘要】" {
                            summary = content.to_string();
                        } else {
                            key_points.push(format!("{}{}", section, content));
                        }
                    }
                    break;
                }
            }
        } else if !line.is_empty() {
            // This is content under a section
            if line.starts_with("•") || line.starts_with("-") || line.starts_with("*") {
                let point = line.trim_start_matches(['•', '-', '*']).trim();
                if !point.is_empty() {
                    key_points.push(point.to_string());
                }
            } else if summary.is_empty() {
                // Fallback: first non-empty line as summary
                summary = line.to_string();
            }
        }
    }

    // Fallback if no structured format found
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
    _total: usize, // Reserved for future use (total message count)
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

    // Enhanced sliding window strategy:
    // 1. Always keep first message (original user request)
    // 2. Summarize middle messages if too long
    // 3. Keep recent messages intact

    let first_msg = messages.first().cloned();
    let recent_start = messages.len().saturating_sub(config.min_preserve_messages);
    let recent_msgs = &messages[recent_start..];

    // Calculate tokens for first + recent
    let first_tokens = first_msg.as_ref().map(estimate_tokens).unwrap_or(0);
    let recent_tokens = estimate_total_tokens(recent_msgs);
    let current_total = estimate_total_tokens(messages);
    let target_tokens = (current_total as f64 * config.target_ratio) as u32;

    // If first + recent already exceeds target, just use recent (drop first)
    if first_tokens + recent_tokens <= target_tokens {
        // We can keep first message + recent messages
        let mut result: Vec<Message> = Vec::new();
        if let Some(first) = first_msg {
            result.push(first);
        }
        result.extend(recent_msgs.iter().cloned());
        return Ok(result);
    }

    // If still too long, try dropping older messages from recent section
    for drop_count in 0..recent_msgs.len() {
        let candidate = &recent_msgs[drop_count..];
        if estimate_total_tokens(candidate) <= target_tokens {
            return Ok(candidate.to_vec());
        }
    }

    // Last resort: just keep minimum recent messages
    Ok(messages[messages.len() - config.min_preserve_messages..].to_vec())
}

// ============================================================================
// Token Estimation
// ============================================================================

/// Count tokens for a message using tiktoken (accurate).
pub fn estimate_tokens(message: &Message) -> u32 {
    let content_text = match &message.content {
        MessageContent::Text(t) => t.clone(),
        MessageContent::Blocks(blocks) => {
            let mut text = String::new();
            for block in blocks {
                match block {
                    ContentBlock::Text { text: t } => {
                        text.push_str(t);
                        text.push(' '); // Separator
                    }
                    ContentBlock::ToolUse { name, input, .. } => {
                        text.push_str(name);
                        text.push(' '); // Separator
                        text.push_str(&input.to_string());
                        text.push(' '); // Separator
                    }
                    ContentBlock::ToolResult { content, .. } => {
                        text.push_str(content);
                        text.push(' '); // Separator
                    }
                    ContentBlock::Thinking { thinking, .. } => {
                        text.push_str(thinking);
                        text.push(' '); // Separator
                    }
                    _ => {}
                }
            }
            text
        }
    };

    // Use tiktoken for accurate token counting
    let content_tokens = count_tokens(&content_text);
    let role_tokens = count_tokens(&format!("{:?}: ", message.role));
    
    // Total = content + role prefix + overhead
    content_tokens + role_tokens + message_overhead()
}

/// Legacy char counting function (deprecated, kept for backward compatibility).
#[deprecated(note = "Use estimate_tokens with tiktoken instead")]
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
// New Pipeline-Based Compression (Async)
// ============================================================================

use super::pipeline::CompressionPipeline;
use super::types::AiCompressionMode;

/// Compress messages with AI assistance (async version).
///
/// This is the new recommended API for compression with intelligent
/// scoring, dependency tracking, and content summarization.
pub async fn compress_messages_with_ai(
    messages: &[Message],
    config: &CompressionConfig,
    ai_mode: AiCompressionMode,
    fast_model: Option<Box<dyn Provider>>,
    token_usage: u32,
    context_window: u32,
) -> Result<Vec<Message>> {
    let mut pipeline = match (ai_mode, fast_model) {
        (AiCompressionMode::None, _) => CompressionPipeline::new_rule_only(config.clone()),
        (AiCompressionMode::Light | AiCompressionMode::Deep, Some(model)) => {
            CompressionPipeline::new_with_ai(config.clone(), model)
        }
        _ => CompressionPipeline::new_rule_only(config.clone()),
    };

    let result = pipeline
        .execute(messages, ai_mode, token_usage, context_window)
        .await?;
    Ok(result.messages)
}

/// Compress messages with full AI support (async version).
///
/// Uses both fast_model and main_model for different compression tasks.
pub async fn compress_messages_with_full_ai(
    messages: &[Message],
    config: &CompressionConfig,
    ai_mode: AiCompressionMode,
    fast_model: Box<dyn Provider>,
    main_model: Box<dyn Provider>,
    token_usage: u32,
    context_window: u32,
) -> Result<Vec<Message>> {
    let mut pipeline =
        CompressionPipeline::new_with_full_ai(config.clone(), fast_model, main_model);

    let result = pipeline
        .execute(messages, ai_mode, token_usage, context_window)
        .await?;
    Ok(result.messages)
}

/// Score messages without compressing (analysis only).
///
/// Useful for debugging and understanding compression decisions.
pub fn score_messages_only(
    messages: &[Message],
    config: &CompressionConfig,
) -> Vec<super::types::ScoredMessage> {
    let pipeline = CompressionPipeline::new_rule_only(config.clone());
    pipeline.score_only(messages)
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
        // Threshold is 0.5, so 100K/200K = 0.5 triggers compression
        assert!(should_compress(100_000, Some(200_000), &config));
        // 80K/200K = 0.4, below threshold
        assert!(!should_compress(80_000, Some(200_000), &config));
    }
}
