use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::providers::{ContentBlock, Message, MessageContent, Provider, Role, ChatRequest, ChatResponse};

/// Compression trigger threshold (percentage of context window).
pub const DEFAULT_COMPRESSION_THRESHOLD: f64 = 0.75;

/// Minimum messages to keep after compression.
pub const MIN_MESSAGES_TO_KEEP: usize = 8;

/// Target ratio after compression (keep this fraction of tokens).
pub const DEFAULT_TARGET_RATIO: f64 = 0.4;

/// Default model for summarization (cost-effective).
pub const DEFAULT_COMPRESSOR_MODEL: &str = "claude-3-5-haiku-20241022";

/// Compression bias - controls what to prioritize during compression.
#[derive(Debug, Clone, Default)]
pub struct CompressionBias {
    /// Preserve tool calls and their results (important operations).
    pub preserve_tools: bool,
    /// Preserve thinking blocks (reasoning process).
    pub preserve_thinking: bool,
    /// Preserve user questions (even if old).
    pub preserve_user_questions: bool,
    /// Compact long outputs instead of removing them.
    pub compact_long_outputs: bool,
    /// Aggressive mode - remove more content.
    pub aggressive: bool,
    /// Custom keywords to preserve messages containing them.
    pub preserve_keywords: Vec<String>,
}

impl CompressionBias {
    /// Default bias - balanced preservation.
    pub fn balanced() -> Self {
        Self {
            preserve_tools: true,
            preserve_thinking: false,
            preserve_user_questions: true,
            compact_long_outputs: false,
            aggressive: false,
            preserve_keywords: vec![
                "决定".to_string(), "decision".to_string(), 
                "重要".to_string(), "important".to_string(), 
                "关键".to_string(), "key".to_string()
            ],
        }
    }

    /// Preserve all important content (tools, thinking, decisions).
    pub fn preserve_important() -> Self {
        Self {
            preserve_tools: true,
            preserve_thinking: true,
            preserve_user_questions: true,
            compact_long_outputs: true,
            aggressive: false,
            preserve_keywords: vec![
                "决定".to_string(), "decision".to_string(), 
                "重要".to_string(), "important".to_string(), 
                "关键".to_string(), "key".to_string(),
                "完成".to_string(), "done".to_string(), 
                "成功".to_string(), "success".to_string()
            ],
        }
    }

    /// Aggressive compression - remove as much as possible.
    pub fn aggressive() -> Self {
        Self {
            preserve_tools: false,
            preserve_thinking: false,
            preserve_user_questions: false,
            compact_long_outputs: false,
            aggressive: true,
            preserve_keywords: vec![],
        }
    }

    /// Focus on preserving tool operations.
    pub fn tool_focused() -> Self {
        Self {
            preserve_tools: true,
            preserve_thinking: false,
            preserve_user_questions: false,
            compact_long_outputs: false,
            aggressive: false,
            preserve_keywords: vec![
                "工具".to_string(), "tool".to_string(), 
                "执行".to_string(), "execute".to_string(), 
                "文件".to_string(), "file".to_string()
            ],
        }
    }

    /// Parse bias from a string specification.
    /// Format: "preserve:tools,thinking,user" or "aggressive" or "balanced"
    pub fn parse(spec: &str) -> Result<Self> {
        let spec = spec.trim().to_lowercase();
        
        if spec == "balanced" || spec == "default" || spec.is_empty() {
            return Ok(Self::balanced());
        }
        if spec == "aggressive" {
            return Ok(Self::aggressive());
        }
        if spec == "preserve_important" || spec == "important" {
            return Ok(Self::preserve_important());
        }
        if spec == "tool_focused" || spec == "tools" {
            return Ok(Self::tool_focused());
        }

        // Parse custom specification: "preserve:tools,thinking,user keywords:决定,重要"
        let mut bias = Self::default();
        
        for part in spec.split_whitespace() {
            if let Some(preserve_list) = part.strip_prefix("preserve:") {
                for item in preserve_list.split(',') {
                    match item.trim() {
                        "tools" | "tool" => bias.preserve_tools = true,
                        "thinking" | "think" => bias.preserve_thinking = true,
                        "user" | "questions" => bias.preserve_user_questions = true,
                        "compact" | "long" => bias.compact_long_outputs = true,
                        _ => {}
                    }
                }
            } else if let Some(keyword_list) = part.strip_prefix("keywords:") {
                bias.preserve_keywords = keyword_list.split(',')
                    .map(|k| k.trim().to_string())
                    .filter(|k| !k.is_empty())
                    .collect();
            } else if part == "aggressive" {
                bias.aggressive = true;
            }
        }

        Ok(bias)
    }

    /// Format bias for display.
    pub fn format(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        
        if self.preserve_tools { parts.push("tools".to_string()); }
        if self.preserve_thinking { parts.push("thinking".to_string()); }
        if self.preserve_user_questions { parts.push("user".to_string()); }
        if self.compact_long_outputs { parts.push("compact".to_string()); }
        if self.aggressive { parts.push("aggressive".to_string()); }
        
        if !self.preserve_keywords.is_empty() {
            parts.push(format!("keywords:{}", self.preserve_keywords.join(",")));
        }

        if parts.is_empty() {
            "default".to_string()
        } else {
            parts.join(", ")
        }
    }
}

/// Configuration for context compression.
#[derive(Debug, Clone)]
pub struct CompressionConfig {
    /// Threshold (0.0-1.0) at which to trigger compression.
    pub threshold: f64,
    /// Maximum tokens to target after compression.
    pub target_ratio: f64,
    /// Minimum recent messages to always preserve.
    pub min_preserve_messages: usize,
    /// Whether to use AI summarization (requires a compressor model).
    pub use_summarization: bool,
    /// Optional model name for summarization (if different from main model).
    pub compressor_model: Option<String>,
    /// Compression bias - what to prioritize during compression.
    pub bias: CompressionBias,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            threshold: DEFAULT_COMPRESSION_THRESHOLD,
            target_ratio: DEFAULT_TARGET_RATIO,
            min_preserve_messages: MIN_MESSAGES_TO_KEEP,
            use_summarization: true,
            compressor_model: None,
            bias: CompressionBias::balanced(),
        }
    }
}

impl CompressionConfig {
    /// Get the compressor model name.
    pub fn compressor_model_name(&self) -> &str {
        self.compressor_model.as_deref().unwrap_or(DEFAULT_COMPRESSOR_MODEL)
    }
}

/// Result of a compression operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionResult {
    /// Original message count.
    pub original_count: usize,
    /// New message count after compression.
    pub new_count: usize,
    /// Estimated token reduction.
    pub tokens_saved: u32,
    /// Summary of removed content (if summarization was used).
    pub summary: Option<String>,
    /// Strategy used for compression.
    pub strategy: CompressionStrategy,
    /// When the compression occurred.
    pub timestamp: DateTime<Utc>,
}

impl CompressionResult {
    /// Create a new compression result.
    pub fn new(
        original_count: usize,
        new_count: usize,
        tokens_saved: u32,
        summary: Option<String>,
        strategy: CompressionStrategy,
    ) -> Self {
        Self {
            original_count,
            new_count,
            tokens_saved,
            summary,
            strategy,
            timestamp: Utc::now(),
        }
    }

    /// Format for display.
    pub fn format_summary(&self) -> String {
        let strategy_name = match self.strategy {
            CompressionStrategy::Truncate => "truncate",
            CompressionStrategy::SlidingWindow => "sliding window",
            CompressionStrategy::Summarize => "AI summarize",
            CompressionStrategy::BiasBased => "bias-based",
        };
        format!(
            "{} messages → {} messages (saved ~{} tokens, {})",
            self.original_count,
            self.new_count,
            format_tokens(self.tokens_saved),
            strategy_name
        )
    }
}

pub fn format_tokens(n: u32) -> String {
    if n < 1_000 {
        n.to_string()
    } else if n < 10_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{:.0}K", n as f64 / 1_000.0)
    }
}

/// Strategy for compressing conversation history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompressionStrategy {
    /// Remove oldest messages, keep recent ones.
    Truncate,
    /// Use sliding window - keep last N message pairs.
    SlidingWindow,
    /// Summarize old messages into a compact summary block.
    Summarize,
    /// Use bias-based scoring to prioritize what to keep.
    BiasBased,
}

/// A segment of conversation history that has been summarized.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummarizedSegment {
    /// Timestamp range of the summarized messages.
    pub time_range: (DateTime<Utc>, DateTime<Utc>),
    /// Number of original messages in this segment.
    pub original_count: usize,
    /// The summary text.
    pub summary: String,
    /// Key decisions or actions taken during this segment.
    pub key_points: Vec<String>,
}

impl SummarizedSegment {
    /// Render as a system message for context injection.
    pub fn to_message(&self) -> Message {
        let key_points_text = if self.key_points.is_empty() {
            "无".to_string()
        } else {
            self.key_points.iter().map(|p| format!("• {}", p)).collect::<Vec<_>>().join("\n")
        };
        
        let content = format!(
            "[对话摘要 - 原 {} 条消息]\n\n{}\n\n关键要点：\n{}",
            self.original_count,
            self.summary,
            key_points_text
        );
        
        Message {
            role: Role::User,
            content: MessageContent::Text(content),
        }
    }
}

/// Compression history entry for session metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionHistoryEntry {
    /// When the compression occurred.
    pub timestamp: DateTime<Utc>,
    /// Strategy used.
    pub strategy: CompressionStrategy,
    /// Original message count.
    pub original_count: usize,
    /// New message count.
    pub new_count: usize,
    /// Estimated tokens saved.
    pub tokens_saved: u32,
    /// Whether summary was generated.
    pub has_summary: bool,
}

impl CompressionHistoryEntry {
    /// Create from a CompressionResult.
    pub fn from_result(result: &CompressionResult) -> Self {
        Self {
            timestamp: result.timestamp,
            strategy: result.strategy,
            original_count: result.original_count,
            new_count: result.new_count,
            tokens_saved: result.tokens_saved,
            has_summary: result.summary.is_some(),
        }
    }

    /// Format for display.
    pub fn format_line(&self) -> String {
        let strategy_name = match self.strategy {
            CompressionStrategy::Truncate => "truncate",
            CompressionStrategy::SlidingWindow => "sliding window",
            CompressionStrategy::Summarize => "AI summarize",
            CompressionStrategy::BiasBased => "bias-based",
        };
        let summary_marker = if self.has_summary { "📝" } else { "✂️" };
        format!(
            "{} {} - {} msgs → {} msgs (~{} tokens saved) {}",
            self.timestamp.format("%Y-%m-%d %H:%M"),
            strategy_name,
            self.original_count,
            self.new_count,
            format_tokens(self.tokens_saved),
            summary_marker
        )
    }
}

/// Compressor trait for different compression implementations.
#[async_trait]
pub trait Compressor: Send + Sync {
    /// Compress messages using AI summarization.
    async fn summarize(&self, messages: &[Message], config: &CompressionConfig) -> Result<SummarizedSegment>;
    
    /// Get the model name used for summarization.
    fn model_name(&self) -> &str;
}

/// AI-based compressor using a Provider.
pub struct AiCompressor {
    provider: Box<dyn Provider>,
    model: String,
}

impl AiCompressor {
    /// Create a new AI compressor.
    pub fn new(provider: Box<dyn Provider>, model: String) -> Self {
        Self { provider, model }
    }
}

#[async_trait]
impl Compressor for AiCompressor {
    async fn summarize(&self, messages: &[Message], _config: &CompressionConfig) -> Result<SummarizedSegment> {
        let prompt = build_summary_prompt(messages);
        
        let request = ChatRequest {
            messages: vec![Message {
                role: Role::User,
                content: MessageContent::Text(prompt),
            }],
            tools: vec![], // No tools for summarization
            system: Some(SUMMARY_SYSTEM_PROMPT.to_string()),
            think: false, // No extended thinking for summarization
            max_tokens: 1024, // Short summary
            server_tools: vec![],
            enable_caching: false, // No caching for summarization
        };
        
        let response = self.provider.chat(request).await?;
        
        // Extract text from response
        let summary_text = extract_text_from_response(&response);
        
        // Parse the summary into structured format
        let (summary, key_points) = parse_summary_response(&summary_text);
        
        Ok(SummarizedSegment {
            time_range: (Utc::now(), Utc::now()), // Approximate
            original_count: messages.len(),
            summary,
            key_points,
        })
    }
    
    fn model_name(&self) -> &str {
        &self.model
    }
}

/// System prompt for summarization.
const SUMMARY_SYSTEM_PROMPT: &str = r#"你是一个对话历史压缩助手。你的任务是将对话历史压缩为简洁的摘要，保留关键信息。

输出要求：
- 简洁：摘要控制在 200 字以内
- 关键：只保留重要操作和决策
- 结构化：使用清晰格式
- 敏感：必须保留用户的敏感指令（如"不要..."、"必须..."、"禁止..."等）
- 偏好：保留用户的偏好设置和决策

请直接输出摘要内容。"#;

/// Extract text content from a chat response.
fn extract_text_from_response(response: &ChatResponse) -> String {
    response.content
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

/// Parse summary response into structured format.
fn parse_summary_response(text: &str) -> (String, Vec<String>) {
    let mut summary = String::new();
    let mut key_points: Vec<String> = Vec::new();
    
    for line in text.lines() {
        let line = line.trim();
        
        // Detect bullet points
        if line.starts_with("•") || line.starts_with("-") || line.starts_with("*") {
            let point = line.trim_start_matches(['•', '-', '*']).trim();
            if !point.is_empty() {
                key_points.push(point.to_string());
            }
        } else if line.starts_with("已完成") || line.starts_with("操作") {
            // Extract operations section
            let ops = line.trim_start_matches(|c: char| c.is_alphabetic() || c == ':' || c == '：').trim();
            if !ops.is_empty() && ops != "：" && ops != ":" {
                key_points.push(ops.to_string());
            }
        } else if !line.is_empty() && summary.is_empty() {
            // First non-empty line is the overview
            summary = line.to_string();
        } else if !line.is_empty() {
            // Append to summary if no key points yet
            if key_points.is_empty() && summary.len() < 200 {
                summary.push(' ');
                summary.push_str(line);
            }
        }
    }
    
    // If no structured parsing worked, use the whole text as summary
    if summary.is_empty() && !text.is_empty() {
        summary = text.lines().take(3).collect::<Vec<_>>().join(" ");
        if summary.len() > 200 {
            summary = truncate_text(&summary, 200);
        }
    }
    
    (summary, key_points)
}

fn truncate_text(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let mut end = max;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &s[..end])
    }
}

/// Compress messages synchronously (for non-AI strategies).
pub fn compress_messages(
    messages: &[Message],
    strategy: CompressionStrategy,
    config: &CompressionConfig,
) -> Result<Vec<Message>> {
    match strategy {
        CompressionStrategy::Truncate => truncate_compress(messages, config),
        CompressionStrategy::SlidingWindow => sliding_window_compress(messages, config),
        CompressionStrategy::Summarize => {
            // Summarize requires async AI call, fall back to sliding window
            sliding_window_compress(messages, config)
        }
        CompressionStrategy::BiasBased => compress_with_bias(messages, config),
    }
}

/// Compress messages with bias - prioritized removal based on configuration.
pub fn compress_with_bias(
    messages: &[Message],
    config: &CompressionConfig,
) -> Result<Vec<Message>> {
    if messages.len() <= config.min_preserve_messages {
        return Ok(messages.to_vec());
    }

    // Calculate preservation score for each message
    let scored_messages: Vec<(usize, Message, f64)> = messages
        .iter()
        .enumerate()
        .map(|(idx, msg)| (idx, msg.clone(), calculate_preservation_score(msg, idx, messages.len(), &config.bias)))
        .collect();

    // Sort by score (higher score = more important to keep)
    // Also factor in recency - recent messages get bonus score
    let mut scored_with_recency: Vec<(usize, Message, f64)> = scored_messages
        .into_iter()
        .map(|(idx, msg, score)| {
            // Recency bonus: later messages get higher score
            let recency_bonus = if idx >= messages.len() - config.min_preserve_messages {
                100.0 // Always keep recent messages
            } else {
                (idx as f64 / messages.len() as f64) * 20.0 // Up to 20 points for being recent
            };
            (idx, msg, score + recency_bonus)
        })
        .collect();

    // Sort by score descending
    scored_with_recency.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    // Determine how many to keep based on target_ratio and aggressive mode
    let target_count = if config.bias.aggressive {
        config.min_preserve_messages
    } else {
        let estimated_tokens = estimate_total_tokens(messages);
        let target_tokens = (estimated_tokens as f64 * config.target_ratio) as u32;
        let avg_tokens_per_msg = estimated_tokens / messages.len() as u32;
        let calculated = (target_tokens / avg_tokens_per_msg.max(1)) as usize;
        calculated.max(config.min_preserve_messages)
    };

    // Keep top-scored messages, but maintain chronological order
    let to_keep_indices: HashSet<usize> = scored_with_recency
        .iter()
        .take(target_count)
        .map(|(idx, _, _)| *idx)
        .collect();

    // Rebuild message list in original order
    let compressed: Vec<Message> = messages
        .iter()
        .enumerate()
        .filter(|(idx, _)| to_keep_indices.contains(idx))
        .map(|(_, msg)| msg.clone())
        .collect();

    Ok(compressed)
}

/// Calculate preservation score for a message (higher = more important to keep).
fn calculate_preservation_score(message: &Message, _index: usize, _total: usize, bias: &CompressionBias) -> f64 {
    let mut score: f64 = 10.0; // Base score

    // Role-based scoring
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
            score += 40.0; // System messages are usually important
        }
    }

    // Content-based scoring
    match &message.content {
        MessageContent::Text(text) => {
            // Check for keywords
            for keyword in &bias.preserve_keywords {
                if text.to_lowercase().contains(&keyword.to_lowercase()) {
                    score += 15.0;
                }
            }
            
            // Check for sensitive instructions (Claude Code inspired)
            if contains_sensitive_instructions(text) {
                score += 50.0; // Highly preserve sensitive instructions
            }
            
            // Penalize very long messages if not compacting
            if !bias.compact_long_outputs && text.len() > 2000 {
                score -= 10.0;
            }
        }
        MessageContent::Blocks(blocks) => {
            for block in blocks {
                match block {
                    ContentBlock::ToolUse { name, .. } => {
                        if bias.preserve_tools {
                            score += 20.0;
                        }
                        // Certain tools are more important
                        if name == "write" || name == "edit" || name == "bash" {
                            score += 10.0;
                        }
                    }
                    ContentBlock::ToolResult { content, .. } => {
                        if bias.preserve_tools {
                            score += 20.0;
                        }
                        // Check for keywords in result
                        for keyword in &bias.preserve_keywords {
                            if content.to_lowercase().contains(&keyword.to_lowercase()) {
                                score += 10.0;
                            }
                        }
                        // Check for sensitive instructions in result
                        if contains_sensitive_instructions(content) {
                            score += 30.0;
                        }
                    }
                    ContentBlock::Thinking { .. } => {
                        if bias.preserve_thinking {
                            score += 25.0;
                        } else {
                            score -= 5.0; // Thinking blocks can be verbose
                        }
                    }
                    ContentBlock::Text { text } => {
                        for keyword in &bias.preserve_keywords {
                            if text.to_lowercase().contains(&keyword.to_lowercase()) {
                                score += 15.0;
                            }
                        }
                        // Check for sensitive instructions
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

/// Check if text contains sensitive user instructions that must be preserved.
/// Inspired by Claude Code's "preserve sensitive user instructions" feature.
fn contains_sensitive_instructions(text: &str) -> bool {
    let text_lower = text.to_lowercase();
    
    // Sensitive instruction patterns (cleaned, no duplicates, more specific)
    let sensitive_patterns = [
        // Negative instructions (must NOT do something)
        "不要", "禁止", "不能", "千万别", "禁止使用",
        "never do", "must not", "should not", "cannot", "avoid",
        
        // Mandatory instructions (MUST do something)
        "必须", "一定要", "务必", "必须使用",
        "must", "required", "mandatory",
        
        // Security/privacy related
        "敏感", "隐私", "密码", "secret", "password", "credential",
        "private", "sensitive", "confidential",
        
        // Critical decisions
        "决定", "决策", "critical", "important", "关键",
        
        // User preferences
        "偏好", "我喜欢", "我习惯", "prefer", "preference",
        
        // Strict constraints
        "严格按照", "遵循", "按原样", "strictly", "exactly",
        "不要修改", "不要改动", "keep original", "as is",
    ];
    
    for pattern in &sensitive_patterns {
        if text_lower.contains(pattern) {
            return true;
        }
    }
    
    false
}

/// Compress messages with AI summarization (async version).
pub async fn compress_messages_with_ai(
    messages: &[Message],
    compressor: &dyn Compressor,
    config: &CompressionConfig,
) -> Result<(Vec<Message>, Option<SummarizedSegment>)> {
    if messages.len() <= config.min_preserve_messages {
        return Ok((messages.to_vec(), None));
    }
    
    // Determine split point: messages to summarize vs messages to keep
    let preserve_count = config.min_preserve_messages;
    let summarize_messages = &messages[..messages.len() - preserve_count];
    let keep_messages = &messages[messages.len() - preserve_count..];
    
    // Generate summary
    let segment = compressor.summarize(summarize_messages, config).await?;
    
    // Build new message list: summary message + kept messages
    let summary_msg = segment.to_message();
    let mut compressed = vec![summary_msg];
    compressed.extend(keep_messages.to_vec());
    
    Ok((compressed, Some(segment)))
}

/// Simple truncation: remove oldest messages.
fn truncate_compress(messages: &[Message], config: &CompressionConfig) -> Result<Vec<Message>> {
    if messages.len() <= config.min_preserve_messages {
        return Ok(messages.to_vec());
    }

    let keep_count = config.min_preserve_messages;
    let start_idx = messages.len().saturating_sub(keep_count);

    Ok(messages[start_idx..].to_vec())
}

/// Sliding window: preserve complete conversation turns.
/// Now uses token-based target instead of turn count for more stable compression.
fn sliding_window_compress(messages: &[Message], config: &CompressionConfig) -> Result<Vec<Message>> {
    if messages.len() <= config.min_preserve_messages {
        return Ok(messages.to_vec());
    }

    // Estimate total tokens
    let total_tokens = estimate_total_tokens(messages);
    let target_tokens = (total_tokens as f64 * config.target_ratio) as u32;
    
    // Find turn boundaries (user messages mark start of each turn)
    let mut turn_boundaries: Vec<usize> = Vec::new();
    for (i, msg) in messages.iter().enumerate() {
        if msg.role == Role::User {
            turn_boundaries.push(i);
        }
    }

    // Minimum start index to ensure we keep at least min_preserve_messages
    let min_start_idx = messages.len().saturating_sub(config.min_preserve_messages);
    
    // Try to find a turn that:
    // 1. Starts at or after min_start_idx (ensures enough messages)
    // 2. Fits within token target
    // Iterate from the earliest acceptable turn
    for &start_idx in turn_boundaries.iter() {
        // Must have enough messages
        if messages.len() - start_idx < config.min_preserve_messages {
            continue;
        }
        
        let candidate_messages = &messages[start_idx..];
        let candidate_tokens = estimate_total_tokens(candidate_messages);
        
        // If this turn fits within token target, use it
        if candidate_tokens <= target_tokens {
            return Ok(candidate_messages.to_vec());
        }
    }

    // Fallback: keep exactly min_preserve_messages from the end
    Ok(messages[min_start_idx..].to_vec())
}

/// Estimate token count for a message (rough approximation).
pub fn estimate_tokens(message: &Message) -> u32 {
    let char_count = match &message.content {
        MessageContent::Text(t) => t.len(),
        MessageContent::Blocks(blocks) => {
            let mut count = 0;
            for block in blocks {
                match block {
                    ContentBlock::Text { text } => count += text.len(),
                    ContentBlock::ToolUse { name, input, .. } => {
                        count += name.len();
                        count += input.to_string().len();
                    }
                    ContentBlock::ToolResult { content, .. } => count += content.len(),
                    ContentBlock::Thinking { thinking, .. } => count += thinking.len(),
                    _ => {}
                }
            }
            count
        }
    };

    (char_count / 3).max(1) as u32
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
        Some(size) => {
            let ratio = current_tokens as f64 / size as f64;
            ratio >= config.threshold
        }
        None => false,
    }
}

/// Build a prompt for AI-based summarization.
pub fn build_summary_prompt(messages: &[Message]) -> String {
    let history_text = messages
        .iter()
        .map(|m| {
            let role = match m.role {
                Role::User => "用户",
                Role::Assistant => "助手",
                Role::Tool => "工具",
                Role::System => "系统",
            };
            let content_preview = match &m.content {
                MessageContent::Text(t) => truncate_for_summary(t, 200),
                MessageContent::Blocks(blocks) => {
                    let preview: Vec<String> = blocks
                        .iter()
                        .map(|b| match b {
                            ContentBlock::Text { text } => truncate_for_summary(text, 100),
                            ContentBlock::ToolUse { name, .. } => format!("[工具: {}]", name),
                            ContentBlock::ToolResult { content, .. } => truncate_for_summary(content, 100),
                            _ => "[...]".to_string(),
                        })
                        .collect();
                    preview.join(" | ")
                }
            };
            format!("{}: {}", role, content_preview)
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"请将以下对话历史压缩为简洁摘要：

对话历史（{} 条消息）：
{}

请输出：
1. 概述（一句话描述主要任务）
2. 已完成的关键操作（2-3 条）
3. 当前状态（如果有）"#,
        messages.len(),
        history_text
    )
}

fn truncate_for_summary(s: &str, max: usize) -> String {
    truncate_text(s, max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_estimate_tokens_simple() {
        let msg = Message {
            role: Role::User,
            content: MessageContent::Text("Hello world".to_string()),
        };
        assert!(estimate_tokens(&msg) >= 3);
    }

    #[test]
    fn test_should_compress_below_threshold() {
        let config = CompressionConfig::default();
        assert!(!should_compress(100_000, Some(200_000), &config));
    }

    #[test]
    fn test_should_compress_above_threshold() {
        let config = CompressionConfig::default();
        assert!(should_compress(160_000, Some(200_000), &config));
    }

    #[test]
    fn test_truncate_compress_keeps_minimum() {
        let messages: Vec<Message> = (0..10)
            .map(|i| Message {
                role: Role::User,
                content: MessageContent::Text(format!("Message {}", i)),
            })
            .collect();

        let config = CompressionConfig {
            min_preserve_messages: 4,
            ..Default::default()
        };

        let compressed = truncate_compress(&messages, &config).unwrap();
        assert_eq!(compressed.len(), 4);
        assert_eq!(compressed[0].content, MessageContent::Text("Message 6".to_string()));
    }

    #[test]
    fn test_sliding_window_preserves_turns() {
        // Create messages with longer content to test token-based compression
        let messages: Vec<Message> = vec![
            Message { role: Role::User, content: MessageContent::Text("Q1 - this is a longer question to test token estimation".to_string()) },
            Message { role: Role::Assistant, content: MessageContent::Text("A1 - this is a longer answer with more content for token estimation".to_string()) },
            Message { role: Role::User, content: MessageContent::Text("Q2 - another longer question for testing".to_string()) },
            Message { role: Role::Assistant, content: MessageContent::Text("A2 - another longer answer for testing token estimation properly".to_string()) },
            Message { role: Role::User, content: MessageContent::Text("Q3 - the third question in this test".to_string()) },
            Message { role: Role::Assistant, content: MessageContent::Text("A3 - the third answer with sufficient content".to_string()) },
        ];

        let config = CompressionConfig {
            min_preserve_messages: 4,
            target_ratio: 0.5,
            ..Default::default()
        };

        let compressed = sliding_window_compress(&messages, &config).unwrap();
        // Should preserve at least min_preserve_messages
        assert!(compressed.len() >= config.min_preserve_messages);
        // Should preserve complete turns (user + assistant pairs)
        assert!(compressed.iter().any(|m| m.role == Role::User));
    }

    #[test]
    fn test_parse_summary_response() {
        let text = "用户请求实现登录功能。\n已完成操作：\n• 创建了 login.rs 文件\n• 添加了密码验证逻辑\n当前状态：测试中";
        let (summary, key_points) = parse_summary_response(text);
        
        assert!(!summary.is_empty());
        assert!(key_points.len() >= 2);
    }

    #[test]
    fn test_compression_result_format() {
        let result = CompressionResult::new(
            20,
            8,
            5000,
            Some("摘要内容".to_string()),
            CompressionStrategy::Summarize,
        );
        
        let formatted = result.format_summary();
        assert!(formatted.contains("20"));
        assert!(formatted.contains("8"));
        assert!(formatted.contains("AI summarize"));
    }

    #[test]
    fn test_compression_history_entry() {
        let result = CompressionResult::new(
            15,
            6,
            3000,
            None,
            CompressionStrategy::SlidingWindow,
        );
        
        let entry = CompressionHistoryEntry::from_result(&result);
        assert_eq!(entry.strategy, CompressionStrategy::SlidingWindow);
        assert!(!entry.has_summary);
    }

    #[test]
    fn test_compression_bias_parse() {
        // Test preset biases
        let balanced = CompressionBias::parse("balanced").unwrap();
        assert!(balanced.preserve_tools);
        assert!(balanced.preserve_user_questions);

        let aggressive = CompressionBias::parse("aggressive").unwrap();
        assert!(!aggressive.preserve_tools);
        assert!(aggressive.aggressive);

        let important = CompressionBias::parse("important").unwrap();
        assert!(important.preserve_thinking);
        assert!(important.preserve_tools);

        let tools = CompressionBias::parse("tools").unwrap();
        assert!(tools.preserve_tools);
        assert!(!tools.preserve_thinking);
    }

    #[test]
    fn test_compression_bias_format() {
        let bias = CompressionBias::balanced();
        let formatted = bias.format();
        assert!(formatted.contains("tools"));
        assert!(formatted.contains("user"));
    }

    #[test]
    fn test_compress_with_bias_preserves_tools() {
        let messages: Vec<Message> = vec![
            Message { role: Role::User, content: MessageContent::Text("Q1".to_string()) },
            Message { 
                role: Role::Assistant, 
                content: MessageContent::Blocks(vec![
                    ContentBlock::ToolUse { id: "1".to_string(), name: "read".to_string(), input: json!({}) }
                ])
            },
            Message { role: Role::Tool, content: MessageContent::Blocks(vec![
                ContentBlock::ToolResult { tool_use_id: "1".to_string(), content: "file content".to_string() }
            ])},
            Message { role: Role::User, content: MessageContent::Text("Q2".to_string()) },
            Message { role: Role::Assistant, content: MessageContent::Text("A2".to_string()) },
            Message { role: Role::User, content: MessageContent::Text("Q3".to_string()) },
            Message { role: Role::Assistant, content: MessageContent::Text("A3".to_string()) },
        ];

        let config = CompressionConfig {
            min_preserve_messages: 2,
            bias: CompressionBias::tool_focused(),
            ..Default::default()
        };

        let compressed = compress_with_bias(&messages, &config).unwrap();
        
        // Tool-focused bias should preserve tool calls
        let has_tool_use = compressed.iter().any(|m| {
            matches!(&m.content, MessageContent::Blocks(blocks) if 
                blocks.iter().any(|b| matches!(b, ContentBlock::ToolUse { .. })))
        });
        assert!(has_tool_use || compressed.len() >= messages.len() - 2);
    }

    #[test]
    fn test_bias_based_strategy() {
        let messages: Vec<Message> = (0..10)
            .map(|i| Message {
                role: if i % 2 == 0 { Role::User } else { Role::Assistant },
                content: MessageContent::Text(format!("Message {}", i)),
            })
            .collect();

        let config = CompressionConfig {
            min_preserve_messages: 4,
            bias: CompressionBias::aggressive(),
            ..Default::default()
        };

        let compressed = compress_messages(&messages, CompressionStrategy::BiasBased, &config).unwrap();
        assert!(compressed.len() <= messages.len());
        assert!(compressed.len() >= config.min_preserve_messages);
    }
}