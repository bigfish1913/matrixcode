//! Conversation pattern types for pattern-based memory system.
//!
//! This module defines the core data structures for conversation patterns,
//! which capture reusable reference and code patterns from conversations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ============================================================================
// Pattern Types
// ============================================================================

/// Types of conversation patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatternType {
    /// Reference pattern - how to refer to things (e.g., "PR", "issue", "commit")
    Reference,
    /// Code pattern - code style and structure patterns
    Code,
}

impl PatternType {
    /// Get display name for the pattern type.
    pub fn display_name(&self) -> &'static str {
        match self {
            PatternType::Reference => "引用模式",
            PatternType::Code => "代码模式",
        }
    }

    /// Get icon for the pattern type.
    pub fn icon(&self) -> &'static str {
        match self {
            PatternType::Reference => "🔗",
            PatternType::Code => "💻",
        }
    }
}

// ============================================================================
// Pattern Sources
// ============================================================================

/// Source of a conversation pattern.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PatternSource {
    /// Learned from user conversation.
    UserConversation {
        /// Example context where this pattern was observed.
        example: String,
    },
    /// Derived from project code style.
    ProjectCodeStyle {
        /// Programming language for this style.
        language: String,
    },
    /// System preset pattern (built-in defaults).
    SystemPreset,
    /// Manually added by user.
    Manual,
}

impl PatternSource {
    /// Create a user conversation source.
    pub fn user_conversation(example: impl Into<String>) -> Self {
        PatternSource::UserConversation {
            example: example.into(),
        }
    }

    /// Create a project code style source.
    pub fn project_code_style(language: impl Into<String>) -> Self {
        PatternSource::ProjectCodeStyle {
            language: language.into(),
        }
    }

    /// Check if this is a system preset.
    pub fn is_preset(&self) -> bool {
        matches!(self, PatternSource::SystemPreset)
    }

    /// Check if this is manually added.
    pub fn is_manual(&self) -> bool {
        matches!(self, PatternSource::Manual)
    }

    /// Get display name for the source.
    pub fn display_name(&self) -> &'static str {
        match self {
            PatternSource::UserConversation { .. } => "用户对话",
            PatternSource::ProjectCodeStyle { .. } => "项目风格",
            PatternSource::SystemPreset => "系统预设",
            PatternSource::Manual => "手动添加",
        }
    }
}

// ============================================================================
// Conversation Pattern
// ============================================================================

/// A conversation pattern captured from user interactions.
///
/// Patterns represent reusable conventions like:
/// - How to refer to pull requests ("PR #123" vs "pull request #123")
/// - Code style preferences (naming conventions, formatting)
/// - Common phrases and their variations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationPattern {
    /// Unique identifier.
    pub id: String,
    /// Type of pattern.
    pub pattern_type: PatternType,
    /// The pattern string (regex or literal).
    pub pattern: String,
    /// Source where this pattern was learned from.
    pub source: PatternSource,
    /// Number of times this pattern has been used/matched.
    pub frequency: u32,
    /// When this pattern was last used.
    pub last_used: DateTime<Utc>,
    /// Confidence score (0.0-1.0), higher means more certain.
    pub confidence: f32,
    /// Whether this pattern is currently active.
    pub is_active: bool,
    /// Optional description of what this pattern represents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Tags for categorization and search.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

impl ConversationPattern {
    /// Create a new conversation pattern.
    pub fn new(
        pattern_type: PatternType,
        pattern: impl Into<String>,
        source: PatternSource,
    ) -> Self {
        let id = uuid::Uuid::new_v4().to_string();
        Self {
            id,
            pattern_type,
            pattern: pattern.into(),
            source,
            frequency: 1,
            last_used: Utc::now(),
            confidence: 0.5,
            is_active: true,
            description: None,
            tags: Vec::new(),
        }
    }

    /// Create a system preset pattern.
    pub fn preset(pattern_type: PatternType, pattern: impl Into<String>) -> Self {
        let mut p = Self::new(pattern_type, pattern, PatternSource::SystemPreset);
        p.confidence = 1.0;
        p.frequency = 100; // Start with high frequency for presets
        p
    }

    /// Create a manually added pattern.
    pub fn manual(pattern_type: PatternType, pattern: impl Into<String>) -> Self {
        let mut p = Self::new(pattern_type, pattern, PatternSource::Manual);
        p.confidence = 0.9;
        p.is_active = true;
        p
    }

    /// Set the description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Add a tag.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Mark this pattern as used (increment frequency, update timestamp).
    pub fn mark_used(&mut self) {
        self.frequency = self.frequency.saturating_add(1);
        self.last_used = Utc::now();
        // Boost confidence slightly with usage
        self.confidence = (self.confidence + 0.01).min(1.0);
    }

    /// Deactivate this pattern.
    pub fn deactivate(&mut self) {
        self.is_active = false;
    }

    /// Activate this pattern.
    pub fn activate(&mut self) {
        self.is_active = true;
    }

    /// Format for display.
    pub fn format_line(&self) -> String {
        let active_marker = if self.is_active { "" } else { "[inactive] " };
        let freq_marker = if self.frequency > 10 { "★" } else { "" };
        format!(
            "{}{} {} {} (freq: {}, conf: {:.2}) {}",
            active_marker,
            self.pattern_type.icon(),
            self.pattern_type.display_name(),
            &self.pattern,
            self.frequency,
            self.confidence,
            freq_marker
        )
    }

    /// Format for inclusion in system prompt.
    pub fn format_for_prompt(&self) -> String {
        match &self.description {
            Some(desc) => format!("{}: {}", &self.pattern, desc),
            None => self.pattern.clone(),
        }
    }
}

// ============================================================================
// Default Presets
// ============================================================================

/// Get default reference patterns (system presets).
pub fn get_default_reference_patterns() -> Vec<ConversationPattern> {
    vec![
        // Git-style references
        ConversationPattern::preset(
            PatternType::Reference,
            r"\bPR\s*#\d+\b",
        )
        .with_description("Pull Request 引用格式: PR #123")
        .with_tag("git"),
        ConversationPattern::preset(
            PatternType::Reference,
            r"\bissue\s*#\d+\b",
        )
        .with_description("Issue 引用格式: issue #123")
        .with_tag("git"),
        ConversationPattern::preset(
            PatternType::Reference,
            r"\bcommit\s+[a-f0-9]{7,40}\b",
        )
        .with_description("Commit 引用格式: commit abc1234")
        .with_tag("git"),
        ConversationPattern::preset(
            PatternType::Reference,
            r"\b[A-Z]+-[0-9]+\b",
        )
        .with_description("JIRA 风格任务引用: PROJ-123")
        .with_tag("jira"),
        // Cross-reference phrases (English)
        ConversationPattern::preset(
            PatternType::Reference,
            r"as i mentioned",
        )
        .with_description("引用上文: as I mentioned")
        .with_tag("cross-ref"),
        ConversationPattern::preset(
            PatternType::Reference,
            r"as mentioned",
        )
        .with_description("引用上文: as mentioned")
        .with_tag("cross-ref"),
        ConversationPattern::preset(
            PatternType::Reference,
            r"as we discussed",
        )
        .with_description("引用上文: as we discussed")
        .with_tag("cross-ref"),
        ConversationPattern::preset(
            PatternType::Reference,
            r"previously",
        )
        .with_description("引用上文: previously")
        .with_tag("cross-ref"),
        ConversationPattern::preset(
            PatternType::Reference,
            r"above",
        )
        .with_description("引用上文: above")
        .with_tag("cross-ref"),
        ConversationPattern::preset(
            PatternType::Reference,
            r"earlier",
        )
        .with_description("引用上文: earlier")
        .with_tag("cross-ref"),
        ConversationPattern::preset(
            PatternType::Reference,
            r"before",
        )
        .with_description("引用上文: before")
        .with_tag("cross-ref"),
        ConversationPattern::preset(
            PatternType::Reference,
            r"that's what",
        )
        .with_description("引用上文: that's what")
        .with_tag("cross-ref"),
        ConversationPattern::preset(
            PatternType::Reference,
            r"that is what",
        )
        .with_description("引用上文: that is what")
        .with_tag("cross-ref"),
        // Cross-reference phrases (Chinese)
        ConversationPattern::preset(
            PatternType::Reference,
            "正如我所说",
        )
        .with_description("引用上文: 正如我所说")
        .with_tag("cross-ref")
        .with_tag("zh"),
        ConversationPattern::preset(
            PatternType::Reference,
            "之前提到",
        )
        .with_description("引用上文: 之前提到")
        .with_tag("cross-ref")
        .with_tag("zh"),
        ConversationPattern::preset(
            PatternType::Reference,
            "前面说",
        )
        .with_description("引用上文: 前面说")
        .with_tag("cross-ref")
        .with_tag("zh"),
        ConversationPattern::preset(
            PatternType::Reference,
            "刚才说",
        )
        .with_description("引用上文: 刚才说")
        .with_tag("cross-ref")
        .with_tag("zh"),
    ]
}

/// Get default code patterns (system presets).
pub fn get_default_code_patterns() -> Vec<ConversationPattern> {
    vec![
        // Rust patterns
        ConversationPattern::preset(
            PatternType::Code,
            r"fn\s+\w+\s*\(",
        )
        .with_description("Rust 函数定义")
        .with_tag("rust"),
        ConversationPattern::preset(
            PatternType::Code,
            r"async\s+fn\s+\w+\s*\(",
        )
        .with_description("Rust 异步函数定义")
        .with_tag("rust"),
        ConversationPattern::preset(
            PatternType::Code,
            r"impl\s+\w+",
        )
        .with_description("Rust impl 块")
        .with_tag("rust"),
        ConversationPattern::preset(
            PatternType::Code,
            r"pub\s+(async\s+)?fn\s+\w+",
        )
        .with_description("Rust 公开函数")
        .with_tag("rust"),
        // Code block markers
        ConversationPattern::preset(
            PatternType::Code,
            "```",
        )
        .with_description("代码块标记")
        .with_tag("markdown"),
        // Multi-language keywords
        ConversationPattern::preset(
            PatternType::Code,
            "function",
        )
        .with_description("函数关键字 (JS/TS/等)")
        .with_tag("js")
        .with_tag("ts"),
        ConversationPattern::preset(
            PatternType::Code,
            "class",
        )
        .with_description("类关键字")
        .with_tag("oop"),
        ConversationPattern::preset(
            PatternType::Code,
            "struct",
        )
        .with_description("结构体关键字 (Rust/C/Go)")
        .with_tag("rust")
        .with_tag("go"),
        ConversationPattern::preset(
            PatternType::Code,
            "impl",
        )
        .with_description("实现关键字 (Rust)")
        .with_tag("rust"),
        ConversationPattern::preset(
            PatternType::Code,
            "fn ",
        )
        .with_description("函数关键字 (Rust)")
        .with_tag("rust"),
        ConversationPattern::preset(
            PatternType::Code,
            "let ",
        )
        .with_description("变量绑定 (Rust/JS)")
        .with_tag("rust")
        .with_tag("js"),
        ConversationPattern::preset(
            PatternType::Code,
            "const ",
        )
        .with_description("常量关键字")
        .with_tag("multi"),
        ConversationPattern::preset(
            PatternType::Code,
            "var ",
        )
        .with_description("变量关键字 (JS)")
        .with_tag("js"),
        ConversationPattern::preset(
            PatternType::Code,
            "import ",
        )
        .with_description("导入关键字")
        .with_tag("multi"),
        ConversationPattern::preset(
            PatternType::Code,
            "use ",
        )
        .with_description("导入关键字 (Rust)")
        .with_tag("rust"),
        ConversationPattern::preset(
            PatternType::Code,
            "pub ",
        )
        .with_description("公开关键字 (Rust)")
        .with_tag("rust"),
        ConversationPattern::preset(
            PatternType::Code,
            "def ",
        )
        .with_description("函数定义 (Python)")
        .with_tag("python"),
        ConversationPattern::preset(
            PatternType::Code,
            "async ",
        )
        .with_description("异步关键字")
        .with_tag("multi"),
        ConversationPattern::preset(
            PatternType::Code,
            "return ",
        )
        .with_description("返回关键字")
        .with_tag("multi"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // PatternType Tests
    // =========================================================================

    #[test]
    fn test_pattern_type_display_name() {
        assert_eq!(PatternType::Reference.display_name(), "引用模式");
        assert_eq!(PatternType::Code.display_name(), "代码模式");
    }

    #[test]
    fn test_pattern_type_icon() {
        assert_eq!(PatternType::Reference.icon(), "🔗");
        assert_eq!(PatternType::Code.icon(), "💻");
    }

    #[test]
    fn test_pattern_type_equality() {
        assert_eq!(PatternType::Reference, PatternType::Reference);
        assert_eq!(PatternType::Code, PatternType::Code);
        assert_ne!(PatternType::Reference, PatternType::Code);
    }

    #[test]
    fn test_pattern_type_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(PatternType::Reference);
        set.insert(PatternType::Code);
        set.insert(PatternType::Reference); // Duplicate

        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_pattern_type_serialization() {
        let pt = PatternType::Reference;
        let json = serde_json::to_string(&pt).unwrap();
        assert_eq!(json, "\"reference\"");

        let decoded: PatternType = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, PatternType::Reference);

        let pt2 = PatternType::Code;
        let json2 = serde_json::to_string(&pt2).unwrap();
        assert_eq!(json2, "\"code\"");
    }

    // =========================================================================
    // PatternSource Tests
    // =========================================================================

    #[test]
    fn test_pattern_source_user_conversation() {
        let source = PatternSource::user_conversation("User mentioned PR #123");
        match source {
            PatternSource::UserConversation { example } => {
                assert_eq!(example, "User mentioned PR #123");
            }
            _ => panic!("Expected UserConversation variant"),
        }
    }

    #[test]
    fn test_pattern_source_project_code_style() {
        let source = PatternSource::project_code_style("rust");
        match source {
            PatternSource::ProjectCodeStyle { language } => {
                assert_eq!(language, "rust");
            }
            _ => panic!("Expected ProjectCodeStyle variant"),
        }
    }

    #[test]
    fn test_pattern_source_is_preset() {
        assert!(PatternSource::SystemPreset.is_preset());
        assert!(!PatternSource::user_conversation("test").is_preset());
        assert!(!PatternSource::project_code_style("rust").is_preset());
        assert!(!PatternSource::Manual.is_preset());
    }

    #[test]
    fn test_pattern_source_is_manual() {
        assert!(PatternSource::Manual.is_manual());
        assert!(!PatternSource::SystemPreset.is_manual());
        assert!(!PatternSource::user_conversation("test").is_manual());
        assert!(!PatternSource::project_code_style("rust").is_manual());
    }

    #[test]
    fn test_pattern_source_display_name() {
        assert_eq!(PatternSource::user_conversation("test").display_name(), "用户对话");
        assert_eq!(PatternSource::project_code_style("rust").display_name(), "项目风格");
        assert_eq!(PatternSource::SystemPreset.display_name(), "系统预设");
        assert_eq!(PatternSource::Manual.display_name(), "手动添加");
    }

    #[test]
    fn test_pattern_source_serialization() {
        // UserConversation
        let source = PatternSource::user_conversation("example context");
        let json = serde_json::to_string(&source).unwrap();
        let decoded: PatternSource = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, source);

        // ProjectCodeStyle
        let source2 = PatternSource::project_code_style("typescript");
        let json2 = serde_json::to_string(&source2).unwrap();
        let decoded2: PatternSource = serde_json::from_str(&json2).unwrap();
        assert_eq!(decoded2, source2);

        // SystemPreset
        let source3 = PatternSource::SystemPreset;
        let json3 = serde_json::to_string(&source3).unwrap();
        assert!(json3.contains("system_preset"));

        // Manual
        let source4 = PatternSource::Manual;
        let json4 = serde_json::to_string(&source4).unwrap();
        assert!(json4.contains("manual"));
    }

    // =========================================================================
    // ConversationPattern Creation Tests
    // =========================================================================

    #[test]
    fn test_pattern_creation() {
        let pattern = ConversationPattern::new(
            PatternType::Reference,
            r"PR #\d+",
            PatternSource::user_conversation("User mentioned PR #123"),
        );
        assert!(pattern.is_active);
        assert_eq!(pattern.frequency, 1);
        assert_eq!(pattern.confidence, 0.5);
        assert!(pattern.description.is_none());
        assert!(pattern.tags.is_empty());
        assert!(!pattern.id.is_empty()); // UUID should be generated
    }

    #[test]
    fn test_pattern_creation_with_all_types() {
        // Reference type
        let ref_pattern = ConversationPattern::new(
            PatternType::Reference,
            r"issue #\d+",
            PatternSource::Manual,
        );
        assert_eq!(ref_pattern.pattern_type, PatternType::Reference);

        // Code type
        let code_pattern = ConversationPattern::new(
            PatternType::Code,
            r"fn \w+\(",
            PatternSource::SystemPreset,
        );
        assert_eq!(code_pattern.pattern_type, PatternType::Code);
    }

    #[test]
    fn test_pattern_preset() {
        let pattern = ConversationPattern::preset(PatternType::Reference, r"PR #\d+");

        assert!(pattern.source.is_preset());
        assert!(pattern.is_active);
        assert_eq!(pattern.confidence, 1.0);
        assert_eq!(pattern.frequency, 100); // Presets start with high frequency
    }

    #[test]
    fn test_pattern_manual() {
        let pattern = ConversationPattern::manual(PatternType::Code, "custom-pattern");

        assert!(pattern.source.is_manual());
        assert!(pattern.is_active);
        assert_eq!(pattern.confidence, 0.9);
    }

    #[test]
    fn test_pattern_with_description() {
        let pattern = ConversationPattern::new(
            PatternType::Reference,
            "test-pattern",
            PatternSource::Manual,
        )
        .with_description("This is a test pattern");

        assert_eq!(pattern.description, Some("This is a test pattern".to_string()));
    }

    #[test]
    fn test_pattern_with_tag() {
        let pattern = ConversationPattern::new(
            PatternType::Code,
            "test-pattern",
            PatternSource::Manual,
        )
        .with_tag("rust")
        .with_tag("async");

        assert_eq!(pattern.tags, vec!["rust", "async"]);
    }

    #[test]
    fn test_pattern_builder_chain() {
        let pattern = ConversationPattern::preset(PatternType::Reference, r"\bPR\s*#\d+\b")
            .with_description("Pull Request reference")
            .with_tag("git")
            .with_tag("github");

        assert_eq!(pattern.pattern, r"\bPR\s*#\d+\b");
        assert_eq!(pattern.description, Some("Pull Request reference".to_string()));
        assert_eq!(pattern.tags, vec!["git", "github"]);
        assert!(pattern.source.is_preset());
    }

    // =========================================================================
    // ConversationPattern State Change Tests
    // =========================================================================

    #[test]
    fn test_pattern_mark_used() {
        let mut pattern = ConversationPattern::new(
            PatternType::Code,
            "fn test()",
            PatternSource::Manual,
        );
        let initial_confidence = pattern.confidence;
        let initial_last_used = pattern.last_used;

        pattern.mark_used();

        assert_eq!(pattern.frequency, 2);
        assert!(pattern.confidence > initial_confidence);
        assert!(pattern.last_used >= initial_last_used);
    }

    #[test]
    fn test_pattern_mark_used_confidence_cap() {
        let mut pattern = ConversationPattern::new(
            PatternType::Code,
            "test",
            PatternSource::Manual,
        );

        // Set confidence near the cap
        pattern.confidence = 0.999;

        pattern.mark_used();

        // Confidence should not exceed 1.0
        assert!(pattern.confidence <= 1.0);
    }

    #[test]
    fn test_pattern_mark_used_frequency_overflow() {
        let mut pattern = ConversationPattern::new(
            PatternType::Code,
            "test",
            PatternSource::Manual,
        );

        // Set frequency near max
        pattern.frequency = u32::MAX - 1;

        pattern.mark_used();

        // Should saturate at max instead of overflowing
        assert_eq!(pattern.frequency, u32::MAX);
    }

    #[test]
    fn test_pattern_deactivate() {
        let mut pattern = ConversationPattern::new(
            PatternType::Reference,
            "test",
            PatternSource::Manual,
        );

        assert!(pattern.is_active);
        pattern.deactivate();
        assert!(!pattern.is_active);
    }

    #[test]
    fn test_pattern_activate() {
        let mut pattern = ConversationPattern::new(
            PatternType::Reference,
            "test",
            PatternSource::Manual,
        );

        pattern.deactivate();
        assert!(!pattern.is_active);

        pattern.activate();
        assert!(pattern.is_active);
    }

    #[test]
    fn test_pattern_activate_deactivate_cycle() {
        let mut pattern = ConversationPattern::preset(PatternType::Code, "test");

        // Multiple cycles
        for _ in 0..3 {
            pattern.deactivate();
            assert!(!pattern.is_active);
            pattern.activate();
            assert!(pattern.is_active);
        }
    }

    // =========================================================================
    // ConversationPattern Formatting Tests
    // =========================================================================

    #[test]
    fn test_format_line_active_high_frequency() {
        let mut pattern = ConversationPattern::preset(PatternType::Reference, "test-pattern");
        pattern.frequency = 15; // Above the 10 threshold for star marker

        let line = pattern.format_line();

        assert!(line.contains("🔗"));
        assert!(line.contains("引用模式"));
        assert!(line.contains("test-pattern"));
        assert!(line.contains("freq: 15"));
        assert!(line.contains("★")); // High frequency marker
        assert!(!line.contains("[inactive]"));
    }

    #[test]
    fn test_format_line_active_low_frequency() {
        let pattern = ConversationPattern::new(
            PatternType::Code,
            "test-pattern",
            PatternSource::Manual,
        );

        let line = pattern.format_line();

        assert!(line.contains("💻"));
        assert!(line.contains("代码模式"));
        assert!(!line.contains("★")); // Low frequency, no star
        assert!(!line.contains("[inactive]"));
    }

    #[test]
    fn test_format_line_inactive() {
        let mut pattern = ConversationPattern::preset(PatternType::Reference, "test-pattern");
        pattern.deactivate();

        let line = pattern.format_line();

        assert!(line.contains("[inactive]"));
    }

    #[test]
    fn test_format_for_prompt_with_description() {
        let pattern = ConversationPattern::preset(PatternType::Reference, r"\bPR\s*#\d+\b")
            .with_description("Pull Request reference format");

        let prompt = pattern.format_for_prompt();

        assert_eq!(prompt, r"\bPR\s*#\d+\b: Pull Request reference format");
    }

    #[test]
    fn test_format_for_prompt_without_description() {
        let pattern = ConversationPattern::preset(PatternType::Reference, "simple-pattern");

        let prompt = pattern.format_for_prompt();

        assert_eq!(prompt, "simple-pattern");
    }

    // =========================================================================
    // Serialization Tests
    // =========================================================================

    #[test]
    fn test_serialization() {
        let pattern = ConversationPattern::preset(PatternType::Reference, r"PR #\d+")
            .with_description("Test pattern");

        let json = serde_json::to_string(&pattern).unwrap();
        let decoded: ConversationPattern = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.pattern, pattern.pattern);
        assert_eq!(decoded.pattern_type, PatternType::Reference);
        assert_eq!(decoded.description, Some("Test pattern".to_string()));
    }

    #[test]
    fn test_serialization_with_tags() {
        let pattern = ConversationPattern::preset(PatternType::Code, r"fn \w+")
            .with_tag("rust")
            .with_tag("function");

        let json = serde_json::to_string(&pattern).unwrap();
        let decoded: ConversationPattern = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.tags, vec!["rust", "function"]);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let original = ConversationPattern::new(
            PatternType::Reference,
            r"issue #\d+",
            PatternSource::user_conversation("User said issue #42"),
        )
        .with_description("Issue reference")
        .with_tag("git");

        let json = serde_json::to_string(&original).unwrap();
        let decoded: ConversationPattern = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.id, original.id);
        assert_eq!(decoded.pattern_type, original.pattern_type);
        assert_eq!(decoded.pattern, original.pattern);
        assert_eq!(decoded.source, original.source);
        assert_eq!(decoded.frequency, original.frequency);
        assert_eq!(decoded.confidence, original.confidence);
        assert_eq!(decoded.is_active, original.is_active);
        assert_eq!(decoded.description, original.description);
        assert_eq!(decoded.tags, original.tags);
    }

    // =========================================================================
    // Default Patterns Tests
    // =========================================================================

    #[test]
    fn test_default_reference_patterns() {
        let refs = get_default_reference_patterns();
        assert!(!refs.is_empty());

        for p in &refs {
            assert_eq!(p.pattern_type, PatternType::Reference);
            assert!(p.source.is_preset());
            assert!(p.is_active);
            assert_eq!(p.confidence, 1.0);
            assert_eq!(p.frequency, 100);
        }
    }

    #[test]
    fn test_default_code_patterns() {
        let codes = get_default_code_patterns();
        assert!(!codes.is_empty());

        for p in &codes {
            assert_eq!(p.pattern_type, PatternType::Code);
            assert!(p.source.is_preset());
            assert!(p.is_active);
            assert_eq!(p.confidence, 1.0);
            assert_eq!(p.frequency, 100);
        }
    }

    #[test]
    fn test_default_patterns_have_descriptions() {
        let refs = get_default_reference_patterns();
        let codes = get_default_code_patterns();

        for p in refs.iter().chain(codes.iter()) {
            assert!(p.description.is_some(), "Pattern {} should have description", p.pattern);
        }
    }

    #[test]
    fn test_default_patterns_have_tags() {
        let refs = get_default_reference_patterns();
        let codes = get_default_code_patterns();

        for p in refs.iter().chain(codes.iter()) {
            assert!(!p.tags.is_empty(), "Pattern {} should have tags", p.pattern);
        }
    }

    // =========================================================================
    // Edge Cases and Boundary Tests
    // =========================================================================

    #[test]
    fn test_empty_pattern_string() {
        let pattern = ConversationPattern::new(
            PatternType::Reference,
            "",
            PatternSource::Manual,
        );

        assert_eq!(pattern.pattern, "");
    }

    #[test]
    fn test_special_regex_chars_in_pattern() {
        let pattern = ConversationPattern::new(
            PatternType::Code,
            r"fn\s+\w+\s*\([^)]*\)\s*\{",
            PatternSource::Manual,
        );

        assert_eq!(pattern.pattern, r"fn\s+\w+\s*\([^)]*\)\s*\{");
    }

    #[test]
    fn test_unicode_pattern() {
        let pattern = ConversationPattern::new(
            PatternType::Reference,
            "中文模式",
            PatternSource::user_conversation("测试"),
        );

        assert_eq!(pattern.pattern, "中文模式");
    }

    #[test]
    fn test_very_long_pattern() {
        let long_pattern = "x".repeat(10000);
        let pattern = ConversationPattern::new(
            PatternType::Code,
            long_pattern.clone(),
            PatternSource::Manual,
        );

        assert_eq!(pattern.pattern.len(), 10000);
    }

    #[test]
    fn test_confidence_boundary() {
        let mut pattern = ConversationPattern::new(
            PatternType::Code,
            "test",
            PatternSource::Manual,
        );

        // Test minimum confidence
        pattern.confidence = 0.0;
        assert_eq!(pattern.confidence, 0.0);

        // Test maximum confidence
        pattern.confidence = 1.0;
        assert_eq!(pattern.confidence, 1.0);
    }

    #[test]
    fn test_unique_ids() {
        let p1 = ConversationPattern::new(PatternType::Code, "test", PatternSource::Manual);
        let p2 = ConversationPattern::new(PatternType::Code, "test", PatternSource::Manual);

        // Each pattern should have a unique ID
        assert_ne!(p1.id, p2.id);
    }
}