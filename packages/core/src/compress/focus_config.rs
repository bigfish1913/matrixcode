//! Focus tracker configuration - eliminates hardcoded values.
//!
//! This module provides configurable settings for focus tracking,
//! using FocusKeywordsRegistry for dynamic keywords.

use serde::{Deserialize, Serialize};

use crate::memory::FocusKeywordsRegistry;

/// Focus tracker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusTrackerConfig {
    /// Keywords registry for dynamic keyword management.
    /// This replaces the previously hardcoded keyword lists.
    #[serde(skip)]
    keywords_registry: Option<FocusKeywordsRegistry>,

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
            // Keywords registry will be initialized lazily
            keywords_registry: None,

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

    /// Get or initialize the keywords registry.
    ///
    /// The registry is loaded lazily to avoid file I/O during struct creation.
    /// Uses default presets if file doesn't exist.
    pub fn keywords_registry(&mut self) -> &FocusKeywordsRegistry {
        if self.keywords_registry.is_none() {
            self.keywords_registry = Some(
                FocusKeywordsRegistry::from_default_file()
                    .unwrap_or_else(|_| FocusKeywordsRegistry::new())
            );
        }
        self.keywords_registry.as_ref().unwrap()
    }

    /// Get keywords registry (immutable, must be initialized).
    pub fn get_keywords_registry(&self) -> Option<&FocusKeywordsRegistry> {
        self.keywords_registry.as_ref()
    }

    /// Set a custom keywords registry.
    pub fn with_keywords_registry(mut self, registry: FocusKeywordsRegistry) -> Self {
        self.keywords_registry = Some(registry);
        self
    }

    /// Get transition keywords from the registry.
    pub fn transition_keywords(&mut self) -> Vec<String> {
        use crate::memory::KeywordCategory;
        self.keywords_registry().get_keywords(KeywordCategory::Transition)
    }

    /// Get question keywords from the registry.
    pub fn question_keywords(&mut self) -> Vec<String> {
        use crate::memory::KeywordCategory;
        self.keywords_registry().get_keywords(KeywordCategory::Question)
    }

    /// Get task keywords from the registry.
    pub fn task_keywords(&mut self) -> Vec<String> {
        use crate::memory::KeywordCategory;
        self.keywords_registry().get_keywords(KeywordCategory::Task)
    }

    /// Get tech keywords from the registry.
    pub fn tech_keywords(&mut self) -> Vec<String> {
        use crate::memory::KeywordCategory;
        self.keywords_registry().get_keywords(KeywordCategory::Tech)
    }

    /// Check if text matches transition keywords.
    pub fn matches_transition(&mut self, text: &str) -> bool {
        use crate::memory::KeywordCategory;
        self.keywords_registry().matches(KeywordCategory::Transition, text)
    }

    /// Check if text matches question keywords.
    pub fn matches_question(&mut self, text: &str) -> bool {
        use crate::memory::KeywordCategory;
        self.keywords_registry().matches(KeywordCategory::Question, text)
    }

    /// Check if text matches task keywords.
    pub fn matches_task(&mut self, text: &str) -> bool {
        use crate::memory::KeywordCategory;
        self.keywords_registry().matches(KeywordCategory::Task, text)
    }

    /// Find matching tech keywords in text.
    pub fn find_tech_keywords(&mut self, text: &str) -> Vec<String> {
        use crate::memory::KeywordCategory;
        self.keywords_registry().find_matches(KeywordCategory::Tech, text)
    }

    /// Learn new keywords from a session.
    pub fn learn_keywords(&mut self, keywords: &[(&str, crate::memory::KeywordCategory)], session_id: &str) {
        // Ensure registry is initialized (lazy init)
        self.keywords_registry();
        if let Some(registry) = self.keywords_registry.as_mut() {
            registry.learn_keywords(keywords, session_id);
        }
    }

    /// Save keywords registry to file.
    pub fn save_keywords(&self) -> anyhow::Result<()> {
        if let Some(registry) = &self.keywords_registry {
            registry.save_to_default_file()?;
        }
        Ok(())
    }

    /// Validate configuration (basic parameters, not keywords).
    pub fn validate(&self) -> bool {
        self.focus_window_size > 0 &&
        self.max_recent_context_count > 0 &&
        self.max_question_extract_length > 0 &&
        self.min_substantial_text_length > 0 &&
        self.focus_score_boost > 0.0 &&
        self.max_focus_score > 0.0 &&
        self.fallback_topic_word_count > 0
    }

    /// Full validation including keywords registry.
    pub fn validate_full(&mut self) -> bool {
        self.validate() && !self.transition_keywords().is_empty()
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

impl From<KeywordType> for crate::memory::KeywordCategory {
    fn from(kw: KeywordType) -> Self {
        match kw {
            KeywordType::Transition => crate::memory::KeywordCategory::Transition,
            KeywordType::Question => crate::memory::KeywordCategory::Question,
            KeywordType::Task => crate::memory::KeywordCategory::Task,
            KeywordType::Tech => crate::memory::KeywordCategory::Tech,
        }
    }
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
    fn test_keywords_registry_lazy_init() {
        let mut config = FocusTrackerConfig::default();

        // Registry should be None initially
        assert!(config.get_keywords_registry().is_none());

        // Accessing keywords should initialize registry
        let keywords = config.transition_keywords();
        assert!(!keywords.is_empty());
        assert!(config.get_keywords_registry().is_some());
    }

    #[test]
    fn test_keywords_from_registry() {
        let mut config = FocusTrackerConfig::default();

        // Should have preset keywords
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
        let mut config = FocusTrackerConfig::default();

        assert!(config.matches_question("How do I do this?"));
        assert!(config.matches_task("Please implement this"));
        assert!(config.matches_transition("However, let's move on"));
    }

    #[test]
    fn test_find_tech_keywords() {
        let mut config = FocusTrackerConfig::default();

        let found = config.find_tech_keywords("Using Rust and Python for development");
        assert!(found.contains(&"rust".to_string()));
        assert!(found.contains(&"python".to_string()));
    }

    #[test]
    fn test_with_custom_registry() {
        let registry = FocusKeywordsRegistry::new();
        let config = FocusTrackerConfig::default()
            .with_keywords_registry(registry);

        assert!(config.get_keywords_registry().is_some());
    }

    #[test]
    fn test_validate_full() {
        let mut config = FocusTrackerConfig::default();
        assert!(config.validate_full());
    }

    #[test]
    fn test_keyword_type_conversion() {
        use crate::memory::KeywordCategory;

        assert_eq!(KeywordCategory::from(KeywordType::Transition), KeywordCategory::Transition);
        assert_eq!(KeywordCategory::from(KeywordType::Question), KeywordCategory::Question);
        assert_eq!(KeywordCategory::from(KeywordType::Task), KeywordCategory::Task);
        assert_eq!(KeywordCategory::from(KeywordType::Tech), KeywordCategory::Tech);
    }
}