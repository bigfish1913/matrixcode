//! Focus keywords registry for managing dynamic keywords.
//!
//! This module provides a registry for storing, retrieving, and learning
//! focus-related keywords. It replaces hardcoded keywords in FocusTrackerConfig
//! with a dynamic, learnable system.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::constants::MATRIX_DIR;

// ============================================================================
// Keyword Types
// ============================================================================

/// Types of keywords used in focus tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeywordCategory {
    /// Keywords that indicate topic transition/change
    Transition,
    /// Keywords that indicate a question
    Question,
    /// Keywords that indicate a task/request
    Task,
    /// Tech/domain keywords for topic extraction
    Tech,
}

impl std::fmt::Display for KeywordCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeywordCategory::Transition => write!(f, "transition"),
            KeywordCategory::Question => write!(f, "question"),
            KeywordCategory::Task => write!(f, "task"),
            KeywordCategory::Tech => write!(f, "tech"),
        }
    }
}

// ============================================================================
// Keyword Entry
// ============================================================================

/// A single keyword entry with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeywordEntry {
    /// The keyword text (lowercase)
    pub keyword: String,
    /// Category of the keyword
    pub category: KeywordCategory,
    /// How many times this keyword has been used/matched
    pub frequency: u32,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,
    /// Source of the keyword
    pub source: KeywordSource,
    /// When the keyword was first added
    pub created_at: DateTime<Utc>,
    /// When the keyword was last used
    pub last_used: DateTime<Utc>,
    /// Whether the keyword is currently active
    pub is_active: bool,
}

impl KeywordEntry {
    /// Create a new keyword entry.
    pub fn new(keyword: &str, category: KeywordCategory, source: KeywordSource) -> Self {
        let now = Utc::now();
        Self {
            keyword: keyword.to_lowercase(),
            category,
            frequency: 1,
            confidence: 0.5,
            source,
            created_at: now,
            last_used: now,
            is_active: true,
        }
    }

    /// Create a preset keyword (built-in default).
    pub fn preset(keyword: &str, category: KeywordCategory) -> Self {
        Self {
            keyword: keyword.to_lowercase(),
            category,
            frequency: 0,
            confidence: 1.0,
            source: KeywordSource::Preset,
            created_at: Utc::now(),
            last_used: Utc::now(),
            is_active: true,
        }
    }

    /// Create a manually added keyword.
    pub fn manual(keyword: &str, category: KeywordCategory) -> Self {
        Self::new(keyword, category, KeywordSource::Manual)
    }

    /// Create a keyword learned from conversation.
    pub fn learned(keyword: &str, category: KeywordCategory, session_id: &str) -> Self {
        Self {
            keyword: keyword.to_lowercase(),
            category,
            frequency: 1,
            confidence: 0.3,
            source: KeywordSource::Learned {
                session_id: session_id.to_string(),
            },
            created_at: Utc::now(),
            last_used: Utc::now(),
            is_active: true,
        }
    }

    /// Mark this keyword as used (increment frequency and update timestamp).
    pub fn mark_used(&mut self) {
        self.frequency = self.frequency.saturating_add(1);
        self.last_used = Utc::now();
    }

    /// Deactivate this keyword.
    pub fn deactivate(&mut self) {
        self.is_active = false;
    }

    /// Activate this keyword.
    pub fn activate(&mut self) {
        self.is_active = true;
    }
}

// ============================================================================
// Keyword Source
// ============================================================================

/// Source of a keyword entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum KeywordSource {
    /// Built-in preset keyword
    Preset,
    /// Manually added by user
    Manual,
    /// Learned from conversation
    Learned { session_id: String },
}

impl KeywordSource {
    /// Check if this is a preset keyword.
    pub fn is_preset(&self) -> bool {
        matches!(self, KeywordSource::Preset)
    }

    /// Check if this is a manually added keyword.
    pub fn is_manual(&self) -> bool {
        matches!(self, KeywordSource::Manual)
    }

    /// Check if this is a learned keyword.
    pub fn is_learned(&self) -> bool {
        matches!(self, KeywordSource::Learned { .. })
    }
}

// ============================================================================
// Registry Configuration
// ============================================================================

/// Configuration for the focus keywords registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusKeywordsRegistryConfig {
    /// Maximum number of keywords per category.
    pub max_keywords_per_category: usize,
    /// Minimum confidence threshold for learned keywords to be active.
    pub min_confidence_threshold: f32,
    /// Minimum frequency for a keyword to be considered established.
    pub min_frequency: u32,
    /// Whether to auto-learn keywords from conversations.
    pub auto_learn: bool,
    /// Days before unused keywords are deactivated.
    pub inactive_after_days: i64,
}

impl Default for FocusKeywordsRegistryConfig {
    fn default() -> Self {
        Self {
            max_keywords_per_category: 200,
            min_confidence_threshold: 0.3,
            min_frequency: 2,
            auto_learn: true,
            inactive_after_days: 90,
        }
    }
}

impl FocusKeywordsRegistryConfig {
    /// Create a config with custom max keywords.
    pub fn with_max_keywords(max: usize) -> Self {
        Self {
            max_keywords_per_category: max,
            ..Self::default()
        }
    }

    /// Create a minimal config for low-memory environments.
    pub fn minimal() -> Self {
        Self {
            max_keywords_per_category: 100,
            min_confidence_threshold: 0.5,
            min_frequency: 3,
            auto_learn: true,
            inactive_after_days: 60,
        }
    }
}

// ============================================================================
// Focus Keywords Registry
// ============================================================================

/// Helper struct for deserialization (without skipped fields).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FocusKeywordsRegistryData {
    keywords: Vec<KeywordEntry>,
    #[serde(default)]
    config: FocusKeywordsRegistryConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_preset_load: Option<DateTime<Utc>>,
}

/// Central registry for focus tracking keywords.
///
/// Manages keywords organized by category, supports preset loading,
/// keyword learning, and persistence.
#[derive(Debug, Clone)]
pub struct FocusKeywordsRegistry {
    /// All keywords indexed by category + keyword.
    keywords: Vec<KeywordEntry>,
    /// Configuration.
    config: FocusKeywordsRegistryConfig,
    /// Index for fast category-based lookup.
    category_index: HashMap<KeywordCategory, Vec<usize>>,
    /// Set for fast keyword existence check.
    keyword_set: HashSet<(KeywordCategory, String)>,
    /// Last time presets were loaded.
    last_preset_load: Option<DateTime<Utc>>,
}

// Implement custom Serialize to use the helper struct
impl Serialize for FocusKeywordsRegistry {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let data = FocusKeywordsRegistryData {
            keywords: self.keywords.clone(),
            config: self.config.clone(),
            last_preset_load: self.last_preset_load,
        };
        data.serialize(serializer)
    }
}

// Implement custom Deserialize to rebuild indexes
impl<'de> Deserialize<'de> for FocusKeywordsRegistry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data = FocusKeywordsRegistryData::deserialize(deserializer)?;
        let mut registry = Self {
            keywords: data.keywords,
            config: data.config,
            category_index: HashMap::new(),
            keyword_set: HashSet::new(),
            last_preset_load: data.last_preset_load,
        };
        registry.rebuild_index();
        Ok(registry)
    }
}

impl Default for FocusKeywordsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FocusKeywordsRegistry {
    /// Create a new empty registry with presets loaded.
    pub fn new() -> Self {
        let mut registry = Self {
            keywords: Vec::new(),
            config: FocusKeywordsRegistryConfig::default(),
            category_index: HashMap::new(),
            keyword_set: HashSet::new(),
            last_preset_load: None,
        };
        registry.load_presets();
        registry
    }

    /// Create a registry with custom configuration.
    pub fn with_config(config: FocusKeywordsRegistryConfig) -> Self {
        let mut registry = Self {
            keywords: Vec::new(),
            config,
            category_index: HashMap::new(),
            keyword_set: HashSet::new(),
            last_preset_load: None,
        };
        registry.load_presets();
        registry
    }

    // ========================================================================
    // Preset Keywords
    // ========================================================================

    /// Get preset keywords for a category.
    fn get_preset_keywords(category: KeywordCategory) -> Vec<&'static str> {
        match category {
            KeywordCategory::Transition => vec![
                // English
                "however", "but", "switching", "moving on", "another question",
                "new topic", "different", "instead", "actually", "wait",
                // Chinese
                "转换", "切换", "换个话题", "等等", "不对",
            ],
            KeywordCategory::Question => vec![
                // English
                "how", "what", "why", "when", "where", "which",
                "can you", "could you", "would you", "please", "help",
                // Chinese
                "如何", "什么", "为什么", "怎么", "请问", "帮我", "能不能",
            ],
            KeywordCategory::Task => vec![
                // English
                "implement", "create", "fix", "update", "refactor",
                // Chinese
                "实现", "创建", "修复", "更新", "重构", "编写", "添加", "删除",
            ],
            KeywordCategory::Tech => vec![
                // English
                "rust", "python", "javascript", "react", "api", "database",
                "function", "class", "module", "error", "bug", "test",
                "performance", "optimization", "compression", "security", "architecture",
                // Chinese
                "压缩", "优化", "性能", "安全", "架构", "代码",
            ],
        }
    }

    /// Load preset keywords (built-in defaults).
    ///
    /// Presets are only loaded once; subsequent calls are no-ops
    /// unless force is true.
    pub fn load_presets(&mut self) {
        self.load_presets_force(false);
    }

    /// Load presets with option to force reload.
    fn load_presets_force(&mut self, force: bool) {
        if !force && self.last_preset_load.is_some() {
            return;
        }

        for category in [
            KeywordCategory::Transition,
            KeywordCategory::Question,
            KeywordCategory::Task,
            KeywordCategory::Tech,
        ] {
            for keyword in Self::get_preset_keywords(category) {
                self.add_keyword_internal(KeywordEntry::preset(keyword, category));
            }
        }

        self.last_preset_load = Some(Utc::now());
        self.rebuild_index();
    }

    /// Force reload presets (clears existing presets first).
    pub fn reload_presets(&mut self) {
        // Remove existing presets
        self.keywords.retain(|k| !k.source.is_preset());
        self.last_preset_load = None;
        self.rebuild_index();
        self.load_presets_force(true);
    }

    // ========================================================================
    // Index Management
    // ========================================================================

    /// Rebuild the category index and keyword set after modifications.
    fn rebuild_index(&mut self) {
        self.category_index.clear();
        self.keyword_set.clear();

        for (idx, entry) in self.keywords.iter().enumerate() {
            self.category_index
                .entry(entry.category)
                .or_default()
                .push(idx);
            self.keyword_set
                .insert((entry.category, entry.keyword.clone()));
        }
    }

    /// Add a keyword internally (without rebuilding index).
    fn add_keyword_internal(&mut self, entry: KeywordEntry) {
        // Check for duplicate
        let key = (entry.category, entry.keyword.clone());
        if self.keyword_set.contains(&key) {
            return;
        }

        // Check capacity per category
        let category_count = self.keywords.iter()
            .filter(|k| k.category == entry.category)
            .count();

        if category_count >= self.config.max_keywords_per_category {
            // Remove lowest frequency keyword of same category
            if let Some(idx) = self.keywords.iter()
                .enumerate()
                .filter(|(_, k)| k.category == entry.category)
                .min_by_key(|(_, k)| (k.frequency, (k.confidence * 100.0) as u32))
                .map(|(i, _)| i)
            {
                self.keywords.remove(idx);
            }
        }

        self.keywords.push(entry);
    }

    // ========================================================================
    // Public API
    // ========================================================================

    /// Add a new keyword to the registry.
    pub fn add_keyword(&mut self, entry: KeywordEntry) {
        self.add_keyword_internal(entry);
        self.rebuild_index();
    }

    /// Add multiple keywords at once.
    pub fn add_keywords(&mut self, entries: Vec<KeywordEntry>) {
        for entry in entries {
            self.add_keyword_internal(entry);
        }
        self.rebuild_index();
    }

    /// Add a simple keyword (shorthand for manual keyword).
    pub fn add(&mut self, keyword: &str, category: KeywordCategory) {
        self.add_keyword(KeywordEntry::manual(keyword, category));
    }

    /// Learn a keyword from conversation.
    pub fn learn(&mut self, keyword: &str, category: KeywordCategory, session_id: &str) {
        let key = (category, keyword.to_lowercase());
        if let Some(idx) = self.keywords.iter().position(|k| {
            k.category == category && k.keyword == keyword.to_lowercase()
        }) {
            // Existing keyword - increment frequency
            self.keywords[idx].mark_used();
        } else if self.config.auto_learn {
            // New keyword
            self.add_keyword(KeywordEntry::learned(keyword, category, session_id));
        }
    }

    /// Learn multiple keywords from conversation.
    pub fn learn_keywords(&mut self, keywords: &[(&str, KeywordCategory)], session_id: &str) {
        for (keyword, category) in keywords {
            self.learn(keyword, *category, session_id);
        }
    }

    /// Get all active keywords for a category.
    pub fn get_keywords(&self, category: KeywordCategory) -> Vec<String> {
        self.keywords.iter()
            .filter(|k| k.category == category && k.is_active)
            .map(|k| k.keyword.clone())
            .collect()
    }

    /// Get all keywords for a category (including inactive).
    pub fn get_all_keywords(&self, category: KeywordCategory) -> Vec<String> {
        self.keywords.iter()
            .filter(|k| k.category == category)
            .map(|k| k.keyword.clone())
            .collect()
    }

    /// Check if a keyword exists in a category.
    pub fn contains(&self, category: KeywordCategory, keyword: &str) -> bool {
        let lower = keyword.to_lowercase();
        self.keyword_set.contains(&(category, lower))
    }

    /// Match a text against keywords in a category.
    ///
    /// Returns true if any keyword in the category is found in the text.
    pub fn matches(&self, category: KeywordCategory, text: &str) -> bool {
        let lower = text.to_lowercase();
        self.keywords.iter()
            .filter(|k| k.category == category && k.is_active)
            .any(|k| lower.contains(&k.keyword))
    }

    /// Find all matching keywords in a text for a category.
    pub fn find_matches(&self, category: KeywordCategory, text: &str) -> Vec<String> {
        let lower = text.to_lowercase();
        self.keywords.iter()
            .filter(|k| k.category == category && k.is_active && lower.contains(&k.keyword))
            .map(|k| k.keyword.clone())
            .collect()
    }

    /// Get keyword entry by keyword and category.
    pub fn get_keyword(&self, category: KeywordCategory, keyword: &str) -> Option<&KeywordEntry> {
        let lower = keyword.to_lowercase();
        self.keywords.iter()
            .find(|k| k.category == category && k.keyword == lower)
    }

    /// Get mutable keyword entry by keyword and category.
    pub fn get_keyword_mut(&mut self, category: KeywordCategory, keyword: &str) -> Option<&mut KeywordEntry> {
        let lower = keyword.to_lowercase();
        self.keywords.iter_mut()
            .find(|k| k.category == category && k.keyword == lower)
    }

    /// Deactivate a keyword.
    pub fn deactivate(&mut self, category: KeywordCategory, keyword: &str) -> bool {
        if let Some(entry) = self.get_keyword_mut(category, keyword) {
            entry.deactivate();
            true
        } else {
            false
        }
    }

    /// Activate a keyword.
    pub fn activate(&mut self, category: KeywordCategory, keyword: &str) -> bool {
        if let Some(entry) = self.get_keyword_mut(category, keyword) {
            entry.activate();
            true
        } else {
            false
        }
    }

    /// Remove a keyword completely.
    pub fn remove(&mut self, category: KeywordCategory, keyword: &str) -> bool {
        let lower = keyword.to_lowercase();
        let len_before = self.keywords.len();
        self.keywords.retain(|k| !(k.category == category && k.keyword == lower));
        if self.keywords.len() < len_before {
            self.rebuild_index();
            true
        } else {
            false
        }
    }

    /// Get total keyword count.
    pub fn len(&self) -> usize {
        self.keywords.len()
    }

    /// Check if registry is empty.
    pub fn is_empty(&self) -> bool {
        self.keywords.is_empty()
    }

    /// Get keyword count by category.
    pub fn count_by_category(&self, category: KeywordCategory) -> usize {
        self.keywords.iter().filter(|k| k.category == category).count()
    }

    /// Get active keyword count by category.
    pub fn active_count_by_category(&self, category: KeywordCategory) -> usize {
        self.keywords.iter()
            .filter(|k| k.category == category && k.is_active)
            .count()
    }

    /// Get all keyword entries (for serialization).
    pub fn all_keywords(&self) -> &[KeywordEntry] {
        &self.keywords
    }

    /// Prune inactive keywords below confidence/frequency thresholds.
    pub fn prune(&mut self) {
        let now = Utc::now();
        let threshold_days = self.config.inactive_after_days;

        self.keywords.retain(|k| {
            // Keep active keywords
            if k.is_active {
                return true;
            }
            // Keep presets
            if k.source.is_preset() {
                return true;
            }
            // Keep manually added keywords
            if k.source.is_manual() {
                return true;
            }
            // Keep if above frequency/confidence thresholds
            if k.frequency >= self.config.min_frequency
                && k.confidence >= self.config.min_confidence_threshold
            {
                return true;
            }
            // Remove if too old
            let age = (now - k.last_used).num_days();
            age < threshold_days
        });

        self.rebuild_index();
    }

    /// Get statistics about the registry.
    pub fn stats(&self) -> FocusKeywordsStats {
        let total = self.keywords.len();
        let active = self.keywords.iter().filter(|k| k.is_active).count();
        let transition_count = self.count_by_category(KeywordCategory::Transition);
        let question_count = self.count_by_category(KeywordCategory::Question);
        let task_count = self.count_by_category(KeywordCategory::Task);
        let tech_count = self.count_by_category(KeywordCategory::Tech);
        let presets = self.keywords.iter().filter(|k| k.source.is_preset()).count();
        let manual = self.keywords.iter().filter(|k| k.source.is_manual()).count();
        let learned = total - presets - manual;

        FocusKeywordsStats {
            total,
            active,
            inactive: total - active,
            transition_count,
            question_count,
            task_count,
            tech_count,
            presets,
            manual,
            learned,
        }
    }

    // ========================================================================
    // File Storage
    // ========================================================================

    /// Get the default keywords file path (~/.matrix/focus_keywords.json).
    pub fn get_keywords_file_path() -> Result<PathBuf> {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .ok_or_else(|| anyhow::anyhow!("HOME or USERPROFILE not set"))?;
        Ok(PathBuf::from(home).join(MATRIX_DIR).join("focus_keywords.json"))
    }

    /// Load keywords from a JSON file.
    ///
    /// If the file doesn't exist, returns a registry with presets loaded.
    pub fn from_file(path: &Path) -> Result<Self> {
        if !path.exists() {
            let mut registry = Self {
                keywords: Vec::new(),
                config: FocusKeywordsRegistryConfig::default(),
                category_index: HashMap::new(),
                keyword_set: HashSet::new(),
                last_preset_load: None,
            };
            registry.load_presets();
            return Ok(registry);
        }

        let data = fs::read_to_string(path)?;

        if data.trim().is_empty() {
            let mut registry = Self {
                keywords: Vec::new(),
                config: FocusKeywordsRegistryConfig::default(),
                category_index: HashMap::new(),
                keyword_set: HashSet::new(),
                last_preset_load: None,
            };
            registry.load_presets();
            return Ok(registry);
        }

        match serde_json::from_str::<FocusKeywordsRegistry>(&data) {
            Ok(mut registry) => {
                if registry.last_preset_load.is_none() {
                    registry.load_presets();
                }
                registry.rebuild_index();
                Ok(registry)
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to parse focus keywords file {:?}: {}. Using defaults.",
                    path,
                    e
                );
                let mut registry = Self {
                    keywords: Vec::new(),
                    config: FocusKeywordsRegistryConfig::default(),
                    category_index: HashMap::new(),
                    keyword_set: HashSet::new(),
                    last_preset_load: None,
                };
                registry.load_presets();
                Ok(registry)
            }
        }
    }

    /// Load keywords from the default file path.
    pub fn from_default_file() -> Result<Self> {
        let path = Self::get_keywords_file_path()?;
        Self::from_file(&path)
    }

    /// Save keywords to a JSON file.
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        let parent = path.parent();
        if let Some(dir) = parent {
            if !dir.exists() {
                fs::create_dir_all(dir)?;
            }
        }

        let json = serde_json::to_string_pretty(self)?;
        let tmp_path = path.with_extension("json.tmp");
        fs::write(&tmp_path, json)?;
        fs::rename(&tmp_path, path)?;

        Ok(())
    }

    /// Save keywords to the default file path.
    pub fn save_to_default_file(&self) -> Result<()> {
        let path = Self::get_keywords_file_path()?;
        self.save_to_file(&path)
    }
}

// ============================================================================
// Statistics
// ============================================================================

/// Statistics about the focus keywords registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusKeywordsStats {
    /// Total keyword count.
    pub total: usize,
    /// Active keyword count.
    pub active: usize,
    /// Inactive keyword count.
    pub inactive: usize,
    /// Transition keyword count.
    pub transition_count: usize,
    /// Question keyword count.
    pub question_count: usize,
    /// Task keyword count.
    pub task_count: usize,
    /// Tech keyword count.
    pub tech_count: usize,
    /// Preset keyword count.
    pub presets: usize,
    /// Manually added keyword count.
    pub manual: usize,
    /// Learned keyword count.
    pub learned: usize,
}

impl std::fmt::Display for FocusKeywordsStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Focus Keywords Stats:")?;
        writeln!(f, "  Total: {} (active: {}, inactive: {})", self.total, self.active, self.inactive)?;
        writeln!(f, "  Transition: {}, Question: {}, Task: {}, Tech: {}",
            self.transition_count, self.question_count, self.task_count, self.tech_count)?;
        writeln!(f, "  Presets: {}, Manual: {}, Learned: {}",
            self.presets, self.manual, self.learned)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // KeywordCategory Tests
    // =========================================================================

    #[test]
    fn test_keyword_category_display() {
        assert_eq!(format!("{}", KeywordCategory::Transition), "transition");
        assert_eq!(format!("{}", KeywordCategory::Question), "question");
        assert_eq!(format!("{}", KeywordCategory::Task), "task");
        assert_eq!(format!("{}", KeywordCategory::Tech), "tech");
    }

    // =========================================================================
    // KeywordEntry Tests
    // =========================================================================

    #[test]
    fn test_keyword_entry_new() {
        let entry = KeywordEntry::new("test", KeywordCategory::Task, KeywordSource::Manual);
        assert_eq!(entry.keyword, "test");
        assert_eq!(entry.category, KeywordCategory::Task);
        assert_eq!(entry.frequency, 1);
        assert!(entry.is_active);
    }

    #[test]
    fn test_keyword_entry_preset() {
        let entry = KeywordEntry::preset("how", KeywordCategory::Question);
        assert_eq!(entry.keyword, "how");
        assert_eq!(entry.frequency, 0);
        assert_eq!(entry.confidence, 1.0);
        assert!(entry.source.is_preset());
    }

    #[test]
    fn test_keyword_entry_manual() {
        let entry = KeywordEntry::manual("implement", KeywordCategory::Task);
        assert!(entry.source.is_manual());
        assert!(entry.is_active);
    }

    #[test]
    fn test_keyword_entry_learned() {
        let entry = KeywordEntry::learned("optimize", KeywordCategory::Task, "session-123");
        assert!(entry.source.is_learned());
        assert_eq!(entry.frequency, 1);
        assert_eq!(entry.confidence, 0.3);
    }

    #[test]
    fn test_keyword_entry_mark_used() {
        let mut entry = KeywordEntry::manual("test", KeywordCategory::Task);
        let initial_freq = entry.frequency;
        entry.mark_used();
        assert_eq!(entry.frequency, initial_freq + 1);
    }

    #[test]
    fn test_keyword_entry_activate_deactivate() {
        let mut entry = KeywordEntry::manual("test", KeywordCategory::Task);
        assert!(entry.is_active);
        entry.deactivate();
        assert!(!entry.is_active);
        entry.activate();
        assert!(entry.is_active);
    }

    // =========================================================================
    // KeywordSource Tests
    // =========================================================================

    #[test]
    fn test_keyword_source_checks() {
        let preset = KeywordSource::Preset;
        assert!(preset.is_preset());
        assert!(!preset.is_manual());
        assert!(!preset.is_learned());

        let manual = KeywordSource::Manual;
        assert!(!manual.is_preset());
        assert!(manual.is_manual());
        assert!(!manual.is_learned());

        let learned = KeywordSource::Learned { session_id: "test".to_string() };
        assert!(!learned.is_preset());
        assert!(!learned.is_manual());
        assert!(learned.is_learned());
    }

    // =========================================================================
    // FocusKeywordsRegistryConfig Tests
    // =========================================================================

    #[test]
    fn test_config_default() {
        let config = FocusKeywordsRegistryConfig::default();
        assert_eq!(config.max_keywords_per_category, 200);
        assert_eq!(config.min_confidence_threshold, 0.3);
        assert_eq!(config.min_frequency, 2);
        assert!(config.auto_learn);
        assert_eq!(config.inactive_after_days, 90);
    }

    #[test]
    fn test_config_with_max_keywords() {
        let config = FocusKeywordsRegistryConfig::with_max_keywords(100);
        assert_eq!(config.max_keywords_per_category, 100);
    }

    #[test]
    fn test_config_minimal() {
        let config = FocusKeywordsRegistryConfig::minimal();
        assert_eq!(config.max_keywords_per_category, 100);
        assert_eq!(config.min_confidence_threshold, 0.5);
        assert_eq!(config.min_frequency, 3);
    }

    // =========================================================================
    // FocusKeywordsRegistry Creation Tests
    // =========================================================================

    #[test]
    fn test_registry_creation() {
        let registry = FocusKeywordsRegistry::new();
        assert!(!registry.is_empty());

        // Should have presets for all categories
        assert!(registry.count_by_category(KeywordCategory::Transition) > 0);
        assert!(registry.count_by_category(KeywordCategory::Question) > 0);
        assert!(registry.count_by_category(KeywordCategory::Task) > 0);
        assert!(registry.count_by_category(KeywordCategory::Tech) > 0);
    }

    #[test]
    fn test_registry_default() {
        let registry = FocusKeywordsRegistry::default();
        assert!(!registry.is_empty());
    }

    #[test]
    fn test_registry_with_config() {
        let config = FocusKeywordsRegistryConfig::minimal();
        let registry = FocusKeywordsRegistry::with_config(config);
        assert_eq!(registry.config.max_keywords_per_category, 100);
    }

    #[test]
    fn test_registry_last_preset_load() {
        let registry = FocusKeywordsRegistry::new();
        assert!(registry.last_preset_load.is_some());
    }

    // =========================================================================
    // Add Keyword Tests
    // =========================================================================

    #[test]
    fn test_add_keyword() {
        let mut registry = FocusKeywordsRegistry::new();
        let initial_count = registry.len();

        registry.add_keyword(KeywordEntry::manual("custom-keyword", KeywordCategory::Task));

        assert_eq!(registry.len(), initial_count + 1);
        assert!(registry.contains(KeywordCategory::Task, "custom-keyword"));
    }

    #[test]
    fn test_add_keywords_batch() {
        let mut registry = FocusKeywordsRegistry::new();
        let initial_count = registry.len();

        let entries = vec![
            KeywordEntry::manual("batch-1", KeywordCategory::Task),
            KeywordEntry::manual("batch-2", KeywordCategory::Question),
            KeywordEntry::manual("batch-3", KeywordCategory::Tech),
        ];

        registry.add_keywords(entries);

        assert_eq!(registry.len(), initial_count + 3);
    }

    #[test]
    fn test_add_shorthand() {
        let mut registry = FocusKeywordsRegistry::new();
        let initial_count = registry.count_by_category(KeywordCategory::Tech);

        registry.add("rustlang", KeywordCategory::Tech);

        assert_eq!(registry.count_by_category(KeywordCategory::Tech), initial_count + 1);
    }

    #[test]
    fn test_duplicate_prevention() {
        let mut registry = FocusKeywordsRegistry::new();
        let initial_count = registry.count_by_category(KeywordCategory::Task);

        registry.add("duplicate-test", KeywordCategory::Task);
        registry.add("duplicate-test", KeywordCategory::Task);

        assert_eq!(registry.count_by_category(KeywordCategory::Task), initial_count + 1);
    }

    #[test]
    fn test_case_insensitivity() {
        let mut registry = FocusKeywordsRegistry::new();

        registry.add("CaseTest", KeywordCategory::Task);

        assert!(registry.contains(KeywordCategory::Task, "casetest"));
        assert!(registry.contains(KeywordCategory::Task, "CASETEST"));
        assert!(registry.contains(KeywordCategory::Task, "CaseTest"));
    }

    // =========================================================================
    // Get Keywords Tests
    // =========================================================================

    #[test]
    fn test_get_keywords() {
        let registry = FocusKeywordsRegistry::new();

        let transition_keywords = registry.get_keywords(KeywordCategory::Transition);
        assert!(!transition_keywords.is_empty());
        assert!(transition_keywords.contains(&"however".to_string()));
        assert!(transition_keywords.contains(&"转换".to_string()));
    }

    #[test]
    fn test_get_keywords_returns_only_active() {
        let mut registry = FocusKeywordsRegistry::new();
        registry.add("inactive-test", KeywordCategory::Task);
        registry.deactivate(KeywordCategory::Task, "inactive-test");

        let keywords = registry.get_keywords(KeywordCategory::Task);
        assert!(!keywords.contains(&"inactive-test".to_string()));
    }

    #[test]
    fn test_get_all_keywords_includes_inactive() {
        let mut registry = FocusKeywordsRegistry::new();
        registry.add("inactive-test", KeywordCategory::Task);
        registry.deactivate(KeywordCategory::Task, "inactive-test");

        let keywords = registry.get_all_keywords(KeywordCategory::Task);
        assert!(keywords.iter().any(|k| k == "inactive-test"));
    }

    // =========================================================================
    // Match Tests
    // =========================================================================

    #[test]
    fn test_matches() {
        let registry = FocusKeywordsRegistry::new();

        assert!(registry.matches(KeywordCategory::Question, "How do I do this?"));
        assert!(registry.matches(KeywordCategory::Question, "请问这个问题"));
        assert!(registry.matches(KeywordCategory::Task, "Please implement this feature"));
        assert!(registry.matches(KeywordCategory::Task, "创建一个新功能"));
        assert!(registry.matches(KeywordCategory::Tech, "We use Rust for performance"));
    }

    #[test]
    fn test_matches_case_insensitive() {
        let registry = FocusKeywordsRegistry::new();

        assert!(registry.matches(KeywordCategory::Question, "HOW DO I DO THIS?"));
        assert!(registry.matches(KeywordCategory::Task, "IMPLEMENT THIS"));
    }

    #[test]
    fn test_find_matches() {
        let registry = FocusKeywordsRegistry::new();

        let matches = registry.find_matches(KeywordCategory::Tech, "We use Rust and Python for development");
        assert!(matches.contains(&"rust".to_string()));
        assert!(matches.contains(&"python".to_string()));
    }

    // =========================================================================
    // Learn Tests
    // =========================================================================

    #[test]
    fn test_learn_new_keyword() {
        let mut registry = FocusKeywordsRegistry::new();
        let initial_count = registry.count_by_category(KeywordCategory::Task);

        registry.learn("brand-new-keyword", KeywordCategory::Task, "session-123");

        assert_eq!(registry.count_by_category(KeywordCategory::Task), initial_count + 1);
    }

    #[test]
    fn test_learn_existing_keyword() {
        let mut registry = FocusKeywordsRegistry::new();
        registry.add("existing-keyword", KeywordCategory::Task);
        let entry = registry.get_keyword(KeywordCategory::Task, "existing-keyword").unwrap();
        let initial_freq = entry.frequency;

        registry.learn("existing-keyword", KeywordCategory::Task, "session-123");

        let entry = registry.get_keyword(KeywordCategory::Task, "existing-keyword").unwrap();
        assert_eq!(entry.frequency, initial_freq + 1);
    }

    #[test]
    fn test_learn_keywords_batch() {
        let mut registry = FocusKeywordsRegistry::new();
        let initial_task = registry.count_by_category(KeywordCategory::Task);
        let initial_tech = registry.count_by_category(KeywordCategory::Tech);

        registry.learn_keywords(
            &[("new-task", KeywordCategory::Task), ("new-tech", KeywordCategory::Tech)],
            "session-123"
        );

        assert_eq!(registry.count_by_category(KeywordCategory::Task), initial_task + 1);
        assert_eq!(registry.count_by_category(KeywordCategory::Tech), initial_tech + 1);
    }

    // =========================================================================
    // Activate/Deactivate Tests
    // =========================================================================

    #[test]
    fn test_deactivate() {
        let mut registry = FocusKeywordsRegistry::new();
        registry.add("test-deactivate", KeywordCategory::Task);

        assert!(registry.deactivate(KeywordCategory::Task, "test-deactivate"));

        let entry = registry.get_keyword(KeywordCategory::Task, "test-deactivate").unwrap();
        assert!(!entry.is_active);
    }

    #[test]
    fn test_activate() {
        let mut registry = FocusKeywordsRegistry::new();
        registry.add("test-activate", KeywordCategory::Task);
        registry.deactivate(KeywordCategory::Task, "test-activate");

        assert!(!registry.get_keyword(KeywordCategory::Task, "test-activate").unwrap().is_active);

        assert!(registry.activate(KeywordCategory::Task, "test-activate"));

        let entry = registry.get_keyword(KeywordCategory::Task, "test-activate").unwrap();
        assert!(entry.is_active);
    }

    #[test]
    fn test_deactivate_nonexistent() {
        let mut registry = FocusKeywordsRegistry::new();
        assert!(!registry.deactivate(KeywordCategory::Task, "nonexistent"));
    }

    // =========================================================================
    // Remove Tests
    // =========================================================================

    #[test]
    fn test_remove() {
        let mut registry = FocusKeywordsRegistry::new();
        registry.add("test-remove", KeywordCategory::Task);
        let count_after_add = registry.count_by_category(KeywordCategory::Task);

        assert!(registry.remove(KeywordCategory::Task, "test-remove"));

        assert_eq!(registry.count_by_category(KeywordCategory::Task), count_after_add - 1);
        assert!(!registry.contains(KeywordCategory::Task, "test-remove"));
    }

    #[test]
    fn test_remove_nonexistent() {
        let mut registry = FocusKeywordsRegistry::new();
        assert!(!registry.remove(KeywordCategory::Task, "nonexistent"));
    }

    // =========================================================================
    // Statistics Tests
    // =========================================================================

    #[test]
    fn test_stats() {
        let registry = FocusKeywordsRegistry::new();
        let stats = registry.stats();

        assert!(stats.total > 0);
        assert!(stats.presets > 0);
        assert_eq!(stats.active, stats.total); // All presets are active
        assert_eq!(
            stats.total,
            stats.transition_count + stats.question_count + stats.task_count + stats.tech_count
        );
    }

    #[test]
    fn test_stats_display() {
        let registry = FocusKeywordsRegistry::new();
        let stats = registry.stats();
        let display = format!("{}", stats);

        assert!(display.contains("Focus Keywords Stats"));
        assert!(display.contains("Total:"));
        assert!(display.contains("Transition:"));
    }

    // =========================================================================
    // Prune Tests
    // =========================================================================

    #[test]
    fn test_prune_keeps_active() {
        let mut registry = FocusKeywordsRegistry::new();
        registry.add("active-keyword", KeywordCategory::Task);

        let len_before = registry.len();
        registry.prune();

        assert!(registry.contains(KeywordCategory::Task, "active-keyword"));
    }

    #[test]
    fn test_prune_keeps_presets() {
        let registry = FocusKeywordsRegistry::new();
        let preset_count = registry.all_keywords().iter().filter(|k| k.source.is_preset()).count();

        let mut registry_clone = registry;
        registry_clone.prune();

        let preset_count_after = registry_clone.all_keywords().iter().filter(|k| k.source.is_preset()).count();
        assert_eq!(preset_count, preset_count_after);
    }

    // =========================================================================
    // Reload Presets Tests
    // =========================================================================

    #[test]
    fn test_reload_presets() {
        let mut registry = FocusKeywordsRegistry::new();
        let initial_count = registry.count_by_category(KeywordCategory::Transition);

        // Remove all presets
        registry.keywords.retain(|k| !k.source.is_preset());
        registry.rebuild_index();

        assert_eq!(registry.count_by_category(KeywordCategory::Transition), 0);

        // Reload
        registry.reload_presets();

        assert!(registry.count_by_category(KeywordCategory::Transition) > 0);
    }

    // =========================================================================
    // Serialization Tests
    // =========================================================================

    #[test]
    fn test_serialization() {
        let registry = FocusKeywordsRegistry::new();
        let json = serde_json::to_string(&registry).unwrap();
        let decoded: FocusKeywordsRegistry = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.len(), registry.len());
        assert_eq!(
            decoded.count_by_category(KeywordCategory::Transition),
            registry.count_by_category(KeywordCategory::Transition)
        );
    }

    #[test]
    fn test_serialization_preserves_keywords() {
        let mut registry = FocusKeywordsRegistry::new();
        registry.add("serialize-test", KeywordCategory::Task);

        let json = serde_json::to_string(&registry).unwrap();
        let decoded: FocusKeywordsRegistry = serde_json::from_str(&json).unwrap();

        assert!(decoded.contains(KeywordCategory::Task, "serialize-test"));
    }

    #[test]
    fn test_serialization_config_preserved() {
        let config = FocusKeywordsRegistryConfig::minimal();
        let registry = FocusKeywordsRegistry::with_config(config);

        let json = serde_json::to_string(&registry).unwrap();
        let decoded: FocusKeywordsRegistry = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.config.max_keywords_per_category, 100);
        assert_eq!(decoded.config.min_confidence_threshold, 0.5);
    }

    // =========================================================================
    // File Storage Tests
    // =========================================================================

    #[test]
    fn test_get_keywords_file_path() {
        let path = FocusKeywordsRegistry::get_keywords_file_path();
        assert!(path.is_ok());

        let path = path.unwrap();
        assert!(path.to_string_lossy().contains(".matrix"));
        assert!(path.to_string_lossy().contains("focus_keywords.json"));
    }

    #[test]
    fn test_from_file_nonexistent() {
        let temp_dir = tempfile::tempdir().unwrap();
        let nonexistent_path = temp_dir.path().join("nonexistent_keywords.json");

        let registry = FocusKeywordsRegistry::from_file(&nonexistent_path).unwrap();

        assert!(!registry.is_empty());
        assert!(registry.last_preset_load.is_some());
    }

    #[test]
    fn test_from_file_empty_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let empty_path = temp_dir.path().join("empty_keywords.json");

        fs::write(&empty_path, "").unwrap();

        let registry = FocusKeywordsRegistry::from_file(&empty_path).unwrap();

        assert!(!registry.is_empty());
        assert!(registry.last_preset_load.is_some());
    }

    #[test]
    fn test_save_to_file_roundtrip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("roundtrip_keywords.json");

        let mut original = FocusKeywordsRegistry::new();
        original.add("custom-keyword-1", KeywordCategory::Task);
        original.add("custom-keyword-2", KeywordCategory::Tech);

        original.save_to_file(&file_path).unwrap();

        let loaded = FocusKeywordsRegistry::from_file(&file_path).unwrap();

        assert!(loaded.contains(KeywordCategory::Task, "custom-keyword-1"));
        assert!(loaded.contains(KeywordCategory::Tech, "custom-keyword-2"));
    }

    #[test]
    fn test_save_to_file_creates_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let nested_path = temp_dir.path().join("nested").join("dir").join("keywords.json");

        assert!(!nested_path.parent().unwrap().exists());

        let registry = FocusKeywordsRegistry::new();
        registry.save_to_file(&nested_path).unwrap();

        assert!(nested_path.parent().unwrap().exists());
        assert!(nested_path.exists());
    }

    #[test]
    fn test_from_file_malformed_json() {
        let temp_dir = tempfile::tempdir().unwrap();
        let malformed_path = temp_dir.path().join("malformed_keywords.json");

        fs::write(&malformed_path, "{ not valid json }").unwrap();

        let registry = FocusKeywordsRegistry::from_file(&malformed_path).unwrap();

        assert!(!registry.is_empty());
        assert!(registry.last_preset_load.is_some());
    }

    // =========================================================================
    // Capacity Tests
    // =========================================================================

    #[test]
    fn test_capacity_limit() {
        let config = FocusKeywordsRegistryConfig::with_max_keywords(3);
        let mut registry = FocusKeywordsRegistry::with_config(config);

        // Clear presets for clean test
        registry.keywords.clear();
        registry.rebuild_index();

        registry.add("keyword-1", KeywordCategory::Task);
        registry.add("keyword-2", KeywordCategory::Task);
        registry.add("keyword-3", KeywordCategory::Task);
        registry.add("keyword-4", KeywordCategory::Task); // Should trigger removal

        assert_eq!(registry.count_by_category(KeywordCategory::Task), 3);
    }

    // =========================================================================
    // Edge Case Tests
    // =========================================================================

    #[test]
    fn test_empty_keyword_handling() {
        let mut registry = FocusKeywordsRegistry::new();
        let count_before = registry.len();

        // Empty string should still be added (though not useful)
        registry.add("", KeywordCategory::Task);
        assert_eq!(registry.len(), count_before + 1);
    }

    #[test]
    fn test_special_characters_in_keyword() {
        let mut registry = FocusKeywordsRegistry::new();

        registry.add("rust-lang", KeywordCategory::Tech);
        registry.add("c++", KeywordCategory::Tech);
        registry.add("node.js", KeywordCategory::Tech);

        assert!(registry.contains(KeywordCategory::Tech, "rust-lang"));
        assert!(registry.contains(KeywordCategory::Tech, "c++"));
        assert!(registry.contains(KeywordCategory::Tech, "node.js"));
    }

    #[test]
    fn test_unicode_keywords() {
        let mut registry = FocusKeywordsRegistry::new();

        registry.add("中文关键词", KeywordCategory::Task);
        registry.add("日本語", KeywordCategory::Tech);

        assert!(registry.contains(KeywordCategory::Task, "中文关键词"));
        assert!(registry.contains(KeywordCategory::Tech, "日本語"));

        // Case insensitivity should work for unicode too
        assert!(registry.contains(KeywordCategory::Task, "中文关键词"));
    }

    #[test]
    fn test_matches_with_unicode() {
        let registry = FocusKeywordsRegistry::new();

        // Preset Chinese keywords should match
        assert!(registry.matches(KeywordCategory::Question, "如何解决这个问题？"));
        assert!(registry.matches(KeywordCategory::Task, "请实现这个功能"));
        assert!(registry.matches(KeywordCategory::Tech, "这是一个性能问题"));
    }
}