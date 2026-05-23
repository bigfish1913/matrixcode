//! Core memory types and manager.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use super::config::*;
use super::retrieval::{
    TfIdfSearch, compute_relevance, expand_semantic_keywords, extract_context_keywords,
    extract_keywords_hybrid, has_contradiction_signal,
};
use crate::providers::Message;
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
    /// Key decisions made during task execution (e.g., "Chose React over Vue for this project")
    KeyDecision,
    /// Failed approaches to avoid repeating (e.g., "Direct file read failed, need to use glob first")
    FailedApproach,
    /// User intent patterns learned from interactions (e.g., "User prefers detailed explanations")
    UserIntentPattern,
    /// Task completion patterns (e.g., "User confirms completion by saying '好的'")
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
    pub fn new(category: MemoryCategory, content: String, source_session: Option<String>) -> Self {
        let id = uuid::Uuid::new_v4().to_string();
        Self {
            id,
            created_at: Utc::now(),
            last_referenced: Utc::now(),
            category,
            content,
            source_session,
            reference_count: 0,
            importance: category.default_importance(),
            tags: Vec::new(),
            is_manual: false,
        }
    }

    /// Create a manually added memory entry.
    pub fn manual(category: MemoryCategory, content: String) -> Self {
        let mut entry = Self::new(category, content, None);
        entry.is_manual = true;
        entry.importance = 95.0;
        entry
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

// ============================================================================
// Auto Memory Manager
// ============================================================================

/// Manager for automatic memory accumulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoMemory {
    /// All memory entries.
    pub entries: Vec<MemoryEntry>,
    /// Configuration for memory management.
    #[serde(default)]
    pub config: MemoryConfig,
    /// Legacy fields for backward compatibility (deprecated).
    #[serde(default = "default_max_entries")]
    pub max_entries: usize,
    #[serde(default = "default_min_importance")]
    pub min_importance: f64,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Search index (not serialized, rebuilt on load).
    #[serde(skip)]
    search_index: Option<SearchIndex>,
}

/// Search index for fast lookups.
#[derive(Debug, Clone)]
struct SearchIndex {
    /// Lowercase content cache for each entry.
    content_lower: Vec<String>,
    /// Entries grouped by category.
    by_category: HashMap<MemoryCategory, Vec<usize>>,
    /// Entries sorted by importance (indices).
    by_importance: Vec<usize>,
    /// Total word frequency for relevance scoring (future use).
    #[allow(dead_code)]
    word_freq: HashMap<String, usize>,
}

impl SearchIndex {
    /// Build index from entries.
    fn build(entries: &[MemoryEntry]) -> Self {
        let content_lower: Vec<String> = entries.iter().map(|e| e.content.to_lowercase()).collect();

        let mut by_category: HashMap<MemoryCategory, Vec<usize>> = HashMap::new();
        for (i, entry) in entries.iter().enumerate() {
            by_category.entry(entry.category).or_default().push(i);
        }

        let mut by_importance: Vec<usize> = (0..entries.len()).collect();
        by_importance.sort_by(|a, b| {
            entries[*b]
                .importance
                .partial_cmp(&entries[*a].importance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut word_freq: HashMap<String, usize> = HashMap::new();
        for content in &content_lower {
            for word in content.split_whitespace() {
                *word_freq.entry(word.to_string()).or_default() += 1;
            }
        }

        Self {
            content_lower,
            by_category,
            by_importance,
            word_freq,
        }
    }

    /// Search by query with optional limit.
    fn search(
        &self,
        _entries: &[MemoryEntry],
        query_lower: &str,
        limit: Option<usize>,
    ) -> Vec<usize> {
        let matches: Vec<usize> = self
            .by_importance
            .iter()
            .filter(|&idx| self.content_lower[*idx].contains(query_lower))
            .copied()
            .collect();

        if let Some(max) = limit {
            matches.into_iter().take(max).collect()
        } else {
            matches
        }
    }

    /// Multi-keyword search (matches any keyword).
    fn search_multi(&self, keywords_lower: &[String]) -> Vec<usize> {
        self.by_importance
            .iter()
            .filter(|&idx| {
                let content = &self.content_lower[*idx];
                keywords_lower.iter().any(|k| content.contains(k))
            })
            .copied()
            .collect()
    }
}

fn default_max_entries() -> usize {
    100
}
fn default_min_importance() -> f64 {
    30.0
}
fn default_enabled() -> bool {
    true
}

impl Default for AutoMemory {
    fn default() -> Self {
        let config = MemoryConfig::default();
        Self {
            entries: Vec::new(),
            config: config.clone(),
            max_entries: config.max_entries,
            min_importance: config.min_importance,
            enabled: config.enabled,
            search_index: None,
        }
    }
}

impl AutoMemory {
    /// Create a new auto memory manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Ensure search index is built.
    fn ensure_index(&mut self) {
        if self.search_index.is_none() {
            self.rebuild_index();
        }
    }

    /// Rebuild search index.
    pub fn rebuild_index(&mut self) {
        self.search_index = Some(SearchIndex::build(&self.entries));
    }

    /// Invalidate search index (call after modifications).
    fn invalidate_index(&mut self) {
        self.search_index = None;
    }

    /// Create with custom configuration.
    pub fn with_config(config: MemoryConfig) -> Self {
        Self {
            entries: Vec::new(),
            config: config.clone(),
            max_entries: config.max_entries,
            min_importance: config.min_importance,
            enabled: config.enabled,
            search_index: None,
        }
    }

    /// Create a minimal memory manager.
    pub fn minimal() -> Self {
        Self::with_config(MemoryConfig::minimal())
    }

    /// Create an archival memory manager.
    pub fn archival() -> Self {
        Self::with_config(MemoryConfig::archival())
    }

    /// Add a new memory entry.
    /// Add entry with duplicate check.
    pub fn add(&mut self, entry: MemoryEntry) {
        // Check for similar content before adding
        if self.has_similar(&entry.content) {
            log::debug!("Skipping duplicate memory: {}", entry.content);
            return;
        }

        // Check for conflicting memories (e.g., "使用 X" vs "使用 Y")
        if let Some(conflict_idx) = self.find_conflict(&entry.content, entry.category) {
            let old_content = self.entries[conflict_idx].content.clone();
            log::info!(
                "Memory conflict: '{}' supersedes '{}'",
                entry.content,
                old_content
            );
            self.entries.remove(conflict_idx);
            self.invalidate_index();
        }

        self.entries.push(entry);
        self.invalidate_index();
        self.prune();
    }

    /// Add memory from detected content.
    pub fn add_memory(
        &mut self,
        category: MemoryCategory,
        content: String,
        source_session: Option<String>,
    ) {
        if self.has_similar(&content) {
            return;
        }

        if let Some(conflict_idx) = self.find_conflict(&content, category) {
            let old_content = self.entries[conflict_idx].content.clone();
            log::debug!(
                "Memory conflict detected: '{}' supersedes '{}'",
                content,
                old_content
            );
            self.entries.remove(conflict_idx);
            self.invalidate_index();
        }

        let entry = MemoryEntry::new(category, content, source_session);
        self.add(entry);
    }

    /// Find a conflicting memory entry.
    fn find_conflict(&self, new_content: &str, category: MemoryCategory) -> Option<usize> {
        let new_lower = new_content.to_lowercase();
        let new_words: HashSet<&str> = new_lower.split_whitespace().collect();

        let has_change_signal = has_contradiction_signal("", &new_lower);
        let overlap_threshold = if has_change_signal {
            CONFLICT_OVERLAY_THRESHOLD_WITH_SIGNAL
        } else {
            CONFLICT_OVERLAY_THRESHOLD
        };

        for (i, entry) in self.entries.iter().enumerate() {
            if entry.category != category {
                continue;
            }

            let entry_lower = entry.content.to_lowercase();
            let entry_words: HashSet<&str> = entry_lower.split_whitespace().collect();

            let intersection = new_words.intersection(&entry_words).count();
            let min_len = new_words.len().min(entry_words.len());

            if min_len == 0 {
                continue;
            }

            let topic_overlap = intersection as f64 / min_len as f64;
            let jaccard = Self::calculate_similarity(&entry_lower, &new_lower);

            if topic_overlap > overlap_threshold
                && jaccard < SIMILARITY_THRESHOLD
                && has_contradiction_signal(&entry_lower, &new_lower)
            {
                return Some(i);
            }

            if has_change_signal {
                let old_key_terms: Vec<&str> = entry_words
                    .iter()
                    .filter(|w| w.len() > 2)
                    .copied()
                    .collect();
                let referenced = old_key_terms.iter().any(|term| new_lower.contains(term));
                if referenced {
                    return Some(i);
                }
            }
        }

        None
    }

    /// Check if similar content already exists.
    pub fn has_similar(&self, content: &str) -> bool {
        let content_lower = content.to_lowercase();

        if content_lower.len() < MIN_SIMILARITY_LENGTH {
            return false;
        }

        for e in &self.entries {
            let entry_lower = e.content.to_lowercase();

            if entry_lower == content_lower {
                log::debug!("Exact duplicate found: {}", content);
                return true;
            }

            if entry_lower.len() < MIN_SIMILARITY_LENGTH {
                continue;
            }

            let similarity = Self::calculate_similarity(&entry_lower, &content_lower);
            if similarity >= SIMILARITY_THRESHOLD {
                log::debug!(
                    "Similar memory found (similarity={:.2}): '{}' vs '{}'",
                    similarity,
                    e.content,
                    content
                );
                crate::debug::debug_log().log("MEMORY_DUPLICATE",
                    &format!("similarity={:.2}, existing='{}', new='{}'",
                        similarity,
                        truncate(&e.content, 50),
                        truncate(content, 50)));
                return true;
            }
        }

        false
    }

    /// Calculate word-based similarity between two strings.
    pub fn calculate_similarity(a: &str, b: &str) -> f64 {
        let a_words: HashSet<&str> = a.split_whitespace().collect();
        let b_words: HashSet<&str> = b.split_whitespace().collect();

        if a_words.is_empty() || b_words.is_empty() {
            return 0.0;
        }

        let intersection = a_words.intersection(&b_words).count();
        let union = a_words.union(&b_words).count();

        if union == 0 {
            0.0
        } else {
            intersection as f64 / union as f64
        }
    }

    /// Remove low-importance entries when exceeding max_entries.
    pub fn prune(&mut self) {
        if self.entries.len() <= self.max_entries {
            return;
        }

        let (manual_entries, auto_entries): (Vec<_>, Vec<_>) =
            self.entries.iter().cloned().partition(|e| e.is_manual);

        let mut sorted_auto = auto_entries;
        sorted_auto.sort_by(|a, b| {
            let importance_cmp = b
                .importance
                .partial_cmp(&a.importance)
                .unwrap_or(std::cmp::Ordering::Equal);
            if importance_cmp == std::cmp::Ordering::Equal {
                b.last_referenced.cmp(&a.last_referenced)
            } else {
                importance_cmp
            }
        });

        let kept_auto: Vec<_> = sorted_auto
            .into_iter()
            .filter(|e| e.importance >= self.min_importance)
            .take(self.max_entries.saturating_sub(manual_entries.len()))
            .collect();

        self.entries = manual_entries.into_iter().chain(kept_auto).collect();

        if self.entries.len() > self.max_entries {
            self.entries.sort_by(|a, b| {
                let importance_cmp = b
                    .importance
                    .partial_cmp(&a.importance)
                    .unwrap_or(std::cmp::Ordering::Equal);
                if importance_cmp == std::cmp::Ordering::Equal {
                    b.last_referenced.cmp(&a.last_referenced)
                } else {
                    importance_cmp
                }
            });
            self.entries.truncate(self.max_entries);
        }

        self.invalidate_index();
    }

    /// Smart merge of similar memories.
    pub fn smart_merge(&mut self) -> usize {
        if self.entries.len() < 2 {
            return 0;
        }

        let mut merged_count = 0;
        let mut to_remove: Vec<String> = Vec::new();
        let mut new_entries: Vec<MemoryEntry> = Vec::new();
        let mut processed: HashSet<String> = HashSet::new();

        for i in 0..self.entries.len() {
            let entry_i = &self.entries[i];
            if processed.contains(&entry_i.id) {
                continue;
            }

            let mut similar_group: Vec<usize> = vec![i];

            for j in (i + 1)..self.entries.len() {
                let entry_j = &self.entries[j];
                if processed.contains(&entry_j.id) {
                    continue;
                }

                if entry_i.category != entry_j.category {
                    continue;
                }

                let similarity = Self::calculate_similarity(&entry_i.content, &entry_j.content);
                if similarity >= MERGE_SIMILARITY_THRESHOLD {
                    similar_group.push(j);
                }
            }

            if similar_group.len() >= 2 {
                let group_entries: Vec<&MemoryEntry> = similar_group
                    .iter()
                    .map(|&idx| &self.entries[idx])
                    .collect();

                let merged = self.merge_group(&group_entries);

                for entry in &group_entries {
                    to_remove.push(entry.id.clone());
                    processed.insert(entry.id.clone());
                }

                new_entries.push(merged);
                merged_count += similar_group.len() - 1;
            } else {
                processed.insert(entry_i.id.clone());
            }
        }

        for id in &to_remove {
            self.remove(id);
        }

        for entry in new_entries {
            self.add(entry);
        }

        if merged_count > 0 {
            log::debug!("Smart merge: reduced {} entries", merged_count);
            self.invalidate_index();
        }

        merged_count
    }

    /// Merge a group of similar entries into one.
    fn merge_group(&self, entries: &[&MemoryEntry]) -> MemoryEntry {
        // SAFETY: entries is guaranteed non-empty by caller (similar_group.len() >= 2)
        let best = entries
            .iter()
            .max_by(|a, b| {
                let score_a = a.importance + (a.content.len() as f64 / 100.0);
                let score_b = b.importance + (b.content.len() as f64 / 100.0);
                score_b
                    .partial_cmp(&score_a)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .expect("merge_group called with empty entries");

        let all_same = entries
            .iter()
            .all(|e| Self::calculate_similarity(&e.content, &best.content) >= 0.95);

        if all_same {
            let mut merged: MemoryEntry = (*best).clone();
            merged.importance = entries
                .iter()
                .map(|e| e.importance)
                .fold(best.importance, |max, val| val.max(max));
            merged.tags.push("merged".to_string());
            return merged;
        }

        let mut merged_content = best.content.clone();

        for entry in entries {
            if entry.id == best.id {
                continue;
            }
            let unique_words = entry
                .content
                .split_whitespace()
                .filter(|word| !best.content.contains(word))
                .take(3)
                .collect::<Vec<_>>();

            if !unique_words.is_empty() {
                let additions = unique_words.join(", ");
                if additions.len() > 10 {
                    merged_content =
                        format!("{} ({})", merged_content.trim_end_matches('.'), additions);
                }
            }
        }

        let mut merged = MemoryEntry::new(best.category, merged_content, None);
        merged.importance = entries
            .iter()
            .map(|e| e.importance)
            .fold(best.importance, |max, val| val.max(max))
            + 5.0;
        merged.importance = merged.importance.min(MAX_IMPORTANCE_CEILING);

        merged.tags.push("merged".to_string());
        for entry in entries {
            for tag in &entry.tags {
                if !merged.tags.contains(tag) && !tag.starts_with("merged") {
                    merged.tags.push(tag.clone());
                }
            }
        }

        merged.is_manual = entries.iter().any(|e| e.is_manual);

        merged
    }

    /// Get entries by category.
    pub fn by_category(&self, category: MemoryCategory) -> Vec<&MemoryEntry> {
        self.entries
            .iter()
            .filter(|e| e.category == category)
            .collect()
    }

    /// Get entries by category using index.
    pub fn by_category_fast(&mut self, category: MemoryCategory) -> Vec<&MemoryEntry> {
        self.ensure_index();
        if let Some(ref index) = self.search_index {
            index
                .by_category
                .get(&category)
                .map(|indices| indices.iter().map(|&i| &self.entries[i]).collect())
                .unwrap_or_default()
        } else {
            self.by_category(category)
        }
    }

    /// Get top N most important entries.
    pub fn top_n(&self, n: usize) -> Vec<&MemoryEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| {
            b.importance
                .partial_cmp(&a.importance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted.into_iter().take(n).collect()
    }

    /// Get top N using index.
    pub fn top_n_fast(&mut self, n: usize) -> Vec<&MemoryEntry> {
        self.ensure_index();
        if let Some(ref index) = self.search_index {
            index
                .by_importance
                .iter()
                .take(n)
                .map(|&i| &self.entries[i])
                .collect()
        } else {
            self.top_n(n)
        }
    }

    /// Search entries by content or tags.
    pub fn search(&self, query: &str) -> Vec<&MemoryEntry> {
        self.search_with_limit(query, None)
    }

    /// Search entries with result limit.
    pub fn search_with_limit(&self, query: &str, limit: Option<usize>) -> Vec<&MemoryEntry> {
        let query_lower = query.to_lowercase();
        let mut results: Vec<_> = self
            .entries
            .iter()
            .filter(|e| {
                e.content.to_lowercase().contains(&query_lower)
                    || e.tags
                        .iter()
                        .any(|t| t.to_lowercase().contains(&query_lower))
            })
            .collect();

        results.sort_by(|a, b| {
            b.importance
                .partial_cmp(&a.importance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        if let Some(max) = limit {
            results.into_iter().take(max).collect()
        } else {
            results
        }
    }

    /// Search using index.
    pub fn search_fast(&mut self, query: &str, limit: Option<usize>) -> Vec<&MemoryEntry> {
        self.ensure_index();
        let query_lower = query.to_lowercase();

        if let Some(ref index) = self.search_index {
            let indices = index.search(&self.entries, &query_lower, limit);
            indices.iter().map(|&i| &self.entries[i]).collect()
        } else {
            self.search_with_limit(query, limit)
        }
    }

    /// Multi-keyword search.
    pub fn search_multi(&self, keywords: &[&str]) -> Vec<&MemoryEntry> {
        if keywords.is_empty() {
            return Vec::new();
        }

        let keywords_lower: Vec<String> = keywords.iter().map(|k| k.to_lowercase()).collect();

        self.entries
            .iter()
            .filter(|e| {
                let content_lower = e.content.to_lowercase();
                keywords_lower.iter().any(|k| content_lower.contains(k))
            })
            .collect()
    }

    /// Multi-keyword search using index.
    pub fn search_multi_fast(&mut self, keywords: &[&str]) -> Vec<&MemoryEntry> {
        if keywords.is_empty() {
            return Vec::new();
        }

        self.ensure_index();
        let keywords_lower: Vec<String> = keywords.iter().map(|k| k.to_lowercase()).collect();

        if let Some(ref index) = self.search_index {
            let indices = index.search_multi(&keywords_lower);
            indices.iter().map(|&i| &self.entries[i]).collect()
        } else {
            self.search_multi(keywords)
        }
    }

    /// Batch add multiple entries.
    pub fn add_batch(&mut self, entries: Vec<MemoryEntry>) {
        for entry in entries {
            if !self.has_similar(&entry.content) {
                self.entries.push(entry);
            }
        }
        self.prune();
    }

    /// Mark entries as referenced.
    pub fn update_references(&mut self, messages: &[Message]) {
        let increment = self.config.reference_increment;

        let texts_lower: Vec<String> = messages
            .iter()
            .filter_map(Self::extract_message_text_lower)
            .collect();

        let entry_contents_lower: Vec<String> = self
            .entries
            .iter()
            .map(|e| e.content.to_lowercase())
            .collect();

        for (i, entry) in self.entries.iter_mut().enumerate() {
            let entry_lower = &entry_contents_lower[i];
            if texts_lower.iter().any(|t| t.contains(entry_lower)) {
                entry.mark_referenced_with_increment(increment);
            }
        }
    }

    /// Extract lowercase text from a message.
    fn extract_message_text_lower(msg: &Message) -> Option<String> {
        match &msg.content {
            crate::providers::MessageContent::Text(t) => Some(t.to_lowercase()),
            crate::providers::MessageContent::Blocks(blocks) => {
                let text = blocks
                    .iter()
                    .filter_map(|b| {
                        if let crate::providers::ContentBlock::Text { text } = b {
                            Some(text.as_str())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                Some(text.to_lowercase())
            }
        }
    }

    /// Generate summary for system prompt.
    pub fn generate_prompt_summary(&self, max_entries: usize) -> String {
        if self.entries.is_empty() {
            return String::new();
        }

        let top_entries = self.top_n(max_entries);
        if top_entries.is_empty() {
            return String::new();
        }

        let mut summary = String::from("【自动记忆摘要】\n\n");

        let mut by_cat: HashMap<MemoryCategory, Vec<&MemoryEntry>> = HashMap::new();
        for entry in top_entries {
            by_cat.entry(entry.category).or_default().push(entry);
        }

        for (cat, entries) in by_cat {
            summary.push_str(&format!("{} {}:\n", cat.icon(), cat.display_name()));
            for entry in entries {
                summary.push_str(&format!("  {}\n", entry.format_for_prompt()));
            }
            summary.push('\n');
        }

        summary
    }

    /// Generate context-aware summary.
    pub fn generate_contextual_summary(&self, context: &str, max_entries: usize) -> String {
        let keywords = extract_context_keywords(context);
        self.generate_contextual_summary_with_keywords(&keywords, max_entries)
    }

    /// Generate context-aware summary with pre-extracted keywords.
    pub fn generate_contextual_summary_with_keywords(
        &self,
        context_keywords: &[String],
        max_entries: usize,
    ) -> String {
        if self.entries.is_empty() {
            return String::new();
        }

        let expanded_keywords = expand_semantic_keywords(context_keywords);

        let mut tfidf = TfIdfSearch::new();
        tfidf.index(self);
        let keywords_slice: Vec<&str> = expanded_keywords.iter().map(|s| s.as_str()).collect();
        let tfidf_results = tfidf.search_multi(&keywords_slice, Some(max_entries * 2));

        let mut tfidf_scores: HashMap<String, f64> = HashMap::new();
        for (content, score) in &tfidf_results {
            if let Some(entry) = self.entries.iter().find(|e| &e.content == content) {
                tfidf_scores.insert(entry.id.clone(), *score);
            }
        }

        let mut scored: Vec<(&MemoryEntry, f64)> = self
            .entries
            .iter()
            .map(|entry| {
                let relevance = compute_relevance(entry, &expanded_keywords);
                let tfidf = tfidf_scores.get(&entry.id).copied().unwrap_or(0.0);
                let combined = tfidf * 0.4 + relevance * 0.6;
                (entry, combined)
            })
            .collect();

        scored.sort_by(|a, b| {
            if a.0.is_manual && !b.0.is_manual {
                return std::cmp::Ordering::Less;
            }
            if !a.0.is_manual && b.0.is_manual {
                return std::cmp::Ordering::Greater;
            }

            let score_a = a.1 * CONTEXT_RELEVANCE_WEIGHT
                + (a.0.importance / MAX_IMPORTANCE_CEILING) * CONTEXT_IMPORTANCE_WEIGHT;
            let score_b = b.1 * CONTEXT_RELEVANCE_WEIGHT
                + (b.0.importance / MAX_IMPORTANCE_CEILING) * CONTEXT_IMPORTANCE_WEIGHT;

            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let selected: Vec<&MemoryEntry> = scored
            .iter()
            .take(max_entries)
            .map(|(entry, _)| *entry)
            .collect();

        if selected.is_empty() {
            return String::new();
        }

        let mut summary = String::from("【跨会话记忆】\n\n");

        let mut by_cat: HashMap<MemoryCategory, Vec<&MemoryEntry>> = HashMap::new();
        for entry in selected {
            by_cat.entry(entry.category).or_default().push(entry);
        }

        for (cat, entries) in by_cat {
            summary.push_str(&format!("{} {}:\n", cat.icon(), cat.display_name()));
            for entry in entries {
                summary.push_str(&format!("  {}\n", entry.format_for_prompt()));
            }
            summary.push('\n');
        }

        summary
    }

    /// Update reference statistics.
    pub fn update_retrieval_stats(&mut self, retrieved_ids: &[String]) {
        for id in retrieved_ids {
            if let Some(entry) = self.entries.iter_mut().find(|e| &e.id == id) {
                entry.mark_referenced();
                log::debug!("Updated reference stats for memory {}", id);
            }
        }
    }

    /// Get IDs of entries for retrieval.
    pub fn get_retrieval_ids(
        &self,
        context_keywords: &[String],
        max_entries: usize,
    ) -> Vec<String> {
        if self.entries.is_empty() {
            return Vec::new();
        }

        let expanded_keywords = expand_semantic_keywords(context_keywords);

        let mut scored: Vec<(&MemoryEntry, f64)> = self
            .entries
            .iter()
            .map(|entry| {
                let relevance = compute_relevance(entry, &expanded_keywords);
                (entry, relevance)
            })
            .collect();

        scored.sort_by(|a, b| {
            if a.0.is_manual && !b.0.is_manual {
                return std::cmp::Ordering::Less;
            }
            if !a.0.is_manual && b.0.is_manual {
                return std::cmp::Ordering::Greater;
            }

            let score_a = a.1 + (a.0.importance / MAX_IMPORTANCE_CEILING);
            let score_b = b.1 + (b.0.importance / MAX_IMPORTANCE_CEILING);

            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        scored
            .iter()
            .take(max_entries)
            .map(|(e, _)| e.id.clone())
            .collect()
    }

    /// Generate context-aware summary async with AI keyword extraction.
    pub async fn generate_contextual_summary_async(
        &self,
        context: &str,
        max_entries: usize,
        fast_provider: Option<&dyn crate::providers::Provider>,
    ) -> String {
        if self.entries.is_empty() {
            return String::new();
        }

        let context_keywords = if let Some(provider) = fast_provider {
            extract_keywords_hybrid(context, Some(provider)).await
        } else {
            extract_context_keywords(context)
        };

        let mut scored: Vec<(&MemoryEntry, f64)> = self
            .entries
            .iter()
            .map(|entry| {
                let relevance = compute_relevance(entry, &context_keywords);
                (entry, relevance)
            })
            .collect();

        scored.sort_by(|a, b| {
            if a.0.is_manual && !b.0.is_manual {
                return std::cmp::Ordering::Less;
            }
            if !a.0.is_manual && b.0.is_manual {
                return std::cmp::Ordering::Greater;
            }

            let score_a = a.1 * CONTEXT_RELEVANCE_WEIGHT
                + (a.0.importance / MAX_IMPORTANCE_CEILING) * CONTEXT_IMPORTANCE_WEIGHT;
            let score_b = b.1 * CONTEXT_RELEVANCE_WEIGHT
                + (b.0.importance / MAX_IMPORTANCE_CEILING) * CONTEXT_IMPORTANCE_WEIGHT;

            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let selected: Vec<&MemoryEntry> = scored
            .iter()
            .take(max_entries)
            .map(|(entry, _)| *entry)
            .collect();

        if selected.is_empty() {
            return String::new();
        }

        let mut summary = String::from("【跨会话记忆】\n\n");

        let mut by_cat: HashMap<MemoryCategory, Vec<&MemoryEntry>> = HashMap::new();
        for entry in selected {
            by_cat.entry(entry.category).or_default().push(entry);
        }

        for (cat, entries) in by_cat {
            summary.push_str(&format!("{} {}:\n", cat.icon(), cat.display_name()));
            for entry in entries {
                summary.push_str(&format!("  {}\n", entry.format_for_prompt()));
            }
            summary.push('\n');
        }

        summary
    }

    /// Format all entries for display.
    pub fn format_all(&self) -> String {
        if self.entries.is_empty() {
            return "[no memories accumulated]".to_string();
        }

        let mut result = String::from("Accumulated memories:\n\n");

        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| {
            b.importance
                .partial_cmp(&a.importance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for entry in sorted {
            result.push_str(&entry.format_line());
            result.push('\n');
        }

        result
    }

    /// Generate statistics summary.
    pub fn generate_statistics(&self) -> MemoryStatistics {
        let total = self.entries.len();
        let manual = self.entries.iter().filter(|e| e.is_manual).count();
        let auto = total - manual;

        let by_category: HashMap<MemoryCategory, usize> =
            self.entries.iter().fold(HashMap::new(), |mut acc, e| {
                *acc.entry(e.category).or_default() += 1;
                acc
            });

        let avg_importance = if total > 0 {
            self.entries.iter().map(|e| e.importance).sum::<f64>() / total as f64
        } else {
            0.0
        };

        let oldest = self
            .entries
            .iter()
            .min_by_key(|e| e.created_at)
            .map(|e| e.created_at);
        let newest = self
            .entries
            .iter()
            .max_by_key(|e| e.created_at)
            .map(|e| e.created_at);

        let highly_referenced = self
            .entries
            .iter()
            .filter(|e| e.reference_count >= 3)
            .count();

        MemoryStatistics {
            total,
            manual,
            auto,
            by_category,
            avg_importance,
            oldest,
            newest,
            highly_referenced,
        }
    }

    /// Clear all memories.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.invalidate_index();
    }

    /// Remove a specific memory by ID.
    pub fn remove(&mut self, id: &str) -> bool {
        let idx = self.entries.iter().position(|e| e.id == id);
        if let Some(i) = idx {
            self.entries.remove(i);
            self.invalidate_index();
            true
        } else {
            false
        }
    }

    /// Apply time decay to memory importance.
    pub fn apply_time_decay(&mut self) {
        let now = Utc::now();
        let decay_start_days = self.config.decay_start_days;
        let decay_rate = self.config.decay_rate;
        let decay_period_days = 30;

        for entry in &mut self.entries {
            if entry.is_manual {
                continue;
            }

            let days_since_reference = (now - entry.last_referenced).num_days().max(0);

            if days_since_reference > decay_start_days {
                let decay_periods = (days_since_reference - decay_start_days) / decay_period_days;
                let decay_factor = decay_rate.powi(decay_periods as i32);
                entry.importance *= decay_factor;
                entry.importance = entry.importance.max(self.min_importance * 0.5);
            }
        }

        self.prune();
    }
}

// ============================================================================
// Memory Statistics
// ============================================================================

/// Statistics about memory collection.
#[derive(Debug, Clone)]
pub struct MemoryStatistics {
    /// Total number of entries.
    pub total: usize,
    /// Number of manually added entries.
    pub manual: usize,
    /// Number of automatically detected entries.
    pub auto: usize,
    /// Count by category.
    pub by_category: HashMap<MemoryCategory, usize>,
    /// Average importance score.
    pub avg_importance: f64,
    /// Oldest entry creation time.
    pub oldest: Option<DateTime<Utc>>,
    /// Newest entry creation time.
    pub newest: Option<DateTime<Utc>>,
    /// Number of entries with high reference count.
    pub highly_referenced: usize,
}

impl MemoryStatistics {
    /// Format statistics for display.
    pub fn format_summary(&self) -> String {
        let mut output = String::new();

        output.push_str("记忆统计：\n");
        output.push_str(&format!("  总计: {} 条\n", self.total));
        output.push_str(&format!("  ├─ 手动添加: {} 条\n", self.manual));
        output.push_str(&format!("  └─ 自动检测: {} 条\n", self.auto));
        output.push('\n');

        output.push_str("分类统计：\n");
        for (cat, count) in &self.by_category {
            output.push_str(&format!(
                "  {} {}: {} 条\n",
                cat.icon(),
                cat.display_name(),
                count
            ));
        }
        output.push('\n');

        output.push_str("质量指标：\n");
        output.push_str(&format!("  平均重要性: {:.1} 分\n", self.avg_importance));
        output.push_str(&format!(
            "  高频引用: {} 条 (≥3次)\n",
            self.highly_referenced
        ));

        if let Some(oldest) = self.oldest {
            let days = (Utc::now() - oldest).num_days();
            output.push_str(&format!("  记忆跨度: {} 天\n", days));
        }

        output
    }
}
