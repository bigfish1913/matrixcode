//! Focus tracking to ensure compression preserves current conversation focus.
//!
//! Problem: After compression, AI may focus on old topics instead of the latest question.
//! Solution: Track conversation focus and ensure recent context is prioritized.

use crate::memory::ExtractedKeywords;
use crate::providers::{ContentBlock, Message, MessageContent, Role};
use super::focus_config::FocusTrackerConfig;

/// Represents the current focus of the conversation.
#[derive(Debug, Clone)]
pub struct ConversationFocus {
    /// Current topic being discussed
    pub current_topic: Option<String>,
    /// Current question or task
    pub current_question: Option<String>,
    /// Recent context snippets (last N messages key points)
    pub recent_context: Vec<String>,
    /// Topic transitions (when topic changed)
    pub topic_transitions: Vec<TopicTransition>,
    /// Timestamp of focus detection
    pub detected_at: usize, // Message index
}

/// Records when the conversation topic changed.
#[derive(Debug, Clone)]
pub struct TopicTransition {
    pub from_topic: String,
    pub to_topic: String,
    pub message_index: usize,
    pub transition_keyword: String,
}

/// Focus tracker that monitors conversation flow.
pub struct FocusTracker {
    /// Configuration (replaces hardcoded values)
    config: FocusTrackerConfig,
}

impl Default for FocusTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl FocusTracker {
    /// Create new focus tracker with default configuration
    pub fn new() -> Self {
        Self {
            config: FocusTrackerConfig::default(),
        }
    }

    /// Create focus tracker with custom configuration
    pub fn with_config(config: FocusTrackerConfig) -> Self {
        Self { config }
    }

    /// Get configuration reference
    pub fn config(&self) -> &FocusTrackerConfig {
        &self.config
    }

    /// Get mutable configuration reference
    pub fn config_mut(&mut self) -> &mut FocusTrackerConfig {
        &mut self.config
    }

    /// Set current keywords from AI extraction (real-time, not persisted).
    ///
    /// These keywords are used for focus tracking in the current conversation.
    pub fn set_current_keywords(&mut self, keywords: &ExtractedKeywords) {
        self.config.set_keywords(keywords);
    }

    /// Merge additional keywords into current keywords.
    pub fn merge_keywords(&mut self, additional: &ExtractedKeywords) {
        self.config.merge_keywords(additional);
    }

    /// Clear current keywords (start fresh for new conversation).
    pub fn clear_keywords(&mut self) {
        self.config.clear_keywords();
    }

    /// Detect current focus from recent messages.
    pub fn detect_focus(&self, messages: &[Message]) -> ConversationFocus {
        self.detect_focus_with_window(messages, self.config.focus_window_size)
    }

    /// Detect current focus with custom window size
    pub fn detect_focus_with_window(&self, messages: &[Message], window_size: usize) -> ConversationFocus {
        let recent_start = messages.len().saturating_sub(window_size);
        let recent_messages = &messages[recent_start..];

        let mut focus = ConversationFocus {
            current_topic: None,
            current_question: None,
            recent_context: Vec::new(),
            topic_transitions: Vec::new(),
            detected_at: messages.len().saturating_sub(1),
        };

        // Extract recent context
        for (_idx, msg) in recent_messages.iter().enumerate().rev() {
            if let Some(key_point) = self.extract_key_point(msg) {
                focus.recent_context.push(key_point);
                if focus.recent_context.len() >= self.config.max_recent_context_count {
                    break;
                }
            }
        }

        // Find current question/task from last few user messages
        for msg in recent_messages.iter().rev() {
            if matches!(msg.role, Role::User) {
                if let Some(question) = self.extract_current_question(msg) {
                    focus.current_question = Some(question);
                    break;
                }
            }
        }

        // Detect topic transitions in full conversation
        focus.topic_transitions = self.detect_topic_transitions(messages);

        // Determine current topic from most recent transition
        if let Some(last_transition) = focus.topic_transitions.last() {
            focus.current_topic = Some(last_transition.to_topic.clone());
        } else {
            // Extract topic from first substantial message
            focus.current_topic = self.extract_initial_topic(messages);
        }

        focus
    }

    /// Extract key point from a message.
    fn extract_key_point(&self, message: &Message) -> Option<String> {
        match &message.content {
            MessageContent::Text(text) => {
                // Extract first sentence or key phrase
                let sentences: Vec<&str> = text.split(|c| c == '.' || c == '。' || c == '\n')
                    .filter(|s| s.trim().len() > self.config.min_substantial_text_length)
                    .collect();

                sentences.first().map(|s| s.trim().to_string())
            }
            MessageContent::Blocks(blocks) => {
                for block in blocks {
                    if let ContentBlock::Text { text } = block {
                        if text.len() > self.config.min_substantial_text_length {
                            return Some(text.split('\n').next()?.trim().to_string());
                        }
                    }
                }
                None
            }
        }
    }

    /// Extract current question or task from a message.
    fn extract_current_question(&self, message: &Message) -> Option<String> {
        match &message.content {
            MessageContent::Text(text) => {
                // Check if it's a question using keywords
                if self.config.matches_question(text) {
                    // Extract the question (up to configured max length)
                    let question = text.chars()
                        .take(self.config.max_question_extract_length)
                        .collect::<String>();
                    return Some(question.trim().to_string());
                }

                // Check if it's a task request using keywords
                if self.config.matches_task(text) {
                    let task = text.chars()
                        .take(self.config.max_question_extract_length)
                        .collect::<String>();
                    return Some(task.trim().to_string());
                }

                // Just return first substantial sentence
                if text.len() > self.config.min_substantial_text_length * 2 {
                    Some(text.chars()
                        .take(self.config.max_question_extract_length)
                        .collect::<String>()
                        .trim()
                        .to_string())
                } else {
                    None
                }
            }
            MessageContent::Blocks(blocks) => {
                for block in blocks {
                    if let ContentBlock::Text { text } = block {
                        if text.len() > self.config.min_substantial_text_length {
                            return Some(text.chars()
                                .take(self.config.max_question_extract_length)
                                .collect::<String>());
                        }
                    }
                }
                None
            }
        }
    }

    /// Detect topic transitions throughout conversation.
    fn detect_topic_transitions(&self, messages: &[Message]) -> Vec<TopicTransition> {
        let mut transitions = Vec::new();
        let mut prev_topic = String::new();

        // Get transition keywords
        let transition_keywords = self.config.transition_keywords();

        for (idx, msg) in messages.iter().enumerate() {
            if matches!(msg.role, Role::User) {
                let text = match &msg.content {
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
                            .join(" ")
                    }
                };

                let lower = text.to_lowercase();

                // Check for transition keywords
                for keyword in &transition_keywords {
                    if lower.contains(&keyword.to_lowercase()) {
                        // Extract new topic
                        let new_topic = self.extract_topic_from_message(&text);

                        if !prev_topic.is_empty() && new_topic != prev_topic {
                            transitions.push(TopicTransition {
                                from_topic: prev_topic.clone(),
                                to_topic: new_topic.clone(),
                                message_index: idx,
                                transition_keyword: keyword.clone(),
                            });
                        }

                        prev_topic = new_topic;
                        break;
                    }
                }

                // If no transition, just update topic if it's first message
                if prev_topic.is_empty() {
                    prev_topic = self.extract_topic_from_message(&text);
                }
            }
        }

        transitions
    }

    /// Extract topic from a message (keyword extraction).
    fn extract_topic_from_message(&self, text: &str) -> String {
        // Find matching tech keywords
        let found = self.config.find_tech_keywords(text);

        if found.is_empty() {
            // Extract first N words (configured)
            text.split_whitespace()
                .take(self.config.fallback_topic_word_count)
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            found.join(", ")
        }
    }

    /// Extract initial topic from first substantial message.
    fn extract_initial_topic(&self, messages: &[Message]) -> Option<String> {
        for msg in messages {
            if matches!(msg.role, Role::User) {
                let text = match &msg.content {
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
                            .join(" ")
                    }
                };

                if text.len() > self.config.min_substantial_text_length {
                    return Some(self.extract_topic_from_message(&text));
                }
            }
        }
        None
    }

    /// Calculate focus score for a message (how relevant to current focus).
    pub fn focus_score(&self, message: &Message, focus: &ConversationFocus) -> f32 {
        let mut score = 0.0;

        // Get message text
        let text = match &message.content {
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
                    .join(" ")
            }
        };

        let lower = text.to_lowercase();

        // Check if message matches current topic
        if let Some(topic) = &focus.current_topic {
            let topic_keywords: Vec<&str> = topic.split(", ").collect();
            for kw in topic_keywords {
                if lower.contains(kw) {
                    score += 0.3;
                }
            }
        }

        // Check if message matches current question keywords
        if let Some(question) = &focus.current_question {
            let question_lower = question.to_lowercase();
            let words: Vec<&str> = question_lower.split_whitespace().collect();
            for word in words {
                if word.len() > 3 && lower.contains(word) {
                    score += 0.1;
                }
            }
        }

        // Check if message is in recent context
        if let Some(key_point) = self.extract_key_point(message) {
            if focus.recent_context.contains(&key_point) {
                score += 0.5;
            }
        }

        // Apply configured boost and cap
        score = (score * self.config.focus_score_boost).min(self.config.max_focus_score);

        score
    }

    /// Calculate focus score using real-time extracted keywords.
    ///
    /// This method uses keywords extracted by AI (via UnifiedExtractor)
    /// for more accurate focus scoring, instead of relying on fallback presets.
    ///
    /// # Arguments
    /// * `message` - Message to score.
    /// * `focus` - Current conversation focus.
    ///
    /// # Returns
    /// Focus relevance score (0.0 to 1.0).
    pub fn focus_score_with_keywords(&self, message: &Message, focus: &ConversationFocus) -> f32 {
        let keywords = self.config.get_keywords();

        // Get message text
        let text = match &message.content {
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
                    .join(" ")
            }
        };

        let lower = text.to_lowercase();
        let mut score = 0.0;

        // Use real-time keywords if available
        if let Some(kw) = keywords {
            // Check transition keywords (topic change indicators)
            for keyword in &kw.transition {
                if lower.contains(&keyword.to_lowercase()) {
                    score += 0.2; // Topic transition detection
                }
            }

            // Check question keywords (current question indicators)
            for keyword in &kw.question {
                if lower.contains(&keyword.to_lowercase()) {
                    score += 0.3; // Current question relevance
                }
            }

            // Check task keywords (current task indicators)
            for keyword in &kw.task {
                if lower.contains(&keyword.to_lowercase()) {
                    score += 0.25; // Current task relevance
                }
            }

            // Check tech keywords (domain relevance)
            for keyword in &kw.tech {
                if lower.contains(&keyword.to_lowercase()) {
                    score += 0.15; // Technical domain match
                }
            }
        } else {
            // No real-time keywords: use traditional focus scoring
            return self.focus_score(message, focus);
        }

        // Also consider focus context
        if let Some(topic) = &focus.current_topic {
            let topic_keywords: Vec<&str> = topic.split(", ").collect();
            for kw in topic_keywords {
                if lower.contains(&kw.to_lowercase()) {
                    score += 0.1;
                }
            }
        }

        if let Some(question) = &focus.current_question {
            let question_lower = question.to_lowercase();
            for word in question_lower.split_whitespace() {
                if word.len() > 3 && lower.contains(word) {
                    score += 0.05;
                }
            }
        }

        // Apply configured boost and cap
        score = (score * self.config.focus_score_boost).min(self.config.max_focus_score);

        score
    }

    /// Create a focus message to inject into compressed conversation.
    pub fn create_focus_message(&self, focus: &ConversationFocus) -> Message {
        let mut content_parts = Vec::new();

        // Add topic
        if let Some(topic) = &focus.current_topic {
            content_parts.push(format!("当前话题: {}", topic));
        }

        // Add current question
        if let Some(question) = &focus.current_question {
            content_parts.push(format!("当前问题/任务: {}", question));
        }

        // Add recent context summary
        if !focus.recent_context.is_empty() {
            content_parts.push(format!("最近上下文摘要: {}", focus.recent_context.join(" | ")));
        }

        // Add topic transitions if any
        if !focus.topic_transitions.is_empty() {
            let transitions: Vec<String> = focus.topic_transitions.iter()
                .map(|t| format!("{} -> {}", t.from_topic, t.to_topic))
                .collect();
            content_parts.push(format!("话题转换历史: {}", transitions.join(", ")));
        }

        let content = if content_parts.is_empty() {
            "[焦点追踪系统初始化]".to_string()
        } else {
            format!("【焦点上下文】\n{}\n请基于上述焦点继续对话。", content_parts.join("\n"))
        };

        Message {
            role: Role::System,
            content: MessageContent::Text(content),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_focus_tracker_creation() {
        let tracker = FocusTracker::new();
        assert!(tracker.config().validate());
    }

    #[test]
    fn test_focus_tracker_with_custom_config() {
        let config = FocusTrackerConfig::simple_conversation();
        let tracker = FocusTracker::with_config(config);
        assert_eq!(tracker.config().focus_window_size, 5);
    }

    #[test]
    fn test_detect_focus() {
        let tracker = FocusTracker::new();
        let messages = vec![
            Message {
                role: Role::User,
                content: MessageContent::Text("如何优化 Rust 性能？".to_string()),
            },
            Message {
                role: Role::Assistant,
                content: MessageContent::Text("我来帮你优化 Rust 代码性能。".to_string()),
            },
            Message {
                role: Role::User,
                content: MessageContent::Text("帮我实现一个压缩算法".to_string()),
            },
        ];

        let focus = tracker.detect_focus(&messages);
        assert!(focus.current_question.is_some());
        assert_eq!(focus.topic_transitions.len(), 0);
    }

    #[test]
    fn test_focus_score() {
        let tracker = FocusTracker::new();
        let messages = vec![
            Message {
                role: Role::User,
                content: MessageContent::Text("如何优化 Rust 性能？".to_string()),
            },
        ];

        let focus = tracker.detect_focus(&messages);

        let relevant_message = Message {
            role: Role::Assistant,
            content: MessageContent::Text("Rust 性能优化的关键是...".to_string()),
        };

        let score = tracker.focus_score(&relevant_message, &focus);
        assert!(score > 0.0);
    }

    #[test]
    fn test_keywords_integration() {
        let mut tracker = FocusTracker::new();

        // Set keywords from AI extraction
        let keywords = ExtractedKeywords {
            transition: vec!["custom_transition".to_string()],
            question: vec!["custom_question".to_string()],
            task: vec!["custom_task".to_string()],
            tech: vec!["customtech".to_string()],
        };
        tracker.set_current_keywords(&keywords);

        // Should use custom keywords
        let tech_keywords = tracker.config().tech_keywords();
        assert!(tech_keywords.contains(&"customtech".to_string()));
    }

    #[test]
    fn test_fallback_keywords() {
        let tracker = FocusTracker::new();

        // Should have fallback presets
        let keywords = tracker.config().transition_keywords();
        assert!(!keywords.is_empty());
        assert!(keywords.contains(&"however".to_string()));
    }

    #[test]
    fn test_matches_keywords() {
        let tracker = FocusTracker::new();

        // Should match fallback presets
        assert!(tracker.config().matches_question("How do I do this?"));
        assert!(tracker.config().matches_task("Please implement this"));
        assert!(tracker.config().matches_transition("However, let's move on"));
    }

    #[test]
    fn test_topic_extraction() {
        let tracker = FocusTracker::new();

        // Topic with tech keywords
        let topic = tracker.extract_topic_from_message("使用 Rust 和 Python 开发项目");
        assert!(topic.contains("rust"));
        assert!(topic.contains("python"));

        // Topic without tech keywords (fallback)
        let topic = tracker.extract_topic_from_message("随便聊聊天气");
        assert!(!topic.is_empty());
    }

    #[test]
    fn test_clear_keywords() {
        let mut tracker = FocusTracker::new();

        // Set keywords
        let keywords = ExtractedKeywords {
            transition: vec!["test".to_string()],
            question: vec![],
            task: vec![],
            tech: vec![],
        };
        tracker.set_current_keywords(&keywords);
        assert!(tracker.config().get_keywords().is_some());

        // Clear keywords
        tracker.clear_keywords();
        assert!(tracker.config().get_keywords().is_none());

        // Should use fallback again
        assert!(tracker.config().transition_keywords().contains(&"however".to_string()));
    }
}