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
/// - Focus decision (AI's selection/creation of focus)
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

    /// AI focus decision: which existing focus matches, or need to create new.
    /// This is the primary output for focus tracking.
    pub focus_decision: Option<FocusDecision>,
}

/// AI focus decision - the AI's judgment on current conversation focus.
///
/// Instead of extracting focus from scratch, the AI selects from existing
/// focuses or decides to create a new one. This ensures focus continuity
/// and proper history tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusDecision {
    /// ID of the selected existing focus (if matched).
    /// None if need_new_focus is true.
    pub selected_focus_id: Option<String>,

    /// Whether none of the existing focuses match and a new focus is needed.
    pub need_new_focus: bool,

    /// New focus topic (only if need_new_focus is true).
    pub new_focus_topic: Option<String>,

    /// Core question/task for the new focus.
    pub new_core_question: Option<String>,

    /// Confidence of the selection/creation (0.0-1.0).
    pub confidence: f32,

    /// Focus type classification.
    pub focus_type: FocusType,

    /// Whether this is a topic switch from a previous focus.
    pub is_topic_switch: bool,

    /// The previous focus being switched from (if is_topic_switch).
    pub previous_focus_id: Option<String>,

    /// Core keywords for this focus (3-5 keywords).
    pub focus_keywords: Vec<String>,

    /// Related entities (files, functions, modules).
    pub related_entities: Vec<String>,

    /// AI's reasoning for this decision.
    pub reasoning: String,
}

/// Focus type classification.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FocusType {
    #[default]
    General,
    /// Fixing bugs, resolving errors.
    ProblemSolving,
    /// Implementing features, completing tasks.
    TaskExecution,
    /// Learning, researching, exploring.
    KnowledgeExploration,
    /// Technical choices, architecture design.
    DecisionMaking,
    /// Performance optimization, refactoring.
    CodeOptimization,
}

impl std::fmt::Display for FocusType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FocusType::General => write!(f, "general"),
            FocusType::ProblemSolving => write!(f, "problem_solving"),
            FocusType::TaskExecution => write!(f, "task_execution"),
            FocusType::KnowledgeExploration => write!(f, "knowledge_exploration"),
            FocusType::DecisionMaking => write!(f, "decision_making"),
            FocusType::CodeOptimization => write!(f, "code_optimization"),
        }
    }
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
        assert!(result.focus_decision.is_none());
    }

    #[test]
    fn test_focus_decision_select_existing() {
        let decision = FocusDecision {
            selected_focus_id: Some("focus-1".to_string()),
            need_new_focus: false,
            new_focus_topic: None,
            new_core_question: None,
            confidence: 0.85,
            focus_type: FocusType::CodeOptimization,
            is_topic_switch: false,
            previous_focus_id: None,
            focus_keywords: vec!["API".to_string(), "performance".to_string()],
            related_entities: vec!["api.rs".to_string()],
            reasoning: "User is continuing API optimization discussion".to_string(),
        };

        assert!(decision.selected_focus_id.is_some());
        assert!(!decision.need_new_focus);
        assert_eq!(decision.confidence, 0.85);
    }

    #[test]
    fn test_focus_decision_create_new() {
        let decision = FocusDecision {
            selected_focus_id: None,
            need_new_focus: true,
            new_focus_topic: Some("Database schema design".to_string()),
            new_core_question: Some("How to design user table?".to_string()),
            confidence: 0.9,
            focus_type: FocusType::DecisionMaking,
            is_topic_switch: true,
            previous_focus_id: Some("focus-1".to_string()),
            focus_keywords: vec!["database".to_string(), "schema".to_string()],
            related_entities: vec!["user.rs".to_string()],
            reasoning: "User switched to new database topic".to_string(),
        };

        assert!(decision.selected_focus_id.is_none());
        assert!(decision.need_new_focus);
        assert!(decision.new_focus_topic.is_some());
        assert!(decision.is_topic_switch);
    }

    #[test]
    fn test_focus_type_display() {
        assert_eq!(FocusType::General.to_string(), "general");
        assert_eq!(FocusType::ProblemSolving.to_string(), "problem_solving");
        assert_eq!(FocusType::TaskExecution.to_string(), "task_execution");
        assert_eq!(FocusType::KnowledgeExploration.to_string(), "knowledge_exploration");
        assert_eq!(FocusType::DecisionMaking.to_string(), "decision_making");
        assert_eq!(FocusType::CodeOptimization.to_string(), "code_optimization");
    }

    #[test]
    fn test_focus_type_default() {
        let focus_type = FocusType::default();
        assert_eq!(focus_type, FocusType::General);
    }

    #[test]
    fn test_focus_decision_serialization() {
        let decision = FocusDecision {
            selected_focus_id: Some("focus-1".to_string()),
            need_new_focus: false,
            new_focus_topic: None,
            new_core_question: None,
            confidence: 0.85,
            focus_type: FocusType::CodeOptimization,
            is_topic_switch: false,
            previous_focus_id: None,
            focus_keywords: vec!["API".to_string()],
            related_entities: vec![],
            reasoning: "test".to_string(),
        };

        // Serialize
        let json = serde_json::to_string(&decision).unwrap();
        assert!(json.contains("focus-1"));
        assert!(json.contains("code_optimization"));

        // Deserialize
        let parsed: FocusDecision = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.selected_focus_id, Some("focus-1".to_string()));
        assert_eq!(parsed.focus_type, FocusType::CodeOptimization);
    }
}