//! Memory entry types and categories.
//!
//! Supports memory linking with `[[name]]` syntax for cross-referencing
//! related memories. Links are parsed and can be resolved during retrieval.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use super::config::*;
use crate::truncate::{find_boundary, truncate_with_suffix};

// ============================================================================
// Helper Functions
// ============================================================================

/// Truncate string with "..." suffix, respecting UTF-8 boundaries.
pub(crate) fn truncate_str(s: &str, max_len: usize) -> String {
    truncate_with_suffix(s, max_len)
}

/// Truncate string without suffix, respecting UTF-8 boundaries.
pub(crate) fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let end = find_boundary(s, max_len);
        s[..end].to_string()
    }
}

/// Parse `[[name]]` link syntax from content.
/// Returns a set of linked memory names/IDs.
pub fn parse_memory_links(content: &str) -> HashSet<String> {
    // Match [[name]] pattern
    let re = regex::Regex::new(r"\[\[([^\]]+)\]\]").unwrap();
    re.captures_iter(content)
        .map(|c| c[1].trim().to_string())
        .collect()
}

/// Check if content contains memory links.
pub fn has_memory_links(content: &str) -> bool {
    content.contains("[[") && content.contains("]]")
}

// ============================================================================
// Memory Categories
// ============================================================================

/// Categories for memory entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum MemoryCategory {
    /// User preferences (e.g., "I prefer vim over nano")
    Preference,
    /// Project decisions (e.g., "Decided to use PostgreSQL")
    Decision,
    /// Key findings (e.g., "API endpoint is at /api/v2")
    Finding,
    /// Problem solutions (e.g., "Fixed auth bug by adding token refresh")
    Solution,
    /// Technical notes (e.g., "React Query is used for data fetching")
    Technical,
    /// Project structure (e.g., "src/index.ts is entry point")
    Structure,
    /// Key decisions made during task execution
    KeyDecision,
    /// Failed approaches to avoid repeating
    FailedApproach,
    /// User intent patterns learned from interactions
    UserIntentPattern,
    /// Task completion patterns
    TaskPattern,
}

impl MemoryCategory {
    /// Get display name for the category.
    pub fn display_name(&self) -> &'static str {
        match self {
            MemoryCategory::Preference => "偏好",
            MemoryCategory::Decision => "决策",
            MemoryCategory::Finding => "发现",
            MemoryCategory::Solution => "解决方案",
            MemoryCategory::Technical => "技术",
            MemoryCategory::Structure => "结构",
            MemoryCategory::KeyDecision => "关键决策",
            MemoryCategory::FailedApproach => "失败方案",
            MemoryCategory::UserIntentPattern => "意图模式",
            MemoryCategory::TaskPattern => "任务模式",
        }
    }

    /// Get icon for the category.
    pub fn icon(&self) -> &'static str {
        match self {
            MemoryCategory::Preference => "👤",
            MemoryCategory::Decision => "🎯",
            MemoryCategory::Finding => "💡",
            MemoryCategory::Solution => "🔧",
            MemoryCategory::Technical => "📚",
            MemoryCategory::Structure => "🏗️",
            MemoryCategory::KeyDecision => "⚡",
            MemoryCategory::FailedApproach => "❌",
            MemoryCategory::UserIntentPattern => "🧠",
            MemoryCategory::TaskPattern => "📋",
        }
    }

    /// Get default importance score for the category.
    pub fn default_importance(&self) -> f64 {
        match self {
            MemoryCategory::Decision => DEFAULT_IMPORTANCE_DECISION,
            MemoryCategory::Solution => DEFAULT_IMPORTANCE_SOLUTION,
            MemoryCategory::Preference => DEFAULT_IMPORTANCE_PREF,
            MemoryCategory::Finding => DEFAULT_IMPORTANCE_FINDING,
            MemoryCategory::Technical => DEFAULT_IMPORTANCE_TECH,
            MemoryCategory::Structure => DEFAULT_IMPORTANCE_STRUCTURE,
            MemoryCategory::KeyDecision => 85.0,
            MemoryCategory::FailedApproach => 70.0,
            MemoryCategory::UserIntentPattern => 80.0,
            MemoryCategory::TaskPattern => 75.0,
        }
    }
}

// ============================================================================
// Memory Entry
// ============================================================================

/// A single memory entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// Unique identifier.
    pub id: String,
    /// Short name for linking (optional). Used in `[[name]]` references.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// When the memory was created.
    pub created_at: DateTime<Utc>,
    /// When the memory was last accessed/referenced.
    pub last_referenced: DateTime<Utc>,
    /// Category of the memory.
    pub category: MemoryCategory,
    /// The memory content.
    pub content: String,
    /// Source session ID (where this memory was created).
    pub source_session: Option<String>,
    /// Project path where this memory was created.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_path: Option<String>,
    /// Number of times this memory has been referenced.
    pub reference_count: u32,
    /// Importance score (0-100, higher = more important).
    pub importance: f64,
    /// Tags for searching/filtering.
    pub tags: Vec<String>,
    /// Whether this memory was manually added by user.
    pub is_manual: bool,
    /// Related memory IDs/names (extracted from `[[name]]` syntax).
    #[serde(default, skip_serializing_if = "HashSet::is_empty")]
    pub related_memories: HashSet<String>,
}

impl MemoryEntry {
    /// Create a new memory entry.
    pub fn new(
        category: MemoryCategory,
        content: String,
        source_session: Option<String>,
        project_path: Option<String>,
    ) -> Self {
        let id = uuid::Uuid::new_v4().to_string();
        // Parse links from content
        let related_memories = parse_memory_links(&content);
        Self {
            id,
            name: None,
            created_at: Utc::now(),
            last_referenced: Utc::now(),
            category,
            content,
            source_session,
            project_path,
            reference_count: 0,
            importance: category.default_importance(),
            tags: Vec::new(),
            is_manual: false,
            related_memories,
        }
    }

    /// Create a new memory entry with a name (for linking).
    pub fn with_name(
        category: MemoryCategory,
        name: String,
        content: String,
        source_session: Option<String>,
        project_path: Option<String>,
    ) -> Self {
        let mut entry = Self::new(category, content, source_session, project_path);
        entry.name = Some(name);
        entry
    }

    /// Create a manually added memory entry.
    pub fn manual(category: MemoryCategory, content: String, project_path: Option<String>) -> Self {
        let mut entry = Self::new(category, content, None, project_path);
        entry.is_manual = true;
        entry.importance = 95.0;
        entry
    }

    /// Create a manually added memory entry with name.
    pub fn manual_with_name(
        category: MemoryCategory,
        name: String,
        content: String,
        project_path: Option<String>,
    ) -> Self {
        let mut entry = Self::manual(category, content, project_path);
        entry.name = Some(name);
        entry
    }

    /// Create a manually added memory entry (global, no project path).
    pub fn manual_global(category: MemoryCategory, content: String) -> Self {
        Self::manual(category, content, None)
    }

    /// Get linked memory names from content.
    pub fn get_links(&self) -> HashSet<String> {
        parse_memory_links(&self.content)
    }

    /// Check if this memory has any links.
    pub fn has_links(&self) -> bool {
        has_memory_links(&self.content)
    }

    /// Update related_memories by re-parsing content.
    pub fn refresh_links(&mut self) {
        self.related_memories = parse_memory_links(&self.content);
    }

    /// Mark this memory as referenced (increases importance over time).
    pub fn mark_referenced(&mut self) {
        self.mark_referenced_with_increment(2.0);
    }

    /// Mark this memory as referenced with custom importance increment.
    pub fn mark_referenced_with_increment(&mut self, increment: f64) {
        self.reference_count += 1;
        self.last_referenced = Utc::now();
        self.importance = (self.importance + increment).min(MAX_IMPORTANCE_CEILING);
    }

    /// Format for display.
    pub fn format_line(&self) -> String {
        let time = self.created_at.format("%Y-%m-%d %H:%M");
        let importance_marker = if self.importance >= IMPORTANCE_STAR_THRESHOLD {
            "⭐"
        } else {
            ""
        };
        let manual_marker = if self.is_manual { "📝" } else { "" };
        let link_marker = if self.has_links() { "🔗" } else { "" };
        let name_display = self.name.as_deref().map(|n| format!("[{}]", n)).unwrap_or_default();
        format!(
            "{} {} {}{}{}{} {}",
            self.category.icon(),
            time,
            importance_marker,
            manual_marker,
            link_marker,
            name_display,
            truncate_str(&self.content, MAX_DISPLAY_LENGTH)
        )
    }

    /// Format for inclusion in system prompt.
    /// Note: This is used inside category groups, so we don't repeat the category name.
    pub fn format_for_prompt(&self) -> String {
        if self.content.len() > MAX_MEMORY_CONTENT_LENGTH {
            format!(
                "{}...",
                truncate(&self.content, MAX_MEMORY_CONTENT_LENGTH - 3)
            )
        } else {
            self.content.clone()
        }
    }

    /// Format for inclusion in system prompt with category name.
    /// Use this when displaying entries outside of category groups.
    pub fn format_for_prompt_with_category(&self) -> String {
        let category_name = self.category.display_name();
        if self.content.len() > MAX_MEMORY_CONTENT_LENGTH {
            format!(
                "{}: {}...",
                category_name,
                truncate(&self.content, MAX_MEMORY_CONTENT_LENGTH - 3)
            )
        } else {
            format!("{}: {}", category_name, self.content)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_memory_links_single() {
        let content = "使用 [[redis-config]] 进行缓存配置";
        let links = parse_memory_links(content);
        assert_eq!(links.len(), 1);
        assert!(links.contains("redis-config"));
    }

    #[test]
    fn test_parse_memory_links_multiple() {
        let content = "参考 [[api-design]] 和 [[database-schema]] 进行开发";
        let links = parse_memory_links(content);
        assert_eq!(links.len(), 2);
        assert!(links.contains("api-design"));
        assert!(links.contains("database-schema"));
    }

    #[test]
    fn test_parse_memory_links_no_links() {
        let content = "这是一条普通记忆，没有链接";
        let links = parse_memory_links(content);
        assert!(links.is_empty());
    }

    #[test]
    fn test_parse_memory_links_with_spaces() {
        let content = "参考 [[  spaced-name  ]] 进行开发";
        let links = parse_memory_links(content);
        assert!(links.contains("spaced-name")); // trimmed
    }

    #[test]
    fn test_has_memory_links() {
        assert!(has_memory_links("参考 [[config]] 设置"));
        assert!(!has_memory_links("没有链接的记忆"));
    }

    #[test]
    fn test_memory_entry_extract_links_on_creation() {
        let entry = MemoryEntry::new(
            MemoryCategory::Decision,
            "使用 [[redis]] 作为缓存，参考 [[config-pattern]]".to_string(),
            None,
            None,
        );
        assert_eq!(entry.related_memories.len(), 2);
        assert!(entry.related_memories.contains("redis"));
        assert!(entry.related_memories.contains("config-pattern"));
    }

    #[test]
    fn test_memory_entry_with_name() {
        let entry = MemoryEntry::with_name(
            MemoryCategory::Technical,
            "api-endpoints".to_string(),
            "API 端点定义".to_string(),
            None,
            None,
        );
        assert_eq!(entry.name, Some("api-endpoints".to_string()));
    }

    #[test]
    fn test_memory_entry_refresh_links() {
        let mut entry = MemoryEntry::new(
            MemoryCategory::Decision,
            "初始内容".to_string(),
            None,
            None,
        );
        assert!(entry.related_memories.is_empty());

        // Update content with links
        entry.content = "更新内容，参考 [[new-link]]".to_string();
        entry.refresh_links();
        assert!(entry.related_memories.contains("new-link"));
    }

    #[test]
    fn test_format_line_with_links() {
        let entry = MemoryEntry::new(
            MemoryCategory::Technical,
            "参考 [[config]] 设置".to_string(),
            None,
            None,
        );
        let line = entry.format_line();
        assert!(line.contains("🔗")); // Link marker
    }

    #[test]
    fn test_format_line_with_name() {
        let entry = MemoryEntry::with_name(
            MemoryCategory::Decision,
            "cache-decision".to_string(),
            "决定使用 Redis".to_string(),
            None,
            None,
        );
        let line = entry.format_line();
        assert!(line.contains("[cache-decision]"));
    }
}
