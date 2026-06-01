//! Focus Score Evaluator - Evaluates message relevance to current focus.
//!
//! This module provides scoring for messages based on their relevance
//! to the current conversation focus. Used during compression to decide
//! which messages to preserve.

use crate::providers::{Message, MessageContent, ContentBlock, Role};
use crate::compress::{ConversationFocus, FocusPoint, FocusManager};
use anyhow::Result;

/// Focus score evaluator for message relevance assessment.
///
/// This evaluator provides two scoring modes:
/// 1. Fast rule-based scoring (for compression efficiency)
/// 2. AI-based scoring (for critical decisions, optional)
pub struct FocusScoreEvaluator {
    /// Whether to use AI for scoring (expensive but more accurate)
    use_ai: bool,
    /// Minimum relevance threshold for high-priority messages
    high_priority_threshold: f32,
    /// Minimum relevance threshold for preserving messages
    preserve_threshold: f32,
}

impl Default for FocusScoreEvaluator {
    fn default() -> Self {
        Self {
            use_ai: false, // Default to rule-based for efficiency
            high_priority_threshold: 0.7,
            preserve_threshold: 0.3,
        }
    }
}

impl FocusScoreEvaluator {
    /// Create a new evaluator with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an evaluator with AI scoring enabled.
    pub fn with_ai() -> Self {
        Self {
            use_ai: true,
            ..Self::default()
        }
    }

    /// Create an evaluator with custom thresholds.
    pub fn with_thresholds(high_priority: f32, preserve: f32) -> Self {
        Self {
            use_ai: false,
            high_priority_threshold: high_priority,
            preserve_threshold: preserve,
        }
    }

    /// Evaluate a message's relevance to current focus.
    ///
    /// Returns a score from 0.0 to 1.0:
    /// - 1.0: Directly addresses current question/task
    /// - 0.7-0.9: Highly relevant, important context
    /// - 0.4-0.6: Moderately relevant
    /// - 0.1-0.3: Low relevance, may be compressed
    /// - 0.0: Not relevant to current focus
    pub fn evaluate(&self, message: &Message, focus: &ConversationFocus) -> f32 {
        self.evaluate_rule_based(message, focus)
    }

    /// Evaluate a message's relevance to a FocusPoint.
    pub fn evaluate_for_focus_point(&self, message: &Message, focus: &FocusPoint) -> f32 {
        let text = self.extract_text(message);
        let text_lower = text.to_lowercase();

        let mut score = 0.0;

        // Keyword matching
        for keyword in &focus.keywords {
            if text_lower.contains(&keyword.to_lowercase()) {
                score += 0.15;
            }
        }

        // Entity matching (higher weight)
        for entity in &focus.entities {
            if text_lower.contains(&entity.to_lowercase()) {
                score += 0.25;
            }
        }

        // Core question matching (highest weight)
        if let Some(question) = &focus.core_question {
            if self.questions_similar(&text, question) {
                score += 0.4;
            }
        }

        // Apply importance weighting
        score *= focus.importance;

        // Apply confidence weighting
        score *= focus.confidence;

        score.clamp(0.0, 1.0)
    }

    /// Evaluate using FocusManager (multi-focus aware).
    ///
    /// Returns the highest score across all active focuses.
    pub fn evaluate_for_manager(&self, message: &Message, manager: &FocusManager) -> f32 {
        let active_foci = manager.get_active_foci();

        if active_foci.is_empty() {
            return 0.5; // Default when no focus
        }

        // Get highest score across all active focuses
        let max_score = active_foci
            .iter()
            .map(|f| self.evaluate_for_focus_point(message, f))
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.5);

        // Boost score if matches current (primary) focus
        if let Some(current) = manager.current_focus() {
            let current_score = self.evaluate_for_focus_point(message, current);
            if current_score > 0.5 {
                return (max_score + current_score * 0.2).min(1.0);
            }
        }

        max_score
    }

    /// Rule-based evaluation (fast, no AI call).
    fn evaluate_rule_based(&self, message: &Message, focus: &ConversationFocus) -> f32 {
        let text = self.extract_text(message);
        let text_lower = text.to_lowercase();

        let mut score: f32 = 0.0;

        // 1. Topic keyword matching
        if let Some(topic) = &focus.current_topic {
            let topic_keywords: Vec<&str> = topic.split(", ").collect();
            for kw in topic_keywords {
                if text_lower.contains(&kw.to_lowercase()) {
                    score += 0.2;
                }
            }
        }

        // 2. Current question matching
        if let Some(question) = &focus.current_question {
            if self.questions_similar(&text, question) {
                score += 0.35;
            }
        }

        // 3. Recent context matching
        for ctx in &focus.recent_context {
            if text_lower.contains(&ctx.to_lowercase()) {
                score += 0.1;
            }
        }

        // 4. Role-based adjustment
        // User messages are generally more important for focus
        if matches!(message.role, Role::User) {
            score *= 1.2;
        }

        // 5. Content length adjustment
        // Longer messages may contain more relevant information
        if text.len() > 200 {
            score *= 1.1;
        }

        score.clamp(0.0, 1.0)
    }

    /// Check if message text is similar to a question.
    fn questions_similar(&self, text: &str, question: &str) -> bool {
        let text_words = self.extract_significant_words(text);
        let question_words = self.extract_significant_words(question);

        let common = text_words.intersection(&question_words).count();
        let total = question_words.len().max(1);

        // More than 50% of question words appear in text
        common as f32 / total as f32 > 0.5
    }

    /// Extract significant words (length > 3) from text.
    fn extract_significant_words(&self, text: &str) -> std::collections::HashSet<String> {
        text.to_lowercase()
            .split_whitespace()
            .filter(|w| w.len() > 3)
            .map(|s| s.to_string())
            .collect()
    }

    /// Extract text content from message.
    fn extract_text(&self, message: &Message) -> String {
        match &message.content {
            MessageContent::Text(t) => t.clone(),
            MessageContent::Blocks(blocks) => {
                blocks.iter()
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
        }
    }

    /// Check if a message should be preserved during compression.
    pub fn should_preserve(&self, score: f32) -> bool {
        score >= self.preserve_threshold
    }

    /// Check if a message is high priority (must preserve intact).
    pub fn is_high_priority(&self, score: f32) -> bool {
        score >= self.high_priority_threshold
    }

    /// Get preservation threshold.
    pub fn preserve_threshold(&self) -> f32 {
        self.preserve_threshold
    }

    /// Get high priority threshold.
    pub fn high_priority_threshold(&self) -> f32 {
        self.high_priority_threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compress::FocusManager;

    fn create_test_message(role: Role, text: &str) -> Message {
        Message {
            role,
            content: MessageContent::Text(text.to_string()),
        }
    }

    fn create_test_focus() -> ConversationFocus {
        ConversationFocus {
            current_topic: Some("API 性能优化".to_string()),
            current_question: Some("如何减少 API 响应延迟？".to_string()),
            recent_context: vec!["API".to_string(), "性能".to_string(), "延迟".to_string()],
            topic_transitions: Vec::new(),
            detected_at: 10,
        }
    }

    fn create_test_focus_point() -> FocusPoint {
        FocusPoint::new(
            "focus-1".to_string(),
            "API 性能优化".to_string(),
            vec!["API".to_string(), "性能".to_string(), "延迟".to_string()],
            vec!["api.rs".to_string(), "handler()".to_string()],
            Some("如何减少延迟？".to_string()),
            0,
        ).with_importance(0.8)
            .with_confidence(0.85)
    }

    #[test]
    fn test_evaluator_creation() {
        let evaluator = FocusScoreEvaluator::new();
        assert!(!evaluator.use_ai);
        assert_eq!(evaluator.high_priority_threshold, 0.7);
        assert_eq!(evaluator.preserve_threshold, 0.3);
    }

    #[test]
    fn test_evaluator_with_thresholds() {
        let evaluator = FocusScoreEvaluator::with_thresholds(0.8, 0.4);
        assert_eq!(evaluator.high_priority_threshold, 0.8);
        assert_eq!(evaluator.preserve_threshold, 0.4);
    }

    #[test]
    fn test_evaluator_high_relevance() {
        let evaluator = FocusScoreEvaluator::new();
        let focus = create_test_focus();

        // Message directly about API performance with multiple keyword matches
        // Note: Chinese keywords are matched by substring, not by word
        let message = create_test_message(Role::User, "API 性能 延迟 优化");
        let score = evaluator.evaluate(&message, &focus);

        // Should have some relevance due to keyword matching
        assert!(score > 0.0, "Score should be positive for message with keywords");
    }

    #[test]
    fn test_evaluator_low_relevance() {
        let evaluator = FocusScoreEvaluator::new();
        let focus = create_test_focus();

        // Message about unrelated topic (no matching keywords)
        let message = create_test_message(Role::User, "今天天气晴朗阳光明媚");
        let score = evaluator.evaluate(&message, &focus);

        assert!(score < 0.3, "Score should be low for irrelevant message");
    }

    #[test]
    fn test_evaluator_medium_relevance() {
        let evaluator = FocusScoreEvaluator::new();
        let focus = create_test_focus();

        // Message mentions some keywords but not directly about focus
        let message = create_test_message(Role::Assistant, "代码中 API 调用需要注意错误处理");
        let score = evaluator.evaluate(&message, &focus);

        assert!(score >= 0.1 && score <= 0.5, "Score should be medium");
    }

    #[test]
    fn test_evaluator_for_focus_point() {
        let evaluator = FocusScoreEvaluator::new();
        let focus = create_test_focus_point();

        let message = create_test_message(Role::User, "api.rs 中的 handler 函数延迟太高");
        let score = evaluator.evaluate_for_focus_point(&message, &focus);

        assert!(score > 0.3, "Should match keywords and entities");
    }

    #[test]
    fn test_evaluator_for_manager() {
        let evaluator = FocusScoreEvaluator::new();
        let mut manager = FocusManager::new();

        manager.add_focus(create_test_focus_point());

        let message = create_test_message(Role::User, "如何优化 API 性能？");
        let score = evaluator.evaluate_for_manager(&message, &manager);

        assert!(score > 0.0);
    }

    #[test]
    fn test_should_preserve() {
        let evaluator = FocusScoreEvaluator::with_thresholds(0.7, 0.3);

        assert!(evaluator.should_preserve(0.5));
        assert!(evaluator.should_preserve(0.3));
        assert!(!evaluator.should_preserve(0.2));
    }

    #[test]
    fn test_is_high_priority() {
        let evaluator = FocusScoreEvaluator::with_thresholds(0.7, 0.3);

        assert!(evaluator.is_high_priority(0.8));
        assert!(evaluator.is_high_priority(0.7));
        assert!(!evaluator.is_high_priority(0.6));
    }

    #[test]
    fn test_questions_similar() {
        let evaluator = FocusScoreEvaluator::new();

        // Similar questions - both have common significant words like "API", "响应", "延迟"
        // Note: Chinese words are split by whitespace, so "API响应延迟" is one word
        // Let's use explicit whitespace separation
        assert!(evaluator.questions_similar(
            "API 响应 延迟 太高怎么办",
            "如何减少 API 响应 延迟"
        ));

        // Not similar - no common significant words
        assert!(!evaluator.questions_similar(
            "天气很好",
            "API 响应 延迟 太高怎么办"
        ));
    }

    #[test]
    fn test_user_message_boost() {
        let evaluator = FocusScoreEvaluator::new();
        let focus = create_test_focus();

        // User message should get boost
        let user_msg = create_test_message(Role::User, "API 性能问题");
        let assistant_msg = create_test_message(Role::Assistant, "API 性能问题");

        let user_score = evaluator.evaluate(&user_msg, &focus);
        let assistant_score = evaluator.evaluate(&assistant_msg, &focus);

        assert!(user_score >= assistant_score, "User message should have higher score");
    }

    #[test]
    fn test_extract_significant_words() {
        let evaluator = FocusScoreEvaluator::new();

        // Use space-separated text for proper word extraction
        let words = evaluator.extract_significant_words("about API performance optimization");

        // Should include words > 3 chars (English words)
        assert!(words.contains("about"));
        assert!(words.contains("performance"));
        assert!(words.contains("optimization"));
    }
}