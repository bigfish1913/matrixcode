//! Dynamic priority scoring for messages.
//!
//! This module implements intelligent message prioritization based on
//! multiple factors such as importance, recency, tool usage, and code content.

use crate::providers::{ContentBlock, Message, MessageContent, Role};
use std::collections::HashSet;

/// Priority score for a message (0.0 to 1.0).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct PriorityScore(pub f32);

impl PriorityScore {
    pub const MIN: f32 = 0.0;
    pub const MAX: f32 = 1.0;

    pub fn new(score: f32) -> Self {
        Self(score.clamp(Self::MIN, Self::MAX))
    }

    pub fn value(&self) -> f32 {
        self.0
    }

    pub fn is_high(&self) -> bool {
        self.0 >= 0.7
    }

    pub fn is_medium(&self) -> bool {
        self.0 >= 0.4 && self.0 < 0.7
    }

    pub fn is_low(&self) -> bool {
        self.0 < 0.4
    }
}

/// Factors that contribute to message priority.
#[derive(Debug, Clone, Default)]
pub struct PriorityFactors {
    /// Message contains decisions or choices
    pub has_decision: bool,
    /// Message contains errors or failures
    pub has_error: bool,
    /// Message contains tool calls
    pub has_tool_use: bool,
    /// Message contains code blocks
    pub has_code: bool,
    /// Message contains important keywords
    pub has_keywords: bool,
    /// Message is from user (usually higher priority)
    pub is_user_message: bool,
    /// Message position in conversation (normalized 0-1)
    pub position_weight: f32,
    /// Message length factor
    pub length_factor: f32,
    /// Number of important entities mentioned (files, functions, etc.)
    pub entity_count: usize,
}

/// Weights for different priority factors.
#[derive(Debug, Clone)]
pub struct PriorityWeights {
    pub decision_weight: f32,
    pub error_weight: f32,
    pub tool_weight: f32,
    pub code_weight: f32,
    pub keyword_weight: f32,
    pub user_message_weight: f32,
    pub recency_weight: f32,
    pub length_weight: f32,
    pub entity_weight: f32,
}

impl Default for PriorityWeights {
    fn default() -> Self {
        Self {
            decision_weight: 0.2,   // High importance
            error_weight: 0.15,     // High importance
            tool_weight: 0.15,     // High importance
            code_weight: 0.1,       // Medium importance
            keyword_weight: 0.1,    // Medium importance
            user_message_weight: 0.1, // Medium importance
            recency_weight: 0.1,    // Medium importance
            length_weight: 0.05,    // Low importance
            entity_weight: 0.05,    // Low importance
        }
    }
}

/// Dynamic priority scorer.
pub struct PriorityScorer {
    weights: PriorityWeights,
    important_keywords: HashSet<String>,
}

impl Default for PriorityScorer {
    fn default() -> Self {
        Self::new(PriorityWeights::default())
    }
}

impl PriorityScorer {
    pub fn new(weights: PriorityWeights) -> Self {
        let important_keywords = Self::build_keyword_set();
        Self {
            weights,
            important_keywords,
        }
    }

    /// Build a set of important keywords that indicate high priority.
    fn build_keyword_set() -> HashSet<String> {
        let keywords = [
            // Decision keywords
            "important", "critical", "essential", "必须", "重要",
            "决定", "选择", "decided", "chose", "selected",
            // Action keywords
            "fix", "解决", "修复", "implement", "实现", "create", "创建",
            // Error keywords
            "error", "错误", "failed", "失败", "exception", "异常",
            // Success keywords
            "success", "成功", "completed", "完成", "done", "完成",
            // Requirement keywords
            "requirement", "需求", "spec", "规范", "constraint", "约束",
        ];

        keywords.iter().map(|s| s.to_lowercase()).collect()
    }

    /// Extract priority factors from a message.
    pub fn extract_factors(message: &Message, position: usize, total: usize) -> PriorityFactors {
        let mut factors = PriorityFactors::default();

        // Check role
        factors.is_user_message = matches!(message.role, Role::User);

        // Position weight (0 = oldest, 1 = newest)
        factors.position_weight = if total > 1 {
            position as f32 / (total - 1) as f32
        } else {
            1.0
        };

        // Analyze content
        match &message.content {
            MessageContent::Text(text) => {
                Self::analyze_text(text, &mut factors);
                factors.length_factor = Self::calculate_length_factor(text.len());
            }
            MessageContent::Blocks(blocks) => {
                let mut combined_text = String::new();
                for block in blocks {
                    match block {
                        ContentBlock::Text { text } => {
                            combined_text.push_str(text);
                            combined_text.push(' ');
                        }
                        ContentBlock::ToolUse { name, input, .. } => {
                            factors.has_tool_use = true;
                            combined_text.push_str(name);
                            combined_text.push(' ');
                            combined_text.push_str(&input.to_string());
                            combined_text.push(' ');
                        }
                        ContentBlock::ToolResult { content, .. } => {
                            combined_text.push_str(content);
                            combined_text.push(' ');
                            if content.contains("error") || content.contains("failed") {
                                factors.has_error = true;
                            }
                        }
                        ContentBlock::Thinking { thinking, .. } => {
                            combined_text.push_str(thinking);
                            combined_text.push(' ');
                        }
                        _ => {}
                    }
                }
                Self::analyze_text(&combined_text, &mut factors);
                factors.length_factor = Self::calculate_length_factor(combined_text.len());
            }
        }

        factors
    }

    /// Analyze text content for priority indicators.
    fn analyze_text(text: &str, factors: &mut PriorityFactors) {
        let lower = text.to_lowercase();

        // Check for decision indicators
        if lower.contains("决定") || lower.contains("decided") || lower.contains("chose")
            || lower.contains("选择") || lower.contains("selected")
        {
            factors.has_decision = true;
        }

        // Check for error indicators
        if lower.contains("error") || lower.contains("错误") || lower.contains("failed")
            || lower.contains("失败") || lower.contains("exception") || lower.contains("异常")
        {
            factors.has_error = true;
        }

        // Check for code blocks
        if text.contains("```") || text.contains("fn ") || text.contains("function ")
            || text.contains("class ") || text.contains("impl ")
        {
            factors.has_code = true;
        }

        // Check for important keywords
        factors.has_keywords = lower.split_whitespace().any(|word| {
            word.trim_matches(|c: char| c.is_ascii_punctuation()).eq_ignore_ascii_case("important")
                || word.eq_ignore_ascii_case("critical")
                || word.eq_ignore_ascii_case("essential")
                || word.eq_ignore_ascii_case("必须")
                || word.eq_ignore_ascii_case("重要")
        });

        // Count entities (files, functions, etc.)
        factors.entity_count = Self::count_entities(text);
    }

    /// Count important entities in text.
    fn count_entities(text: &str) -> usize {
        let mut count = 0;

        // Count file references (e.g., "src/main.rs", "package.json")
        if text.contains(".rs") || text.contains(".py") || text.contains(".js")
            || text.contains(".ts") || text.contains(".json") || text.contains(".toml")
        {
            count += 1;
        }

        // Count function references (e.g., "fn main", "function test")
        for pattern in &["fn ", "function ", "def ", "class ", "impl "] {
            if text.contains(pattern) {
                count += 1;
            }
        }

        // Count API endpoints
        if text.contains("GET /") || text.contains("POST /") || text.contains("PUT /")
            || text.contains("DELETE /")
        {
            count += 1;
        }

        count
    }

    /// Calculate length factor (longer messages may be more important).
    fn calculate_length_factor(len: usize) -> f32 {
        // Normalize length: 0-100 chars = 0.0-1.0
        // Cap at 1.0 for very long messages
        (len as f32 / 100.0).min(1.0)
    }

    /// Calculate priority score for a message.
    pub fn score(&self, message: &Message, position: usize, total: usize) -> PriorityScore {
        let factors = Self::extract_factors(message, position, total);
        self.score_from_factors(&factors)
    }

    /// Calculate priority score from factors.
    pub fn score_from_factors(&self, factors: &PriorityFactors) -> PriorityScore {
        let mut score = 0.0;

        if factors.has_decision {
            score += self.weights.decision_weight;
        }
        if factors.has_error {
            score += self.weights.error_weight;
        }
        if factors.has_tool_use {
            score += self.weights.tool_weight;
        }
        if factors.has_code {
            score += self.weights.code_weight;
        }
        if factors.has_keywords {
            score += self.weights.keyword_weight;
        }
        if factors.is_user_message {
            score += self.weights.user_message_weight;
        }

        // Add recency weight (more recent = higher priority)
        score += factors.position_weight * self.weights.recency_weight;

        // Add length weight
        score += factors.length_factor * self.weights.length_weight;

        // Add entity weight
        score += (factors.entity_count as f32 * 0.02).min(self.weights.entity_weight);

        PriorityScore::new(score)
    }

    /// Get priority level description.
    pub fn level(score: PriorityScore) -> &'static str {
        if score.is_high() {
            "High"
        } else if score.is_medium() {
            "Medium"
        } else {
            "Low"
        }
    }
}

/// Message with priority score.
#[derive(Debug, Clone)]
pub struct ScoredMessage {
    pub message: Message,
    pub score: PriorityScore,
    pub position: usize,
    pub factors: PriorityFactors,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_score_clamping() {
        assert_eq!(PriorityScore::new(-1.0).value(), 0.0);
        assert_eq!(PriorityScore::new(2.0).value(), 1.0);
        assert_eq!(PriorityScore::new(0.5).value(), 0.5);
    }

    #[test]
    fn test_priority_levels() {
        let high = PriorityScore::new(0.8);
        assert!(high.is_high());
        assert!(!high.is_medium());
        assert!(!high.is_low());

        let medium = PriorityScore::new(0.5);
        assert!(!medium.is_high());
        assert!(medium.is_medium());
        assert!(!medium.is_low());

        let low = PriorityScore::new(0.2);
        assert!(!low.is_high());
        assert!(!low.is_medium());
        assert!(low.is_low());
    }

    #[test]
    fn test_extract_factors_user_message() {
        let msg = Message {
            role: Role::User,
            content: MessageContent::Text("Hello".to_string()),
        };
        let factors = PriorityScorer::extract_factors(&msg, 0, 1);
        assert!(factors.is_user_message);
    }

    #[test]
    fn test_extract_factors_decision() {
        let msg = Message {
            role: Role::Assistant,
            content: MessageContent::Text("I decided to use Rust.".to_string()),
        };
        let factors = PriorityScorer::extract_factors(&msg, 0, 1);
        assert!(factors.has_decision);
    }

    #[test]
    fn test_extract_factors_error() {
        let msg = Message {
            role: Role::Assistant,
            content: MessageContent::Text("The operation failed with error.".to_string()),
        };
        let factors = PriorityScorer::extract_factors(&msg, 0, 1);
        assert!(factors.has_error);
    }

    #[test]
    fn test_extract_factors_code() {
        let msg = Message {
            role: Role::Assistant,
            content: MessageContent::Text("Here's the code:\n```rust\nfn main() {}\n```".to_string()),
        };
        let factors = PriorityScorer::extract_factors(&msg, 0, 1);
        assert!(factors.has_code);
    }

    #[test]
    fn test_extract_factors_tool_use() {
        let msg = Message {
            role: Role::Assistant,
            content: MessageContent::Blocks(vec![ContentBlock::ToolUse {
                id: "tool_1".to_string(),
                name: "bash".to_string(),
                input: serde_json::json!({"command": "ls"}),
            }]),
        };
        let factors = PriorityScorer::extract_factors(&msg, 0, 1);
        assert!(factors.has_tool_use);
    }

    #[test]
    fn test_score_calculation() {
        let scorer = PriorityScorer::default();
        
        // High priority message
        let msg = Message {
            role: Role::User,
            content: MessageContent::Text("I decided to use Rust for this important project. The error was fixed.".to_string()),
        };
        let score = scorer.score(&msg, 9, 10);
        assert!(score.is_high());
        
        // Low priority message
        let msg = Message {
            role: Role::Assistant,
            content: MessageContent::Text("ok".to_string()),
        };
        let score = scorer.score(&msg, 0, 10);
        assert!(score.is_low());
    }

    #[test]
    fn test_position_weight() {
        let scorer = PriorityScorer::default();
        
        // Old message (position 0)
        let msg = Message {
            role: Role::User,
            content: MessageContent::Text("Test".to_string()),
        };
        let factors1 = PriorityScorer::extract_factors(&msg, 0, 10);
        assert!(factors1.position_weight < 0.2);
        
        // New message (position 9)
        let factors2 = PriorityScorer::extract_factors(&msg, 9, 10);
        assert!(factors2.position_weight > 0.8);
    }

    #[test]
    fn test_entity_counting() {
        let text = "In src/main.rs, we have fn main() and fn helper()";
        let count = PriorityScorer::count_entities(text);
        assert!(count >= 2); // At least .rs and fn mentions
    }
}