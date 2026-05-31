//! Focus tracker configuration - eliminates hardcoded values.
//!
//! This module provides configurable settings for focus tracking,
//! using real-time extracted keywords from AI instead of persistent registry.

use serde::{Deserialize, Serialize};

use crate::memory::ExtractedKeywords;

/// Focus tracker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusTrackerConfig {
    /// Current keywords extracted from the conversation (real-time, not persisted).
    /// These are set by AI extraction and used for focus detection.
    #[serde(skip)]
    current_keywords: Option<ExtractedKeywords>,

    /// Number of words to extract when no keywords found
    pub fallback_topic_word_count: usize,

    /// Window size for focus detection (number of recent messages to analyze)
    pub focus_window_size: usize,

    /// Maximum recent context snippets to keep
    pub max_recent_context_count: usize,

    /// Maximum characters to extract for question/task
    pub max_question_extract_length: usize,

    /// Minimum text length to consider as substantial
    pub min_substantial_text_length: usize,

    /// Focus score boost for messages matching current focus
    pub focus_score_boost: f32,

    /// Maximum focus score (cap)
    pub max_focus_score: f32,
}

impl Default for FocusTrackerConfig {
    fn default() -> Self {
        Self {
            // Keywords are set in real-time via set_keywords()
            current_keywords: None,

            // Fallback: extract N words when no keywords found
            fallback_topic_word_count: 3,

            // Window sizes and limits
            focus_window_size: 10,              // Analyze last 10 messages
            max_recent_context_count: 5,        // Keep up to 5 context snippets
            max_question_extract_length: 100,   // Extract up to 100 chars for question
            min_substantial_text_length: 10,    // Minimum 10 chars to be substantial

            // Scoring parameters
            focus_score_boost: 0.3,             // Focus can boost priority by up to 0.3
            max_focus_score: 1.0,               // Cap focus score at 1.0
        }
    }
}

impl FocusTrackerConfig {
    /// Create config for simple conversations (lower thresholds)
    pub fn simple_conversation() -> Self {
        Self {
            focus_window_size: 5,
            max_recent_context_count: 3,
            min_substantial_text_length: 5,
            ..Self::default()
        }
    }

    /// Create config for complex technical discussions (higher thresholds)
    pub fn complex_technical() -> Self {
        Self {
            focus_window_size: 15,
            max_recent_context_count: 7,
            max_question_extract_length: 150,
            min_substantial_text_length: 20,
            focus_score_boost: 0.4,
            ..Self::default()
        }
    }

    /// Create config from complexity level
    pub fn from_complexity(level: crate::compress::complexity::ComplexityLevel) -> Self {
        match level {
            crate::compress::complexity::ComplexityLevel::High => Self::complex_technical(),
            crate::compress::complexity::ComplexityLevel::Medium => Self::default(),
            crate::compress::complexity::ComplexityLevel::Low => Self::simple_conversation(),
        }
    }

    /// Set keywords extracted from AI (real-time).
    ///
    /// These keywords are used for focus tracking in the current conversation
    /// and are not persisted.
    pub fn set_keywords(&mut self, keywords: &ExtractedKeywords) {
        self.current_keywords = Some(keywords.clone());
    }

    /// Get current keywords (if set).
    pub fn get_keywords(&self) -> Option<&ExtractedKeywords> {
        self.current_keywords.as_ref()
    }

    /// Get transition keywords (from AI extraction or fallback presets).
    pub fn transition_keywords(&self) -> Vec<String> {
        if let Some(kw) = &self.current_keywords {
            kw.transition.clone()
        } else {
            // Fallback to hardcoded presets
            vec![
                "however".to_string(), "but".to_string(), "switching".to_string(),
                "转换".to_string(), "切换".to_string(), "换个话题".to_string(),
            ]
        }
    }

    /// Get question keywords (from AI extraction or fallback presets).
    pub fn question_keywords(&self) -> Vec<String> {
        if let Some(kw) = &self.current_keywords {
            kw.question.clone()
        } else {
            // Fallback to hardcoded presets
            vec![
                "how".to_string(), "what".to_string(), "why".to_string(),
                "如何".to_string(), "什么".to_string(), "为什么".to_string(),
            ]
        }
    }

    /// Get task keywords (from AI extraction or fallback presets).
    pub fn task_keywords(&self) -> Vec<String> {
        if let Some(kw) = &self.current_keywords {
            kw.task.clone()
        } else {
            // Fallback to hardcoded presets
            vec![
                "implement".to_string(), "create".to_string(), "fix".to_string(),
                "实现".to_string(), "创建".to_string(), "修复".to_string(),
            ]
        }
    }

    /// Get tech keywords (from AI extraction or fallback presets).
    pub fn tech_keywords(&self) -> Vec<String> {
        if let Some(kw) = &self.current_keywords {
            kw.tech.clone()
        } else {
            // Fallback to hardcoded presets
            vec![
                "rust".to_string(), "python".to_string(), "javascript".to_string(),
                "api".to_string(), "database".to_string(), "performance".to_string(),
            ]
        }
    }

    /// Check if text matches transition keywords.
    pub fn matches_transition(&self, text: &str) -> bool {
        let lower = text.to_lowercase();
        self.transition_keywords().iter().any(|kw| lower.contains(&kw.to_lowercase()))
    }

    /// Check if text matches question keywords.
    pub fn matches_question(&self, text: &str) -> bool {
        let lower = text.to_lowercase();
        self.question_keywords().iter().any(|kw| lower.contains(&kw.to_lowercase()))
    }

    /// Check if text matches task keywords.
    pub fn matches_task(&self, text: &str) -> bool {
        let lower = text.to_lowercase();
        self.task_keywords().iter().any(|kw| lower.contains(&kw.to_lowercase()))
    }

    /// Find matching tech keywords in text.
    pub fn find_tech_keywords(&self, text: &str) -> Vec<String> {
        let lower = text.to_lowercase();
        self.tech_keywords()
            .iter()
            .filter(|kw| lower.contains(&kw.to_lowercase()))
            .cloned()
            .collect()
    }

    /// Merge additional keywords into current keywords.
    pub fn merge_keywords(&mut self, additional: &ExtractedKeywords) {
        match self.current_keywords.take() {
            Some(mut current) => {
                current.merge(additional);
                self.current_keywords = Some(current);
            }
            None => {
                self.current_keywords = Some(additional.clone());
            }
        }
    }

    /// Clear current keywords (start fresh for new conversation).
    pub fn clear_keywords(&mut self) {
        self.current_keywords = None;
    }

    /// Validate configuration (basic parameters).
    pub fn validate(&self) -> bool {
        self.focus_window_size > 0 &&
        self.max_recent_context_count > 0 &&
        self.max_question_extract_length > 0 &&
        self.min_substantial_text_length > 0 &&
        self.focus_score_boost > 0.0 &&
        self.max_focus_score > 0.0 &&
        self.fallback_topic_word_count > 0
    }
}

/// Keyword type for custom keyword additions (legacy compatibility).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeywordType {
    Transition,
    Question,
    Task,
    Tech,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = FocusTrackerConfig::default();
        assert!(config.validate());
        assert_eq!(config.focus_window_size, 10);
        assert_eq!(config.max_recent_context_count, 5);
    }

    #[test]
    fn test_simple_conversation_config() {
        let config = FocusTrackerConfig::simple_conversation();
        assert_eq!(config.focus_window_size, 5);
        assert_eq!(config.max_recent_context_count, 3);
    }

    #[test]
    fn test_complex_technical_config() {
        let config = FocusTrackerConfig::complex_technical();
        assert_eq!(config.focus_window_size, 15);
        assert_eq!(config.max_question_extract_length, 150);
    }

    #[test]
    fn test_set_keywords() {
        let mut config = FocusTrackerConfig::default();

        // Initially no keywords
        assert!(config.get_keywords().is_none());

        // Set keywords
        let keywords = ExtractedKeywords {
            transition: vec!["new_transition".to_string()],
            question: vec!["new_question".to_string()],
            task: vec!["new_task".to_string()],
            tech: vec!["new_tech".to_string()],
        };
        config.set_keywords(&keywords);

        // Should now have keywords
        assert!(config.get_keywords().is_some());
        assert_eq!(config.transition_keywords(), vec!["new_transition".to_string()]);
        assert_eq!(config.question_keywords(), vec!["new_question".to_string()]);
    }

    #[test]
    fn test_fallback_keywords() {
        let config = FocusTrackerConfig::default();

        // Should use fallback presets when no keywords set
        assert!(!config.transition_keywords().is_empty());
        assert!(!config.question_keywords().is_empty());
        assert!(!config.task_keywords().is_empty());
        assert!(!config.tech_keywords().is_empty());

        // Should contain expected presets
        assert!(config.transition_keywords().contains(&"however".to_string()));
        assert!(config.question_keywords().contains(&"how".to_string()));
        assert!(config.task_keywords().contains(&"implement".to_string()));
        assert!(config.tech_keywords().contains(&"rust".to_string()));
    }

    #[test]
    fn test_matches_keywords() {
        let config = FocusTrackerConfig::default();

        // Should match fallback presets
        assert!(config.matches_question("How do I do this?"));
        assert!(config.matches_task("Please implement this"));
        assert!(config.matches_transition("However, let's move on"));
    }

    #[test]
    fn test_find_tech_keywords() {
        let config = FocusTrackerConfig::default();

        let found = config.find_tech_keywords("Using Rust and Python for development");
        assert!(found.contains(&"rust".to_string()));
        assert!(found.contains(&"python".to_string()));
    }

    #[test]
    fn test_merge_keywords() {
        let mut config = FocusTrackerConfig::default();

        // Set initial keywords
        let initial = ExtractedKeywords {
            transition: vec!["switch".to_string()],
            question: vec!["how".to_string()],
            task: vec!["create".to_string()],
            tech: vec!["rust".to_string()],
        };
        config.set_keywords(&initial);

        // Merge additional keywords
        let additional = ExtractedKeywords {
            transition: vec!["new".to_string()],
            question: vec!["why".to_string()],
            task: vec!["delete".to_string()],
            tech: vec!["python".to_string()],
        };
        config.merge_keywords(&additional);

        // Should have merged keywords
        let merged = config.get_keywords().unwrap();
        assert!(merged.transition.contains(&"switch".to_string()));
        assert!(merged.transition.contains(&"new".to_string()));
        assert!(merged.tech.contains(&"rust".to_string()));
        assert!(merged.tech.contains(&"python".to_string()));
    }

    #[test]
    fn test_clear_keywords() {
        let mut config = FocusTrackerConfig::default();

        // Set keywords
        let keywords = ExtractedKeywords {
            transition: vec!["test".to_string()],
            question: vec![],
            task: vec![],
            tech: vec![],
        };
        config.set_keywords(&keywords);
        assert!(config.get_keywords().is_some());

        // Clear keywords
        config.clear_keywords();
        assert!(config.get_keywords().is_none());

        // Should use fallback again
        assert!(config.transition_keywords().contains(&"however".to_string()));
    }
}