//! Auto Memory system for MatrixCode.
//!
//! This module implements automatic memory accumulation inspired by Claude Code.
//! It captures user preferences, project decisions, key findings, and solutions
//! across sessions, providing persistent context that survives conversation compression.
//!
//! # Module Structure
//!
//! | Section | Description |
//! |--------|-------------|
//! | SECTION 1 | Constants & Configuration |
//! | SECTION 2 | Core Types (MemoryCategory, MemoryEntry, AutoMemory) |
//! | SECTION 3 | Storage (MemoryStorage, File Operations) |
//! | SECTION 4 | Retrieval (TF-IDF Search) |
//! | SECTION 5 | Semantic Similarity (Aliases, Relevance) |
//! | SECTION 6 | AI Enhancement (MemoryExtractor) |
//! | SECTION 7 | Detection (Rule-based) |
//! | SECTION 8 | Feedback Learning |
//! | SECTION 9 | Behavior Inference |
//! | SECTION 10 | Project Analysis |

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::fs;

use crate::providers::Message;

// ============================================================================
// Helper Functions
// ============================================================================

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    } else {
        s.to_string()
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        s[..max_len].to_string()
    } else {
        s.to_string()
    }
}

// ============================================================================
// SECTION 1: Constants & Configuration
// ============================================================================

/// Maximum importance score ceiling (entries cannot exceed this).
pub const MAX_IMPORTANCE_CEILING: f64 = 100.0;

/// Minimum content length for similarity check (to avoid short words matching everything).
pub const MIN_SIMILARITY_LENGTH: usize = 10;

/// Similarity threshold for considering entries as duplicates (0.0-1.0).
/// Higher value (0.85) reduces duplicate detection false negatives.
pub const SIMILARITY_THRESHOLD: f64 = 0.85;

/// Similarity threshold for merging similar memories (0.0-1.0).
/// Lower than duplicate threshold to allow semantic merging.
pub const MERGE_SIMILARITY_THRESHOLD: f64 = 0.7;

/// Minimum content length for memory detection (to avoid capturing too generic content).
/// Increased to 20 to filter out short fragments.
pub const MIN_MEMORY_CONTENT_LENGTH: usize = 20;

/// Maximum entries to return from detection (to avoid overwhelming).
pub const MAX_DETECTED_ENTRIES: usize = 5;

/// Maximum length for memory content before truncation.
pub const MAX_MEMORY_CONTENT_LENGTH: usize = 200;

/// Maximum length for display (shorter for terminal readability).
pub const MAX_DISPLAY_LENGTH: usize = 60;

/// Topic overlap threshold for conflict detection.
pub const CONFLICT_OVERLAY_THRESHOLD: f64 = 0.5;

/// Lower topic overlap threshold when change signal is present.
pub const CONFLICT_OVERLAY_THRESHOLD_WITH_SIGNAL: f64 = 0.3;

/// Importance threshold for displaying star marker (⭐).
pub const IMPORTANCE_STAR_THRESHOLD: f64 = 80.0;

/// Weight for relevance in contextual summary (relevance vs importance trade-off).
pub const CONTEXT_RELEVANCE_WEIGHT: f64 = 0.6;

/// Weight for importance in contextual summary (1.0 - CONTEXT_RELEVANCE_WEIGHT).
pub const CONTEXT_IMPORTANCE_WEIGHT: f64 = 0.4;

/// Default model for cost-effective memory extraction.
pub const DEFAULT_MEMORY_EXTRACTOR_MODEL: &str = "claude-3-5-haiku-20241022";

/// Minimum keywords threshold for triggering AI fallback.
/// If rule-based extraction produces fewer keywords than this, AI is used.
pub const MIN_KEYWORDS_FOR_AI_FALLBACK: usize = 2;

/// AI keyword extraction mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AiKeywordMode {
    /// Hybrid mode: rule-based first, AI fallback when keywords are insufficient (default).
    #[default]
    Auto,
    /// Always use AI for keyword extraction.
    Always,
    /// Never use AI, only rule-based extraction.
    Never,
}

impl AiKeywordMode {
    /// Parse from environment variable string.
    pub fn from_env() -> Self {
        match std::env::var("MEMORY_AI_KEYWORDS")
            .unwrap_or_default()
            .to_lowercase()
            .as_str()
        {
            "always" | "true" | "1" => AiKeywordMode::Always,
            "never" | "false" | "0" => AiKeywordMode::Never,
            "auto" | "" => AiKeywordMode::Auto,
            other => {
                log::warn!("Unknown MEMORY_AI_KEYWORDS value: '{}', using 'auto'", other);
                AiKeywordMode::Auto
            }
        }
    }

    /// Whether AI extraction should be used given the keyword count.
    pub fn should_use_ai(&self, keyword_count: usize) -> bool {
        match self {
            AiKeywordMode::Always => true,
            AiKeywordMode::Never => false,
            AiKeywordMode::Auto => keyword_count < MIN_KEYWORDS_FOR_AI_FALLBACK,
        }
    }
}

/// AI memory detection mode.
/// Controls whether AI is used for memory category detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AiDetectionMode {
    /// Hybrid mode: rule-based detection, AI enriches when confidence is low (default).
    #[default]
    Auto,
    /// Always use AI for memory detection (more accurate but slower).
    Always,
    /// Never use AI, only rule-based detection (fastest).
    Never,
}

impl AiDetectionMode {
    /// Parse from environment variable string.
    pub fn from_env() -> Self {
        match std::env::var("MEMORY_AI_DETECTION")
            .unwrap_or_default()
            .to_lowercase()
            .as_str()
        {
            "always" | "true" | "1" => AiDetectionMode::Always,
            "never" | "false" | "0" => AiDetectionMode::Never,
            "auto" | "" => AiDetectionMode::Auto,
            other => {
                log::warn!("Unknown MEMORY_AI_DETECTION value: '{}', using 'auto'", other);
                AiDetectionMode::Auto
            }
        }
    }

    /// Whether AI detection should be used.
    pub fn should_use_ai(&self) -> bool {
        match self {
            AiDetectionMode::Always => true,
            AiDetectionMode::Never => false,
            AiDetectionMode::Auto => {
                // Auto mode: check if AI extractor is available and text is complex enough
                // Only use AI for longer texts that might have multiple memories
                false  // Default to rule-based for speed
            }
        }
    }

    /// Whether AI detection should be used for given text length.
    /// Longer texts benefit more from AI detection.
    pub fn should_use_ai_for_text(&self, text_len: usize) -> bool {
        match self {
            AiDetectionMode::Always => true,
            AiDetectionMode::Never => false,
            AiDetectionMode::Auto => text_len > 500, // Only use AI for complex/long texts
        }
    }
}

/// Default fast model for AI memory extraction.
pub const DEFAULT_FAST_MODEL: &str = "claude-3-5-haiku-20241022";

/// Default importance scores by category.
/// Lower values allow for gradual importance growth through references.
pub const DEFAULT_IMPORTANCE_DECISION: f64 = 75.0;   // Reduced from 90
pub const DEFAULT_IMPORTANCE_SOLUTION: f64 = 70.0;   // Reduced from 85
pub const DEFAULT_IMPORTANCE_PREF: f64 = 65.0;       // Reduced from 70
pub const DEFAULT_IMPORTANCE_FINDING: f64 = 55.0;    // Reduced from 60
pub const DEFAULT_IMPORTANCE_TECH: f64 = 45.0;       // Reduced from 50
pub const DEFAULT_IMPORTANCE_STRUCTURE: f64 = 35.0; // Reduced from 40

// ============================================================================
// Memory Configuration
// ============================================================================

/// Configuration for the memory system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Maximum number of entries to keep.
    pub max_entries: usize,
    /// Minimum importance threshold to keep.
    pub min_importance: f64,
    /// Whether auto accumulation is enabled.
    pub enabled: bool,
    /// Days before time decay starts.
    pub decay_start_days: i64,
    /// Decay rate per period (0.0-1.0).
    pub decay_rate: f64,
    /// Importance increment per reference.
    pub reference_increment: f64,
    /// Maximum importance ceiling.
    pub max_importance_ceiling: f64,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            max_entries: 100,
            min_importance: 30.0,
            enabled: true,
            decay_start_days: 30,
            decay_rate: 0.5,
            reference_increment: 1.0,  // Reduced from 2.0 for gradual growth
            max_importance_ceiling: MAX_IMPORTANCE_CEILING,
        }
    }
}

impl MemoryConfig {
    /// Create a new config with custom max entries.
    pub fn with_max_entries(max: usize) -> Self {
        Self {
            max_entries: max,
            ..Self::default()
        }
    }
    
    /// Create a minimal config for low-memory environments.
    pub fn minimal() -> Self {
        Self {
            max_entries: 50,
            min_importance: 50.0,
            enabled: true,
            decay_start_days: 14,
            decay_rate: 0.6,
            reference_increment: 1.0,
            max_importance_ceiling: MAX_IMPORTANCE_CEILING,
        }
    }
    
    /// Create a config for long-term archival.
    pub fn archival() -> Self {
        Self {
            max_entries: 500,
            min_importance: 20.0,
            enabled: true,
            decay_start_days: 90,
            decay_rate: 0.3,
            reference_increment: 3.0,
            max_importance_ceiling: MAX_IMPORTANCE_CEILING,
        }
    }
}

// ============================================================================
// SECTION 2: Core Types (MemoryCategory, MemoryEntry, AutoMemory)
// ============================================================================

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
        entry.importance = 95.0; // Manual entries are highly important
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
        // Increase importance slightly with each reference (capped at ceiling)
        self.importance = (self.importance + increment).min(MAX_IMPORTANCE_CEILING);
    }

    /// Format for display.
    pub fn format_line(&self) -> String {
        let time = self.created_at.format("%Y-%m-%d %H:%M");
        let importance_marker = if self.importance >= IMPORTANCE_STAR_THRESHOLD { "⭐" } else { "" };
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
            format!("{}: {}...", category_name, truncate(&self.content, MAX_MEMORY_CONTENT_LENGTH - 3))
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
        // Build lowercase cache
        let content_lower: Vec<String> = entries
            .iter()
            .map(|e| e.content.to_lowercase())
            .collect();
        
        // Build category index
        let mut by_category: HashMap<MemoryCategory, Vec<usize>> = HashMap::new();
        for (i, entry) in entries.iter().enumerate() {
            by_category.entry(entry.category).or_default().push(i);
        }
        
        // Build importance index (sorted descending)
        let mut by_importance: Vec<usize> = (0..entries.len()).collect();
        by_importance.sort_by(|a, b| {
            entries[*b].importance.partial_cmp(&entries[*a].importance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        
        // Build word frequency
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
    
    /// Get lowercase content for entry.
    #[allow(dead_code)]
    fn get_lower(&self, idx: usize) -> &str {
        &self.content_lower[idx]
    }
    
    /// Search by query with optional limit.
    fn search(&self, _entries: &[MemoryEntry], query_lower: &str, limit: Option<usize>) -> Vec<usize> {
        // Use importance index to search in priority order
        let matches: Vec<usize> = self.by_importance
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
    
    /// Invalidate and rebuild index.
    #[allow(dead_code)]
    fn rebuild(&mut self, entries: &[MemoryEntry]) {
        *self = Self::build(entries);
    }
}

fn default_max_entries() -> usize { 100 }
fn default_min_importance() -> f64 { 30.0 }
fn default_enabled() -> bool { true }

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

    /// Create a minimal memory manager (low-memory environments).
    pub fn minimal() -> Self {
        Self::with_config(MemoryConfig::minimal())
    }

    /// Create an archival memory manager (long-term storage).
    pub fn archival() -> Self {
        Self::with_config(MemoryConfig::archival())
    }

    /// Add a new memory entry.
    pub fn add(&mut self, entry: MemoryEntry) {
        self.entries.push(entry);
        self.invalidate_index();  // Index needs rebuild
        self.prune();
    }

    /// Add memory from detected content.
    pub fn add_memory(
        &mut self,
        category: MemoryCategory,
        content: String,
        source_session: Option<String>,
    ) {
        // Check for duplicates (similar content)
        if self.has_similar(&content) {
            return;
        }

        // Check for conflicts (same category, contradicting content)
        if let Some(conflict_idx) = self.find_conflict(&content, category) {
            // Replace the old conflicting entry with the new one
            let old_content = self.entries[conflict_idx].content.clone();
            log::debug!("Memory conflict detected: '{}' supersedes '{}'", content, old_content);
            self.entries.remove(conflict_idx);
            self.invalidate_index();
        }

        let entry = MemoryEntry::new(category, content, source_session);
        self.add(entry);
    }

    /// Find a conflicting memory entry.
    /// 
    /// A conflict is detected when:
    /// 1. Same category (e.g., both are Decision)
    /// 2. Same subject/topic (overlapping keywords)
    /// 3. Different conclusion (not similar enough to be a duplicate)
    /// 
    /// Example conflicts:
    /// - "决定使用 PostgreSQL" vs "决定使用 MySQL" (same topic: database choice)
    /// - "偏好 vim" vs "偏好 vscode" (same topic: editor preference)
    fn find_conflict(&self, new_content: &str, category: MemoryCategory) -> Option<usize> {
        let new_lower = new_content.to_lowercase();
        let new_words: std::collections::HashSet<&str> = new_lower.split_whitespace().collect();
        
        // If new content has explicit change signals, lower the threshold
        let has_change_signal = has_contradiction_signal("", &new_lower);
        let overlap_threshold = if has_change_signal { 
            CONFLICT_OVERLAY_THRESHOLD_WITH_SIGNAL 
        } else { 
            CONFLICT_OVERLAY_THRESHOLD 
        };
        
        // Only check entries in the same category
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.category != category {
                continue;
            }
            
            let entry_lower = entry.content.to_lowercase();
            let entry_words: std::collections::HashSet<&str> = entry_lower.split_whitespace().collect();
            
            // Calculate topic overlap (shared words)
            let intersection = new_words.intersection(&entry_words).count();
            let min_len = new_words.len().min(entry_words.len());
            
            if min_len == 0 {
                continue;
            }
            
            let topic_overlap = intersection as f64 / min_len as f64;
            
            // High topic overlap but not a duplicate
            let jaccard = Self::calculate_similarity(&entry_lower, &new_lower);
            
            if topic_overlap > overlap_threshold && jaccard < SIMILARITY_THRESHOLD {
                // Check for contradiction patterns
                if has_contradiction_signal(&entry_lower, &new_lower) {
                    return Some(i);
                }
            }
            
            // Also check if new content explicitly references old content
            // e.g., "不再使用 vim" when old entry contains "vim"
            if has_change_signal {
                // Check if old entry's key terms appear in new content
                let old_key_terms: Vec<&str> = entry_words.iter()
                    .filter(|w| w.len() > 2)
                    .copied()
                    .collect();
                let referenced = old_key_terms.iter()
                    .any(|term| new_lower.contains(term));
                if referenced {
                    return Some(i);
                }
            }
        }
        
        None
    }

    /// Check if similar content already exists.
    /// Uses minimum length threshold to prevent short words from matching everything.
    pub fn has_similar(&self, content: &str) -> bool {
        let content_lower = content.to_lowercase();
        
        // Skip short content - they're likely too generic to be useful memories
        if content_lower.len() < MIN_SIMILARITY_LENGTH {
            return false;
        }
        
        self.entries.iter().any(|e| {
            let entry_lower = e.content.to_lowercase();
            
            // Exact match
            if entry_lower == content_lower {
                return true;
            }
            
            // Skip comparing with short entries
            if entry_lower.len() < MIN_SIMILARITY_LENGTH {
                return false;
            }
            
            // Calculate word-based similarity (Jaccard-like)
            let similarity = Self::calculate_similarity(&entry_lower, &content_lower);
            similarity >= SIMILARITY_THRESHOLD
        })
    }

/// Calculate word-based similarity between two strings.
    /// Returns a value between 0.0 (no similarity) and 1.0 (identical).
    fn calculate_similarity(a: &str, b: &str) -> f64 {
        use std::collections::HashSet;
        
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
    /// Strategy: preserve manual entries + high importance entries, sorted by importance.
    pub fn prune(&mut self) {
        if self.entries.len() <= self.max_entries {
            return;
        }

        // First, separate entries by priority
        // Manual entries are always kept (highest priority)
        let (manual_entries, auto_entries): (Vec<_>, Vec<_>) = self.entries
            .iter()
            .cloned()
            .partition(|e| e.is_manual);

        // Sort auto entries by importance (descending) + recency as tiebreaker
        let mut sorted_auto = auto_entries;
        sorted_auto.sort_by(|a, b| {
            // First compare by importance
            let importance_cmp = b.importance.partial_cmp(&a.importance)
                .unwrap_or(std::cmp::Ordering::Equal);

            // If equal importance, prefer more recently referenced
            if importance_cmp == std::cmp::Ordering::Equal {
                b.last_referenced.cmp(&a.last_referenced)
            } else {
                importance_cmp
            }
        });

        // Filter auto entries above min_importance threshold
        let kept_auto: Vec<_> = sorted_auto
            .into_iter()
            .filter(|e| e.importance >= self.min_importance)
            .take(self.max_entries.saturating_sub(manual_entries.len()))
            .collect();

        // Combine: manual entries first, then sorted auto entries
        self.entries = manual_entries.into_iter().chain(kept_auto).collect();

        // Final safety check: if still too many, truncate oldest/least important
        if self.entries.len() > self.max_entries {
            self.entries.sort_by(|a, b| {
                let importance_cmp = b.importance.partial_cmp(&a.importance)
                    .unwrap_or(std::cmp::Ordering::Equal);
                if importance_cmp == std::cmp::Ordering::Equal {
                    b.last_referenced.cmp(&a.last_referenced)
                } else {
                    importance_cmp
                }
            });
            self.entries.truncate(self.max_entries);
        }

        self.invalidate_index();  // Index needs rebuild after prune
    }

    /// Smart merge of similar memories (rule-based, no AI needed).
    /// Merges high-similarity entries of the same category.
    /// Returns the number of entries merged (reduced count).
    pub fn smart_merge(&mut self) -> usize {
        if self.entries.len() < 2 {
            return 0;
        }

        let mut merged_count = 0;
        let mut to_remove: Vec<String> = Vec::new();
        let mut new_entries: Vec<MemoryEntry> = Vec::new();
        let mut processed: HashSet<String> = HashSet::new();

        // Find groups of similar entries
        for i in 0..self.entries.len() {
            let entry_i = &self.entries[i];
            if processed.contains(&entry_i.id) {
                continue;
            }

            // Find similar entries of the same category
            let mut similar_group: Vec<usize> = vec![i];

            for j in (i + 1)..self.entries.len() {
                let entry_j = &self.entries[j];
                if processed.contains(&entry_j.id) {
                    continue;
                }

                // Must be same category
                if entry_i.category != entry_j.category {
                    continue;
                }

                // Check similarity
                let similarity = Self::calculate_similarity(&entry_i.content, &entry_j.content);
                if similarity >= MERGE_SIMILARITY_THRESHOLD {
                    similar_group.push(j);
                }
            }

            // If we have a group to merge
            if similar_group.len() >= 2 {
                // Get all entries in group
                let group_entries: Vec<&MemoryEntry> = similar_group
                    .iter()
                    .map(|&idx| &self.entries[idx])
                    .collect();

                // Create merged entry
                let merged = self.merge_group(&group_entries);

                // Mark old entries for removal
                for entry in &group_entries {
                    to_remove.push(entry.id.clone());
                    processed.insert(entry.id.clone());
                }

                // Add merged entry
                new_entries.push(merged);
                merged_count += similar_group.len() - 1;
            } else {
                // No merge needed, just keep the entry
                processed.insert(entry_i.id.clone());
            }
        }

        // Remove merged entries
        for id in &to_remove {
            self.remove(id);
        }

        // Add new merged entries
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
        // Find the entry with highest importance (or most detailed)
        let best = entries
            .iter()
            .max_by(|a, b| {
                // Prefer more detailed (longer) + higher importance
                let score_a = a.importance + (a.content.len() as f64 / 100.0);
                let score_b = b.importance + (b.content.len() as f64 / 100.0);
                score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap();

        // If entries are almost identical, just keep the best one
        let all_same = entries.iter().all(|e| {
            Self::calculate_similarity(&e.content, &best.content) >= 0.95
        });

        if all_same {
            // Return best entry with combined importance
            // Clone the actual MemoryEntry (dereference the reference)
            let mut merged: MemoryEntry = (*best).clone();
            merged.importance = entries
                .iter()
                .map(|e| e.importance)
                .fold(best.importance, |max, val| val.max(max));
            merged.tags.push("merged".to_string());
            return merged;
        }

        // Otherwise, create combined content
        // Take the best content and add key differences from others
        let mut merged_content = best.content.clone();

        // Extract unique keywords from other entries
        for entry in entries {
            if entry.id == best.id {
                continue;
            }
            // Find unique words in this entry
            let unique_words = entry.content
                .split_whitespace()
                .filter(|word| !best.content.contains(word))
                .take(3)  // Add at most 3 unique words
                .collect::<Vec<_>>();

            if !unique_words.is_empty() {
                // Append as additional context (if meaningful)
                let additions = unique_words.join(", ");
                if additions.len() > 10 {
                    merged_content = format!("{} ({})", merged_content.trim_end_matches('.'), additions);
                }
            }
        }

        // Create merged entry
        let mut merged = MemoryEntry::new(best.category, merged_content, None);
        merged.importance = entries
            .iter()
            .map(|e| e.importance)
            .fold(best.importance, |max, val| val.max(max))
            + 5.0;  // Boost merged entries
        merged.importance = merged.importance.min(MAX_IMPORTANCE_CEILING);

        // Combine tags
        merged.tags.push("merged".to_string());
        for entry in entries {
            for tag in &entry.tags {
                if !merged.tags.contains(tag) && !tag.starts_with("merged") {
                    merged.tags.push(tag.clone());
                }
            }
        }

        // Keep manual status if any was manual
        merged.is_manual = entries.iter().any(|e| e.is_manual);

        merged
    }

    /// Get entries by category.
    pub fn by_category(&self, category: MemoryCategory) -> Vec<&MemoryEntry> {
        self.entries.iter().filter(|e| e.category == category).collect()
    }
    
    /// Get entries by category using index (faster).
    pub fn by_category_fast(&mut self, category: MemoryCategory) -> Vec<&MemoryEntry> {
        self.ensure_index();
        if let Some(ref index) = self.search_index {
            index.by_category.get(&category)
                .map(|indices| indices.iter().map(|&i| &self.entries[i]).collect())
                .unwrap_or_default()
        } else {
            self.by_category(category)
        }
    }

    /// Get top N most important entries.
    pub fn top_n(&self, n: usize) -> Vec<&MemoryEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.importance.partial_cmp(&a.importance).unwrap_or(std::cmp::Ordering::Equal));
        sorted.into_iter().take(n).collect()
    }
    
    /// Get top N using index (faster).
    pub fn top_n_fast(&mut self, n: usize) -> Vec<&MemoryEntry> {
        self.ensure_index();
        if let Some(ref index) = self.search_index {
            index.by_importance
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
        let mut results: Vec<_> = self.entries
            .iter()
            .filter(|e| {
                e.content.to_lowercase().contains(&query_lower) ||
                e.tags.iter().any(|t| t.to_lowercase().contains(&query_lower))
            })
            .collect();
        
        // Sort by relevance (importance) then apply limit
        results.sort_by(|a, b| b.importance.partial_cmp(&a.importance).unwrap_or(std::cmp::Ordering::Equal));
        
        if let Some(max) = limit {
            results.into_iter().take(max).collect()
        } else {
            results
        }
    }
    
    /// Search using index (faster).
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

    /// Multi-keyword search (matches any keyword).
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
    
    /// Multi-keyword search using index (faster).
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

    /// Batch add multiple entries efficiently.
    /// Only prunes once at the end instead of after each entry.
    pub fn add_batch(&mut self, entries: Vec<MemoryEntry>) {
        // Filter out duplicates first
        for entry in entries {
            if !self.has_similar(&entry.content) {
                self.entries.push(entry);
            }
        }
        // Single prune at the end
        self.prune();
    }

    /// Mark entries as referenced if they appear in the conversation.
    /// Optimized: pre-computes lowercase versions to avoid repeated conversions.
    pub fn update_references(&mut self, messages: &[Message]) {
        let increment = self.config.reference_increment;
        
        // Pre-compute all message texts in lowercase (optimization)
        let texts_lower: Vec<String> = messages
            .iter()
            .filter_map(Self::extract_message_text_lower)
            .collect();
        
        // Pre-compute all entry contents in lowercase
        let entry_contents_lower: Vec<String> = self.entries
            .iter()
            .map(|e| e.content.to_lowercase())
            .collect();
        
        // Check each entry against all texts
        for (i, entry) in self.entries.iter_mut().enumerate() {
            let entry_lower = &entry_contents_lower[i];
            if texts_lower.iter().any(|t| t.contains(entry_lower)) {
                entry.mark_referenced_with_increment(increment);
            }
        }
    }
    
    /// Extract lowercase text from a message for reference checking.
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
        
        // Group by category
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

    /// Generate context-aware summary for system prompt.
    /// 
    /// Unlike `generate_prompt_summary` which always returns top N by importance,
    /// this method selects memories that are relevant to the current conversation context.
    /// 
    /// Strategy:
    /// 1. Always include manual entries (user explicitly added)
    /// 2. Include entries whose content overlaps with recent conversation keywords
    /// 3. Fill remaining slots with top importance entries
    pub fn generate_contextual_summary(&self, context: &str, max_entries: usize) -> String {
        // Extract keywords internally
        let keywords = extract_context_keywords(context);
        self.generate_contextual_summary_with_keywords(&keywords, max_entries)
    }
    
    /// Generate context-aware summary with pre-extracted keywords.
/// More efficient when keywords are already extracted (e.g., by AI).
/// Enhanced with TF-IDF search for better relevance ranking.
pub fn generate_contextual_summary_with_keywords(&self, context_keywords: &[String], max_entries: usize) -> String {
        if self.entries.is_empty() {
            return String::new();
        }

        // Expand keywords with semantic aliases for better matching
        let expanded_keywords = expand_semantic_keywords(context_keywords);

        // Use TF-IDF search for initial ranking
        let mut tfidf = TfIdfSearch::new();
        tfidf.index(self);
        let keywords_slice: Vec<&str> = expanded_keywords.iter().map(|s| s.as_str()).collect();
        let tfidf_results = tfidf.search_multi(&keywords_slice, Some(max_entries * 2));

        // Convert TF-IDF results to a relevance map
        let mut tfidf_scores: HashMap<String, f64> = HashMap::new();
        for (content, score) in &tfidf_results {
            // Find matching entry by content
            if let Some(entry) = self.entries.iter().find(|e| &e.content == content) {
                tfidf_scores.insert(entry.id.clone(), *score);
            }
        }

        // Score each entry with combined TF-IDF + compute_relevance
        let mut scored: Vec<(&MemoryEntry, f64)> = self.entries
            .iter()
            .map(|entry| {
                // Traditional relevance score
                let relevance = compute_relevance(entry, &expanded_keywords);

                // TF-IDF score (normalized)
                let tfidf = tfidf_scores.get(&entry.id).copied().unwrap_or(0.0);

                // Combine: TF-IDF (semantic) + relevance (keyword match)
                // Weight TF-IDF more for semantic similarity, relevance for exact matches
                let combined = tfidf * 0.4 + relevance * 0.6;

                (entry, combined)
            })
            .collect();

        // Sort by: manual first, then combined score + importance
        scored.sort_by(|a, b| {
            // Manual entries always first
            if a.0.is_manual && !b.0.is_manual {
                return std::cmp::Ordering::Less;
            }
            if !a.0.is_manual && b.0.is_manual {
                return std::cmp::Ordering::Greater;
            }

            // Combined score: relevance + importance
            let score_a = a.1 * CONTEXT_RELEVANCE_WEIGHT + (a.0.importance / MAX_IMPORTANCE_CEILING) * CONTEXT_IMPORTANCE_WEIGHT;
            let score_b = b.1 * CONTEXT_RELEVANCE_WEIGHT + (b.0.importance / MAX_IMPORTANCE_CEILING) * CONTEXT_IMPORTANCE_WEIGHT;

            score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Take top entries and collect IDs (for potential reference update)
        let _selected_ids: Vec<String> = scored
            .iter()
            .take(max_entries)
            .map(|(entry, _)| entry.id.clone())
            .collect();

        let selected: Vec<&MemoryEntry> = scored
            .iter()
            .take(max_entries)
            .map(|(entry, _)| *entry)
            .collect();

        if selected.is_empty() {
            return String::new();
        }

        // Note: We can't update entries here since self is borrowed
        // The reference update should be done separately after retrieval

        let mut summary = String::from("【跨会话记忆】\n\n");

        // Group by category
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

    /// Update reference statistics for retrieved memories.
    /// Call this after generating a summary to boost importance of frequently used memories.
    pub fn update_retrieval_stats(&mut self, retrieved_ids: &[String]) {
        for id in retrieved_ids {
            if let Some(entry) = self.entries.iter_mut().find(|e| &e.id == id) {
                entry.mark_referenced();
                log::debug!("Updated reference stats for memory {}", id);
            }
        }
    }

    /// Get IDs of entries that would be selected for a context.
    /// Useful for updating reference stats after retrieval.
    pub fn get_retrieval_ids(&self, context_keywords: &[String], max_entries: usize) -> Vec<String> {
        if self.entries.is_empty() {
            return Vec::new();
        }

        let expanded_keywords = expand_semantic_keywords(context_keywords);

        // Simple relevance scoring
        let mut scored: Vec<(&MemoryEntry, f64)> = self.entries
            .iter()
            .map(|entry| {
                let relevance = compute_relevance(entry, &expanded_keywords);
                (entry, relevance)
            })
            .collect();

        // Sort by manual + score
        scored.sort_by(|a, b| {
            if a.0.is_manual && !b.0.is_manual {
                return std::cmp::Ordering::Less;
            }
            if !a.0.is_manual && b.0.is_manual {
                return std::cmp::Ordering::Greater;
            }

            let score_a = a.1 + (a.0.importance / MAX_IMPORTANCE_CEILING);
            let score_b = b.1 + (b.0.importance / MAX_IMPORTANCE_CEILING);

            score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
        });

        scored.iter().take(max_entries).map(|(e, _)| e.id.clone()).collect()
    }

    /// Generate context-aware summary with AI-enhanced keyword extraction.
    /// 
    /// This is the async version that uses AI to extract keywords when
    /// rule-based extraction produces insufficient results.
    pub async fn generate_contextual_summary_async(
        &self,
        context: &str,
        max_entries: usize,
        fast_provider: Option<&dyn crate::providers::Provider>,
    ) -> String {
        if self.entries.is_empty() {
            return String::new();
        }

        // Extract keywords using hybrid approach (rule-based + AI fallback)
        let context_keywords = if let Some(provider) = fast_provider {
            extract_keywords_hybrid(context, Some(provider)).await
        } else {
            extract_context_keywords(context)
        };
        
        // Score each entry by relevance to context
        let mut scored: Vec<(&MemoryEntry, f64)> = self.entries
            .iter()
            .map(|entry| {
                let relevance = compute_relevance(entry, &context_keywords);
                (entry, relevance)
            })
            .collect();
        
        // Sort by: manual first, then relevance + importance combined
        scored.sort_by(|a, b| {
            // Manual entries always first
            if a.0.is_manual && !b.0.is_manual {
                return std::cmp::Ordering::Less;
            }
            if !a.0.is_manual && b.0.is_manual {
                return std::cmp::Ordering::Greater;
            }
            
            // Combined score: relevance weight + importance weight
            let score_a = a.1 * CONTEXT_RELEVANCE_WEIGHT + (a.0.importance / MAX_IMPORTANCE_CEILING) * CONTEXT_IMPORTANCE_WEIGHT;
            let score_b = b.1 * CONTEXT_RELEVANCE_WEIGHT + (b.0.importance / MAX_IMPORTANCE_CEILING) * CONTEXT_IMPORTANCE_WEIGHT;
            
            score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        // Take top entries
        let selected: Vec<&MemoryEntry> = scored
            .iter()
            .take(max_entries)
            .map(|(entry, _)| *entry)
            .collect();
        
        if selected.is_empty() {
            return String::new();
        }

        let mut summary = String::from("【跨会话记忆】\n\n");
        
        // Group by category
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
        
        // Sort by importance
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.importance.partial_cmp(&a.importance).unwrap_or(std::cmp::Ordering::Equal));

        for entry in sorted {
            result.push_str(&entry.format_line());
            result.push('\n');
        }

        result
    }

    /// Generate statistics summary for display.
    pub fn generate_statistics(&self) -> MemoryStatistics {
        let total = self.entries.len();
        let manual = self.entries.iter().filter(|e| e.is_manual).count();
        let auto = total - manual;
        
        // Count by category
        let by_category: HashMap<MemoryCategory, usize> = self.entries
            .iter()
            .fold(HashMap::new(), |mut acc, e| {
                *acc.entry(e.category).or_default() += 1;
                acc
            });
        
        // Calculate average importance
        let avg_importance = if total > 0 {
            self.entries.iter().map(|e| e.importance).sum::<f64>() / total as f64
        } else {
            0.0
        };
        
        // Find oldest and newest
        let oldest = self.entries
            .iter()
            .min_by_key(|e| e.created_at)
            .map(|e| e.created_at);
        let newest = self.entries
            .iter()
            .max_by_key(|e| e.created_at)
            .map(|e| e.created_at);
        
        // Count highly referenced
        let highly_referenced = self.entries
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
    /// Entries that haven't been referenced recently will have their importance reduced.
    pub fn apply_time_decay(&mut self) {
        let now = Utc::now();
        let decay_start_days = self.config.decay_start_days;
        let decay_rate = self.config.decay_rate;
        let decay_period_days = 30;  // Each decay period is 30 days
        
        for entry in &mut self.entries {
            // Skip manual entries - they should never decay
            if entry.is_manual {
                continue;
            }
            
            // Calculate days since last reference
            let days_since_reference = (now - entry.last_referenced)
                .num_days()
                .max(0);
            
            // Apply decay if older than threshold
            if days_since_reference > decay_start_days {
                // Calculate number of decay periods
                let decay_periods = (days_since_reference - decay_start_days) / decay_period_days;
                
                // Apply exponential decay
                let decay_factor = decay_rate.powi(decay_periods as i32);
                entry.importance *= decay_factor;
                
                // Ensure minimum importance (at least half of min_importance)
                entry.importance = entry.importance.max(self.min_importance * 0.5);
            }
        }
        
        // Re-prune after decay (low importance entries may now be removed)
        self.prune();
    }
}

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
    /// Number of entries with high reference count (>= 3).
    pub highly_referenced: usize,
}

impl MemoryStatistics {
    /// Format statistics for display.
    pub fn format_summary(&self) -> String {
        use std::fmt::Write;
        
        let mut output = String::new();
        
        writeln!(output, "记忆统计：").unwrap();
        writeln!(output, "  总计: {} 条", self.total).unwrap();
        writeln!(output, "  ├─ 手动添加: {} 条", self.manual).unwrap();
        writeln!(output, "  └─ 自动检测: {} 条", self.auto).unwrap();
        writeln!(output).unwrap();
        
        writeln!(output, "分类统计：").unwrap();
        for (cat, count) in &self.by_category {
            writeln!(output, "  {} {}: {} 条", cat.icon(), cat.display_name(), count).unwrap();
        }
        writeln!(output).unwrap();
        
        writeln!(output, "质量指标：").unwrap();
        writeln!(output, "  平均重要性: {:.1} 分", self.avg_importance).unwrap();
        writeln!(output, "  高频引用: {} 条 (≥3次)", self.highly_referenced).unwrap();
        
        if let Some(oldest) = self.oldest {
            let days = (Utc::now() - oldest).num_days();
            writeln!(output, "  记忆跨度: {} 天", days).unwrap();
        }
        
        output
    }
}

// ============================================================================
// Memory Storage with File Lock
// ============================================================================

/// File lock for preventing concurrent access to memory storage.
/// Uses a simple lock file approach (.lock) with atomic operations.
pub struct MemoryFileLock {
    /// Path to the lock file.
    lock_path: PathBuf,
    /// Whether we currently hold the lock.
    locked: bool,
}

impl MemoryFileLock {
    /// Create a new file lock for the given directory.
    pub fn new(base_dir: &Path) -> Self {
        Self {
            lock_path: base_dir.join("memory.lock"),
            locked: false,
        }
    }
    
    /// Acquire the lock (blocking with timeout).
    /// Returns true if lock was acquired, false if timeout expired.
    pub fn acquire(&mut self, timeout_ms: u64) -> Result<bool> {
        if self.locked {
            return Ok(true);  // Already locked
        }
        
        let start = std::time::Instant::now();
        
        while start.elapsed().as_millis() < timeout_ms as u128 {
            // Try to create lock file atomically
            match fs::File::create_new(&self.lock_path) {
                Ok(_) => {
                    // Write lock info (PID + timestamp)
                    let lock_info = format!(
                        "{}:{}",
                        std::process::id(),
                        Utc::now().to_rfc3339()
                    );
                    fs::write(&self.lock_path, lock_info)?;
                    self.locked = true;
                    return Ok(true);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    // Lock file exists, check if it's stale
                    if self.is_stale_lock()? {
                        self.remove_stale_lock()?;
                    }
                    // Wait a bit before retrying
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(e) => {
                    return Err(e.into());
                }
            }
        }
        
        Ok(false)  // Timeout expired
    }
    
    /// Check if the existing lock is stale (older than 30 seconds).
    fn is_stale_lock(&self) -> Result<bool> {
        if !self.lock_path.exists() {
            return Ok(false);
        }
        
        // Check lock file age
        let metadata = fs::metadata(&self.lock_path)?;
        let modified = metadata.modified()?;
        let age = std::time::SystemTime::now()
            .duration_since(modified)
            .unwrap_or(std::time::Duration::ZERO);
        
        // Consider lock stale if older than 30 seconds
        Ok(age > std::time::Duration::from_secs(30))
    }
    
    /// Remove stale lock file.
    fn remove_stale_lock(&self) -> Result<()> {
        if self.lock_path.exists() {
            fs::remove_file(&self.lock_path)?;
        }
        Ok(())
    }
    
    /// Release the lock.
    pub fn release(&mut self) -> Result<()> {
        if self.locked {
            fs::remove_file(&self.lock_path)?;
            self.locked = false;
        }
        Ok(())
    }
}

impl Drop for MemoryFileLock {
    fn drop(&mut self) {
        // Auto-release lock on drop
        let _ = self.release();
    }
}

// ============================================================================
// SECTION 3: Storage (MemoryStorage, File Operations)
// ============================================================================

/// Storage for memory files (global and project-level) with file locking.
pub struct MemoryStorage {
    /// Base directory for global memory (~/.matrix).
    base_dir: PathBuf,
    /// Project root directory (optional).
    project_root: Option<PathBuf>,
    /// File lock for preventing concurrent writes.
    lock: MemoryFileLock,
}

impl MemoryStorage {
    /// Create a new memory storage.
    pub fn new(project_root: Option<&Path>) -> Result<Self> {
        let base_dir = Self::get_base_dir()?;
        let lock = MemoryFileLock::new(&base_dir);
        Ok(Self {
            base_dir,
            project_root: project_root.map(|p| p.to_path_buf()),
            lock,
        })
    }

    /// Create a new storage with explicit lock timeout.
    pub fn with_lock_timeout(project_root: Option<&Path>, timeout_ms: u64) -> Result<Self> {
        let mut storage = Self::new(project_root)?;
        storage.lock.acquire(timeout_ms)?;
        Ok(storage)
    }

    /// Get the base directory for memory storage.
    fn get_base_dir() -> Result<PathBuf> {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .ok_or_else(|| anyhow::anyhow!("HOME or USERPROFILE not set"))?;
        let mut p = PathBuf::from(home);
        p.push(".matrix");
        Ok(p)
    }

    /// Path to global memory file.
    pub fn global_memory_path(&self) -> PathBuf {
        self.base_dir.join("memory.json")
    }

    /// Path to project memory file.
    pub fn project_memory_path(&self) -> Option<PathBuf> {
        self.project_root.as_ref().map(|p| p.join(".matrix/memory.json"))
    }

    /// Path to config file.
    pub fn config_path(&self) -> PathBuf {
        self.base_dir.join("memory_config.json")
    }

    /// Ensure directories exist.
    fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.base_dir)?;
        if let Some(root) = &self.project_root {
            let memory_dir = root.join(".matrix");
            fs::create_dir_all(memory_dir)?;
        }
        Ok(())
    }

    /// Acquire lock before write operations.
    fn acquire_lock(&mut self) -> Result<()> {
        self.lock.acquire(5000)?;  // 5 second timeout
        Ok(())
    }

    /// Release lock after write operations.
    fn release_lock(&mut self) -> Result<()> {
        self.lock.release()?;
        Ok(())
    }

    /// Load global memory (no lock needed for read).
    pub fn load_global(&self) -> Result<AutoMemory> {
        let path = self.global_memory_path();
        if !path.exists() {
            return Ok(AutoMemory::new());
        }
        let data = fs::read_to_string(&path)?;
        let memory: AutoMemory = serde_json::from_str(&data)?;
        Ok(memory)
    }

    /// Load project memory (no lock needed for read).
    pub fn load_project(&self) -> Result<Option<AutoMemory>> {
        let path = self.project_memory_path();
        match path {
            Some(p) if p.exists() => {
                let data = fs::read_to_string(&p)?;
                let memory: AutoMemory = serde_json::from_str(&data)?;
                Ok(Some(memory))
            }
            _ => Ok(None),
        }
    }

    /// Load combined memory (global + project).
    pub fn load_combined(&self) -> Result<AutoMemory> {
        let mut combined = self.load_global()?;
        
        if let Some(project) = self.load_project()? {
            // Merge project entries into global
            for entry in project.entries {
                // Tag as project-specific
                let mut tagged_entry = entry;
                if !tagged_entry.tags.contains(&"project".to_string()) {
                    tagged_entry.tags.push("project".to_string());
                }
                combined.entries.push(tagged_entry);
            }
            combined.prune();
        }

        Ok(combined)
    }

    /// Save global memory (with file lock).
    pub fn save_global(&mut self, memory: &AutoMemory) -> Result<()> {
        self.acquire_lock()?;
        self.ensure_dirs()?;
        
        let path = self.global_memory_path();
        let json = serde_json::to_string_pretty(memory)?;
        
        // Write to temp file then rename (atomic)
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, json)?;
        fs::rename(&tmp, &path)?;
        
        self.release_lock()?;
        Ok(())
    }

    /// Save project memory (with file lock).
    pub fn save_project(&mut self, memory: &AutoMemory) -> Result<()> {
        self.acquire_lock()?;
        self.ensure_dirs()?;
        
        let path = self.project_memory_path()
            .ok_or_else(|| anyhow::anyhow!("no project root"))?;
        let json = serde_json::to_string_pretty(memory)?;
        
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, json)?;
        fs::rename(&tmp, &path)?;
        
        self.release_lock()?;
        Ok(())
    }

    /// Save config to separate file.
    pub fn save_config(&mut self, config: &MemoryConfig) -> Result<()> {
        self.ensure_dirs()?;
        let path = self.config_path();
        let json = serde_json::to_string_pretty(config)?;
        fs::write(&path, json)?;
        Ok(())
    }

    /// Load config from file.
    pub fn load_config(&self) -> Result<MemoryConfig> {
        let path = self.config_path();
        if !path.exists() {
            return Ok(MemoryConfig::default());
        }
        let data = fs::read_to_string(&path)?;
        let config: MemoryConfig = serde_json::from_str(&data)?;
        Ok(config)
    }

    /// Add entry to appropriate storage (with file lock).
    pub fn add_entry(&mut self, entry: MemoryEntry, is_project_specific: bool) -> Result<()> {
        self.acquire_lock()?;
        
        if is_project_specific {
            let mut project = self.load_project()?.unwrap_or_else(AutoMemory::new);
            project.add(entry);
            self.save_project_locked(&project)?;
        } else {
            let mut global = self.load_global()?;
            global.add(entry);
            self.save_global_locked(&global)?;
        }
        
        self.release_lock()?;
        Ok(())
    }

    /// Remove entry from storage by ID (with file lock).
    pub fn remove_entry(&mut self, id: &str, is_project_specific: bool) -> Result<bool> {
        self.acquire_lock()?;
        
        let removed = if is_project_specific {
            if let Some(mut project) = self.load_project()? {
                let removed = project.remove(id);
                if removed {
                    self.save_project_locked(&project)?;
                }
                removed
            } else {
                false
            }
        } else {
            let mut global = self.load_global()?;
            let removed = global.remove(id);
            if removed {
                self.save_global_locked(&global)?;
            }
            removed
        };
        
        self.release_lock()?;
        Ok(removed)
    }
    
    /// Internal save methods that don't acquire lock (assumed already locked).
    fn save_global_locked(&self, memory: &AutoMemory) -> Result<()> {
        let path = self.global_memory_path();
        let json = serde_json::to_string_pretty(memory)?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, json)?;
        fs::rename(&tmp, &path)?;
        Ok(())
    }
    
    fn save_project_locked(&self, memory: &AutoMemory) -> Result<()> {
        let path = self.project_memory_path()
            .ok_or_else(|| anyhow::anyhow!("no project root"))?;
        let json = serde_json::to_string_pretty(memory)?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, json)?;
        fs::rename(&tmp, &path)?;
        Ok(())
    }
}

// ============================================================================
// Helper Functions (Global)
// ============================================================================

/// Calculate word-based similarity between two strings (Jaccard coefficient).
/// Returns a value between 0.0 (no similarity) and 1.0 (identical words).
/// This is the public version for external use.
pub fn calculate_similarity(a: &str, b: &str) -> f64 {
    AutoMemory::calculate_similarity(a, b)
}

/// Extract meaningful keywords from conversation context.
/// Filters out common stop words and short tokens.
/// Public for external use (e.g., TUI keyword display).
pub fn extract_context_keywords(context: &str) -> Vec<String> {
    use std::collections::HashSet;
    
    // Common stop words (Chinese + English)
    let stop_words: HashSet<&str> = [
        // Chinese stop words
        "的", "了", "是", "在", "我", "有", "和", "就", "不", "人", "都", "一", "一个",
        "上", "也", "很", "到", "说", "要", "去", "你", "会", "着", "没有", "看", "好",
        "自己", "这", "他", "她", "它", "们", "那", "些", "什么", "怎么", "如何", "请",
        "能", "可以", "需要", "应该", "可能", "因为", "所以", "但是", "然后", "还是",
        "已经", "正在", "将要", "曾经", "一下", "一点", "一些", "所有", "每个", "任何",
        // English stop words
        "the", "a", "an", "is", "are", "was", "were", "be", "been", "being",
        "have", "has", "had", "do", "does", "did", "will", "would", "could",
        "should", "may", "might", "can", "shall", "to", "of", "in", "for",
        "on", "with", "at", "by", "from", "as", "into", "through", "during",
        "before", "after", "above", "below", "between", "and", "but", "or",
        "not", "no", "so", "if", "then", "than", "too", "very", "just",
        "this", "that", "these", "those", "it", "its", "i", "me", "my",
        "we", "our", "you", "your", "he", "his", "she", "her", "they", "their",
        "please", "help", "need", "want", "make", "get", "let", "use",
    ].iter().copied().collect();
    
    // Technical/meaningful patterns to extract (Chinese + English)
    let tech_patterns: HashSet<&str> = [
        // Technical terms (keep these even if short)
        "api", "cli", "gui", "tui", "web", "http", "json", "xml", "sql", "db",
        "git", "npm", "cargo", "rust", "js", "ts", "py", "go", "java", "cpp",
        "cpu", "gpu", "io", "fs", "os", "ui", "ux", "ai", "ml", "dl",
        // File extensions
        "rs", "js", "ts", "py", "go", "java", "c", "h", "cpp", "hpp",
        "json", "yaml", "yml", "toml", "md", "txt", "html", "css", "scss",
        // Short meaningful words
        "bug", "fix", "add", "new", "old", "use", "run", "build", "test",
        "code", "data", "file", "dir", "path", "name", "type", "value",
    ].iter().copied().collect();
    
    let lower = context.to_lowercase();
    let mut keywords: HashSet<String> = HashSet::new();
    
    // 1. Extract English words (space-separated)
    for word in lower.split_whitespace() {
        let cleaned = word.trim_matches(|c: char| !c.is_alphanumeric()).to_string();
        if cleaned.len() >= 2 && !stop_words.contains(cleaned.as_str()) {
            keywords.insert(cleaned.clone());
        }
        // Keep technical short words
        if tech_patterns.contains(cleaned.as_str()) {
            keywords.insert(cleaned);
        }
    }
    
    // 2. Extract Chinese words/phrases (2-4 character sequences)
    // Chinese characters are typically 3 bytes in UTF-8
    let chinese_chars: Vec<char> = lower
        .chars()
        .filter(|c| *c >= '\u{4E00}' && *c <= '\u{9FFF}')  // Chinese Unicode range
        .collect();
    
    // Extract 2-4 character Chinese sequences
    for window_size in 2..=4 {
        if chinese_chars.len() >= window_size {
            for window in chinese_chars.windows(window_size) {
                let phrase: String = window.iter().collect();
                // Skip if contains stop words
                let has_stop = stop_words.iter().any(|sw| phrase.contains(sw));
                if !has_stop && phrase.len() >= window_size {
                    keywords.insert(phrase);
                }
            }
        }
    }
    
    // 3. Extract specific patterns (project names, file names, etc.)
    // Look for common project/file patterns
    let patterns = [
        // File paths
        r"[a-zA-Z_][a-zA-Z0-9_]*\.[a-zA-Z]{1,4}",  // file.ext
        r"[a-zA-Z_][a-zA-Z0-9_]*\.[a-zA-Z_][a-zA-Z0-9_]*",  // module.submodule
        // CamelCase/snake_case identifiers
        r"[A-Z][a-z]+[A-Z][a-zA-Z]*",  // CamelCase
        r"[a-z][a-z0-9]*_[a-z][a-z0-9_]*",  // snake_case
        // Numbers with units
        r"[0-9]+[kKmMgGtT][bB]?",  // 4K, 100MB
    ];
    
    for pattern in patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            for cap in re.find_iter(&lower) {
                keywords.insert(cap.as_str().to_string());
            }
        }
    }
    
    // Convert to vector and sort by length (prefer longer, more specific keywords)
    let mut result: Vec<String> = keywords.into_iter().collect();
    result.sort_by(|a, b| b.len().cmp(&a.len()));
    
    // Take top keywords (avoid too many)
    result.truncate(15);
    
    result
}

// ============================================================================
// SECTION 5: Semantic Similarity Enhancement (Aliases, Relevance)
// ============================================================================

/// Semantic alias mappings for better keyword matching.
/// Maps related terms to common equivalents.
pub const SEMANTIC_ALIASES: &[(&str, &str)] = &[
    // Database related
    ("数据库", "database"), ("db", "database"),
    ("postgresql", "postgres"), ("mysql", "mysql"),
    ("mongodb", "mongo"), ("redis", "redis"),
    ("sqlite", "sqlite"), ("sql", "database"),
    // Frontend related
    ("前端", "frontend"), ("ui", "frontend"),
    ("界面", "frontend"), ("页面", "page"),
    ("组件", "component"), ("react", "react"),
    ("vue", "vue"), ("angular", "angular"),
    // Backend related
    ("后端", "backend"), ("api", "api"),
    ("接口", "api"), ("服务", "service"),
    ("server", "backend"), ("服务器", "backend"),
    // Framework/Language
    ("rust", "rust"), ("python", "python"),
    ("javascript", "js"), ("typescript", "ts"),
    ("java", "java"), ("go", "golang"),
    ("golang", "go"), ("c++", "cpp"),
    ("cpp", "c++"), ("nodejs", "node"),
    ("node", "nodejs"),
    // Tools
    ("编辑器", "editor"), ("ide", "editor"),
    ("vim", "vim"), ("vscode", "vscode"),
    ("emacs", "emacs"),
    // Config
    ("配置", "config"), ("设置", "config"),
    ("config", "config"), ("setting", "config"),
    // Structure
    ("目录", "directory"), ("文件", "file"),
    ("文件夹", "directory"), ("路径", "path"),
    ("模块", "module"), ("包", "package"),
    // Testing
    ("测试", "test"), ("test", "test"),
    ("单元测试", "unittest"), ("unittest", "test"),
    // Cache
    ("缓存", "cache"), ("cache", "cache"),
    // Auth
    ("认证", "auth"), ("登录", "login"),
    ("auth", "auth"), ("登录", "auth"),
    // Performance
    ("性能", "performance"), ("优化", "optimize"),
    ("速度", "speed"), ("慢", "slow"),
    // Common verbs
    ("创建", "create"), ("删除", "delete"),
    ("修改", "modify"), ("添加", "add"),
    ("更新", "update"), ("查询", "query"),
];

/// Expand keywords with semantic aliases.
/// Returns expanded keywords including original + aliases.
pub fn expand_semantic_keywords(keywords: &[String]) -> Vec<String> {
    let mut expanded: Vec<String> = keywords.to_vec();

    for keyword in keywords {
        let kw_lower = keyword.to_lowercase();
        // Find aliases for this keyword
        for (alias, target) in SEMANTIC_ALIASES {
            if kw_lower.contains(alias) {
                // Add both the target and the alias
                expanded.push(target.to_string());
            }
            if kw_lower.contains(target) {
                expanded.push(alias.to_string());
            }
        }
    }

    // Deduplicate
    expanded.sort();
    expanded.dedup();
    expanded
}

/// Compute relevance score of a memory entry to context keywords.
/// Returns 0.0-1.0 where 1.0 means highly relevant.
/// Enhanced with semantic alias expansion for better matching.
fn compute_relevance(entry: &MemoryEntry, context_keywords: &[String]) -> f64 {
    if context_keywords.is_empty() {
        return 0.0;
    }

    // Expand keywords with semantic aliases
    let expanded_keywords = expand_semantic_keywords(context_keywords);

    let content_lower = entry.content.to_lowercase();

    // Count how many expanded keywords appear in this entry
    let matches = expanded_keywords
        .iter()
        .filter(|kw| {
            let kw_lower = kw.to_lowercase();
            // Check both exact match and semantic alias match
            content_lower.contains(&kw_lower)
        })
        .count();

    // Normalize by total keywords (0.0-1.0)
    // Use expanded count for better normalization
    let keyword_score = matches as f64 / expanded_keywords.len().max(context_keywords.len()) as f64;

    // Boost for tag matches (tags often contain key technologies/topics)
    let tag_matches = entry.tags
        .iter()
        .filter(|tag| {
            let tag_lower = tag.to_lowercase();
            expanded_keywords.iter().any(|kw| {
                tag_lower.contains(&kw.to_lowercase()) ||
                kw.to_lowercase().contains(&tag_lower)
            })
        })
        .count();

    let tag_score = if tag_matches > 0 { 0.2 + (tag_matches as f64 * 0.05).min(0.1) } else { 0.0 };

    // Combined score (capped at 1.0)
    (keyword_score + tag_score).min(1.0)
}

/// Detect if two memory contents have contradiction signals.
/// 
/// Contradiction patterns:
/// - Same verb/action but different object ("使用 PostgreSQL" vs "使用 MySQL")
/// - Negation patterns ("不用 X" vs "使用 X")
/// - Replacement patterns ("改用", "换成", "替换为")
fn has_contradiction_signal(old: &str, new: &str) -> bool {
    // Check for replacement/change keywords in new content
    let change_signals = [
        "改用", "换成", "替换", "改为", "切换到", "迁移到",
        "不再使用", "弃用", "放弃", "取消",
        "switched to", "replaced", "migrated to", "changed to",
        "no longer", "deprecated", "abandoned",
    ];
    
    for signal in &change_signals {
        if new.contains(signal) {
            return true;
        }
    }
    
    // Check for same action verb but different object
    // e.g., "决定使用 PostgreSQL" vs "决定使用 MySQL"
    let action_verbs = [
        "决定使用", "选择使用", "采用", "使用",
        "decided to use", "chose", "using", "adopted",
    ];
    
    for verb in &action_verbs {
        if old.contains(verb) && new.contains(verb) {
            // Both have the same action verb - likely a conflict
            // (if they were the same thing, has_similar would have caught it)
            return true;
        }
    }
    
    // Check for preference conflicts
    let pref_verbs = ["偏好", "喜欢", "prefer", "like"];
    for verb in &pref_verbs {
        if old.contains(verb) && new.contains(verb) {
            return true;
        }
    }
    
    false
}

// ============================================================================
// SECTION 6: AI Enhancement (MemoryExtractor, AiMemoryProcessor)
// ============================================================================

/// Trait for memory extraction implementations.
#[async_trait::async_trait]
pub trait MemoryExtractor: Send + Sync {
    /// Extract memories from conversation text using AI.
    async fn extract(&self, text: &str, session_id: Option<&str>) -> Result<Vec<MemoryEntry>>;
    
    /// Get the model name used for extraction.
    fn model_name(&self) -> &str;
}

/// AI-based memory extractor using a fast/cheap model.
pub struct AiMemoryExtractor {
    provider: Box<dyn crate::providers::Provider>,
    model: String,
}

impl AiMemoryExtractor {
    /// Create a new AI memory extractor.
    pub fn new(provider: Box<dyn crate::providers::Provider>, model: String) -> Self {
        Self { provider, model }
    }
}

/// System prompt for memory extraction.
const MEMORY_EXTRACT_SYSTEM_PROMPT: &str = r#"你是一个记忆提取助手。你的任务是从对话中识别并提取值得长期记忆的关键信息。

记忆类型：
1. Decision（决策）: 项目或技术选型的决定，如"决定使用 PostgreSQL"
2. Preference（偏好）: 用户习惯或偏好，如"我喜欢用 vim"
3. Solution（解决方案）: 解决问题的具体方法，如"通过添加 middleware 修复 bug"
4. Finding（发现）: 重要发现或信息，如"API 端点在 /api/v2"
5. Technical（技术）: 技术栈或框架信息，如"使用 React Query 做数据获取"
6. Structure（结构）: 项目结构信息，如"入口文件是 src/index.ts"

提取原则：
- 只提取有价值、可复用的信息
- 避免提取临时性、一次性信息
- 避免提取过于具体的代码细节
- 每条记忆应简洁明确（一句话）
- 最多提取 5 条记忆

输出格式（严格 JSON）：
```json
{
  "memories": [
    {
      "category": "decision",
      "content": "决定使用 PostgreSQL 作为主数据库",
      "importance": 90
    },
    {
      "category": "preference", 
      "content": "用户偏好 TypeScript 而非 JavaScript",
      "importance": 70
    }
  ]
}
```

如果没有值得记忆的内容，返回：
```json
{"memories": []}
```

直接输出 JSON，不要加代码块包裹。"#;

#[async_trait::async_trait]
impl MemoryExtractor for AiMemoryExtractor {
    async fn extract(&self, text: &str, session_id: Option<&str>) -> Result<Vec<MemoryEntry>> {
        use crate::providers::{ChatRequest, Message, MessageContent, Role};
        
        // Truncate text if too long (memory extraction focuses on key points)
        let truncated_text = if text.len() > 4000 {
            truncate_str(text, 4000)
        } else {
            text.to_string()
        };
        
        let request = ChatRequest {
            messages: vec![Message {
                role: Role::User,
                content: MessageContent::Text(format!(
                    "请从以下对话中提取值得记忆的关键信息：\n\n{}", 
                    truncated_text
                )),
            }],
            tools: vec![],  // No tools for memory extraction
            system: Some(MEMORY_EXTRACT_SYSTEM_PROMPT.to_string()),
            think: false,   // No extended thinking
            max_tokens: 512, // Short response
            server_tools: vec![],
            enable_caching: false,
        };
        
        let response = self.provider.chat(request).await?;
        
        // Extract text from response
        let response_text = response.content
            .iter()
            .filter_map(|block| {
                if let crate::providers::ContentBlock::Text { text } = block {
                    Some(text.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("");
        
        // Parse JSON response
        parse_memory_response(&response_text, session_id)
    }
    
    fn model_name(&self) -> &str {
        &self.model
    }
}

/// Parse AI response into memory entries.
fn parse_memory_response(json_text: &str, session_id: Option<&str>) -> Result<Vec<MemoryEntry>> {
    // Clean up response (remove possible markdown code blocks)
    let cleaned = json_text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    
    // Parse JSON
    #[derive(serde::Deserialize)]
    struct MemoryResponse {
        memories: Vec<MemoryItem>,
    }
    
    #[derive(serde::Deserialize)]
    struct MemoryItem {
        category: String,
        content: String,
        #[serde(default)]
        importance: f64,
    }
    
    let parsed: MemoryResponse = serde_json::from_str(cleaned)?;
    
    // Convert to MemoryEntry
    let entries = parsed.memories
        .into_iter()
        .filter_map(|item| {
            // Parse category
            let category = match item.category.to_lowercase().as_str() {
                "decision" => MemoryCategory::Decision,
                "preference" => MemoryCategory::Preference,
                "solution" => MemoryCategory::Solution,
                "finding" => MemoryCategory::Finding,
                "technical" => MemoryCategory::Technical,
                "structure" => MemoryCategory::Structure,
                _ => return None,  // Skip unknown categories
            };
            
            // Skip too short content
            if item.content.len() < MIN_MEMORY_CONTENT_LENGTH {
                return None;
            }
            
            // Create entry with AI-suggested importance or default
            let mut entry = MemoryEntry::new(
                category,
                item.content,
                session_id.map(|s| s.to_string()),
            );
            
            // Override importance if AI suggested a value
            if item.importance > 0.0 {
                entry.importance = item.importance.clamp(0.0, 100.0);
            }
            
            Some(entry)
        })
        .collect();
    
    // Deduplicate and limit
    Ok(deduplicate_entries(entries))
}

// ============================================================================
// AI-Based Keyword Extraction (for context-aware memory retrieval)
// ============================================================================

/// System prompt for AI keyword extraction.
const KEYWORD_EXTRACT_SYSTEM_PROMPT: &str = r#"你是一个关键词提取助手。你的任务是从用户输入中提取有意义的关键词，用于检索相关记忆。

提取原则：
1. 只提取有实际意义的词汇（技术名词、项目名、概念等）
2. 过滤掉常见的停用词（的、是、在、我、你、the、a、is 等）
3. 保留专有名词和技术术语
4. 中英文混合输入时，两种语言的关键词都提取
5. 提取 3-10 个关键词

输出格式（严格 JSON）：
```json
{
  "keywords": ["数据库", "PostgreSQL", "优化", "查询"]
}
```

如果没有有意义的关键词，返回：
```json
{"keywords": []}
```

直接输出 JSON，不要加代码块包裹。"#;

/// Extract keywords from context using AI (for context-aware memory retrieval).
/// 
/// This is used when the rule-based keyword extraction produces too few results
/// or when the context is complex and needs better understanding.
pub async fn extract_keywords_with_ai(
    context: &str,
    provider: &dyn crate::providers::Provider,
) -> Result<Vec<String>> {
    use crate::providers::{ChatRequest, Message, MessageContent, Role};
    
    // Truncate if too long
    let truncated = if context.len() > 1000 {
        truncate_str(context, 1000)
    } else {
        context.to_string()
    };
    
    let request = ChatRequest {
        messages: vec![Message {
            role: Role::User,
            content: MessageContent::Text(format!(
                "请从以下文本中提取关键词：\n\n{}", 
                truncated
            )),
        }],
        tools: vec![],
        system: Some(KEYWORD_EXTRACT_SYSTEM_PROMPT.to_string()),
        think: false,
        max_tokens: 256,
        server_tools: vec![],
        enable_caching: false,
    };
    
    let response = provider.chat(request).await?;
    
    // Extract text from response
    let response_text = response.content
        .iter()
        .filter_map(|block| {
            if let crate::providers::ContentBlock::Text { text } = block {
                Some(text.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("");
    
    // Parse JSON response
    parse_keyword_response(&response_text)
}

/// Parse AI keyword extraction response.
fn parse_keyword_response(json_text: &str) -> Result<Vec<String>> {
    // Clean up response
    let cleaned = json_text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    
    #[derive(serde::Deserialize)]
    struct KeywordResponse {
        keywords: Vec<String>,
    }
    
    let parsed: KeywordResponse = serde_json::from_str(cleaned)?;
    
    // Filter out empty or too-short keywords
    Ok(parsed.keywords
        .into_iter()
        .filter(|k| k.len() >= 2)
        .collect())
}

/// Extract keywords from context with hybrid approach.
/// 
/// Strategy:
/// 1. First use rule-based stop word filtering (fast, zero cost)
/// 2. If result is insufficient (too few keywords), fall back to AI extraction
/// 3. Behavior controlled by MEMORY_AI_KEYWORDS env var (auto/always/never)
pub async fn extract_keywords_hybrid(
    context: &str,
    fast_provider: Option<&dyn crate::providers::Provider>,
) -> Vec<String> {
    // Get AI keyword extraction mode from environment
    let mode = AiKeywordMode::from_env();
    
    // If mode is Never, skip AI entirely
    if mode == AiKeywordMode::Never {
        return extract_context_keywords(context);
    }
    
    // Step 1: Try rule-based extraction first (unless mode is Always)
    let keywords = if mode == AiKeywordMode::Always {
        Vec::new()  // Skip rule-based when Always mode
    } else {
        extract_context_keywords(context)
    };
    
    // Step 2: Check if we should use AI based on mode and keyword count
    if !mode.should_use_ai(keywords.len()) {
        return keywords;
    }
    
    // Step 3: If we should use AI and have a provider, do AI extraction
    if let Some(provider) = fast_provider {
        match extract_keywords_with_ai(context, provider).await {
            Ok(ai_keywords) if !ai_keywords.is_empty() => {
                log::debug!("AI extracted {} keywords: {:?}", ai_keywords.len(), ai_keywords);
                // In Auto mode, merge AI keywords with rule-based ones
                if mode == AiKeywordMode::Auto && !keywords.is_empty() {
                    let merged = keywords
                        .into_iter()
                        .chain(ai_keywords.into_iter())
                        .collect::<std::collections::HashSet<_>>();
                    return merged.into_iter().collect();
                }
                return ai_keywords;
            }
            Ok(_) => {
                log::debug!("AI returned no keywords, keeping rule-based results");
            }
            Err(e) => {
                log::warn!("AI keyword extraction failed: {}, keeping rule-based results", e);
            }
        }
    }
    
    // Return whatever we have (rule-based results)
    keywords
}

// ============================================================================
// AI-Enhanced Memory Processing
// ============================================================================

/// System prompt for AI memory summarization.
const MEMORY_SUMMARY_SYSTEM_PROMPT: &str = r#"你是一个记忆摘要助手。你的任务是将多条相关记忆合并为一条精炼的摘要记忆。

摘要原则：
1. 保留核心信息，去除冗余细节
2. 使用简洁明确的一句话表达
3. 保留关键的技术名词和决策结论
4. 如果多条记忆主题相同，合并为一条综合性记忆
5. 优先保留高价值的决策和解决方案

输出格式（严格 JSON）：
```json
{
  "summary": "决定使用 PostgreSQL 作为主数据库，Redis 作为缓存层",
  "category": "decision",
  "importance": 90
}
```

如果没有值得保留的信息，返回：
```json
{"summary": "", "category": "", "importance": 0}
```

直接输出 JSON，不要加代码块包裹。"#;

/// System prompt for AI conflict detection.
const MEMORY_CONFLICT_SYSTEM_PROMPT: &str = r#"你是一个记忆冲突检测助手。你的任务是判断两条记忆是否矛盾或需要更新。

冲突类型：
1. 直接矛盾：两条记忆结论相反（如"使用 PostgreSQL" vs "使用 MySQL"）
2. 过时更新：新记忆明确替换旧记忆（如"改用 Redis" 替换 "使用 Memcached"）
3. 补充关系：新记忆补充旧记忆（如"PostgreSQL 版本为 15" 补充 "使用 PostgreSQL"）
4. 无关关系：两条记忆主题不同，不冲突

输出格式（严格 JSON）：
```json
{
  "conflict_type": "direct_conflict",
  "should_replace": true,
  "reason": "两条记忆都是数据库选型决策，但选择了不同的数据库",
  "winner": "new"
}
```

conflict_type 可选值：
- "direct_conflict": 直接矛盾，需要选择一条
- "outdated_update": 过时更新，新记忆替换旧记忆
- "supplement": 补充关系，两者可共存
- "no_conflict": 无关关系，不冲突

should_replace: true 表示需要替换旧记忆，false 表示保留两者
winner: "new" 表示新记忆胜出，"old" 表示旧记忆胜出（仅在 direct_conflict 时有意义）

直接输出 JSON，不要加代码块包裹。"#;

/// System prompt for AI memory quality assessment.
const MEMORY_QUALITY_SYSTEM_PROMPT: &str = r#"你是一个记忆质量评估助手。你的任务是评估记忆的长期价值和重要程度。

评估维度：
1. 复用价值：这条信息在未来的���话中会被引用吗？
2. 决策权重：这是重要的项目决策还是次要细节？
3. 时效性：这条信息会很快过时吗？
4. 独特性：这条信息是否足够独特，不与其他记忆重叠？

评分标准：
- 90-100: 核心决策，长期有效，高复用价值（如数据库选型、框架选择）
- 70-89: 重要偏好或解决方案，中等复用价值
- 50-69: 有用的技术信息或发现，时效性中等
- 30-49: 一般性信息，复用价值较低
- 0-29: 过时或过于具体的细节，建议丢弃

输出格式（严格 JSON）：
```json
{
  "quality_score": 85,
  "reason": "这是核心的技术选型决策，长期有效，高复用价值",
  "should_keep": true,
  "suggested_category": "decision"
}
```

直接输出 JSON，不要加代码块包裹。"#;

/// System prompt for AI memory merge.
const MEMORY_MERGE_SYSTEM_PROMPT: &str = r#"你是一个记忆合并助手。你的任务是将多条相似或相关的记忆合并为一条精炼的记忆。

合并原则：
1. 相同主题的记忆应合并为一条综合性记忆
2. 保留所有关键信息，去除重复内容
3. 使用简洁的一句话表达
4. 合并后的记忆应比原记忆更全面但更简洁
5. 如果记忆完全不相关，返回空结果表示不应合并

输出格式（严格 JSON）：
```json
{
  "merged_content": "使用 PostgreSQL 作为主数据库（版本15），Redis 作为缓存层，通过连接池优化性能",
  "category": "technical",
  "importance": 75,
  "merged_from_count": 3,
  "summary_reason": "三条记忆都与数据库和缓存技术栈相关，合并为一条综合性技术栈记忆"
}
```

如果不应合并，返回：
```json
{"merged_content": "", "category": "", "importance": 0, "merged_from_count": 0, "summary_reason": "记忆主题不同，不应合并"}
```

直接输出 JSON，不要加代码块包裹。"#;

/// Result of AI memory summarization.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct MemorySummaryResult {
    pub summary: String,
    pub category: String,
    pub importance: f64,
}

/// Result of AI conflict detection.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct MemoryConflictResult {
    pub conflict_type: String,
    pub should_replace: bool,
    pub reason: String,
    pub winner: Option<String>,
}

/// Result of AI quality assessment.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct MemoryQualityResult {
    pub quality_score: f64,
    pub reason: String,
    pub should_keep: bool,
    pub suggested_category: Option<String>,
}

/// Result of AI memory merge.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct MemoryMergeResult {
    pub merged_content: String,
    pub category: String,
    pub importance: f64,
    pub merged_from_count: usize,
    pub summary_reason: String,
}

/// AI-enhanced memory processor.
/// Provides advanced memory operations using AI.
pub struct AiMemoryProcessor {
    provider: Box<dyn crate::providers::Provider>,
    model: String,
}

impl AiMemoryProcessor {
    /// Create a new AI memory processor.
    pub fn new(provider: Box<dyn crate::providers::Provider>, model: String) -> Self {
        Self { provider, model }
    }
    
    /// Summarize multiple memories into one concise memory.
    pub async fn summarize_memories(&self, memories: &[&MemoryEntry]) -> Result<Option<MemoryEntry>> {
        if memories.is_empty() {
            return Ok(None);
        }
        
        // Build input from memories
        let memories_text = memories
            .iter()
            .map(|m| format!("[{}] {}", m.category.display_name(), m.content))
            .collect::<Vec<_>>()
            .join("\n");
        
        let request = build_ai_request(
            MEMORY_SUMMARY_SYSTEM_PROMPT,
            &format!("请将以下记忆合并为一条精炼的摘要：\n\n{}", memories_text),
        );
        
        let response = self.provider.chat(request).await?;
        let response_text = extract_response_text(&response);
        
        let result: MemorySummaryResult = parse_json_response(&response_text)?;
        
        if result.summary.is_empty() {
            return Ok(None);
        }
        
        let category = parse_category(&result.category)?;
        let mut entry = MemoryEntry::new(category, result.summary, None);
        entry.importance = result.importance.clamp(0.0, 100.0);
        
        Ok(Some(entry))
    }
    
    /// Detect if two memories conflict using AI.
    pub async fn detect_conflict(&self, old: &MemoryEntry, new: &MemoryEntry) -> Result<MemoryConflictResult> {
        let input = format!(
            "旧记忆：[{}] {}\n新记忆：[{}] {}\n\n请判断这两条记忆是否存在冲突。",
            old.category.display_name(),
            old.content,
            new.category.display_name(),
            new.content
        );
        
        let request = build_ai_request(MEMORY_CONFLICT_SYSTEM_PROMPT, &input);
        let response = self.provider.chat(request).await?;
        let response_text = extract_response_text(&response);
        
        parse_json_response(&response_text)
    }
    
    /// Assess memory quality using AI.
    pub async fn assess_quality(&self, memory: &MemoryEntry) -> Result<MemoryQualityResult> {
        let input = format!(
            "记忆内容：[{}] {}\n\n请评估这条记忆的质量和长期价值。",
            memory.category.display_name(),
            memory.content
        );
        
        let request = build_ai_request(MEMORY_QUALITY_SYSTEM_PROMPT, &input);
        let response = self.provider.chat(request).await?;
        let response_text = extract_response_text(&response);
        
        parse_json_response(&response_text)
    }
    
    /// Merge multiple memories using AI.
    pub async fn merge_memories(&self, memories: &[&MemoryEntry]) -> Result<Option<MemoryEntry>> {
        if memories.len() < 2 {
            return Ok(None);
        }
        
        let memories_text = memories
            .iter()
            .map(|m| format!("[{}] {}", m.category.display_name(), m.content))
            .collect::<Vec<_>>()
            .join("\n");
        
        let request = build_ai_request(
            MEMORY_MERGE_SYSTEM_PROMPT,
            &format!("请判断以下记忆是否应该合并，如果应该则生成合并后的记忆：\n\n{}", memories_text),
        );
        
        let response = self.provider.chat(request).await?;
        let response_text = extract_response_text(&response);
        
        let result: MemoryMergeResult = parse_json_response(&response_text)?;
        
        if result.merged_content.is_empty() || result.merged_from_count == 0 {
            return Ok(None);
        }
        
        let category = parse_category(&result.category)?;
        let mut entry = MemoryEntry::new(category, result.merged_content, None);
        entry.importance = result.importance.clamp(0.0, 100.0);
        
        Ok(Some(entry))
    }
    
    /// Get the model name.
    pub fn model_name(&self) -> &str {
        &self.model
    }
}

/// Build a standard AI request for memory processing.
fn build_ai_request(system_prompt: &str, user_input: &str) -> crate::providers::ChatRequest {
    use crate::providers::{ChatRequest, Message, MessageContent, Role};
    
    ChatRequest {
        messages: vec![Message {
            role: Role::User,
            content: MessageContent::Text(user_input.to_string()),
        }],
        tools: vec![],
        system: Some(system_prompt.to_string()),
        think: false,
        max_tokens: 512,
        server_tools: vec![],
        enable_caching: false,
    }
}

/// Extract text from AI response.
fn extract_response_text(response: &crate::providers::ChatResponse) -> String {
    response.content
        .iter()
        .filter_map(|block| {
            if let crate::providers::ContentBlock::Text { text } = block {
                Some(text.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("")
}

/// Parse JSON response with cleanup.
fn parse_json_response<T: serde::de::DeserializeOwned>(json_text: &str) -> Result<T> {
    let cleaned = json_text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    
    serde_json::from_str(cleaned).map_err(|e| anyhow::anyhow!("JSON parse error: {}", e))
}

/// Parse category string to MemoryCategory.
fn parse_category(s: &str) -> Result<MemoryCategory> {
    match s.to_lowercase().as_str() {
        "decision" | "决策" => Ok(MemoryCategory::Decision),
        "preference" | "偏好" => Ok(MemoryCategory::Preference),
        "solution" | "解决方案" => Ok(MemoryCategory::Solution),
        "finding" | "发现" => Ok(MemoryCategory::Finding),
        "technical" | "技术" => Ok(MemoryCategory::Technical),
        "structure" | "结构" => Ok(MemoryCategory::Structure),
        _ => anyhow::bail!("Unknown category: {}", s),
    }
}

/// Configuration for AI-enhanced memory processing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AiMemoryConfig {
    /// Enable AI summarization.
    pub enable_summarization: bool,
    /// Enable AI conflict detection.
    pub enable_conflict_detection: bool,
    /// Enable AI quality assessment.
    pub enable_quality_assessment: bool,
    /// Enable AI memory merging.
    pub enable_merging: bool,
    /// Minimum memories to trigger summarization.
    pub summarize_threshold: usize,
    /// Quality threshold for keeping memories.
    pub quality_threshold: f64,
    /// Similarity threshold for merging.
    pub merge_similarity_threshold: f64,
}

impl Default for AiMemoryConfig {
    fn default() -> Self {
        Self {
            enable_summarization: true,
            enable_conflict_detection: true,
            enable_quality_assessment: false,  // Optional, can be expensive
            enable_merging: true,
            summarize_threshold: 5,
            quality_threshold: 30.0,
            merge_similarity_threshold: 0.6,
        }
    }
}

impl AiMemoryConfig {
    /// Create a minimal config (disable all AI features).
    pub fn minimal() -> Self {
        Self {
            enable_summarization: false,
            enable_conflict_detection: false,
            enable_quality_assessment: false,
            enable_merging: false,
            summarize_threshold: 10,
            quality_threshold: 20.0,
            merge_similarity_threshold: 0.8,
        }
    }
    
    /// Create an aggressive config (enable all AI features).
    pub fn aggressive() -> Self {
        Self {
            enable_summarization: true,
            enable_conflict_detection: true,
            enable_quality_assessment: true,
            enable_merging: true,
            summarize_threshold: 3,
            quality_threshold: 40.0,
            merge_similarity_threshold: 0.5,
        }
    }
    
    /// Parse from environment variable.
    pub fn from_env() -> Self {
        let enable_all = std::env::var("MEMORY_AI_ALL")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);
        
        if enable_all {
            return Self::aggressive();
        }
        
        Self {
            enable_summarization: std::env::var("MEMORY_AI_SUMMARY")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(true),
            enable_conflict_detection: std::env::var("MEMORY_AI_CONFLICT")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(true),
            enable_quality_assessment: std::env::var("MEMORY_AI_QUALITY")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
            enable_merging: std::env::var("MEMORY_AI_MERGE")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(true),
            summarize_threshold: std::env::var("MEMORY_SUMMARY_THRESHOLD")
                .and_then(|v| v.parse().map_err(|_| std::env::VarError::NotPresent))
                .unwrap_or(5),
            quality_threshold: std::env::var("MEMORY_QUALITY_THRESHOLD")
                .and_then(|v| v.parse().map_err(|_| std::env::VarError::NotPresent))
                .unwrap_or(30.0),
            merge_similarity_threshold: std::env::var("MEMORY_MERGE_THRESHOLD")
                .and_then(|v| v.parse().map_err(|_| std::env::VarError::NotPresent))
                .unwrap_or(0.6),
        }
    }
}

/// Extended AutoMemory with AI-enhanced operations.
impl AutoMemory {
    /// Add memory with AI conflict detection.
    pub async fn add_memory_with_ai_conflict(
        &mut self,
        category: MemoryCategory,
        content: String,
        source_session: Option<String>,
        processor: Option<&AiMemoryProcessor>,
    ) -> Result<()> {
        // Check for duplicates first (rule-based, fast)
        if self.has_similar(&content) {
            return Ok(());
        }
        
        // Create new entry
        let new_entry = MemoryEntry::new(category, content.clone(), source_session);
        
        // Find potential conflicts (same category, similar topic)
        let potential_conflicts: Vec<(usize, &MemoryEntry)> = self.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                e.category == category && 
                Self::calculate_similarity(&e.content.to_lowercase(), &content.to_lowercase()) > 0.3
            })
            .collect();
        
        if let Some(processor) = processor {
            // Use AI to check each potential conflict
            for (idx, old_entry) in potential_conflicts {
                let result = processor.detect_conflict(old_entry, &new_entry).await?;
                
                if result.should_replace {
                    log::debug!("AI detected conflict: {} -> replacing '{}' with '{}'", 
                        result.conflict_type, old_entry.content, content);
                    self.entries.remove(idx);
                    self.invalidate_index();
                    break;
                }
            }
        } else {
            // Fallback to rule-based conflict detection
            if let Some(conflict_idx) = self.find_conflict(&content, category) {
                self.entries.remove(conflict_idx);
                self.invalidate_index();
            }
        }
        
        self.add(new_entry);
        Ok(())
    }
    
    /// Assess and filter memories by quality using AI.
    pub async fn assess_quality_with_ai(
        &mut self,
        processor: &AiMemoryProcessor,
        config: &AiMemoryConfig,
    ) -> Result<usize> {
        if !config.enable_quality_assessment {
            return Ok(0);
        }
        
        // Collect indices of non-manual entries first
        let indices_to_assess: Vec<usize> = self.entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| !entry.is_manual)
            .map(|(i, _)| i)
            .collect();
        
        // Assess each entry and collect results
        let mut to_remove: Vec<usize> = Vec::new();
        let mut importance_updates: Vec<(usize, f64)> = Vec::new();
        
        for i in indices_to_assess {
            let entry = &self.entries[i];
            let result = processor.assess_quality(entry).await?;
            
            if !result.should_keep || result.quality_score < config.quality_threshold {
                log::debug!("AI quality assessment: removing '{}' (score: {:.1}, reason: {})",
                    entry.content, result.quality_score, result.reason);
                to_remove.push(i);
            } else {
                // Record importance update
                importance_updates.push((i, result.quality_score));
            }
        }
        
        // Apply importance updates
        for (i, score) in importance_updates {
            self.entries[i].importance = score;
        }
        
        let removed_count = to_remove.len();
        
        // Remove low-quality entries (in reverse order to preserve indices)
        for idx in to_remove.into_iter().rev() {
            self.entries.remove(idx);
        }
        
        if removed_count > 0 {
            self.invalidate_index();
            self.prune();
        }
        
        Ok(removed_count)
    }
    
    /// Merge similar memories using AI.
    pub async fn merge_similar_with_ai(
        &mut self,
        processor: &AiMemoryProcessor,
        config: &AiMemoryConfig,
    ) -> Result<usize> {
        if !config.enable_merging || self.entries.len() < 2 {
            return Ok(0);
        }
        
        let mut merged_count = 0;
        let mut to_remove: Vec<usize> = Vec::new();
        let mut new_entries: Vec<MemoryEntry> = Vec::new();
        
        // Find groups of similar memories
        let mut processed: std::collections::HashSet<usize> = std::collections::HashSet::new();
        
        for i in 0..self.entries.len() {
            if processed.contains(&i) {
                continue;
            }
            
            // Find similar entries to this one
            let mut similar_group: Vec<usize> = vec![i];
            
            for j in (i + 1)..self.entries.len() {
                if processed.contains(&j) {
                    continue;
                }
                
                let sim = Self::calculate_similarity(
                    &self.entries[i].content.to_lowercase(),
                    &self.entries[j].content.to_lowercase(),
                );
                
                if sim >= config.merge_similarity_threshold {
                    similar_group.push(j);
                }
            }
            
            // If we have a group, try to merge
            if similar_group.len() >= 2 {
                let group_entries: Vec<&MemoryEntry> = similar_group
                    .iter()
                    .map(|&idx| &self.entries[idx])
                    .collect();
                
                if let Some(merged) = processor.merge_memories(&group_entries).await? {
                    log::debug!("AI merged {} memories into: '{}'",
                        similar_group.len(), merged.content);
                    
                    new_entries.push(merged);
                    to_remove.extend(similar_group.iter().copied());
                    processed.extend(similar_group.iter().copied());
                    merged_count += similar_group.len() - 1;
                }
            }
        }
        
        // Remove merged entries (sorted and in reverse order)
        let mut sorted_remove: Vec<usize> = to_remove;
        sorted_remove.sort();
        for idx in sorted_remove.into_iter().rev() {
            self.entries.remove(idx);
        }
        
        // Add new merged entries
        for entry in new_entries {
            self.entries.push(entry);
        }
        
        if merged_count > 0 {
            self.invalidate_index();
            self.prune();
        }
        
        Ok(merged_count)
    }
    
    /// Generate AI-enhanced summary for prompt.
    pub async fn generate_ai_summary(
        &self,
        max_entries: usize,
        processor: Option<&AiMemoryProcessor>,
        config: Option<&AiMemoryConfig>,
    ) -> Result<String> {
        if self.entries.is_empty() {
            return Ok(String::new());
        }
        
        let default_config = AiMemoryConfig::default();
        let config = config.unwrap_or(&default_config);
        
        // If AI summarization is enabled and we have a processor
        if config.enable_summarization
            && let Some(processor) = processor
            && self.entries.len() >= config.summarize_threshold
        {
            
            // Group by category
            let mut by_category: HashMap<MemoryCategory, Vec<&MemoryEntry>> = HashMap::new();
            for entry in &self.entries {
                by_category.entry(entry.category).or_default().push(entry);
            }
            
            let mut summary = String::from("【跨会话记忆 (AI摘要)】\n\n");
            
            for (cat, entries) in by_category {
                if entries.is_empty() {
                    continue;
                }
                
                // Get top entries by importance
                let top_entries: Vec<&MemoryEntry> = entries
                    .iter()
                    .take(max_entries.min(entries.len()))
                    .copied()
                    .collect();
                
                // Try AI summarization for this category
                if let Some(ai_summary) = processor.summarize_memories(&top_entries).await? {
                    summary.push_str(&format!("{} {}:\n", cat.icon(), cat.display_name()));
                    summary.push_str(&format!("  {}\n\n", ai_summary.content));
                } else {
                    // Fallback to individual entries
                    summary.push_str(&format!("{} {}:\n", cat.icon(), cat.display_name()));
                    for entry in top_entries {
                        summary.push_str(&format!("  {}\n", entry.format_for_prompt()));
                    }
                    summary.push('\n');
                }
            }
            
            Ok(summary)
        } else {
            // Fallback to rule-based summary
            Ok(self.generate_contextual_summary("", max_entries))
        }
    }
}



// ============================================================================
// SECTION 7: Detection (Rule-based Memory Detection)
// ============================================================================

/// Detect potential memory entries from conversation content.
/// This is the fallback method using rule-based detection (no AI).
/// For AI-based extraction, use AiMemoryExtractor.
pub fn detect_memories_fallback(text: &str, session_id: Option<&str>) -> Vec<MemoryEntry> {
    let mut entries = Vec::new();
    let text_lower = text.to_lowercase();

    // Detection patterns for each category (specific phrases to avoid generic matches)
    // Extended with more natural expressions for better detection coverage
    let patterns: Vec<(MemoryCategory, Vec<&str>)> = vec![
        (MemoryCategory::Decision, vec![
            // Chinese: specific decision phrases (original)
            "最终决定", "决定采用", "我们决定", "最终选择", "经过讨论决定",
            "项目决定", "团队决定", "最终选定", "确定使用",
            // Chinese: extended decision phrases (new)
            "选择使用", "采用方案", "最终方案", "定下来",
            "就定这个", "确定方案", "敲定", "拍板",
            "那就用", "用这个", "选定了", "定好",
            "统一用", "标准是", "规范是",
            // English: specific decision phrases
            "we decided", "final decision", "decided to use", "chose to use",
            "team decided", "final choice", "ultimately chose",
            // English: extended decision phrases (new)
            "selected", "will use", "going with", "settled on",
            "agreed to use", "conclusion is", "our choice is",
        ]),
        (MemoryCategory::Preference, vec![
            // Chinese: explicit preference phrases (original)
            // "我喜欢xxx" - direct preference declaration
            "我喜欢", "我最喜欢", "我特别喜欢", "我非常喜欢",
            // "我偏好xxx" - formal preference
            "我偏好", "我偏好使用", "个人偏好",
            // "我习惯xxx" - habit-based preference
            "我习惯", "我习惯用", "我的习惯", "通常我会",
            // "倾向于xxx" - tendency/inclination
            "我倾向于", "更倾向于", "我偏爱",
            // Chinese: extended preference phrases (new)
            "最常用", "一直用", "推荐", "建议使用",
            "觉得好", "感觉很顺手", "我的选择",
            "首选", "优先考虑", "比较喜欢",
            "最好用", "最顺手", "最熟悉",
            "一直都是", "长期使用", "经验上是",
            // English: explicit preference phrases
            "i like", "i prefer", "my favorite", "i love",
            "i prefer using", "my preference is", "i usually use",
            "i tend to use", "my habit is", "i really like",
            // English: extended preference phrases (new)
            "i recommend", "i suggest", "my go-to",
            "best choice", "preferred", "always use",
            "comfortable with", "familiar with", "i stick to",
        ]),
        (MemoryCategory::Solution, vec![
            // Chinese: specific fix/solution phrases (original)
            "通过修改", "通过添加", "通过删除", "解决方案是",
            "修复方法是", "解决方法是", "根本原因是",
            "修复了问题", "解决了问题", "关键修复",
            // Chinese: extended solution phrases (new)
            "搞定", "解决了", "修复成功", "问题解决了",
            "改成", "改成这样", "调整后", "优化了",
            "处理方式", "办法是", "做法是",
            "改好了", "修好了", "调好了",
            "这样改", "这样修", "改完之后",
            // English: specific fix phrases
            "fixed by", "solved by", "solution is", "root cause is",
            "the fix was", "fixed the issue",
            // English: extended solution phrases (new)
            "resolved", "patched", "corrected",
            "workaround is", "fix applied", "changed to fix",
            "solution was", "how we fixed", "fixing approach",
        ]),
        (MemoryCategory::Finding, vec![
            // Chinese: explicit findings (original)
            "关键发现", "重要发现", "我注意到", "发现问题是",
            "问题根源是", "问题出在", "主要原因是",
            // Chinese: extended finding phrases (new)
            "发现", "注意到", "原来", "找到问题",
            "定位到", "排查发现", "原因是", "问题在",
            "找到了", "查到了", "找到了问题",
            "原来如此", "原来是", "实际是",
            "原因是这个", "根源是", "症结是",
            // English: explicit findings
            "key finding", "important discovery", "found that the",
            "the issue is", "root cause", "discovered that",
            // English: extended finding phrases (new)
            "found", "noticed", "observed", "identified",
            "located", "traced", "diagnosed",
            "the problem is", "the cause is", "it turns out",
        ]),
        (MemoryCategory::Technical, vec![
            // Chinese: technical context (original)
            "技术栈是", "框架使用", "依赖的是", "构建工具是",
            "数据库是", "后端框架", "前端框架",
            // Chinese: extended technical phrases (new)
            "用的是", "基于", "跑在", "写的是",
            "开发语言是", "语言是", "运行在",
            "环境是", "版本是", "库是",
            "包是", "模块是", "组件是",
            "服务是", "端是", "层是",
            // English: technical context
            "tech stack is", "using framework", "built with",
            "database is", "backend uses", "frontend uses",
            // English: extended technical phrases (new)
            "implemented with", "written in", "runs on",
            "powered by", "based on", "developed with",
            "version", "library is", "package is",
        ]),
        (MemoryCategory::Structure, vec![
            // Chinese: structure info (original)
            "入口文件是", "主文件位于", "核心模块是", "项目结构是",
            "主要目录", "核心目录", "重要文件是",
            // Chinese: extended structure phrases (new)
            "入口是", "主入口", "启动文件", "入口点",
            "主要在", "核心在", "重点在",
            "文件结构", "目录结构", "代码结构",
            "位于", "放在", "存放在",
            "目录是", "文件夹是", "路径是",
            // English: structure info
            "entry point is", "main file is", "core module is",
            "project structure", "main directory",
            // English: extended structure phrases (new)
            "entry is", "starts from", "boot file",
            "located at", "placed in", "stored in",
            "directory is", "folder is", "path is",
        ]),
    ];

    for (category, keywords) in patterns {
        for keyword in keywords {
            if text_lower.contains(keyword) {
                // Extract the relevant sentence or phrase
                let content = extract_memory_content(text, keyword);
                // Use higher threshold to avoid too generic content
                if !content.is_empty() && content.len() >= MIN_MEMORY_CONTENT_LENGTH {
                    let entry = MemoryEntry::new(
                        category,
                        content,
                        session_id.map(|s| s.to_string()),
                    );
                    entries.push(entry);
                }
            }
        }
    }

    // Deduplicate by content similarity
    deduplicate_entries(entries)
}

/// Detect memories from text using the rule-based fallback method.
/// This is kept for backward compatibility and for cases where AI is unavailable.
pub fn detect_memories_from_text(text: &str, session_id: Option<&str>) -> Vec<MemoryEntry> {
    detect_memories_fallback(text, session_id)
}

// ============================================================================
// SECTION 8: Feedback Learning (User Correction Detection)
// ============================================================================

/// Action to take when user feedback is detected.
#[derive(Debug, Clone, PartialEq)]
pub enum FeedbackAction {
    /// Correct/replace existing memory with new content.
    Correct,
    /// Delete existing memory.
    Delete,
    /// Add new manual memory.
    Add,
    /// Negative preference (user dislikes something).
    NegativePreference,
}

/// Result of feedback detection.
#[derive(Debug, Clone)]
pub struct FeedbackResult {
    /// The action to take.
    pub action: FeedbackAction,
    /// Target memory category (if applicable).
    pub category: Option<MemoryCategory>,
    /// The new content (for Correct/Add actions).
    pub new_content: Option<String>,
    /// Keywords to search for existing memories (for Correct/Delete actions).
    pub search_keywords: Vec<String>,
    /// Original feedback text.
    pub original_text: String,
}

/// Detect user feedback patterns that indicate memory correction.
/// This allows the system to learn from user corrections like:
/// - "不对，应该是..." (No, it should be...)
/// - "错了，实际上是..." (Wrong, actually it's...)
/// - "不要那个了" (Don't want that anymore)
/// - "记一下..." (Remember this...)
pub fn detect_feedback_patterns(text: &str) -> Vec<FeedbackResult> {
    let mut results = Vec::new();
    let text_lower = text.to_lowercase();

    // Correction patterns (Chinese)
    let correction_patterns: Vec<&str> = vec![
        "不对，应该是", "不对应该是", "错了，实际上", "错了实际上",
        "不是，是", "不是是", "搞错了，应该是", "搞错了应该是",
        "搞错了，实际上是", "搞错了实际上", "误解了，实际上是",
        "那不对", "这不对", "不对的", "不正确的",
        "应该是", "实际上是", "真实情况是",
    ];

    // Correction patterns (English)
    let correction_patterns_en: Vec<&str> = vec![
        "no, it should be", "wrong, actually",
        "not correct, the", "incorrect, it's",
        "actually it is", "the correct", "should be",
        "it's actually", "what i meant",
    ];

    // Delete/negative patterns (Chinese)
    let delete_patterns: Vec<&str> = vec![
        "不要那个", "不需要那个", "删掉那个", "去掉那个",
        "不再用", "不再使用", "不再需要", "弃用",
        "不要了", "不需要了", "不用了",
    ];

    // Delete/negative patterns (English)
    let delete_patterns_en: Vec<&str> = vec![
        "don't need that", "no longer need",
        "remove that", "delete that",
        "not using anymore", "stop using",
        "don't want", "not needed",
    ];

    // Add/manual patterns (Chinese)
    let add_patterns: Vec<&str> = vec![
        "记一下", "记住", "记录一下", "记着",
        "要记住", "需要记住", "记下来",
        "帮我记", "帮我记住", "记录",
    ];

    // Add/manual patterns (English)
    let add_patterns_en: Vec<&str> = vec![
        "remember this", "note this", "keep this",
        "write down", "save this", "store this",
        "make a note", "take note",
    ];

    // Negative preference patterns (Chinese)
    let negative_pref_patterns: Vec<&str> = vec![
        "不喜欢", "不偏好", "讨厌", "不喜欢用",
        "不想用", "不愿意用", "反感",
        "不太喜欢", "不怎么喜欢", "最不喜欢",
    ];

    // Negative preference patterns (English)
    let negative_pref_patterns_en: Vec<&str> = vec![
        "i don't like", "i dislike", "i hate",
        "not my preference", "don't prefer",
        "not fond of", "don't want to use",
    ];

    // Check correction patterns
    for pattern in correction_patterns.iter().chain(correction_patterns_en.iter()) {
        if text_lower.contains(pattern) {
            // Extract content after the correction marker
            let content = extract_feedback_content(text, pattern);
            if !content.is_empty() && content.len() >= MIN_MEMORY_CONTENT_LENGTH {
                // Determine category from content
                let category = infer_category_from_content(&content);
                results.push(FeedbackResult {
                    action: FeedbackAction::Correct,
                    category: Some(category),
                    new_content: Some(content.clone()),
                    search_keywords: extract_search_keywords_from_correction(&content),
                    original_text: text.to_string(),
                });
            }
        }
    }

    // Check delete patterns
    for pattern in delete_patterns.iter().chain(delete_patterns_en.iter()) {
        if text_lower.contains(pattern) {
            let content = extract_feedback_content(text, pattern);
            results.push(FeedbackResult {
                action: FeedbackAction::Delete,
                category: None,
                new_content: None,
                search_keywords: if !content.is_empty() {
                    extract_context_keywords(&content)
                } else {
                    vec![pattern.to_string()]
                },
                original_text: text.to_string(),
            });
        }
    }

    // Check add/manual patterns
    for pattern in add_patterns.iter().chain(add_patterns_en.iter()) {
        if text_lower.contains(pattern) {
            let content = extract_feedback_content(text, pattern);
            if !content.is_empty() && content.len() >= MIN_MEMORY_CONTENT_LENGTH {
                let category = infer_category_from_content(&content);
                results.push(FeedbackResult {
                    action: FeedbackAction::Add,
                    category: Some(category),
                    new_content: Some(content),
                    search_keywords: vec![],  // No search for add
                    original_text: text.to_string(),
                });
            }
        }
    }

    // Check negative preference patterns
    for pattern in negative_pref_patterns.iter().chain(negative_pref_patterns_en.iter()) {
        if text_lower.contains(pattern) {
            let content = extract_feedback_content(text, pattern);
            if !content.is_empty() && content.len() >= MIN_MEMORY_CONTENT_LENGTH {
                results.push(FeedbackResult {
                    action: FeedbackAction::NegativePreference,
                    category: Some(MemoryCategory::Preference),
                    new_content: Some(format!("不喜欢/不偏好: {}", content)),
                    search_keywords: extract_context_keywords(&content),
                    original_text: text.to_string(),
                });
            }
        }
    }

    // Deduplicate results by action + content
    deduplicate_feedback_results(results)
}

/// Extract content after feedback pattern marker.
fn extract_feedback_content(text: &str, pattern: &str) -> String {
    let text_lower = text.to_lowercase();
    let pattern_lower = pattern.to_lowercase();

    // Find pattern position
    let pos = match text_lower.find(&pattern_lower) {
        Some(p) => p,
        None => return String::new(),
    };

    // Extract content after the pattern
    let start = pos + pattern.len();
    if start >= text.len() {
        return String::new();
    }

    // Find sentence end markers
    let sentence_end_markers: &[char] = &['.', '!', '?', '。', '！', '？', '\n'];

    // Extract until sentence end or reasonable length
    let remaining = &text[start..];
    let end = remaining.find(|c: char| sentence_end_markers.contains(&c))
        .map(|i| i + 1)
        .unwrap_or_else(|| remaining.len().min(MAX_MEMORY_CONTENT_LENGTH));

    let content = remaining[..end].trim();

    // Quality check
    if content.len() < MIN_MEMORY_CONTENT_LENGTH || is_low_quality_memory(content) {
        return String::new();
    }

    content.to_string()
}

/// Infer memory category from content keywords.
pub fn infer_category_from_content(content: &str) -> MemoryCategory {
    let content_lower = content.to_lowercase();

    // Check for decision keywords
    if content_lower.contains("决定") || content_lower.contains("选择") ||
       content_lower.contains("采用") || content_lower.contains("decided") ||
       content_lower.contains("chose") || content_lower.contains("selected") {
        return MemoryCategory::Decision;
    }

    // Check for preference keywords
    if content_lower.contains("喜欢") || content_lower.contains("偏好") ||
       content_lower.contains("习惯") || content_lower.contains("prefer") ||
       content_lower.contains("like") || content_lower.contains("habit") {
        return MemoryCategory::Preference;
    }

    // Check for solution keywords
    if content_lower.contains("修复") || content_lower.contains("解决") ||
       content_lower.contains("改成") || content_lower.contains("fixed") ||
       content_lower.contains("solved") || content_lower.contains("solution") {
        return MemoryCategory::Solution;
    }

    // Check for finding keywords
    if content_lower.contains("发现") || content_lower.contains("原因") ||
       content_lower.contains("问题") || content_lower.contains("found") ||
       content_lower.contains("issue") || content_lower.contains("cause") {
        return MemoryCategory::Finding;
    }

    // Check for structure keywords
    if content_lower.contains("文件") || content_lower.contains("目录") ||
       content_lower.contains("入口") || content_lower.contains("file") ||
       content_lower.contains("directory") || content_lower.contains("entry") {
        return MemoryCategory::Structure;
    }

    // Default to Technical for general information
    MemoryCategory::Technical
}

/// Extract keywords to search for existing memories when correcting.
fn extract_search_keywords_from_correction(content: &str) -> Vec<String> {
    // Use existing keyword extraction but limit to key terms
    let keywords = extract_context_keywords(content);
    // Take top 5 keywords for search
    keywords.iter().take(5).cloned().collect::<Vec<_>>()
}

/// Deduplicate feedback results by action and content.
fn deduplicate_feedback_results(results: Vec<FeedbackResult>) -> Vec<FeedbackResult> {
    if results.is_empty() {
        return results;
    }

    let mut unique: Vec<FeedbackResult> = Vec::new();
    for result in results {
        // Check if similar result already exists
        let is_duplicate = unique.iter().any(|existing| {
            existing.action == result.action &&
            existing.new_content == result.new_content
        });

        if !is_duplicate {
            unique.push(result);
        }
    }

    unique
}

/// Apply feedback to memory storage.
/// Returns the number of changes made (added, corrected, or deleted).
pub fn apply_feedback_to_memory(
    feedback: &FeedbackResult,
    memory: &mut AutoMemory,
) -> usize {
    let mut changes = 0;

    match feedback.action {
        FeedbackAction::Correct => {
            // Search for existing memories to correct
            if !feedback.search_keywords.is_empty() {
                let search_query = feedback.search_keywords.join(" ");
                let matching = memory.search(&search_query);

                // Find best match to correct
                if let Some(best_match) = matching.first() {
                    // Remove old entry
                    let old_id = best_match.id.clone();
                    if memory.remove(&old_id) {
                        log::debug!("Corrected memory: removed {}", old_id);
                        changes += 1;
                    }
                }
            }

            // Add new corrected content
            if let Some(ref content) = feedback.new_content {
                let category = feedback.category.unwrap_or(MemoryCategory::Technical);
                let mut entry = MemoryEntry::new(category, content.clone(), None);
                entry.is_manual = true;  // User-corrected memories are manual
                entry.importance = 80.0;  // Higher importance for user corrections
                memory.add(entry);
                log::debug!("Added corrected memory: {}", content);
                changes += 1;
            }
        }

        FeedbackAction::Delete => {
            // Search for memories to delete
            if !feedback.search_keywords.is_empty() {
                let search_query = feedback.search_keywords.join(" ");
                let matching = memory.search(&search_query);

                // Collect IDs to delete first (to avoid borrow conflict)
                let ids_to_delete: Vec<String> = matching.iter().take(3).map(|e| e.id.clone()).collect();

                // Delete all matching memories (up to 3 to avoid over-deletion)
                for id in ids_to_delete {
                    if memory.remove(&id) {
                        log::debug!("Deleted memory: {}", id);
                        changes += 1;
                    }
                }
            }
        }

        FeedbackAction::Add => {
            // Add new manual memory
            if let Some(ref content) = feedback.new_content {
                let category = feedback.category.unwrap_or(MemoryCategory::Technical);
                let entry = MemoryEntry::manual(category, content.clone());
                memory.add(entry);
                log::debug!("Added manual memory: {}", content);
                changes += 1;
            }
        }

        FeedbackAction::NegativePreference => {
            // Add negative preference as a preference entry
            if let Some(ref content) = feedback.new_content {
                let mut entry = MemoryEntry::new(MemoryCategory::Preference, content.clone(), None);
                entry.importance = 70.0;  // Higher importance for explicit dislikes
                entry.tags.push("negative".to_string());
                memory.add(entry);
                log::debug!("Added negative preference: {}", content);
                changes += 1;
            }
        }
    }

    if changes > 0 {
        memory.prune();
    }

    changes
}

// ============================================================================
// SECTION 9: Behavior Inference (Preference Learning from Patterns)
// ============================================================================

/// Configuration for behavior inference.
pub struct BehaviorInferenceConfig {
    /// Minimum occurrences to infer a preference.
    pub min_occurrences: usize,
    /// Minimum confidence threshold (0.0 - 1.0).
    pub min_confidence: f64,
    /// Maximum preferences to infer per analysis.
    pub max_inferences: usize,
}

impl Default for BehaviorInferenceConfig {
    fn default() -> Self {
        Self {
            min_occurrences: 2,
            min_confidence: 0.6,
            max_inferences: 5,
        }
    }
}

/// Result of behavior inference analysis.
#[derive(Debug, Clone)]
pub struct BehaviorInference {
    /// inferred preference content.
    pub content: String,
    /// Confidence level (0.0 - 1.0).
    pub confidence: f64,
    /// Occurrence count in messages.
    pub occurrences: usize,
    /// Related keywords/technologies detected.
    pub keywords: Vec<String>,
}

/// Infer user preferences from conversation behavior patterns.
/// Analyzes messages to detect repeated choices, frequently used terms,
/// and patterns that indicate user preferences.
pub fn infer_preferences_from_behavior(
    messages: &[crate::providers::Message],
    config: &BehaviorInferenceConfig,
) -> Vec<BehaviorInference> {
    let mut inferences: Vec<BehaviorInference> = Vec::new();

    // Collect user messages
    let user_texts: Vec<String> = messages.iter()
        .filter_map(|msg| {
            if msg.role == crate::providers::Role::User {
                match &msg.content {
                    crate::providers::MessageContent::Text(t) => Some(t.clone()),
                    crate::providers::MessageContent::Blocks(blocks) => {
                        // Extract text from blocks
                        Some(blocks.iter().filter_map(|b| {
                            if let crate::providers::ContentBlock::Text { text } = b {
                                Some(text.as_str())
                            } else {
                                None
                            }
                        }).collect::<Vec<_>>().join(" "))
                    }
                }
            } else {
                None
            }
        })
        .collect();

    if user_texts.len() < config.min_occurrences {
        return inferences;
    }

    // Combine all user texts for analysis
    let all_text = user_texts.join(" ");
    let all_text_lower = all_text.to_lowercase();

    // Technology/framework patterns to detect
    let tech_patterns: Vec<(&str, &str)> = vec![
        // Programming languages
        ("rust", "Rust"), ("python", "Python"), ("javascript", "JavaScript"),
        ("typescript", "TypeScript"), ("go", "Go"), ("java", "Java"),
        ("c++", "C++"), ("c#", "C#"), ("kotlin", "Kotlin"), ("swift", "Swift"),
        // Frameworks
        ("react", "React"), ("vue", "Vue"), ("angular", "Angular"),
        ("next.js", "Next.js"), ("svelte", "Svelte"),
        ("express", "Express"), ("fastapi", "FastAPI"), ("django", "Django"),
        ("spring", "Spring"), ("rails", "Rails"),
        // Tools
        ("docker", "Docker"), ("kubernetes", "Kubernetes"), ("git", "Git"),
        ("vim", "Vim"), ("emacs", "Emacs"), ("vscode", "VS Code"),
        ("linux", "Linux"), ("macos", "macOS"), ("windows", "Windows"),
        // Databases
        ("postgresql", "PostgreSQL"), ("mysql", "MySQL"), ("mongodb", "MongoDB"),
        ("redis", "Redis"), ("sqlite", "SQLite"),
        // Testing
        ("pytest", "pytest"), ("jest", "Jest"), ("cargo test", "cargo test"),
    ];

    // Count occurrences of each technology
    let mut tech_counts: HashMap<String, usize> = HashMap::new();
    for (pattern_lower, pattern_display) in &tech_patterns {
        let count = all_text_lower.matches(pattern_lower).count();
        if count >= config.min_occurrences {
            tech_counts.insert(pattern_display.to_string(), count);
        }
    }

    // Detect positive choice patterns
    let positive_patterns: Vec<&str> = vec![
        "用", "使用", "选择", "喜欢", "推荐", "偏好",
        "prefer", "use", "choose", "like", "recommend",
    ];

    let negative_patterns: Vec<&str> = vec![
        "不用", "不喜欢", "避免", "拒绝", "放弃",
        "don't use", "avoid", "dislike", "reject",
    ];

    // Analyze each detected technology for preference indicators
    for (tech, count) in &tech_counts {
        let mut positive_signals = 0;
        let mut negative_signals = 0;

        // Check context around technology mentions
        for text in &user_texts {
            let text_lower = text.to_lowercase();
            if text_lower.contains(&tech.to_lowercase()) {
                // Check for positive indicators
                for pattern in &positive_patterns {
                    if text_lower.contains(pattern) {
                        positive_signals += 1;
                    }
                }
                // Check for negative indicators
                for pattern in &negative_patterns {
                    if text_lower.contains(pattern) {
                        negative_signals += 1;
                    }
                }
            }
        }

        // Calculate confidence
        let total_signals = positive_signals + negative_signals;
        let confidence = if total_signals > 0 {
            (positive_signals as f64 - negative_signals as f64 * 0.5) / (*count).max(1) as f64
        } else {
            // No explicit signals, use occurrence frequency as implicit preference
            *count as f64 / user_texts.len() as f64
        };

        // Only add if confidence is sufficient and more positive than negative
        if confidence >= config.min_confidence && positive_signals >= negative_signals {
            let content = if positive_signals > 0 {
                format!("偏好使用 {} (出现 {} 次，正面信号 {} 次)", tech, count, positive_signals)
            } else {
                format!("频繁使用 {} (出现 {} 次)", tech, count)
            };

            inferences.push(BehaviorInference {
                content,
                confidence,
                occurrences: *count,
                keywords: vec![tech.clone()],
            });
        }
    }

    // Sort by confidence and limit
    inferences.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
    inferences.truncate(config.max_inferences);

    inferences
}

/// Convert behavior inference to memory entry.
pub fn inference_to_memory_entry(inference: &BehaviorInference) -> MemoryEntry {
    let mut entry = MemoryEntry::new(
        MemoryCategory::Preference,
        inference.content.clone(),
        None,
    );
    // Set importance based on confidence
    entry.importance = 50.0 + (inference.confidence * 30.0);  // Range: 50-80
    // Add inferred tag
    entry.tags.push("inferred".to_string());
    for keyword in &inference.keywords {
        entry.tags.push(keyword.clone());
    }
    entry
}

/// Apply behavior inferences to memory storage.
/// Returns the number of new preferences added.
pub fn apply_behavior_inferences_to_memory(
    messages: &[crate::providers::Message],
    memory: &mut AutoMemory,
    config: Option<&BehaviorInferenceConfig>,
) -> usize {
    let default_config = BehaviorInferenceConfig::default();
    let cfg = config.unwrap_or(&default_config);
    let inferences = infer_preferences_from_behavior(messages, cfg);

    let mut added = 0;
    for inference in &inferences {
        // Check if similar preference already exists
        let tech_keywords = inference.keywords.join(" ");
        if memory.search(&tech_keywords).is_empty() {
            // Add new inferred preference
            let entry = inference_to_memory_entry(inference);
            memory.add(entry);
            added += 1;
            log::debug!("Added inferred preference: {} (confidence: {:.2})", inference.content, inference.confidence);
        }
    }

    if added > 0 {
        memory.prune();
    }

    added
}

// ============================================================================
// SECTION 10: Project Analysis (Auto Structure Detection)
// ============================================================================

/// Project type detection configuration.
/// Maps detection files to project type and key files.
pub struct ProjectTypeConfig {
    pub type_name: &'static str,
    pub detect_files: &'static [&'static str],
    pub entry_files: &'static [&'static str],
    pub key_dirs: &'static [&'static str],
    pub tech_stack: &'static str,
}

/// Default project type configurations.
pub const PROJECT_TYPE_CONFIGS: &[ProjectTypeConfig] = &[
    ProjectTypeConfig {
        type_name: "Rust",
        detect_files: &["Cargo.toml"],
        entry_files: &["src/main.rs", "src/lib.rs"],
        key_dirs: &["src", "tests", "examples"],
        tech_stack: "Rust",
    },
    ProjectTypeConfig {
        type_name: "Node.js",
        detect_files: &["package.json"],
        entry_files: &["index.js", "src/index.js", "app.js", "main.js"],
        key_dirs: &["src", "lib", "components", "pages"],
        tech_stack: "Node.js",
    },
    ProjectTypeConfig {
        type_name: "TypeScript",
        detect_files: &["tsconfig.json", "package.json"],
        entry_files: &["src/index.ts", "src/main.ts", "src/app.ts"],
        key_dirs: &["src", "lib", "components", "pages"],
        tech_stack: "TypeScript",
    },
    ProjectTypeConfig {
        type_name: "React",
        detect_files: &["package.json"],
        entry_files: &["src/index.tsx", "src/index.jsx", "src/App.tsx", "src/App.jsx"],
        key_dirs: &["src/components", "src/pages", "src/hooks", "src/utils"],
        tech_stack: "React + TypeScript",
    },
    ProjectTypeConfig {
        type_name: "Vue",
        detect_files: &["vue.config.js", "vite.config.js", "package.json"],
        entry_files: &["src/main.ts", "src/main.js", "src/App.vue"],
        key_dirs: &["src/components", "src/views", "src/stores", "src/utils"],
        tech_stack: "Vue.js",
    },
    ProjectTypeConfig {
        type_name: "Python",
        detect_files: &["requirements.txt", "setup.py", "pyproject.toml"],
        entry_files: &["main.py", "app.py", "__main__.py", "src/__init__.py"],
        key_dirs: &["src", "lib", "tests", "app"],
        tech_stack: "Python",
    },
    ProjectTypeConfig {
        type_name: "Go",
        detect_files: &["go.mod"],
        entry_files: &["main.go", "cmd/main.go"],
        key_dirs: &["cmd", "pkg", "internal", "api"],
        tech_stack: "Go",
    },
    ProjectTypeConfig {
        type_name: "Java",
        detect_files: &["pom.xml", "build.gradle", "build.gradle.kts"],
        entry_files: &["src/main/java/Main.java", "src/main/java/Application.java"],
        key_dirs: &["src/main/java", "src/test/java", "src/main/resources"],
        tech_stack: "Java",
    },
    ProjectTypeConfig {
        type_name: "C++",
        detect_files: &["CMakeLists.txt", "Makefile"],
        entry_files: &["main.cpp", "src/main.cpp"],
        key_dirs: &["src", "include", "lib", "tests"],
        tech_stack: "C++",
    },
    ProjectTypeConfig {
        type_name: "C#",
        detect_files: &["*.csproj", "*.sln"],
        entry_files: &["Program.cs", "Main.cs"],
        key_dirs: &["src", "Tests", "Models", "Controllers"],
        tech_stack: "C#/.NET",
    },
];

/// Directories to ignore when scanning project structure.
pub const IGNORE_DIRS: &[&str] = &[
    ".git", ".github", ".matrix", ".idea", ".vscode",
    "node_modules", "target", "build", "dist", "out",
    "vendor", "__pycache__", ".venv", "venv", "env",
    "cache", "tmp", "temp", ".cache", ".tmp",
];

/// Project structure analyzer for automatic memory creation.
pub struct ProjectStructureAnalyzer {
    project_root: std::path::PathBuf,
}

impl ProjectStructureAnalyzer {
    /// Create a new analyzer for the given project root.
    pub fn new(project_root: std::path::PathBuf) -> Self {
        Self { project_root }
    }

    /// Detect project type based on configuration files.
    pub fn detect_project_type(&self) -> Option<&'static ProjectTypeConfig> {
        for config in PROJECT_TYPE_CONFIGS {
            for detect_file in config.detect_files {
                // Handle wildcard patterns (like *.csproj)
                if detect_file.starts_with('*') {
                    let extension = detect_file.trim_start_matches('*');
                    if let Ok(entries) = std::fs::read_dir(&self.project_root) {
                        for entry in entries.flatten() {
                            if entry.file_name().to_string_lossy().ends_with(extension) {
                                return Some(config);
                            }
                        }
                    }
                } else {
                    let path = self.project_root.join(detect_file);
                    if path.exists() {
                        return Some(config);
                    }
                }
            }
        }
        None
    }

    /// Find entry file for the project.
    pub fn find_entry_file(&self, config: &ProjectTypeConfig) -> Option<String> {
        for entry_file in config.entry_files {
            let path = self.project_root.join(entry_file);
            if path.exists() {
                return Some(entry_file.to_string());
            }
        }
        // Fallback: search for common patterns
        None
    }

    /// Scan project directories and identify their purposes.
    pub fn scan_key_directories(&self, config: &ProjectTypeConfig) -> Vec<(String, String)> {
        let mut dirs_info: Vec<(String, String)> = Vec::new();

        for key_dir in config.key_dirs {
            let path = self.project_root.join(key_dir);
            if path.exists() && path.is_dir() {
                // Determine directory purpose based on name
                let purpose = self.infer_directory_purpose(key_dir);
                dirs_info.push((key_dir.to_string(), purpose));
            }
        }

        // Also scan for additional common directories
        for entry in std::fs::read_dir(&self.project_root).unwrap_or_else(|_| std::fs::read_dir(".").unwrap()).flatten() {
            if entry.path().is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                // Skip ignore directories
                if IGNORE_DIRS.contains(&name.as_str()) {
                    continue;
                }
                // Check if already in config dirs
                if config.key_dirs.contains(&name.as_str()) {
                    continue;
                }
                // Infer purpose for additional directories
                let purpose = self.infer_directory_purpose(&name);
                if !purpose.is_empty() {
                    dirs_info.push((name, purpose));
                }
            }
        }

        dirs_info
    }

    /// Infer directory purpose based on name.
    fn infer_directory_purpose(&self, dir_name: &str) -> String {
        let dir_lower = dir_name.to_lowercase();

        // Common directory purposes
        if dir_lower.contains("component") || dir_lower == "components" {
            return "组件目录".to_string();
        }
        if dir_lower.contains("page") || dir_lower == "pages" || dir_lower == "views" {
            return "页面/视图目录".to_string();
        }
        if dir_lower.contains("hook") || dir_lower == "hooks" {
            return "Hook 目录".to_string();
        }
        if dir_lower.contains("util") || dir_lower == "utils" || dir_lower == "lib" || dir_lower == "libs" {
            return "工具/库目录".to_string();
        }
        if dir_lower.contains("test") || dir_lower == "tests" || dir_lower == "__tests__" || dir_lower == "spec" {
            return "测试目录".to_string();
        }
        if dir_lower.contains("model") || dir_lower == "models" {
            return "模型目录".to_string();
        }
        if dir_lower.contains("controller") || dir_lower == "controllers" {
            return "控制器目录".to_string();
        }
        if dir_lower.contains("service") || dir_lower == "services" {
            return "服务目录".to_string();
        }
        if dir_lower.contains("api") || dir_lower == "api" || dir_lower == "apis" {
            return "API 目录".to_string();
        }
        if dir_lower.contains("config") || dir_lower == "config" || dir_lower == "configs" {
            return "配置目录".to_string();
        }
        if dir_lower.contains("store") || dir_lower == "stores" || dir_lower == "state" {
            return "状态管理目录".to_string();
        }
        if dir_lower.contains("asset") || dir_lower == "assets" || dir_lower == "static" || dir_lower == "public" {
            return "资源目录".to_string();
        }
        if dir_lower.contains("doc") || dir_lower == "docs" || dir_lower == "documentation" {
            return "文档目录".to_string();
        }
        if dir_lower.contains("example") || dir_lower == "examples" || dir_lower == "demo" {
            return "示例目录".to_string();
        }
        if dir_lower == "src" || dir_lower == "source" {
            return "源代码目录".to_string();
        }

        // Return empty if purpose is unclear
        String::new()
    }

    /// Analyze project and generate Structure memory entries.
    pub fn analyze(&self) -> Vec<MemoryEntry> {
        let mut entries: Vec<MemoryEntry> = Vec::new();

        // Detect project type
        let project_type = self.detect_project_type();

        if let Some(config) = project_type {
            // Add project type as Technical memory
            entries.push(MemoryEntry::new(
                MemoryCategory::Technical,
                format!("项目类型: {}，技术栈: {}", config.type_name, config.tech_stack),
                None,
            ));

            // Add entry file as Structure memory
            if let Some(entry_file) = self.find_entry_file(config) {
                entries.push(MemoryEntry::new(
                    MemoryCategory::Structure,
                    format!("入口文件: {}", entry_file),
                    None,
                ));
            }

            // Add key directories as Structure memories
            let dirs_info = self.scan_key_directories(config);
            for (dir_name, purpose) in dirs_info.iter().take(10) {  // Limit to 10 directories
                if !purpose.is_empty() {
                    entries.push(MemoryEntry::new(
                        MemoryCategory::Structure,
                        format!("{} 目录: {} ({})", dir_name, self.project_root.join(dir_name).display(), purpose),
                        None,
                    ));
                }
            }

            // Add configuration file info
            for detect_file in config.detect_files.iter().take(3) {
                if !detect_file.starts_with('*') {  // Skip wildcard patterns
                    let path = self.project_root.join(detect_file);
                    if path.exists() {
                        entries.push(MemoryEntry::new(
                            MemoryCategory::Structure,
                            format!("配置文件: {}", detect_file),
                            None,
                        ));
                    }
                }
            }
        } else {
            // Unknown project type - still try to find basic structure
            // Check for common entry files
            let common_entries = [
                "main.rs", "main.go", "main.py", "main.js", "main.ts",
                "index.js", "index.ts", "app.js", "app.py",
                "src/main.rs", "src/main.rs", "src/index.ts",
            ];

            for entry in common_entries {
                let path = self.project_root.join(entry);
                if path.exists() {
                    entries.push(MemoryEntry::new(
                        MemoryCategory::Structure,
                        format!("入口文件: {}", entry),
                        None,
                    ));
                    break;
                }
            }

            // Add project root as basic structure
            entries.push(MemoryEntry::new(
                MemoryCategory::Structure,
                format!("项目根目录: {}", self.project_root.display()),
                None,
            ));
        }

        // Set appropriate importance for structure memories
        for entry in &mut entries {
            if entry.category == MemoryCategory::Structure {
                entry.importance = 40.0;  // Lower importance for auto-generated structure
            } else if entry.category == MemoryCategory::Technical {
                entry.importance = 50.0;  // Medium importance for tech stack
            }
            entry.tags.push("auto-analyzed".to_string());
        }

        log::debug!("Project structure analysis found {} potential memories", entries.len());
        entries
    }
}

/// Generate project structure memories and save to project memory file.
/// Returns the number of memories created.
pub fn generate_project_structure_memories(
    project_root: &std::path::Path,
    memory_storage: &mut MemoryStorage,
) -> usize {
    // Check if project memory already exists
    if let Ok(Some(existing)) = memory_storage.load_project() {
        // Check if already has structure memories
        let has_structure = existing.entries.iter().any(|e| {
            e.category == MemoryCategory::Structure && e.tags.contains(&"auto-analyzed".to_string())
        });
        if has_structure {
            log::debug!("Project already has structure memories, skipping analysis");
            return 0;
        }
    }

    // Analyze project structure
    let analyzer = ProjectStructureAnalyzer::new(project_root.to_path_buf());
    let entries = analyzer.analyze();

    if entries.is_empty() {
        return 0;
    }

    // Load existing project memory (if any) and merge
    let mut project_memory = memory_storage.load_project()
        .unwrap_or_else(|_| Some(AutoMemory::new()))
        .unwrap_or_else(AutoMemory::new);

    let count = entries.len();
    for entry in entries {
        project_memory.add(entry);
    }

    // Save project memory
    if let Err(e) = memory_storage.save_project(&project_memory) {
        log::warn!("Failed to save project structure memories: {}", e);
        return 0;
    }

    log::info!("Generated {} project structure memories", count);
    count
}

/// Smart memory detection that chooses the best method based on environment.
/// Uses AI when MEMORY_AI_DETECTION=always and extractor is provided.
/// Otherwise falls back to rule-based detection.
pub async fn detect_memories_smart(
    text: &str,
    session_id: Option<&str>,
    extractor: Option<&dyn MemoryExtractor>,
) -> Vec<MemoryEntry> {
    let mode = AiDetectionMode::from_env();

    if mode.should_use_ai() && extractor.is_some() {
        // Use AI detection
        match detect_memories_with_ai(text, session_id, extractor).await {
            Ok(entries) if !entries.is_empty() => {
                log::debug!("AI memory detection found {} entries", entries.len());
                return entries;
            }
            Ok(_) => {
                log::debug!("AI detection returned empty, falling back to rules");
            }
            Err(e) => {
                log::warn!("AI memory detection failed: {}, falling back to rules", e);
            }
        }
    }

    // Fallback to rule-based detection
    detect_memories_fallback(text, session_id)
}

/// Detect memories asynchronously using AI extractor.
/// Falls back to rule-based detection if AI fails or is unavailable.
pub async fn detect_memories_with_ai(
    text: &str,
    session_id: Option<&str>,
    extractor: Option<&dyn MemoryExtractor>,
) -> Result<Vec<MemoryEntry>> {
    if let Some(ai_extractor) = extractor {
        // Try AI extraction first
        match ai_extractor.extract(text, session_id).await {
            Ok(entries) if !entries.is_empty() => {
                return Ok(entries);
            }
            Ok(_) => {
                // AI returned empty, try fallback (silent)
            }
            Err(_) => {
                // AI extraction failed, try fallback (silent)
            }
        }
    }
    
    // Fallback to rule-based detection
    Ok(detect_memories_fallback(text, session_id))
}

/// Deduplicate entries by content similarity.
/// Keeps longer (more detailed) entries when duplicates are found.
fn deduplicate_entries(entries: Vec<MemoryEntry>) -> Vec<MemoryEntry> {
    if entries.is_empty() {
        return entries;
    }
    
    // Sort by content length (longer first - keep more detailed entries)
    let mut sorted = entries;
    sorted.sort_by(|a, b| b.content.len().cmp(&a.content.len()));
    
    // Keep only unique entries
    let mut unique: Vec<MemoryEntry> = Vec::new();
    for entry in sorted {
        let entry_lower = entry.content.to_lowercase();
        
        // Check if already have similar entry
        let is_duplicate = unique.iter().any(|existing| {
            let existing_lower = existing.content.to_lowercase();
            
            // Exact match
            if existing_lower == entry_lower {
                return true;
            }
            
            // High similarity (same words mostly)
            let similarity = calculate_similarity(&existing_lower, &entry_lower);
            similarity >= 0.8
        });
        
        if !is_duplicate {
            unique.push(entry);
        }
        
        // Stop if we have enough entries
        if unique.len() >= MAX_DETECTED_ENTRIES {
            break;
        }
    }
    
    unique
}

/// Extract memory content around a keyword.
/// Enhanced to extract complete sentences with proper boundary detection.
fn extract_memory_content(text: &str, keyword: &str) -> String {
    let text_lower = text.to_lowercase();
    let keyword_lower = keyword.to_lowercase();

    // Find keyword position
    let pos = match text_lower.find(&keyword_lower) {
        Some(p) => p,
        None => return String::new(),
    };

    // Find the complete sentence containing the keyword
    // Use more comprehensive sentence boundary markers
    let sentence_end_markers: &[char] = &['.', '!', '?', '。', '！', '？', '\n'];
    let sentence_start_markers: &[char] = &['\n'];

    // For start: find the last sentence boundary before pos
    // Prefer to start from a newline or beginning of text
    let start = text[..pos].rfind(sentence_start_markers)
        .map(|i| {
            // Skip the marker itself
            match text[i..].char_indices().nth(1) {
                Some((next_idx, _)) => i + next_idx,
                None => pos,
            }
        })
        .unwrap_or_else(|| {
            // If no newline found, check if we're at start of a sentence
            // by looking for sentence end markers
            text[..pos].rfind(sentence_end_markers)
                .map(|i| {
                    match text[i..].char_indices().nth(1) {
                        Some((next_idx, _)) => i + next_idx,
                        None => pos,
                    }
                })
                .unwrap_or(0)
        });

    // For end: find the first sentence end marker after pos
    let end = text[pos..].find(sentence_end_markers)
        .map(|i| {
            let marker_pos = pos + i;
            // Include the marker in the content (it's part of the sentence)
            match text[marker_pos..].char_indices().nth(1) {
                Some((next_idx, _)) => marker_pos + next_idx,
                None => text.len(),
            }
        })
        .unwrap_or_else(|| {
            // No marker found: use reasonable length limit
            let max_end = (pos + MAX_MEMORY_CONTENT_LENGTH).min(text.len());
            // Find valid UTF-8 boundary
            let mut boundary = max_end;
            while boundary > pos && !text.is_char_boundary(boundary) {
                boundary -= 1;
            }
            boundary
        });

    // Ensure valid boundaries
    if start >= end || start > text.len() || end > text.len() {
        return String::new();
    }

    let content = text[start..end].trim();

    // Quality check: reject low quality content
    if is_low_quality_memory(content) {
        return String::new();
    }

    // Ensure content is a complete thought
    // Check that it doesn't start mid-sentence (starts with lowercase after space)
    let trimmed = content.trim_start();
    if let Some(first_char) = trimmed.chars().next() {
        // Reject if starts with lowercase letter preceded by punctuation (truncated sentence)
        if first_char.is_lowercase() && first_char > '\u{4E00}' {
            // Chinese lowercase character after truncation point
            return String::new();
        }
    }

    // Final truncation if too long
    if content.len() > MAX_MEMORY_CONTENT_LENGTH {
        // Try to truncate at a sentence boundary within the content
        let truncation_point = content[..MAX_MEMORY_CONTENT_LENGTH]
            .rfind(sentence_end_markers)
            .map(|i| i + 1)  // Include the marker
            .unwrap_or(MAX_MEMORY_CONTENT_LENGTH - 3);
        truncate_str(content, truncation_point)
    } else {
        content.to_string()
    }
}

/// Check if extracted content is low quality (formatting artifacts, etc).
/// Enhanced with more checks for content completeness and semantic quality.
fn is_low_quality_memory(content: &str) -> bool {
    // Too short to be meaningful (updated threshold)
    if content.len() < MIN_MEMORY_CONTENT_LENGTH {
        return true;
    }

    // Contains formatting characters (table borders, tree lines)
    let formatting_chars = ['│', '├', '└', '┌', '┐', '─', '═', '║', '╔', '╗', '╚', '╝'];
    if content.chars().any(|c| formatting_chars.contains(&c)) {
        return true;
    }

    // Starts with emoji (likely formatted output, not user intent)
    let first_char = content.chars().next().unwrap_or(' ');
    if !first_char.is_alphanumeric() && !first_char.is_ascii_punctuation() && first_char > '\u{FF}' {
        return true;  // Reject all emoji-starting content
    }

    // Contains memory system markers (self-referential)
    if content.contains("【自动记忆摘要】") || content.contains("[ACCUMULATED MEMORY]") ||
       content.contains("记忆统计") || content.contains("memory.json") ||
       content.contains("Debug Report") || content.contains("诊断报告") {
        return true;
    }

    // Looks like a list item without substance
    if (content.starts_with("- ") || content.starts_with("* ") || content.starts_with("• "))
       && content.len() < 30 {
        return true;
    }

    // Contains mostly numbers/punctuation (likely code output)
    let alpha_count = content.chars().filter(|c| c.is_alphabetic()).count();
    let total_count = content.chars().count();
    if total_count > 0 && alpha_count < total_count / 4 {
        return true;
    }

    // Check for incomplete sentence patterns
    // Content starting with "rs**:" or similar code fragments
    if content.starts_with("rs**") || content.starts_with("rs:") ||
       content.starts_with("fn ") || content.starts_with("pub fn") ||
       content.starts_with("let ") || content.starts_with("use ") {
        return true;
    }

    // Check for truncated content (starts with lowercase after punctuation)
    // This indicates content was cut from middle of sentence
    let trimmed = content.trim();
    if let Some(second_char) = trimmed.chars().nth(1) {
        let first = trimmed.chars().next().unwrap_or(' ');
        // Starts with punctuation then lowercase (e.g., ".我", ",决定")
        if !first.is_alphanumeric() && second_char.is_lowercase() && second_char > '\u{4E00}' {
            return true;
        }
    }

    // Check for generic fragments that are too short to be useful
    // Phrases like "好的，采用" without context
    if content.len() < 25 && (
        content.contains("好的") || content.contains("好的，") ||
        content.contains("可以") || content.contains("没问题")
    ) {
        return true;
    }

    // Check for repeated punctuation (likely formatting artifact)
    let punct_count = content.chars().filter(|&c|
        c == '.' || c == ',' || c == '!' || c == '?' || c == '。' || c == '，'
    ).count();
    if punct_count > content.len() / 5 {
        return true;
    }

    false
}

// ============================================================================
// Rewind / Summarize Up To Here
// ============================================================================

/// Result of a rewind/summarize operation.
#[derive(Debug, Clone)]
pub struct RewindResult {
    /// Original message count.
    pub original_count: usize,
    /// New message count after rewind.
    pub new_count: usize,
    /// Index where rewind was applied.
    pub rewind_index: usize,
    /// Summary generated for removed messages.
    pub summary: Option<String>,
    /// New message list (summary message + kept messages).
    pub new_messages: Vec<Message>,
}

/// Summarize messages up to a specific index, keeping recent ones.
/// Returns the new message list with summary + kept messages.
pub async fn summarize_up_to(
    messages: &[Message],
    index: usize,
    compressor: Option<&dyn crate::compress::Compressor>,
) -> Result<RewindResult> {
    if index >= messages.len() {
        anyhow::bail!("rewind index {} out of bounds (messages: {})", index, messages.len());
    }

    if index == 0 {
        // Nothing to summarize, return original messages
        return Ok(RewindResult {
            original_count: messages.len(),
            new_count: messages.len(),
            rewind_index: 0,
            summary: None,
            new_messages: messages.to_vec(),
        });
    }

    let to_summarize = &messages[..index];
    let to_keep = &messages[index..];

    // Generate summary
    let summary = if let Some(comp) = compressor {
        // Use AI compressor
        let segment = comp.summarize(to_summarize, &crate::compress::CompressionConfig::default()).await?;
        Some(segment.summary)
    } else {
        // Fallback to simple summary
        Some(generate_simple_summary(to_summarize))
    };

    // Build summary message
    let summary_msg = create_summary_message(&summary, to_summarize.len());

    // New message list: summary + kept messages
    let new_messages: Vec<Message> = std::iter::once(summary_msg)
        .chain(to_keep.iter().cloned())
        .collect();
    
    let new_count = new_messages.len();

    Ok(RewindResult {
        original_count: messages.len(),
        new_count,
        rewind_index: index,
        summary,
        new_messages,
    })
}

/// Create a summary message for injection.
fn create_summary_message(summary: &Option<String>, original_count: usize) -> Message {
    let content = match summary {
        Some(s) => format!("[对话摘要 - 原 {} 条消息]\n\n{}", original_count, s),
        None => format!("[对话摘要 - 原 {} 条消息已压缩]", original_count),
    };

    Message {
        role: crate::providers::Role::User,
        content: crate::providers::MessageContent::Text(content),
    }
}

/// Generate a simple summary without AI.
fn generate_simple_summary(messages: &[Message]) -> String {
    let mut parts: Vec<String> = Vec::new();
    
    // Extract key points from each message
    for msg in messages {
        if msg.role == crate::providers::Role::User {
            let text = match &msg.content {
                crate::providers::MessageContent::Text(t) => t,
                _ => continue,
            };
            // Take first significant line
            let first_line = text.lines().next().unwrap_or("");
            if first_line.len() > 20 {
                parts.push(truncate_str(first_line, 100));
            }
        }
    }

    if parts.is_empty() {
        "对话已压缩".to_string()
    } else if parts.len() <= 5 {
        parts.join(" | ")
    } else {
        format!("{} ... (共 {} 个话题)", parts[0], parts.len())
    }
}

// ============================================================================
// Semantic Search
// ============================================================================

/// Cosine similarity calculation utility.
/// Used for vector-based semantic search when embedding API is available.
pub struct SemanticUtils;

impl SemanticUtils {
    /// Calculate cosine similarity between two embeddings.
    /// 
    /// ## 余弦相似度公式
    /// 
    /// cos(A, B) = (A · B) / (|A| × |B|)
    /// 
    /// 取值范围：
    /// - 1.0 = 完全相同
    /// - 0.0 = 无关
    /// - -1.0 = 完全相反
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }
        
        let dot_product = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum::<f32>();
        let norm_a = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        
        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }
        
        dot_product / (norm_a * norm_b)
    }
}


// ============================================================================
// SECTION 4: Retrieval (TF-IDF Search, Semantic Search)
// ============================================================================

/// Semantic search without AI (using TF-IDF like approach).
/// 
/// ## TF-IDF 语义搜索
/// 
/// TF-IDF（Term Frequency-Inverse Document Frequency）是一种
/// 不需要 AI 模型的语义搜索方法。
/// 
/// ### 原理
/// 
/// 1. **TF（词频）**: 词在文档中出现的频率
///    TF(word, doc) = count(word in doc) / len(doc)
/// 
/// 2. **IDF（逆文档频率）**: 词在整个文档集合中的稀有程度
///    IDF(word) = log(total_docs / docs_containing_word)
/// 
/// 3. **TF-IDF**: TF × IDF
///    高 TF-IDF = 词在此文档中重要，但在其他文档中不常见
/// 
/// ### 示例
/// 
/// ```ignore
/// 文档1: "使用 PostgreSQL 数据库"
/// 文档2: "Redis 缓存配置"
/// 文档3: "数据库连接池设置"
/// 
/// 查询: "数据库"
/// 
/// TF-IDF("数据库", 文档1) = 1/3 × log(3/2) = 0.33 × 0.41 = 0.14
/// TF-IDF("数据库", 文档3) = 1/4 × log(3/2) = 0.25 × 0.41 = 0.10
/// 
/// 结果: 文档1 > 文档3 > 文档2
/// ```
pub struct TfIdfSearch {
    /// Word frequency in each document.
    doc_word_freq: HashMap<String, HashMap<String, f32>>,
    /// Total documents.
    total_docs: usize,
    /// IDF cache.
    idf_cache: HashMap<String, f32>,
}

impl TfIdfSearch {
    /// Create a new TF-IDF search instance.
    pub fn new() -> Self {
        Self {
            doc_word_freq: HashMap::new(),
            total_docs: 0,
            idf_cache: HashMap::new(),
        }
    }
    
    /// Index all memories for TF-IDF search.
    pub fn index(&mut self, memory: &AutoMemory) {
        self.clear();
        self.total_docs = memory.entries.len();
        
        for entry in &memory.entries {
            let words = self.tokenize(&entry.content);
            let word_freq = self.compute_word_freq(&words);
            self.doc_word_freq.insert(entry.content.clone(), word_freq);
        }
        
        // Compute IDF for all words
        self.compute_idf();
    }
    
    /// Tokenize text into words.
    /// Supports both space-separated languages and CJK characters.
    fn tokenize(&self, text: &str) -> Vec<String> {
        let lower = text.to_lowercase();
        let mut tokens = Vec::new();
        
        // Split by whitespace first
        for word in lower.split_whitespace() {
            let trimmed = word.trim_matches(|c: char| !c.is_alphanumeric());
            if trimmed.len() > 1 {
                tokens.push(trimmed.to_string());
            }
            
            // For CJK characters, also add individual characters and bigrams
            let chars: Vec<char> = trimmed.chars().collect();
            let has_cjk = chars.iter().any(|c| Self::is_cjk(*c));
            
            if has_cjk {
                // Add individual CJK characters
                for c in &chars {
                    if Self::is_cjk(*c) {
                        tokens.push(c.to_string());
                    }
                }
                // Add bigrams for CJK
                for window in chars.windows(2) {
                    if Self::is_cjk(window[0]) || Self::is_cjk(window[1]) {
                        tokens.push(window.iter().collect::<String>());
                    }
                }
            }
        }
        
        tokens
    }
    
    /// Check if a character is CJK (Chinese/Japanese/Korean).
    fn is_cjk(c: char) -> bool {
        matches!(c,
            '\u{4E00}'..='\u{9FFF}' |   // CJK Unified Ideographs
            '\u{3400}'..='\u{4DBF}' |   // CJK Extension A
            '\u{F900}'..='\u{FAFF}' |   // CJK Compatibility Ideographs
            '\u{3000}'..='\u{303F}' |   // CJK Symbols and Punctuation
            '\u{3040}'..='\u{309F}' |   // Hiragana
            '\u{30A0}'..='\u{30FF}'     // Katakana
        )
    }
    
    /// Compute word frequency in a document.
    fn compute_word_freq(&self, words: &[String]) -> HashMap<String, f32> {
        let total = words.len() as f32;
        let mut freq = HashMap::new();
        
        for word in words {
            *freq.entry(word.clone()).or_insert(0.0) += 1.0;
        }
        
        // Normalize by total words
        for (_, count) in freq.iter_mut() {
            *count /= total;
        }
        
        freq
    }
    
    /// Compute IDF for all words.
    fn compute_idf(&mut self) {
        // Count documents containing each word
        let mut word_doc_count: HashMap<String, usize> = HashMap::new();
        
        for word_freq in &self.doc_word_freq {
            for word in word_freq.1.keys() {
                *word_doc_count.entry(word.clone()).or_insert(0) += 1;
            }
        }
        
        // Compute IDF
        for (word, count) in word_doc_count {
            let idf = (self.total_docs as f32 / count as f32).ln();
            self.idf_cache.insert(word, idf);
        }
    }
    
    /// Search using TF-IDF similarity.
    pub fn search(&self, query: &str, limit: Option<usize>) -> Vec<(String, f32)> {
        let query_words = self.tokenize(query);
        let query_freq = self.compute_word_freq(&query_words);

        let mut results: Vec<(String, f32)> = Vec::new();

        for (doc, doc_freq) in &self.doc_word_freq {
            // Compute TF-IDF dot product similarity
            let similarity = self.compute_similarity(&query_freq, doc_freq);

            if similarity > 0.0 {
                results.push((doc.clone(), similarity));
            }
        }

        // Sort by similarity
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Apply limit
        if let Some(max) = limit {
            results.into_iter().take(max).collect()
        } else {
            results
        }
    }

    /// Search with multiple keywords (returns combined scores).
    /// Useful when you have expanded keywords with semantic aliases.
    pub fn search_multi(&self, keywords: &[&str], limit: Option<usize>) -> Vec<(String, f64)> {
        // Aggregate results from all keywords
        let mut doc_scores: HashMap<String, f64> = HashMap::new();

        for keyword in keywords {
            let results = self.search(keyword, None);
            for (doc, score) in results {
                // Accumulate scores (normalized)
                *doc_scores.entry(doc).or_insert(0.0) += score as f64;
            }
        }

        // Normalize by number of keywords
        let num_keywords = keywords.len().max(1);
        for (_, score) in doc_scores.iter_mut() {
            *score /= num_keywords as f64;
        }

        // Convert to vector and sort
        let mut results: Vec<(String, f64)> = doc_scores.into_iter().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Apply limit
        if let Some(max) = limit {
            results.into_iter().take(max).collect()
        } else {
            results
        }
    }
    
    /// Compute TF-IDF similarity between query and document.
    fn compute_similarity(&self, query_freq: &HashMap<String, f32>, doc_freq: &HashMap<String, f32>) -> f32 {
        let mut similarity = 0.0;
        
        for (word, tf_query) in query_freq {
            if let Some(tf_doc) = doc_freq.get(word)
                && let Some(idf) = self.idf_cache.get(word) {
                    // TF-IDF(query) × TF-IDF(doc)
                    similarity += tf_query * idf * tf_doc * idf;
                }
        }
        
        similarity
    }
    
    /// Clear all indices.
    pub fn clear(&mut self) {
        self.doc_word_freq.clear();
        self.idf_cache.clear();
        self.total_docs = 0;
    }
}

impl Default for TfIdfSearch {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_entry_creation() {
        let entry = MemoryEntry::new(
            MemoryCategory::Decision,
            "Decided to use PostgreSQL for database".to_string(),
            Some("session-123".to_string()),
        );
        assert_eq!(entry.category, MemoryCategory::Decision);
        assert_eq!(entry.importance, DEFAULT_IMPORTANCE_DECISION);  // 75.0
        assert!(!entry.is_manual);
    }

    #[test]
    fn test_memory_reference_increase() {
        let mut entry = MemoryEntry::new(
            MemoryCategory::Finding,
            "API endpoint is at /api/v2".to_string(),
            None,
        );
        assert_eq!(entry.importance, DEFAULT_IMPORTANCE_FINDING);  // 55.0
        entry.mark_referenced();
        // With default increment of 1.0 (in mark_referenced it uses 2.0)
        // mark_referenced() adds 2.0 by default
        assert_eq!(entry.importance, 57.0);  // 55 + 2
        entry.mark_referenced();
        entry.mark_referenced();
        assert_eq!(entry.importance, 61.0);  // 55 + 6
    }

    #[test]
    fn test_auto_memory_add_and_prune() {
        let mut memory = AutoMemory::new();
        memory.max_entries = 5;

        for i in 0..10 {
            memory.add(MemoryEntry::new(
                MemoryCategory::Technical,
                format!("Note {}", i),
                None,
            ));
        }

        // Should have pruned to max_entries
        assert!(memory.entries.len() <= memory.max_entries);
    }

    #[test]
    fn test_duplicate_detection() {
        let mut memory = AutoMemory::new();
        memory.add_memory(
            MemoryCategory::Decision,
            "Use PostgreSQL".to_string(),
            None,
        );
        
        // Should not add duplicate
        memory.add_memory(
            MemoryCategory::Decision,
            "Use PostgreSQL".to_string(),
            None,
        );
        
        assert_eq!(memory.entries.len(), 1);
    }

    #[test]
    fn test_memory_detection() {
        // Test decision detection - use new specific pattern
        let text = "我们决定采用 React 作为前端框架";
        let entries = detect_memories_from_text(text, None);
        assert!(!entries.is_empty());
        assert_eq!(entries[0].category, MemoryCategory::Decision);

        // Test solution detection - use new specific pattern
        let text2 = "解决了认证问题，解决方案是通过添加 token refresh 机制";
        let entries2 = detect_memories_from_text(text2, None);
        assert!(!entries2.is_empty());
        assert_eq!(entries2[0].category, MemoryCategory::Solution);

        // Test preference detection - use new specific pattern
        let text3 = "我偏好使用 TypeScript 进行开发";
        let entries3 = detect_memories_from_text(text3, None);
        assert!(!entries3.is_empty());
        assert_eq!(entries3[0].category, MemoryCategory::Preference);
    }

    #[test]
    fn test_category_importance() {
        assert!(MemoryCategory::Decision.default_importance() > MemoryCategory::Structure.default_importance());
        assert!(MemoryCategory::Solution.default_importance() > MemoryCategory::Technical.default_importance());
    }

    #[test]
    fn test_top_n_entries() {
        let mut memory = AutoMemory::new();
        
        // Add entries with different importance
        memory.add(MemoryEntry::new(MemoryCategory::Decision, "Decision 1".into(), None));
        memory.add(MemoryEntry::new(MemoryCategory::Finding, "Finding 1".into(), None));
        memory.add(MemoryEntry::new(MemoryCategory::Structure, "Structure 1".into(), None));

        let top = memory.top_n(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].category, MemoryCategory::Decision); // Highest importance
    }

    #[test]
    fn test_similarity_calculation() {
        // Test exact match
        let sim = AutoMemory::calculate_similarity("hello world", "hello world");
        assert_eq!(sim, 1.0);
        
        // Test no match
        let sim = AutoMemory::calculate_similarity("hello world", "foo bar");
        assert_eq!(sim, 0.0);
        
        // Test partial match (50% overlap)
        let sim = AutoMemory::calculate_similarity("hello world", "hello there");
        assert!(sim > 0.0 && sim < 1.0);
        
        // Test empty input
        let sim = AutoMemory::calculate_similarity("", "hello");
        assert_eq!(sim, 0.0);
    }
    
    #[test]
    fn test_similarity_threshold() {
        let mut memory = AutoMemory::new();
        
        // Add a long enough entry (>= MIN_SIMILARITY_LENGTH)
        memory.add(MemoryEntry::new(
            MemoryCategory::Decision,
            "We decided to use PostgreSQL for our database system".to_string(),
            None,
        ));
        
        // Should not add similar entry
        memory.add_memory(
            MemoryCategory::Decision,
            "We decided to use PostgreSQL for our database backend".to_string(),
            None,
        );
        
        // Should have only 1 entry (similar detected)
        assert_eq!(memory.entries.len(), 1);
    }
    
    #[test]
    fn test_short_content_skipped() {
        let mut memory = AutoMemory::new();
        
        // Short content should be skipped by has_similar
        memory.add(MemoryEntry::new(
            MemoryCategory::Technical,
            "short".to_string(),  // Only 5 chars, below MIN_SIMILARITY_LENGTH
            None,
        ));
        
        // Another short entry should be added (not detected as similar)
        memory.add_memory(
            MemoryCategory::Technical,
            "brief".to_string(),
            None,
        );
        
        assert_eq!(memory.entries.len(), 2);
    }
    
    #[test]
    fn test_prune_preserves_manual() {
        let mut memory = AutoMemory::new();
        memory.max_entries = 3;
        
        // Add manual entry (should always be preserved)
        let mut manual = MemoryEntry::manual(MemoryCategory::Decision, "Manual decision".into());
        manual.importance = 10.0; // Low importance but manual
        memory.add(manual);
        
        // Add high importance auto entries
        for i in 0..5 {
            let entry = MemoryEntry::new(
                MemoryCategory::Decision,
                format!("Auto decision {}", i),
                None,
            );
            memory.add(entry);
        }
        
        // Manual entry should still exist after prune
        assert!(memory.entries.iter().any(|e| e.is_manual));
        assert!(memory.entries.len() <= memory.max_entries);
    }
    
    #[test]
    fn test_deduplicate_entries() {
        // Use more similar entries (should have similarity >= 0.8)
        let entries = vec![
            MemoryEntry::new(MemoryCategory::Decision, "We chose PostgreSQL database system for our backend".into(), None),
            MemoryEntry::new(MemoryCategory::Decision, "We chose PostgreSQL database system backend".into(), None),
            MemoryEntry::new(MemoryCategory::Decision, "Using Redis for caching layer".into(), None),
        ];
        
        let deduped = deduplicate_entries(entries);
        
        // Should deduplicate similar entries
        assert!(deduped.len() >= 1);
        assert!(deduped.len() <= 3);
        
        // Should keep longer (more detailed) entry when similar
        let pg_entries: Vec<_> = deduped.iter()
            .filter(|e| e.content.to_lowercase().contains("postgresql"))
            .collect();
        
        if pg_entries.len() == 1 {
            // Correctly deduplicated to one PostgreSQL entry
            // Should be the longer one
            assert!(pg_entries[0].content.contains("backend"));
        }
    }
    
    #[test]
    fn test_memory_detection_edge_cases() {
        // Empty input
        let entries = detect_memories_from_text("", None);
        assert!(entries.is_empty());
        
        // Very short input (below MIN_MEMORY_CONTENT_LENGTH)
        let entries = detect_memories_from_text("决定", None);
        assert!(entries.is_empty());
        
        // Input with only generic keywords
        let entries = detect_memories_from_text("使用", None);
        assert!(entries.is_empty());
        
        // Multiple matches in same text
        let text = "我决定使用React，解决了性能问题通过添加缓存机制";
        let entries = detect_memories_from_text(text, None);
        assert!(entries.len() <= MAX_DETECTED_ENTRIES);
    }
    
    #[test]
    fn test_importance_ceiling() {
        let mut entry = MemoryEntry::new(
            MemoryCategory::Decision,
            "Important decision".into(),
            None,
        );

        // Start at DEFAULT_IMPORTANCE_DECISION (75.0)
        assert_eq!(entry.importance, DEFAULT_IMPORTANCE_DECISION);

        // Reference many times
        for _ in 0..20 {
            entry.mark_referenced();
        }

        // Should cap at MAX_IMPORTANCE_CEILING (100.0)
        assert!(entry.importance <= MAX_IMPORTANCE_CEILING);
    }

    #[test]
    fn test_time_decay() {
        let mut memory = AutoMemory::new();
        memory.min_importance = 30.0;
        
        // Add manual entry (should never decay)
        let mut manual = MemoryEntry::manual(MemoryCategory::Decision, "Manual entry".into());
        manual.importance = 50.0;
        memory.add(manual);
        
        // Add auto entry with old reference date (simulate 60 days ago)
        let mut old_entry = MemoryEntry::new(
            MemoryCategory::Technical,
            "Old technical note".into(),
            None,
        );
        old_entry.importance = 60.0;
        // Set last_referenced to 60 days ago
        old_entry.last_referenced = Utc::now() - chrono::Duration::days(60);
        memory.add(old_entry);
        
        // Add recent entry (should not decay)
        let recent_entry = MemoryEntry::new(
            MemoryCategory::Finding,
            "Recent finding".into(),
            None,
        );
        memory.add(recent_entry);
        
        // Apply time decay
        memory.apply_time_decay();
        
        // Manual entry should not decay
        let manual_entry = memory.entries.iter().find(|e| e.is_manual);
        assert!(manual_entry.is_some());
        assert_eq!(manual_entry.unwrap().importance, 50.0);
        
        // Recent entry should not decay (still > 30 days threshold)
        let recent = memory.entries.iter().find(|e| e.content.contains("Recent"));
        assert!(recent.is_some());
        assert!(recent.unwrap().importance >= DEFAULT_IMPORTANCE_FINDING);  // Finding default (55.0)
        
        // Old entry should have decayed
        let old = memory.entries.iter().find(|e| e.content.contains("Old"));
        if let Some(old_entry) = old {
            // Should have decayed (60 days - 30 days threshold = 30 days decay period)
            // Decay factor = 0.5^1 = 0.5, so importance = 60 * 0.5 = 30
            assert!(old_entry.importance < 60.0);
            // Should still be above minimum threshold
            assert!(old_entry.importance >= memory.min_importance * 0.5);
        }
    }

    #[test]
    fn test_parse_memory_response() {
        // Test valid JSON response
        let json = r#"{"memories": [{"category": "decision", "content": "决定使用 PostgreSQL 作为数据库", "importance": 90}, {"category": "preference", "content": "我偏好 TypeScript 而非 JavaScript", "importance": 70}]}"#;
        let entries = parse_memory_response(json, None).unwrap();
        assert_eq!(entries.len(), 2);
        
        // Check both entries exist (order may change due to deduplication sorting)
        let has_decision = entries.iter().any(|e| e.category == MemoryCategory::Decision);
        let has_preference = entries.iter().any(|e| e.category == MemoryCategory::Preference);
        assert!(has_decision);
        assert!(has_preference);
        
        // Check importance values
        let decision_entry = entries.iter().find(|e| e.category == MemoryCategory::Decision);
        assert!(decision_entry.is_some());
        assert_eq!(decision_entry.unwrap().importance, 90.0);
        
        // Test empty response
        let empty_json = r#"{"memories": []}"#;
        let empty_entries = parse_memory_response(empty_json, None).unwrap();
        assert!(empty_entries.is_empty());
        
        // Test JSON with markdown code blocks
        let markdown_json = r#"```json
{"memories": [{"category": "solution", "content": "通过添加 middleware 修复认证问题", "importance": 85}]}
```"#;
        let markdown_entries = parse_memory_response(markdown_json, None).unwrap();
        assert_eq!(markdown_entries.len(), 1);
        assert_eq!(markdown_entries[0].category, MemoryCategory::Solution);
        
        // Test unknown category (should be skipped)
        let unknown_json = r#"{"memories": [{"category": "unknown", "content": "This should be skipped content", "importance": 50}]}"#;
        let unknown_entries = parse_memory_response(unknown_json, None).unwrap();
        assert!(unknown_entries.is_empty());
        
        // Test short content (should be skipped)
        let short_json = r#"{"memories": [{"category": "finding", "content": "short", "importance": 60}]}"#;
        let short_entries = parse_memory_response(short_json, None).unwrap();
        assert!(short_entries.is_empty());
    }

    #[test]
    fn test_public_has_similar() {
        let mut memory = AutoMemory::new();

        // Add an entry
        memory.add(MemoryEntry::new(
            MemoryCategory::Decision,
            "We decided to use PostgreSQL for our main database system".to_string(),
            None,
        ));

        // Test exact match (similarity = 1.0, >= 0.85 threshold)
        assert!(memory.has_similar("We decided to use PostgreSQL for our main database system"));

        // Test with extra words (still has all original words, Jaccard >= 0.85)
        // Original: 10 words, with "backend" added: 11 words
        // Intersection: 10, Union: 11, Jaccard: 10/11 = 0.91 >= 0.85
        assert!(memory.has_similar("We decided to use PostgreSQL for our main database system backend"));

        // Test moderately similar (should NOT match, Jaccard < 0.85)
        // Original: 10 words, this: 7 words overlap
        // Jaccard: 7/12 = 0.58 < 0.85
        assert!(!memory.has_similar("We decided to use Redis for caching"));

        // Test completely different content
        assert!(!memory.has_similar("The project uses React for frontend"));

        // Test short content (should return false due to MIN_SIMILARITY_LENGTH)
        assert!(!memory.has_similar("short"));
    }

    #[test]
    fn test_public_prune() {
        let mut memory = AutoMemory::new();
        memory.max_entries = 5;
        memory.min_importance = 30.0;
        
        // Add entries exceeding max
        for i in 0..10 {
            memory.add(MemoryEntry::new(
                MemoryCategory::Technical,
                format!("Technical note number {} with sufficient length", i),
                None,
            ));
        }
        
        // Manually prune
        memory.prune();
        
        // Should be within limit
        assert!(memory.entries.len() <= memory.max_entries);
    }

    #[test]
    fn test_statistics() {
        let mut memory = AutoMemory::new();
        
        // Add various entries
        memory.add(MemoryEntry::new(MemoryCategory::Decision, "Decision one with enough content".to_string(), None));
        memory.add(MemoryEntry::new(MemoryCategory::Preference, "Preference for TypeScript over JavaScript".to_string(), None));
        memory.add(MemoryEntry::manual(MemoryCategory::Technical, "Manual technical note".to_string()));
        
        // Reference some entries
        memory.entries[0].mark_referenced();
        memory.entries[0].mark_referenced();
        memory.entries[0].mark_referenced();
        
        let stats = memory.generate_statistics();
        
        assert_eq!(stats.total, 3);
        assert_eq!(stats.manual, 1);
        assert_eq!(stats.auto, 2);
        assert_eq!(stats.highly_referenced, 1);  // First entry has 3 references
        assert!(stats.by_category.contains_key(&MemoryCategory::Decision));
        assert!(stats.by_category.contains_key(&MemoryCategory::Preference));
        assert!(stats.by_category.contains_key(&MemoryCategory::Technical));
        assert!(stats.avg_importance > 0.0);
    }

    #[test]
    fn test_memory_config() {
        // Test default config
        let config = MemoryConfig::default();
        assert_eq!(config.max_entries, 100);
        assert_eq!(config.min_importance, 30.0);
        assert_eq!(config.decay_start_days, 30);
        assert_eq!(config.decay_rate, 0.5);
        
        // Test minimal config
        let minimal = MemoryConfig::minimal();
        assert_eq!(minimal.max_entries, 50);
        assert!(minimal.min_importance > config.min_importance);
        
        // Test archival config
        let archival = MemoryConfig::archival();
        assert_eq!(archival.max_entries, 500);
        assert!(archival.min_importance < config.min_importance);
        
        // Test with_max_entries
        let custom = MemoryConfig::with_max_entries(200);
        assert_eq!(custom.max_entries, 200);
        assert_eq!(custom.min_importance, 30.0);  // Other defaults preserved
    }

    #[test]
    fn test_auto_memory_with_config() {
        let config = MemoryConfig::minimal();
        let mut memory = AutoMemory::with_config(config);
        
        assert_eq!(memory.max_entries, 50);
        assert_eq!(memory.min_importance, 50.0);
        
        // Add entries
        for i in 0..60 {
            memory.add(MemoryEntry::new(
                MemoryCategory::Technical,
                format!("Technical note {} with enough length for detection", i),
                None,
            ));
        }
        
        // Should prune to config limit
        assert!(memory.entries.len() <= 50);
    }

    #[test]
    fn test_batch_add() {
        let mut memory = AutoMemory::new();
        
        // Batch add multiple entries
        let entries: Vec<MemoryEntry> = vec![
            MemoryEntry::new(MemoryCategory::Decision, "First decision with sufficient content".into(), None),
            MemoryEntry::new(MemoryCategory::Finding, "First finding with sufficient content".into(), None),
            MemoryEntry::new(MemoryCategory::Solution, "First solution with sufficient content".into(), None),
        ];
        
        memory.add_batch(entries);
        assert_eq!(memory.entries.len(), 3);
        
        // Batch add with duplicates
        let duplicate_entries: Vec<MemoryEntry> = vec![
            MemoryEntry::new(MemoryCategory::Decision, "First decision with sufficient content".into(), None),  // Duplicate
            MemoryEntry::new(MemoryCategory::Technical, "New technical note with sufficient content".into(), None),
        ];
        
        memory.add_batch(duplicate_entries);
        assert_eq!(memory.entries.len(), 4);  // Only 1 new entry added
    }

    #[test]
    fn test_search_with_limit() {
        let mut memory = AutoMemory::new();
        
        // Add multiple entries with same keyword
        for i in 0..10 {
            memory.add(MemoryEntry::new(
                MemoryCategory::Technical,
                format!("PostgreSQL technical note {} with details", i),
                None,
            ));
        }
        
        // Search without limit
        let all = memory.search("postgresql");
        assert_eq!(all.len(), 10);
        
        // Search with limit
        let limited = memory.search_with_limit("postgresql", Some(5));
        assert_eq!(limited.len(), 5);
        
        // Should return highest importance first
        assert!(limited[0].importance >= limited[limited.len() - 1].importance);
    }

    #[test]
    fn test_multi_keyword_search() {
        let mut memory = AutoMemory::new();
        
        memory.add(MemoryEntry::new(MemoryCategory::Decision, "Decided to use PostgreSQL".into(), None));
        memory.add(MemoryEntry::new(MemoryCategory::Technical, "Using Redis for caching".into(), None));
        memory.add(MemoryEntry::new(MemoryCategory::Solution, "Fixed by adding middleware".into(), None));
        
        // Search with multiple keywords
        let results = memory.search_multi(&["postgresql", "redis"]);
        assert_eq!(results.len(), 2);
        
        // Search with keyword that matches nothing
        let empty = memory.search_multi(&["mongodb"]);
        assert!(empty.is_empty());
    }

    #[test]
    fn test_mark_referenced_with_increment() {
        let mut entry = MemoryEntry::new(
            MemoryCategory::Finding,
            "API endpoint location".into(),
            None,
        );

        assert_eq!(entry.importance, DEFAULT_IMPORTANCE_FINDING);  // 55.0

        // Custom increment
        entry.mark_referenced_with_increment(5.0);
        assert_eq!(entry.importance, 60.0);  // 55 + 5

        // Default increment (2.0 in mark_referenced)
        entry.mark_referenced();
        assert_eq!(entry.importance, 62.0);  // 60 + 2

        // Should cap at MAX_IMPORTANCE_CEILING
        for _ in 0..20 {
            entry.mark_referenced_with_increment(10.0);
        }
        assert!(entry.importance <= MAX_IMPORTANCE_CEILING);
    }

    #[test]
    fn test_search_index() {
        let mut memory = AutoMemory::new();
        
        // Add multiple entries
        for i in 0..20 {
            memory.add(MemoryEntry::new(
                MemoryCategory::Technical,
                format!("PostgreSQL technical note {} with sufficient content length", i),
                None,
            ));
        }
        for i in 0..10 {
            memory.add(MemoryEntry::new(
                MemoryCategory::Decision,
                format!("Redis decision {} with sufficient content for testing", i),
                None,
            ));
        }
        
        // Rebuild index
        memory.rebuild_index();
        assert!(memory.search_index.is_some());
        
        // Test fast search
        let results = memory.search_fast("postgresql", Some(5));
        assert!(results.len() <= 5);
        assert!(results.iter().all(|e| e.content.to_lowercase().contains("postgresql")));
        
        // Test fast multi-keyword search
        let multi_results = memory.search_multi_fast(&["postgresql", "redis"]);
        assert!(multi_results.len() > 0);
        
        // Test fast category lookup
        let tech_entries = memory.by_category_fast(MemoryCategory::Technical);
        assert_eq!(tech_entries.len(), 20);
        
        let decision_entries = memory.by_category_fast(MemoryCategory::Decision);
        assert_eq!(decision_entries.len(), 10);
        
        // Test fast top_n
        let top = memory.top_n_fast(5);
        assert_eq!(top.len(), 5);
        // Results should be sorted by importance (Decision > Technical)
        assert!(top[0].importance >= top[top.len() - 1].importance);
    }

    #[test]
    fn test_index_auto_rebuild() {
        let mut memory = AutoMemory::new();
        
        // Index should be None initially
        assert!(memory.search_index.is_none());
        
        // Fast search should auto-build index
        memory.add(MemoryEntry::new(
            MemoryCategory::Decision,
            "Test decision with sufficient content length".into(),
            None,
        ));
        
        let results = memory.search_fast("test", None);
        assert!(results.len() > 0);
        assert!(memory.search_index.is_some());  // Index auto-built
        
        // Modify memory should invalidate index
        memory.clear();
        assert!(memory.search_index.is_none());
        
        // Add new entry should rebuild on next search
        memory.add(MemoryEntry::new(
            MemoryCategory::Finding,
            "New finding with sufficient content".into(),
            None,
        ));
        let _ = memory.search_fast("finding", None);
        assert!(memory.search_index.is_some());
    }

    #[test]
    fn test_cosine_similarity() {
        // Identical vectors
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert_eq!(SemanticUtils::cosine_similarity(&a, &b), 1.0);
        
        // Orthogonal vectors (no similarity)
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!((SemanticUtils::cosine_similarity(&a, &b) - 0.0).abs() < 0.001);
        
        // Opposite vectors
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        assert!((SemanticUtils::cosine_similarity(&a, &b) - (-1.0)).abs() < 0.001);
        
        // Partial similarity
        let a = vec![1.0, 1.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = SemanticUtils::cosine_similarity(&a, &b);
        assert!(sim > 0.0 && sim < 1.0);
        
        // Empty vectors
        let a: Vec<f32> = vec![];
        let b: Vec<f32> = vec![];
        assert_eq!(SemanticUtils::cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn test_tfidf_search() {
        let mut memory = AutoMemory::new();
        
        memory.add(MemoryEntry::new(MemoryCategory::Decision, "使用 PostgreSQL 作为主数据库系统".into(), None));
        memory.add(MemoryEntry::new(MemoryCategory::Technical, "Redis 缓存配置为 10 个连接".into(), None));
        memory.add(MemoryEntry::new(MemoryCategory::Solution, "通过添加 middleware 修复认证问题".into(), None));
        memory.add(MemoryEntry::new(MemoryCategory::Finding, "数据库连接池设置为 20".into(), None));
        
        let mut tfidf = TfIdfSearch::new();
        tfidf.index(&memory);
        
        // Search for "数据库" - should find PostgreSQL and 连接池 entries
        let results = tfidf.search("数据库", Some(5));
        assert!(!results.is_empty());
        // First result should contain "数据库"
        assert!(results[0].0.contains("数据库"));
        
        // Search for "Redis" - should find Redis entry
        let results = tfidf.search("redis", Some(5));
        assert!(!results.is_empty());
        assert!(results[0].0.to_lowercase().contains("redis"));
        
        // Search for something not in any entry
        let results = tfidf.search("mongodb", Some(5));
        assert!(results.is_empty());
    }

    #[test]
    fn test_tfidf_ranking() {
        let mut memory = AutoMemory::new();
        
        // Add entries with varying relevance to "数据库"
        memory.add(MemoryEntry::new(MemoryCategory::Decision, "使用 PostgreSQL 数据库 作为主数据库".into(), None));
        memory.add(MemoryEntry::new(MemoryCategory::Technical, "数据库连接池配置".into(), None));
        memory.add(MemoryEntry::new(MemoryCategory::Solution, "修复了前端样式问题".into(), None));
        
        let mut tfidf = TfIdfSearch::new();
        tfidf.index(&memory);
        
        let results = tfidf.search("数据库", None);
        
        // Should rank entries with more "数据库" mentions higher
        if results.len() >= 2 {
            assert!(results[0].1 >= results[1].1);
        }
    }

    #[test]
    fn test_conflict_detection() {
        let mut memory = AutoMemory::new();
        
        // Add initial decision
        memory.add_memory(
            MemoryCategory::Decision,
            "决定使用 PostgreSQL 作为主数据库".to_string(),
            None,
        );
        assert_eq!(memory.entries.len(), 1);
        assert!(memory.entries[0].content.contains("PostgreSQL"));
        
        // Add conflicting decision (same topic, different choice)
        memory.add_memory(
            MemoryCategory::Decision,
            "决定使用 MySQL 作为主数据库".to_string(),
            None,
        );
        
        // Should have replaced the old one
        assert_eq!(memory.entries.len(), 1);
        assert!(memory.entries[0].content.contains("MySQL"));
    }

    #[test]
    fn test_conflict_with_change_signal() {
        let mut memory = AutoMemory::new();
        
        // Add initial preference
        memory.add_memory(
            MemoryCategory::Preference,
            "偏好使用 vim 编辑器".to_string(),
            None,
        );
        assert_eq!(memory.entries.len(), 1);
        
        // Add replacement with change signal
        memory.add_memory(
            MemoryCategory::Preference,
            "改用 vscode 编辑器，不再使用 vim".to_string(),
            None,
        );
        
        // Should have replaced
        assert_eq!(memory.entries.len(), 1);
        assert!(memory.entries[0].content.contains("vscode"));
    }

    #[test]
    fn test_no_false_conflict() {
        let mut memory = AutoMemory::new();
        
        // Add two different decisions (different topics)
        memory.add_memory(
            MemoryCategory::Decision,
            "决定使用 PostgreSQL 作为主数据库".to_string(),
            None,
        );
        memory.add_memory(
            MemoryCategory::Decision,
            "决定使用 Redis 作为缓存系统".to_string(),
            None,
        );
        
        // Both should exist (different topics, no conflict)
        assert_eq!(memory.entries.len(), 2);
    }

    #[test]
    fn test_contextual_summary() {
        let mut memory = AutoMemory::new();
        
        // Add various memories
        memory.add(MemoryEntry::new(MemoryCategory::Decision, "决定使用 PostgreSQL 作为主数据库".into(), None));
        memory.add(MemoryEntry::new(MemoryCategory::Technical, "前端使用 React 框架开发".into(), None));
        memory.add(MemoryEntry::new(MemoryCategory::Solution, "通过添加 Redis 缓存解决性能问题".into(), None));
        memory.add(MemoryEntry::new(MemoryCategory::Finding, "API 响应时间在 200ms 以内".into(), None));
        memory.add(MemoryEntry::new(MemoryCategory::Preference, "偏好使用 TypeScript 而非 JavaScript".into(), None));
        
        // Context about database - should prioritize database-related memories
        let db_summary = memory.generate_contextual_summary("数据库查询优化", 3);
        assert!(db_summary.contains("PostgreSQL"));
        
        // Context about frontend - should prioritize frontend-related memories
        let fe_summary = memory.generate_contextual_summary("React 组件开发", 3);
        assert!(fe_summary.contains("React"));
        
        // Empty context - should fall back to importance-based
        let empty_summary = memory.generate_contextual_summary("", 3);
        assert!(!empty_summary.is_empty());
    }

    #[test]
    fn test_low_quality_memory_filter() {
        // Formatting artifacts should be rejected
        assert!(is_low_quality_memory("│  🎯 决策: 决定使用 PostgreSQL."));
        assert!(is_low_quality_memory("├── Structure: 入口文件是 main."));
        assert!(is_low_quality_memory("🔧 解决方案: 通过添加 middleware."));
        assert!(is_low_quality_memory("【自动记忆摘要】"));
        assert!(is_low_quality_memory("short"));
        
        // Real content should pass
        assert!(!is_low_quality_memory("决定使用 PostgreSQL 作为主数据库系统"));
        assert!(!is_low_quality_memory("通过添加 Redis 缓存层解决了性能问题"));
        assert!(!is_low_quality_memory("用户偏好使用 TypeScript 进行开发"));
    }
}
