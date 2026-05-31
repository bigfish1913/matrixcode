//! Unified extraction result structure.
//!
//! This module defines the result structure for unified extraction,
//! which captures all extracted information in a single AI call.

use serde::{Deserialize, Serialize};

use super::entry::MemoryEntry;
use super::conversation_pattern::ConversationPattern;
use crate::compress::FocusPoint;

/// Result of unified extraction from conversation.
///
/// Contains all extracted information from a single AI call:
/// - Long-term memories (decisions, preferences, solutions, etc.)
/// - Current focus points (topics being discussed)
/// - Conversation patterns (reference patterns, code patterns)
/// - Focus keywords (transition, question, task, tech keywords)
#[derive(Debug, Clone, Default)]
pub struct UnifiedExtractionResult {
    /// Extracted long-term memories.
    pub memories: Vec<MemoryEntry>,
    /// Extracted focus points (current discussion topics).
    pub focus_points: Vec<FocusPoint>,
    /// Extracted conversation patterns.
    pub conversation_patterns: Vec<ConversationPattern>,
    /// Extracted focus keywords organized by category.
    /// These keywords are used in real-time for focus tracking,
    /// not persisted in the registry.
    pub focus_keywords: ExtractedKeywords,
}

/// Extracted keywords organized by category.
///
/// These keywords are used for focus tracking and topic detection.
/// They are passed to FocusTracker in real-time and not persisted.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtractedKeywords {
    /// Keywords indicating topic transition/change.
    /// Examples: "换个话题", "switching", "however"
    pub transition: Vec<String>,
    /// Keywords indicating questions.
    /// Examples: "怎么", "how", "为什么", "why"
    pub question: Vec<String>,
    /// Keywords indicating tasks/requests.
    /// Examples: "帮我", "implement", "创建", "create"
    pub task: Vec<String>,
    /// Technical/domain keywords.
    /// Examples: "rust", "数据库", "api", "performance"
    pub tech: Vec<String>,
}

impl ExtractedKeywords {
    /// Create empty extracted keywords.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if all keyword categories are empty.
    pub fn is_empty(&self) -> bool {
        self.transition.is_empty()
            && self.question.is_empty()
            && self.task.is_empty()
            && self.tech.is_empty()
    }

    /// Get total keyword count across all categories.
    pub fn total_count(&self) -> usize {
        self.transition.len() + self.question.len() + self.task.len() + self.tech.len()
    }

    /// Merge with another ExtractedKeywords, combining all categories.
    pub fn merge(&mut self, other: &ExtractedKeywords) {
        for kw in &other.transition {
            if !self.transition.contains(kw) {
                self.transition.push(kw.clone());
            }
        }
        for kw in &other.question {
            if !self.question.contains(kw) {
                self.question.push(kw.clone());
            }
        }
        for kw in &other.task {
            if !self.task.contains(kw) {
                self.task.push(kw.clone());
            }
        }
        for kw in &other.tech {
            if !self.tech.contains(kw) {
                self.tech.push(kw.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extracted_keywords_new() {
        let keywords = ExtractedKeywords::new();
        assert!(keywords.is_empty());
        assert_eq!(keywords.total_count(), 0);
    }

    #[test]
    fn test_extracted_keywords_is_empty() {
        let empty = ExtractedKeywords::new();
        assert!(empty.is_empty());

        let non_empty = ExtractedKeywords {
            transition: vec!["test".to_string()],
            question: vec![],
            task: vec![],
            tech: vec![],
        };
        assert!(!non_empty.is_empty());
    }

    #[test]
    fn test_extracted_keywords_total_count() {
        let keywords = ExtractedKeywords {
            transition: vec!["a".to_string(), "b".to_string()],
            question: vec!["c".to_string()],
            task: vec!["d".to_string(), "e".to_string(), "f".to_string()],
            tech: vec!["g".to_string()],
        };
        assert_eq!(keywords.total_count(), 7);
    }

    #[test]
    fn test_extracted_keywords_merge() {
        let mut keywords1 = ExtractedKeywords {
            transition: vec!["switch".to_string()],
            question: vec!["how".to_string()],
            task: vec!["create".to_string()],
            tech: vec!["rust".to_string()],
        };

        let keywords2 = ExtractedKeywords {
            transition: vec!["switch".to_string(), "new".to_string()],
            question: vec!["why".to_string()],
            task: vec!["create".to_string(), "delete".to_string()],
            tech: vec!["python".to_string()],
        };

        keywords1.merge(&keywords2);

        // Should have unique keywords
        assert_eq!(keywords1.transition.len(), 2); // "switch", "new"
        assert_eq!(keywords1.question.len(), 2); // "how", "why"
        assert_eq!(keywords1.task.len(), 2); // "create", "delete"
        assert_eq!(keywords1.tech.len(), 2); // "rust", "python"
    }

    #[test]
    fn test_unified_extraction_result_default() {
        let result = UnifiedExtractionResult::default();
        assert!(result.memories.is_empty());
        assert!(result.focus_points.is_empty());
        assert!(result.conversation_patterns.is_empty());
        assert!(result.focus_keywords.is_empty());
    }
}