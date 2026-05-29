//! Intelligent scoring for message preservation decisions.
//!
//! Combines rule-based scoring, optional AI assistance, and dependency
//! bonuses to determine which messages to keep during compression.

use anyhow::Result;

use crate::providers::{ContentBlock, Message, MessageContent, Provider, Role};

use super::types::{AiCompressionMode, DependencyGraph, PhaseWeights, ScoredMessage};

/// Scorer for message preservation decisions.
pub struct Scorer {
    /// Optional fast model for AI-assisted scoring.
    fast_model: Option<Box<dyn Provider>>,
}

impl Scorer {
    /// Create a new scorer without AI assistance.
    pub fn new_rule_only() -> Self {
        Self { fast_model: None }
    }

    /// Create a new scorer with AI assistance.
    pub fn new_with_ai(fast_model: Box<dyn Provider>) -> Self {
        Self {
            fast_model: Some(fast_model),
        }
    }

    /// Score all messages.
    pub async fn score_all(
        &self,
        messages: &[Message],
        weights: &PhaseWeights,
        deps: &DependencyGraph,
        ai_mode: AiCompressionMode,
    ) -> Result<Vec<ScoredMessage>> {
        let mut scored: Vec<ScoredMessage> = Vec::new();

        // Phase 1: Rule-based scoring
        for (idx, msg) in messages.iter().enumerate() {
            let base_score = score_by_rules(msg, idx, weights);
            scored.push(ScoredMessage::new(idx, msg.clone(), base_score));
        }

        // Phase 2: AI-assisted scoring (optional)
        if ai_mode != AiCompressionMode::None && self.fast_model.is_some() {
            for sm in &mut scored {
                if should_ai_score(&sm.message) {
                    let ai_score = self.score_with_ai(&sm.message, ai_mode).await?;
                    sm.with_ai_score(ai_score);
                }
            }
        }

        // Phase 3: Dependency bonus
        apply_dependency_bonus(&mut scored, deps, weights.dependency_pair_bonus);

        Ok(scored)
    }

    /// Score a single message with AI assistance.
    async fn score_with_ai(&self, message: &Message, mode: AiCompressionMode) -> Result<f64> {
        if self.fast_model.is_none() {
            return Ok(0.0);
        }

        let content_preview = get_content_preview(message, 500);
        let prompt = build_ai_score_prompt(&content_preview, mode);

        // Use fast model for quick judgment
        let provider = self.fast_model.as_ref().unwrap();
        let response = provider
            .chat(crate::providers::ChatRequest {
                messages: vec![Message {
                    role: Role::User,
                    content: MessageContent::Text(prompt),
                }],
                tools: vec![],
                system: Some(AI_SCORE_SYSTEM_PROMPT.to_string()),
                think: false,
                max_tokens: 100,
                server_tools: vec![],
                enable_caching: false,
            })
            .await?;

        // Extract score from response (0-30 range)
        let score_text = extract_text_from_response(&response);
        parse_ai_score(&score_text)
    }
}

/// Rule-based scoring for a message (public for pipeline use).
pub fn score_by_rules(message: &Message, index: usize, weights: &PhaseWeights) -> f64 {
    let mut score: f64 = 10.0; // Base score

    // First message gets highest priority
    if index == 0 {
        score += weights.first_msg_bonus;
    }

    // Role-based scoring
    match message.role {
        Role::User => {
            score += weights.user_msg_bonus;
        }
        Role::Assistant => {
            score += 5.0; // Lower base for assistant messages
        }
        Role::Tool => {
            score += weights.tool_result_bonus;
        }
        Role::System => {
            score += 40.0; // System messages are important
        }
    }

    // Content-based scoring
    score += content_score(&message.content, weights);

    score
}

/// Score based on content blocks.
fn content_score(content: &MessageContent, weights: &PhaseWeights) -> f64 {
    let mut score: f64 = 0.0;

    match content {
        MessageContent::Text(text) => {
            // Check for sensitive instructions
            if contains_sensitive_instructions(text) {
                score += 50.0;
            }

            // Check for important keywords
            let keywords = [
                "决定",
                "decision",
                "重要",
                "important",
                "关键",
                "key",
                "完成",
                "done",
            ];
            for kw in keywords {
                if text.to_lowercase().contains(kw) {
                    score += 15.0;
                }
            }
        }
        MessageContent::Blocks(blocks) => {
            for block in blocks {
                match block {
                    ContentBlock::ToolUse { name, .. } => {
                        score += weights.tool_use_bonus;

                        // Critical tools get extra bonus
                        if is_critical_tool(name) {
                            score += weights.critical_tool_bonus;
                        }

                        // todo_write is very important for task tracking
                        if name == "todo_write" {
                            score += 60.0;
                        }

                        // ask contains user decisions
                        if name == "ask" {
                            score += 50.0;
                        }
                    }
                    ContentBlock::ToolResult { content, .. } => {
                        score += weights.tool_result_bonus;

                        // Preserve important results
                        if contains_sensitive_instructions(content) {
                            score += 30.0;
                        }

                        // todo_write results
                        if content.contains("TodoWrite") || content.contains("todo") {
                            score += 40.0;
                        }

                        // ask responses
                        if content.contains("AskUserQuestion") || content.contains("answer") {
                            score += 30.0;
                        }
                    }
                    ContentBlock::Thinking { thinking, .. } => {
                        // Thinking can contain key insights
                        if thinking.contains("决定")
                            || thinking.contains("问题")
                            || thinking.contains("关键")
                        {
                            score += 30.0;
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

/// Apply dependency bonus to scored messages.
fn apply_dependency_bonus(scored: &mut [ScoredMessage], deps: &DependencyGraph, bonus: f64) {
    for dep in &deps.dependencies {
        // Add bonus to ToolUse message
        if let Some(sm) = scored.get_mut(dep.tool_use_idx) {
            sm.with_dependency_bonus(bonus);
        }

        // Add bonus to ToolResult message
        if let Some(sm) = scored.get_mut(dep.tool_result_idx) {
            sm.with_dependency_bonus(bonus);
        }

        // Extra bonus for critical tools
        if dep.is_critical {
            if let Some(sm) = scored.get_mut(dep.tool_use_idx) {
                sm.with_dependency_bonus(bonus * 0.5);
            }
            if let Some(sm) = scored.get_mut(dep.tool_result_idx) {
                sm.with_dependency_bonus(bonus * 0.5);
            }
        }
    }
}

/// Check if a tool is critical (modifies state).
fn is_critical_tool(name: &str) -> bool {
    let critical_tools = ["write", "edit", "multi_edit", "bash"];
    critical_tools.contains(&name)
}

/// Check if text contains sensitive instructions.
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
        "important",
    ];
    patterns.iter().any(|p| lower.contains(p))
}

/// Check if a message should be AI-scored.
fn should_ai_score(message: &Message) -> bool {
    // Only score longer user or assistant messages
    match message.role {
        Role::User | Role::Assistant => {
            let len = estimate_content_length(&message.content);
            len > 100 // Only AI-score substantial content
        }
        _ => false,
    }
}

/// Estimate content length.
fn estimate_content_length(content: &MessageContent) -> usize {
    match content {
        MessageContent::Text(text) => text.len(),
        MessageContent::Blocks(blocks) => blocks
            .iter()
            .map(|b| match b {
                ContentBlock::Text { text } => text.len(),
                ContentBlock::ToolUse { input, .. } => input.to_string().len(),
                ContentBlock::ToolResult { content, .. } => content.len(),
                ContentBlock::Thinking { thinking, .. } => thinking.len(),
                _ => 0,
            })
            .sum(),
    }
}

/// Get content preview for AI scoring.
fn get_content_preview(message: &Message, max_len: usize) -> String {
    match &message.content {
        MessageContent::Text(text) => {
            if text.len() > max_len {
                text[..max_len].to_string() + "..."
            } else {
                text.clone()
            }
        }
        MessageContent::Blocks(blocks) => {
            let preview: Vec<String> = blocks
                .iter()
                .take(3)
                .map(|b| match b {
                    ContentBlock::Text { text } => text.chars().take(100).collect(),
                    ContentBlock::ToolUse { name, .. } => format!("[Tool: {}]", name),
                    ContentBlock::ToolResult { content, .. } => {
                        content.chars().take(100).collect::<String>() + "..."
                    }
                    _ => "...".to_string(),
                })
                .collect();
            preview.join(" | ")
        }
    }
}

/// Build prompt for AI scoring.
fn build_ai_score_prompt(content: &str, mode: AiCompressionMode) -> String {
    match mode {
        AiCompressionMode::Light => format!(
            "判断这段内容对当前任务的重要性（0-30分，0=无关，30=关键）:\n{}",
            content
        ),
        AiCompressionMode::Deep => format!(
            "深入分析这段内容的重要性，考虑：\n1. 是否包含关键决策\n2. 是否包含未完成任务\n3. 是否包含敏感指令\n输出重要性评分（0-30分）:\n{}",
            content
        ),
        AiCompressionMode::None => String::new(),
    }
}

/// Extract text from response.
fn extract_text_from_response(response: &crate::providers::ChatResponse) -> String {
    response
        .content
        .iter()
        .filter_map(|b| {
            if let ContentBlock::Text { text } = b {
                Some(text.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Parse AI score from text.
fn parse_ai_score(text: &str) -> Result<f64> {
    // Try to find a number in the text
    let text = text.trim();

    // Direct number
    if let Ok(score) = text.parse::<f64>() {
        return Ok(score.clamp(0.0, 30.0));
    }

    // Look for "评分: X" or "score: X"
    for line in text.lines() {
        let lower = line.to_lowercase();
        if lower.contains("评分") || lower.contains("score") {
            // Extract number
            let nums: Vec<f64> = line
                .split_whitespace()
                .filter_map(|s| s.parse::<f64>().ok())
                .collect();
            if let Some(score) = nums.first() {
                return Ok(score.clamp(0.0, 30.0));
            }
        }
    }

    // Default score
    Ok(10.0)
}

const AI_SCORE_SYSTEM_PROMPT: &str = r#"你是一个内容重要性评估助手。快速判断内容的重要性并输出评分。

输出要求：
- 仅输出一个数字（0-30）
- 0 = 完全不重要，可以删除
- 10 = 一般重要，可保留可删除
- 20 = 重要，建议保留
- 30 = 关键，必须保留

请直接输出评分数字。"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_by_rules_first_message() {
        let weights = PhaseWeights::balanced();
        let message = Message {
            role: Role::User,
            content: MessageContent::Text("Hello".to_string()),
        };
        let score = score_by_rules(&message, 0, &weights);
        assert!(score > 100.0); // Should have first_msg_bonus
    }

    #[test]
    fn test_score_by_rules_sensitive() {
        let weights = PhaseWeights::balanced();
        let message = Message {
            role: Role::User,
            content: MessageContent::Text("不要删除这个文件".to_string()),
        };
        let score = score_by_rules(&message, 5, &weights);
        assert!(score > 50.0); // Should have sensitive instruction bonus
    }

    #[test]
    fn test_contains_sensitive_instructions() {
        assert!(contains_sensitive_instructions("不要删除"));
        assert!(contains_sensitive_instructions("must not do this"));
        assert!(!contains_sensitive_instructions("普通文本"));
    }

    #[test]
    fn test_is_critical_tool() {
        assert!(is_critical_tool("write"));
        assert!(is_critical_tool("bash"));
        assert!(!is_critical_tool("read"));
    }

    #[test]
    fn test_parse_ai_score() {
        assert_eq!(parse_ai_score("15").unwrap(), 15.0);
        assert_eq!(parse_ai_score("评分: 20").unwrap(), 20.0);
        assert_eq!(parse_ai_score("score: 25").unwrap(), 25.0);
        assert_eq!(parse_ai_score("unknown").unwrap(), 10.0); // Default
    }
}
