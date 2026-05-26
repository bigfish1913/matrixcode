//! Memory entry types and categories.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
        Self {
            id,
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
        }
    }

    /// Create a manually added memory entry.
    pub fn manual(category: MemoryCategory, content: String, project_path: Option<String>) -> Self {
        let mut entry = Self::new(category, content, None, project_path);
        entry.is_manual = true;
        entry.importance = 95.0;
        entry
    }

    /// Create a manually added memory entry (global, no project path).
    pub fn manual_global(category: MemoryCategory, content: String) -> Self {
        Self::manual(category, content, None)
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
        format!(
            "{} {} {}{}{} {}",
            self.category.icon(),
            time,
            importance_marker,
            manual_marker,
            self.category.display_name(),
            truncate_str(&self.content, MAX_DISPLAY_LENGTH)
        )
    }

    /// Format for inclusion in system prompt.
    pub fn format_for_prompt(&self) -> String {
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