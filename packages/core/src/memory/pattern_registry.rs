//! Pattern registry for managing conversation patterns.
//!
//! This module provides the central registry for storing, retrieving, and
//! learning conversation patterns. All patterns are learned from conversations,
//! no hardcoded presets are loaded.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::conversation_pattern::{ConversationPattern, PatternType};
use crate::constants::MATRIX_DIR;

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for the pattern registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternRegistryConfig {
    /// Maximum number of patterns to store per type.
    pub max_patterns_per_type: usize,
    /// Minimum confidence threshold for patterns to be active.
    pub min_confidence_threshold: f32,
    /// Minimum frequency for a pattern to be considered established.
    pub min_frequency: u32,
    /// Whether to auto-learn patterns from conversations.
    pub auto_learn: bool,
    /// Days before unused patterns are deactivated.
    pub inactive_after_days: i64,
}

impl Default for PatternRegistryConfig {
    fn default() -> Self {
        Self {
            max_patterns_per_type: 100,
            min_confidence_threshold: 0.3,
            min_frequency: 2,
            auto_learn: true,
            inactive_after_days: 90,
        }
    }
}

impl PatternRegistryConfig {
    /// Create a config with custom max patterns.
    pub fn with_max_patterns(max: usize) -> Self {
        Self {
            max_patterns_per_type: max,
            ..Self::default()
        }
    }

    /// Create a minimal config for low-memory environments.
    pub fn minimal() -> Self {
        Self {
            max_patterns_per_type: 50,
            min_confidence_threshold: 0.5,
            min_frequency: 3,
            auto_learn: true,
            inactive_after_days: 60,
        }
    }
}

// ============================================================================
// Pattern Registry
// ============================================================================

/// Central registry for conversation patterns.
///
/// Manages a collection of patterns organized by type, supports
/// pattern learning, and persistence. All patterns are learned
/// from conversations, no hardcoded presets are loaded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternRegistry {
    /// All patterns indexed by ID.
    patterns: Vec<ConversationPattern>,
    /// Configuration.
    #[serde(default)]
    config: PatternRegistryConfig,
    /// Index for fast type-based lookup.
    #[serde(skip)]
    type_index: HashMap<PatternType, Vec<usize>>,
}

impl Default for PatternRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl PatternRegistry {
    /// Create a new empty pattern registry.
    ///
    /// All patterns are learned from conversations, no presets are loaded.
    pub fn new() -> Self {
        Self {
            patterns: Vec::new(),
            config: PatternRegistryConfig::default(),
            type_index: HashMap::new(),
        }
    }

    /// Create a registry with custom configuration.
    ///
    /// All patterns are learned from conversations, no presets are loaded.
    pub fn with_config(config: PatternRegistryConfig) -> Self {
        Self {
            patterns: Vec::new(),
            config,
            type_index: HashMap::new(),
        }
    }

    /// Rebuild the type index after modifications.
    fn rebuild_index(&mut self) {
        self.type_index.clear();
        for (idx, pattern) in self.patterns.iter().enumerate() {
            self.type_index
                .entry(pattern.pattern_type)
                .or_default()
                .push(idx);
        }
    }

    /// Add a pattern internally (without rebuilding index).
    fn add_pattern_internal(&mut self, pattern: ConversationPattern) {
        // Check for duplicate patterns
        if self.patterns.iter().any(|p| p.pattern == pattern.pattern) {
            return;
        }

        // Check capacity per type
        let type_count = self
            .patterns
            .iter()
            .filter(|p| p.pattern_type == pattern.pattern_type)
            .count();

        if type_count >= self.config.max_patterns_per_type {
            // Remove lowest frequency pattern of same type
            if let Some(idx) = self
                .patterns
                .iter()
                .enumerate()
                .filter(|(_, p)| p.pattern_type == pattern.pattern_type)
                .min_by_key(|(_, p)| (p.frequency, (p.confidence * 100.0) as u32))
                .map(|(i, _)| i)
            {
                self.patterns.remove(idx);
            }
        }

        self.patterns.push(pattern);
    }

    /// Add a new pattern to the registry.
    pub fn add_pattern(&mut self, pattern: ConversationPattern) {
        self.add_pattern_internal(pattern);
        self.rebuild_index();
    }

    /// Add multiple patterns at once.
    pub fn add_patterns(&mut self, patterns: Vec<ConversationPattern>) {
        for pattern in patterns {
            self.add_pattern_internal(pattern);
        }
        self.rebuild_index();
    }

    /// Learn patterns from a list of detected patterns.
    ///
    /// Existing patterns have their frequency incremented;
    /// new patterns are added with initial confidence.
    pub fn learn_patterns(&mut self, patterns: &[ConversationPattern]) {
        for new_pattern in patterns {
            if let Some(existing) = self
                .patterns
                .iter_mut()
                .find(|p| p.pattern == new_pattern.pattern)
            {
                existing.mark_used();
            } else if self.config.auto_learn {
                self.add_pattern_internal(new_pattern.clone());
            }
        }
        self.rebuild_index();
    }

    /// Get all active patterns.
    pub fn get_active_patterns(&self) -> Vec<&ConversationPattern> {
        self.patterns.iter().filter(|p| p.is_active).collect()
    }

    /// Get active patterns of a specific type.
    pub fn get_active_patterns_by_type(&self, pattern_type: PatternType) -> Vec<&ConversationPattern> {
        self.patterns
            .iter()
            .filter(|p| p.is_active && p.pattern_type == pattern_type)
            .collect()
    }

    /// Get all active reference patterns.
    pub fn get_active_reference_patterns(&self) -> Vec<String> {
        self.get_active_patterns_by_type(PatternType::Reference)
            .iter()
            .map(|p| p.pattern.clone())
            .collect()
    }

    /// Get all active code patterns.
    pub fn get_active_code_patterns(&self) -> Vec<String> {
        self.get_active_patterns_by_type(PatternType::Code)
            .iter()
            .map(|p| p.pattern.clone())
            .collect()
    }

    /// Get pattern by ID.
    pub fn get_pattern(&self, id: &str) -> Option<&ConversationPattern> {
        self.patterns.iter().find(|p| p.id == id)
    }

    /// Get pattern by pattern string.
    pub fn get_pattern_by_pattern(&self, pattern: &str) -> Option<&ConversationPattern> {
        self.patterns.iter().find(|p| p.pattern == pattern)
    }

    /// Get pattern by ID for mutation.
    pub fn get_pattern_mut(&mut self, id: &str) -> Option<&mut ConversationPattern> {
        self.patterns.iter_mut().find(|p| p.id == id)
    }

    /// Deactivate a pattern by ID.
    pub fn deactivate_pattern(&mut self, id: &str) -> bool {
        if let Some(pattern) = self.get_pattern_mut(id) {
            pattern.deactivate();
            true
        } else {
            false
        }
    }

    /// Activate a pattern by ID.
    pub fn activate_pattern(&mut self, id: &str) -> bool {
        if let Some(pattern) = self.get_pattern_mut(id) {
            pattern.activate();
            true
        } else {
            false
        }
    }

    /// Remove a pattern by ID.
    pub fn remove_pattern(&mut self, id: &str) -> bool {
        let len_before = self.patterns.len();
        self.patterns.retain(|p| p.id != id);
        if self.patterns.len() < len_before {
            self.rebuild_index();
            true
        } else {
            false
        }
    }

    /// Get total pattern count.
    pub fn len(&self) -> usize {
        self.patterns.len()
    }

    /// Check if registry is empty.
    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    /// Get pattern count by type.
    pub fn count_by_type(&self, pattern_type: PatternType) -> usize {
        self.patterns
            .iter()
            .filter(|p| p.pattern_type == pattern_type)
            .count()
    }

    /// Get active pattern count by type.
    pub fn active_count_by_type(&self, pattern_type: PatternType) -> usize {
        self.get_active_patterns_by_type(pattern_type).len()
    }

    /// Get all patterns (for serialization).
    pub fn all_patterns(&self) -> &[ConversationPattern] {
        &self.patterns
    }

    /// Prune inactive patterns below confidence/frequency thresholds.
    pub fn prune(&mut self) {
        let now = Utc::now();
        let threshold_days = self.config.inactive_after_days;

        self.patterns.retain(|p| {
            // Keep active patterns
            if p.is_active {
                return true;
            }
            // Keep presets
            if p.source.is_preset() {
                return true;
            }
            // Keep manually added patterns
            if p.source.is_manual() {
                return true;
            }
            // Keep if above frequency/confidence thresholds
            if p.frequency >= self.config.min_frequency
                && p.confidence >= self.config.min_confidence_threshold
            {
                return true;
            }
            // Remove if too old
            let age = (now - p.last_used).num_days();
            age < threshold_days
        });

        self.rebuild_index();
    }

    /// Get statistics about the registry.
    pub fn stats(&self) -> PatternRegistryStats {
        let total = self.patterns.len();
        let active = self.patterns.iter().filter(|p| p.is_active).count();
        let reference_count = self.count_by_type(PatternType::Reference);
        let code_count = self.count_by_type(PatternType::Code);
        let presets = self.patterns.iter().filter(|p| p.source.is_preset()).count();
        let manual = self.patterns.iter().filter(|p| p.source.is_manual()).count();
        let learned = total - presets - manual;

        PatternRegistryStats {
            total,
            active,
            inactive: total - active,
            reference_count,
            code_count,
            presets,
            manual,
            learned,
        }
    }

    // ========================================================================
    // File Storage Methods
    // ========================================================================

    /// Get the default patterns file path (~/.matrix/patterns.json).
    pub fn get_patterns_file_path() -> Result<PathBuf> {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .ok_or_else(|| anyhow::anyhow!("HOME or USERPROFILE not set"))?;
        Ok(PathBuf::from(home).join(MATRIX_DIR).join("patterns.json"))
    }

    /// Load patterns from a JSON file.
    ///
    /// If the file doesn't exist, returns an empty registry.
    /// If the file exists but is empty or malformed, returns an empty registry.
    pub fn from_file(path: &Path) -> Result<Self> {
        if !path.exists() {
            // File doesn't exist - create empty registry
            return Ok(Self::new());
        }

        let data = fs::read_to_string(path)?;

        // Handle empty file
        if data.trim().is_empty() {
            return Ok(Self::new());
        }

        // Try to parse JSON
        match serde_json::from_str::<PatternRegistry>(&data) {
            Ok(mut registry) => {
                registry.rebuild_index();
                Ok(registry)
            }
            Err(e) => {
                // JSON parse error - log and return empty registry
                tracing::warn!(
                    "Failed to parse patterns file {:?}: {}. Using empty registry.",
                    path,
                    e
                );
                Ok(Self::new())
            }
        }
    }

    /// Load patterns from the default file path (~/.matrix/patterns.json).
    pub fn from_default_file() -> Result<Self> {
        let path = Self::get_patterns_file_path()?;
        Self::from_file(&path)
    }

    /// Save patterns to a JSON file.
    ///
    /// Creates the parent directory if it doesn't exist.
    /// Uses atomic write (write to temp file, then rename) for safety.
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        // Ensure parent directory exists
        let parent = path.parent();
        if let Some(dir) = parent {
            if !dir.exists() {
                fs::create_dir_all(dir)?;
            }
        }

        // Serialize to JSON
        let json = serde_json::to_string_pretty(self)?;

        // Atomic write: write to temp file first
        let tmp_path = path.with_extension("json.tmp");
        fs::write(&tmp_path, json)?;

        // Rename temp file to final path (atomic on most systems)
        fs::rename(&tmp_path, path)?;

        Ok(())
    }

    /// Save patterns to the default file path (~/.matrix/patterns.json).
    pub fn save_to_default_file(&self) -> Result<()> {
        let path = Self::get_patterns_file_path()?;
        self.save_to_file(&path)
    }
}

// ============================================================================
// Statistics
// ============================================================================

/// Statistics about the pattern registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternRegistryStats {
    /// Total pattern count.
    pub total: usize,
    /// Active pattern count.
    pub active: usize,
    /// Inactive pattern count.
    pub inactive: usize,
    /// Reference pattern count.
    pub reference_count: usize,
    /// Code pattern count.
    pub code_count: usize,
    /// Preset pattern count.
    pub presets: usize,
    /// Manually added pattern count.
    pub manual: usize,
    /// Learned pattern count.
    pub learned: usize,
}

impl std::fmt::Display for PatternRegistryStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Pattern Registry Stats:")?;
        writeln!(f, "  Total: {} (active: {}, inactive: {})", self.total, self.active, self.inactive)?;
        writeln!(f, "  Reference: {}, Code: {}", self.reference_count, self.code_count)?;
        writeln!(f, "  Presets: {}, Manual: {}, Learned: {}", self.presets, self.manual, self.learned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::conversation_pattern::PatternSource;

    // =========================================================================
    // PatternRegistryConfig Tests
    // =========================================================================

    #[test]
    fn test_config_default() {
        let config = PatternRegistryConfig::default();

        assert_eq!(config.max_patterns_per_type, 100);
        assert_eq!(config.min_confidence_threshold, 0.3);
        assert_eq!(config.min_frequency, 2);
        assert!(config.auto_learn);
        assert_eq!(config.inactive_after_days, 90);
    }

    #[test]
    fn test_config_with_max_patterns() {
        let config = PatternRegistryConfig::with_max_patterns(50);

        assert_eq!(config.max_patterns_per_type, 50);
        // Other fields should be default
        assert_eq!(config.min_confidence_threshold, 0.3);
        assert_eq!(config.min_frequency, 2);
    }

    #[test]
    fn test_config_minimal() {
        let config = PatternRegistryConfig::minimal();

        assert_eq!(config.max_patterns_per_type, 50);
        assert_eq!(config.min_confidence_threshold, 0.5);
        assert_eq!(config.min_frequency, 3);
        assert!(config.auto_learn);
        assert_eq!(config.inactive_after_days, 60);
    }

    #[test]
    fn test_config_serialization() {
        let config = PatternRegistryConfig::with_max_patterns(75);
        let json = serde_json::to_string(&config).unwrap();
        let decoded: PatternRegistryConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.max_patterns_per_type, 75);
        assert_eq!(decoded.min_confidence_threshold, config.min_confidence_threshold);
    }

    // =========================================================================
    // PatternRegistry Creation Tests
    // =========================================================================

    #[test]
    fn test_registry_creation() {
        let registry = PatternRegistry::new();
        // Should be empty - no presets loaded
        assert!(registry.is_empty());
        assert_eq!(registry.count_by_type(PatternType::Reference), 0);
        assert_eq!(registry.count_by_type(PatternType::Code), 0);
    }

    #[test]
    fn test_registry_default() {
        let registry = PatternRegistry::default();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_registry_with_config() {
        let config = PatternRegistryConfig::minimal();
        let registry = PatternRegistry::with_config(config);

        assert_eq!(registry.config.max_patterns_per_type, 50);
        assert!(registry.is_empty()); // No presets loaded
    }

    // =========================================================================
    // Add Pattern Tests
    // =========================================================================

    #[test]
    fn test_add_pattern() {
        let mut registry = PatternRegistry::new();
        assert!(registry.is_empty());

        let pattern = ConversationPattern::new(
            PatternType::Reference,
            r"test-pattern-\d+",
            PatternSource::Manual,
        );
        registry.add_pattern(pattern);

        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_add_patterns_batch() {
        let mut registry = PatternRegistry::new();
        assert!(registry.is_empty());

        let patterns = vec![
            ConversationPattern::new(PatternType::Reference, "batch-1", PatternSource::Manual),
            ConversationPattern::new(PatternType::Code, "batch-2", PatternSource::Manual),
            ConversationPattern::new(PatternType::Reference, "batch-3", PatternSource::Manual),
        ];

        registry.add_patterns(patterns);

        assert_eq!(registry.len(), 3);
    }

    #[test]
    fn test_duplicate_prevention() {
        let mut registry = PatternRegistry::new();
        assert!(registry.is_empty());

        let pattern1 = ConversationPattern::new(
            PatternType::Reference,
            "duplicate-test",
            PatternSource::Manual,
        );
        let pattern2 = ConversationPattern::new(
            PatternType::Reference,
            "duplicate-test",
            PatternSource::user_conversation("test"),
        );

        registry.add_pattern(pattern1);
        registry.add_pattern(pattern2);

        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_duplicate_prevention_in_batch() {
        let mut registry = PatternRegistry::new();
        assert!(registry.is_empty());

        let patterns = vec![
            ConversationPattern::new(PatternType::Reference, "same-pattern", PatternSource::Manual),
            ConversationPattern::new(PatternType::Reference, "same-pattern", PatternSource::user_conversation("test")),
        ];

        registry.add_patterns(patterns);

        // Only one should be added
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_capacity_limit_removes_lowest_frequency() {
        let config = PatternRegistryConfig::with_max_patterns(2);
        let mut registry = PatternRegistry::with_config(config);
        assert!(registry.is_empty());

        // Add patterns with different frequencies
        let mut p1 = ConversationPattern::new(PatternType::Reference, "pattern-1", PatternSource::Manual);
        p1.frequency = 10;

        let mut p2 = ConversationPattern::new(PatternType::Reference, "pattern-2", PatternSource::Manual);
        p2.frequency = 5;

        let p3 = ConversationPattern::new(PatternType::Reference, "pattern-3", PatternSource::Manual);

        registry.add_pattern(p1);
        registry.add_pattern(p2);
        registry.add_pattern(p3); // This should trigger removal of p2 (lowest frequency)

        // pattern-2 should have been removed
        assert!(registry.get_pattern_by_pattern("pattern-1").is_some());
        assert!(registry.get_pattern_by_pattern("pattern-3").is_some());
    }

    // =========================================================================
    // Get Pattern Tests
    // =========================================================================

    #[test]
    fn test_get_active_patterns_empty() {
        let registry = PatternRegistry::new();

        let refs = registry.get_active_reference_patterns();
        let codes = registry.get_active_code_patterns();

        assert!(refs.is_empty());
        assert!(codes.is_empty());
    }

    #[test]
    fn test_get_active_patterns_by_type_empty() {
        let registry = PatternRegistry::new();

        let ref_patterns = registry.get_active_patterns_by_type(PatternType::Reference);
        let code_patterns = registry.get_active_patterns_by_type(PatternType::Code);

        assert!(ref_patterns.is_empty());
        assert!(code_patterns.is_empty());
    }

    #[test]
    fn test_get_active_patterns_with_added_patterns() {
        let mut registry = PatternRegistry::new();

        registry.add_pattern(ConversationPattern::manual(PatternType::Reference, "ref-pattern"));
        registry.add_pattern(ConversationPattern::manual(PatternType::Code, "code-pattern"));

        let ref_patterns = registry.get_active_patterns_by_type(PatternType::Reference);
        let code_patterns = registry.get_active_patterns_by_type(PatternType::Code);

        assert_eq!(ref_patterns.len(), 1);
        assert_eq!(code_patterns.len(), 1);

        // All returned patterns should be active and of correct type
        for p in &ref_patterns {
            assert!(p.is_active);
            assert_eq!(p.pattern_type, PatternType::Reference);
        }
        for p in &code_patterns {
            assert!(p.is_active);
            assert_eq!(p.pattern_type, PatternType::Code);
        }
    }

    #[test]
    fn test_get_pattern_by_id() {
        let mut registry = PatternRegistry::new();

        let pattern = ConversationPattern::manual(PatternType::Code, "test-get-by-id");
        let id = pattern.id.clone();
        registry.add_pattern(pattern);

        let found = registry.get_pattern(&id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().pattern, "test-get-by-id");
    }

    #[test]
    fn test_get_pattern_by_id_not_found() {
        let registry = PatternRegistry::new();

        let found = registry.get_pattern("nonexistent-id");
        assert!(found.is_none());
    }

    #[test]
    fn test_get_pattern_mut() {
        let mut registry = PatternRegistry::new();

        let pattern = ConversationPattern::manual(PatternType::Code, "test-get-mut");
        let id = pattern.id.clone();
        registry.add_pattern(pattern);

        let found = registry.get_pattern_mut(&id);
        assert!(found.is_some());
        found.unwrap().frequency = 100;

        let found_again = registry.get_pattern(&id).unwrap();
        assert_eq!(found_again.frequency, 100);
    }

    #[test]
    fn test_all_patterns_empty() {
        let registry = PatternRegistry::new();
        let patterns = registry.all_patterns();

        assert!(patterns.is_empty());
        assert_eq!(patterns.len(), registry.len());
    }

    #[test]
    fn test_all_patterns_with_added() {
        let mut registry = PatternRegistry::new();
        registry.add_pattern(ConversationPattern::manual(PatternType::Code, "test-pattern"));

        let patterns = registry.all_patterns();
        assert_eq!(patterns.len(), 1);
    }

    // =========================================================================
    // Pattern Activation Tests
    // =========================================================================

    #[test]
    fn test_deactivate_pattern() {
        let mut registry = PatternRegistry::new();

        // Add a pattern
        let pattern = ConversationPattern::manual(PatternType::Code, "test-deactivate");
        let id = pattern.id.clone();
        registry.add_pattern(pattern);

        // Deactivate it
        assert!(registry.deactivate_pattern(&id));
        let p = registry.get_pattern(&id).unwrap();
        assert!(!p.is_active);
    }

    #[test]
    fn test_deactivate_pattern_not_found() {
        let mut registry = PatternRegistry::new();

        assert!(!registry.deactivate_pattern("nonexistent-id"));
    }

    #[test]
    fn test_activate_pattern() {
        let mut registry = PatternRegistry::new();

        let pattern = ConversationPattern::manual(PatternType::Code, "test-activate");
        let id = pattern.id.clone();
        registry.add_pattern(pattern);

        // First deactivate
        registry.deactivate_pattern(&id);
        assert!(!registry.get_pattern(&id).unwrap().is_active);

        // Then activate again
        assert!(registry.activate_pattern(&id));
        assert!(registry.get_pattern(&id).unwrap().is_active);
    }

    #[test]
    fn test_activate_pattern_not_found() {
        let mut registry = PatternRegistry::new();

        assert!(!registry.activate_pattern("nonexistent-id"));
    }

    #[test]
    fn test_active_count_by_type() {
        let mut registry = PatternRegistry::new();
        let initial_active = registry.active_count_by_type(PatternType::Code);

        // Add and deactivate a pattern
        let pattern = ConversationPattern::manual(PatternType::Code, "test-count");
        let id = pattern.id.clone();
        registry.add_pattern(pattern);

        assert_eq!(registry.active_count_by_type(PatternType::Code), initial_active + 1);

        registry.deactivate_pattern(&id);
        assert_eq!(registry.active_count_by_type(PatternType::Code), initial_active);
    }

    // =========================================================================
    // Remove Pattern Tests
    // =========================================================================

    #[test]
    fn test_remove_pattern() {
        let mut registry = PatternRegistry::new();

        let pattern = ConversationPattern::manual(PatternType::Code, "test-remove");
        let id = pattern.id.clone();
        registry.add_pattern(pattern);

        let len_before = registry.len();
        assert!(registry.remove_pattern(&id));
        assert_eq!(registry.len(), len_before - 1);
        assert!(registry.get_pattern(&id).is_none());
    }

    #[test]
    fn test_remove_pattern_not_found() {
        let mut registry = PatternRegistry::new();

        assert!(!registry.remove_pattern("nonexistent-id"));
    }

    // =========================================================================
    // Learn Patterns Tests
    // =========================================================================

    #[test]
    fn test_learn_patterns() {
        let mut registry = PatternRegistry::new();
        let initial_ref_count = registry.count_by_type(PatternType::Reference);

        // Learn a new pattern
        let new_pattern =
            ConversationPattern::new(PatternType::Reference, "LEARN-123", PatternSource::Manual);
        registry.learn_patterns(&[new_pattern]);

        assert_eq!(
            registry.count_by_type(PatternType::Reference),
            initial_ref_count + 1
        );

        // Learn the same pattern again (should increment frequency)
        let same_pattern =
            ConversationPattern::new(PatternType::Reference, "LEARN-123", PatternSource::Manual);
        registry.learn_patterns(&[same_pattern]);

        let p = registry.patterns.iter().find(|p| p.pattern == "LEARN-123").unwrap();
        assert_eq!(p.frequency, 2); // 1 (initial from new) + 1 (mark_used from second learn)
    }

    #[test]
    fn test_learn_patterns_multiple_new() {
        let mut registry = PatternRegistry::new();
        let initial_count = registry.len();

        let patterns = vec![
            ConversationPattern::new(PatternType::Reference, "learn-1", PatternSource::Manual),
            ConversationPattern::new(PatternType::Code, "learn-2", PatternSource::Manual),
        ];

        registry.learn_patterns(&patterns);

        assert_eq!(registry.len(), initial_count + 2);
    }

    #[test]
    fn test_learn_patterns_empty() {
        let mut registry = PatternRegistry::new();
        let initial_count = registry.len();

        registry.learn_patterns(&[]);

        assert_eq!(registry.len(), initial_count);
    }

    // =========================================================================
    // Statistics Tests
    // =========================================================================

    #[test]
    fn test_stats_empty() {
        let registry = PatternRegistry::new();
        let stats = registry.stats();

        assert_eq!(stats.total, 0);
        assert_eq!(stats.presets, 0);
        assert_eq!(stats.active, 0);
        assert_eq!(stats.inactive, 0);
        assert_eq!(stats.reference_count, 0);
        assert_eq!(stats.code_count, 0);
    }

    #[test]
    fn test_stats_with_manual_patterns() {
        let mut registry = PatternRegistry::new();

        registry.add_pattern(ConversationPattern::manual(PatternType::Code, "manual-1"));
        registry.add_pattern(ConversationPattern::manual(PatternType::Reference, "manual-2"));

        let stats = registry.stats();
        assert_eq!(stats.total, 2);
        assert_eq!(stats.manual, 2);
        assert_eq!(stats.presets, 0);
    }

    #[test]
    fn test_stats_display() {
        let registry = PatternRegistry::new();
        let stats = registry.stats();
        let display = format!("{}", stats);

        assert!(display.contains("Pattern Registry Stats"));
        assert!(display.contains("Total:"));
        assert!(display.contains("active:"));
        assert!(display.contains("Reference:"));
        assert!(display.contains("Code:"));
    }

    // =========================================================================
    // Serialization Tests
    // =========================================================================

    #[test]
    fn test_serialization() {
        let registry = PatternRegistry::new();
        let json = serde_json::to_string(&registry).unwrap();
        let decoded: PatternRegistry = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.len(), registry.len());
        assert_eq!(
            decoded.count_by_type(PatternType::Reference),
            registry.count_by_type(PatternType::Reference)
        );
    }

    #[test]
    fn test_serialization_preserves_patterns() {
        let mut registry = PatternRegistry::new();

        let pattern = ConversationPattern::manual(PatternType::Code, "serialize-test")
            .with_description("Test serialization")
            .with_tag("test");
        let pattern_id = pattern.id.clone();
        registry.add_pattern(pattern);

        let json = serde_json::to_string(&registry).unwrap();
        let decoded: PatternRegistry = serde_json::from_str(&json).unwrap();

        let found = decoded.get_pattern(&pattern_id).unwrap();
        assert_eq!(found.pattern, "serialize-test");
        assert_eq!(found.description, Some("Test serialization".to_string()));
        assert_eq!(found.tags, vec!["test"]);
    }

    #[test]
    fn test_serialization_config_preserved() {
        let config = PatternRegistryConfig::minimal();
        let registry = PatternRegistry::with_config(config);

        let json = serde_json::to_string(&registry).unwrap();
        let decoded: PatternRegistry = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.config.max_patterns_per_type, 50);
        assert_eq!(decoded.config.min_confidence_threshold, 0.5);
    }

    // =========================================================================
    // Prune Tests
    // =========================================================================

    #[test]
    fn test_prune_keeps_active_patterns() {
        let mut registry = PatternRegistry::new();

        let pattern = ConversationPattern::manual(PatternType::Code, "active-pattern");
        registry.add_pattern(pattern);

        let len_before = registry.len();
        registry.prune();
        let len_after = registry.len();

        // Active patterns should be kept
        assert_eq!(len_after, len_before);
    }

    #[test]
    fn test_prune_keeps_manual_patterns() {
        let mut registry = PatternRegistry::new();

        let pattern = ConversationPattern::manual(PatternType::Code, "manual-pattern");
        registry.add_pattern(pattern);

        let manual_count_before = registry.patterns.iter().filter(|p| p.source.is_manual()).count();
        registry.prune();
        let manual_count_after = registry.patterns.iter().filter(|p| p.source.is_manual()).count();

        assert_eq!(manual_count_after, manual_count_before);
    }

    // =========================================================================
    // Length and Empty Tests
    // =========================================================================

    #[test]
    fn test_len_and_is_empty() {
        let registry = PatternRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_count_by_type_empty() {
        let registry = PatternRegistry::new();

        let ref_count = registry.count_by_type(PatternType::Reference);
        let code_count = registry.count_by_type(PatternType::Code);

        assert_eq!(ref_count, 0);
        assert_eq!(code_count, 0);
        assert_eq!(ref_count + code_count, registry.len());
    }

    #[test]
    fn test_count_by_type_with_patterns() {
        let mut registry = PatternRegistry::new();
        registry.add_pattern(ConversationPattern::manual(PatternType::Reference, "ref-1"));
        registry.add_pattern(ConversationPattern::manual(PatternType::Code, "code-1"));

        assert_eq!(registry.count_by_type(PatternType::Reference), 1);
        assert_eq!(registry.count_by_type(PatternType::Code), 1);
    }

    // =========================================================================
    // Edge Cases and Boundary Tests
    // =========================================================================

    #[test]
    fn test_config_custom() {
        let config = PatternRegistryConfig::with_max_patterns(50);
        let registry = PatternRegistry::with_config(config);

        assert_eq!(registry.config.max_patterns_per_type, 50);
    }

    #[test]
    fn test_add_pattern_same_pattern_different_types() {
        let mut registry = PatternRegistry::new();
        assert!(registry.is_empty());

        // Same pattern string but different types
        // Note: The current implementation checks duplicates by pattern string only,
        // so the second pattern with the same string won't be added regardless of type
        let p1 = ConversationPattern::new(PatternType::Reference, "same-text", PatternSource::Manual);
        let p2 = ConversationPattern::new(PatternType::Code, "same-text", PatternSource::Manual);

        registry.add_pattern(p1);
        registry.add_pattern(p2);

        // Only one should be added since duplicates are detected by pattern string
        assert_eq!(registry.len(), 1);
        // The first pattern (Reference type) should be the one added
        assert!(registry.all_patterns().iter().any(|p| p.pattern == "same-text" && p.pattern_type == PatternType::Reference));
    }

    #[test]
    fn test_get_active_patterns_includes_only_active() {
        let mut registry = PatternRegistry::new();

        let active_pattern = ConversationPattern::manual(PatternType::Code, "active-test");
        let active_id = active_pattern.id.clone();

        let mut inactive_pattern = ConversationPattern::manual(PatternType::Code, "inactive-test");
        inactive_pattern.deactivate();
        let inactive_id = inactive_pattern.id.clone();

        registry.add_pattern(active_pattern);
        registry.add_pattern(inactive_pattern);

        let active_patterns = registry.get_active_patterns_by_type(PatternType::Code);

        assert!(active_patterns.iter().any(|p| p.id == active_id));
        assert!(!active_patterns.iter().any(|p| p.id == inactive_id));
    }

    #[test]
    fn test_type_index_rebuild_on_remove() {
        let mut registry = PatternRegistry::new();

        let pattern = ConversationPattern::manual(PatternType::Code, "test-index");
        let id = pattern.id.clone();
        registry.add_pattern(pattern);

        // Verify pattern exists
        assert!(registry.get_pattern(&id).is_some());

        // Remove and verify index is updated
        registry.remove_pattern(&id);
        assert!(registry.get_pattern(&id).is_none());

        // Index should still be valid for other operations
        let active = registry.get_active_patterns_by_type(PatternType::Code);
        assert!(!active.iter().any(|p| p.id == id));
    }

    #[test]
    fn test_deactivate_then_activate_affects_active_count() {
        let mut registry = PatternRegistry::new();

        let pattern = ConversationPattern::manual(PatternType::Code, "toggle-test");
        let id = pattern.id.clone();
        registry.add_pattern(pattern);

        let initial_active = registry.active_count_by_type(PatternType::Code);

        registry.deactivate_pattern(&id);
        assert_eq!(registry.active_count_by_type(PatternType::Code), initial_active - 1);

        registry.activate_pattern(&id);
        assert_eq!(registry.active_count_by_type(PatternType::Code), initial_active);
    }

    #[test]
    fn test_pattern_order_preserved_after_multiple_operations() {
        let mut registry = PatternRegistry::new();

        let p1 = ConversationPattern::manual(PatternType::Reference, "order-1");
        let p2 = ConversationPattern::manual(PatternType::Code, "order-2");
        let p3 = ConversationPattern::manual(PatternType::Reference, "order-3");

        registry.add_patterns(vec![p1, p2, p3]);

        // All should be present
        assert!(registry.all_patterns().iter().any(|p| p.pattern == "order-1"));
        assert!(registry.all_patterns().iter().any(|p| p.pattern == "order-2"));
        assert!(registry.all_patterns().iter().any(|p| p.pattern == "order-3"));
    }

    // =========================================================================
    // File Storage Tests
    // =========================================================================

    #[test]
    fn test_get_patterns_file_path() {
        let path = PatternRegistry::get_patterns_file_path();
        assert!(path.is_ok());

        let path = path.unwrap();
        // Should end with .matrix/patterns.json
        assert!(path.to_string_lossy().contains(".matrix"));
        assert!(path.to_string_lossy().contains("patterns.json"));
    }

    #[test]
    fn test_from_file_nonexistent() {
        let temp_dir = tempfile::tempdir().unwrap();
        let nonexistent_path = temp_dir.path().join("nonexistent_patterns.json");

        let registry = PatternRegistry::from_file(&nonexistent_path).unwrap();

        // Should be empty - no presets loaded
        assert!(registry.is_empty());
        assert_eq!(registry.count_by_type(PatternType::Reference), 0);
        assert_eq!(registry.count_by_type(PatternType::Code), 0);
    }

    #[test]
    fn test_from_file_empty_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let empty_path = temp_dir.path().join("empty_patterns.json");

        // Create empty file
        fs::write(&empty_path, "").unwrap();

        let registry = PatternRegistry::from_file(&empty_path).unwrap();

        // Should be empty - no presets loaded
        assert!(registry.is_empty());
    }

    #[test]
    fn test_from_file_valid_json() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("valid_patterns.json");

        // Create a registry and save it
        let mut original = PatternRegistry::new();
        original.add_pattern(
            ConversationPattern::manual(PatternType::Code, "custom-pattern")
                .with_description("Custom test pattern"),
        );
        original.save_to_file(&file_path).unwrap();

        // Load it back
        let loaded = PatternRegistry::from_file(&file_path).unwrap();

        // Should have the custom pattern
        assert!(loaded.patterns.iter().any(|p| p.pattern == "custom-pattern"));
        assert!(loaded.patterns.iter().any(|p| p.description == Some("Custom test pattern".to_string())));
    }

    #[test]
    fn test_from_file_malformed_json() {
        let temp_dir = tempfile::tempdir().unwrap();
        let malformed_path = temp_dir.path().join("malformed_patterns.json");

        // Create file with malformed JSON
        fs::write(&malformed_path, "{ not valid json }").unwrap();

        // Should return empty registry (logged warning)
        let registry = PatternRegistry::from_file(&malformed_path).unwrap();

        assert!(registry.is_empty());
    }

    #[test]
    fn test_save_to_file_creates_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let nested_path = temp_dir.path().join("nested").join("dir").join("patterns.json");

        // Nested directory doesn't exist
        assert!(!nested_path.parent().unwrap().exists());

        let registry = PatternRegistry::new();
        registry.save_to_file(&nested_path).unwrap();

        // Directory and file should now exist
        assert!(nested_path.parent().unwrap().exists());
        assert!(nested_path.exists());
    }

    #[test]
    fn test_save_to_file_roundtrip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("roundtrip_patterns.json");

        // Create registry with custom patterns
        let mut original = PatternRegistry::new();
        let p1 = ConversationPattern::manual(PatternType::Reference, "roundtrip-ref")
            .with_description("Reference pattern for roundtrip test")
            .with_tag("test");
        let p1_id = p1.id.clone();
        original.add_pattern(p1);

        let p2 = ConversationPattern::manual(PatternType::Code, "roundtrip-code")
            .with_description("Code pattern for roundtrip test");
        let p2_id = p2.id.clone();
        original.add_pattern(p2);

        // Save
        original.save_to_file(&file_path).unwrap();

        // Load
        let loaded = PatternRegistry::from_file(&file_path).unwrap();

        // Verify patterns preserved
        let loaded_p1 = loaded.get_pattern(&p1_id).unwrap();
        assert_eq!(loaded_p1.pattern, "roundtrip-ref");
        assert_eq!(loaded_p1.pattern_type, PatternType::Reference);
        assert_eq!(loaded_p1.description, Some("Reference pattern for roundtrip test".to_string()));
        assert_eq!(loaded_p1.tags, vec!["test"]);

        let loaded_p2 = loaded.get_pattern(&p2_id).unwrap();
        assert_eq!(loaded_p2.pattern, "roundtrip-code");
        assert_eq!(loaded_p2.pattern_type, PatternType::Code);
    }

    #[test]
    fn test_save_to_file_preserves_config() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("config_patterns.json");

        // Create registry with custom config
        let config = PatternRegistryConfig::minimal();
        let original = PatternRegistry::with_config(config);

        original.save_to_file(&file_path).unwrap();

        let loaded = PatternRegistry::from_file(&file_path).unwrap();

        assert_eq!(loaded.config.max_patterns_per_type, 50);
        assert_eq!(loaded.config.min_confidence_threshold, 0.5);
        assert_eq!(loaded.config.min_frequency, 3);
    }

    #[test]
    fn test_from_default_file() {
        // This test verifies the method works
        // The default file may or may not exist, so we just verify the method doesn't error
        let result = PatternRegistry::from_default_file();
        assert!(result.is_ok());
    }

    #[test]
    fn test_file_storage_integration() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("integration_patterns.json");

        // Start with empty registry
        let mut registry = PatternRegistry::from_file(&file_path).unwrap();
        assert!(registry.is_empty());

        // Add custom patterns
        let custom = ConversationPattern::manual(PatternType::Code, "integration-test")
            .with_description("Integration test pattern")
            .with_tag("integration");
        registry.add_pattern(custom);

        // Save
        registry.save_to_file(&file_path).unwrap();

        // Load again
        let reloaded = PatternRegistry::from_file(&file_path).unwrap();

        // Custom pattern should be present
        assert!(reloaded.patterns.iter().any(|p| p.pattern == "integration-test"));
        assert!(reloaded.patterns.iter().any(|p| p.tags.contains(&"integration".to_string())));
    }

    // =========================================================================
    // Boundary Tests for File Storage
    // =========================================================================

    #[test]
    fn test_large_patterns_save_and_load() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("large_patterns.json");

        // Create empty registry and add patterns
        let mut registry = PatternRegistry::new();

        // Add 50 patterns of each type
        for i in 0..50 {
            let ref_pattern = ConversationPattern::manual(PatternType::Reference, &format!("large-ref-{}", i))
                .with_description(&format!("Reference pattern {}", i))
                .with_tag(&format!("tag{}", i % 5));
            registry.add_pattern(ref_pattern);

            let code_pattern = ConversationPattern::manual(PatternType::Code, &format!("large-code-{}", i))
                .with_description(&format!("Code pattern {}", i))
                .with_tag(&format!("codetag{}", i % 3));
            registry.add_pattern(code_pattern);
        }

        let expected_count = 100;
        assert_eq!(registry.len(), expected_count);

        // Save
        registry.save_to_file(&file_path).unwrap();

        // Verify file was created and has reasonable size
        let metadata = fs::metadata(&file_path).unwrap();
        assert!(metadata.len() > 1000); // Should be substantial

        // Load
        let loaded = PatternRegistry::from_file(&file_path).unwrap();

        // Verify all patterns preserved
        assert_eq!(loaded.len(), expected_count);

        // Verify some specific patterns
        for i in 0..50 {
            assert!(
                loaded.patterns.iter().any(|p| p.pattern == format!("large-ref-{}", i)),
                "Missing large-ref-{}",
                i
            );
            assert!(
                loaded.patterns.iter().any(|p| p.pattern == format!("large-code-{}", i)),
                "Missing large-code-{}",
                i
            );
        }
    }

    #[test]
    fn test_special_characters_in_patterns() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("special_chars_patterns.json");

        // Test various special characters
        let special_patterns = vec![
            // Unicode characters
            ("unicode-你好", "Chinese characters"),
            ("unicode-日本語", "Japanese characters"),
            ("unicode-한국어", "Korean characters"),
            ("unicode-مرحبا", "Arabic characters"),
            ("unicode-🎉🔥", "Emoji"),
            // Regex special characters (should be preserved literally)
            ("regex-\\d+\\.\\w*", "Regex pattern with escapes"),
            ("regex-[a-z]+", "Regex character class"),
            ("regex-(foo|bar)", "Regex alternation"),
            // Special JSON characters
            ("json-\"quotes\"", "Contains quotes"),
            ("json-\\n\\t", "Escape sequences"),
            ("json-日本語\"test\"", "Mixed special chars"),
            // Whitespace and control characters
            ("whitespace-tab\ttab", "Contains tab"),
            ("whitespace-newline\nline", "Contains newline"),
        ];

        let mut registry = PatternRegistry::new();
        let mut added_ids = Vec::new();

        for (pattern_str, desc) in &special_patterns {
            let pattern = ConversationPattern::manual(PatternType::Code, *pattern_str)
                .with_description(*desc);
            let id = pattern.id.clone();
            registry.add_pattern(pattern);
            added_ids.push((id, *pattern_str, *desc));
        }

        // Save
        registry.save_to_file(&file_path).unwrap();

        // Load
        let loaded = PatternRegistry::from_file(&file_path).unwrap();

        // Verify all special patterns preserved
        for (id, pattern_str, desc) in &added_ids {
            let found = loaded.get_pattern(id);
            assert!(found.is_some(), "Pattern {} not found after reload", pattern_str);
            let p = found.unwrap();
            assert_eq!(p.pattern, *pattern_str, "Pattern mismatch for {}", pattern_str);
            assert_eq!(p.description, Some(desc.to_string()), "Description mismatch for {}", pattern_str);
        }
    }

    #[test]
    fn test_empty_registry_save_and_load() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("empty_registry.json");

        // Create empty registry
        let registry = PatternRegistry::new();
        assert!(registry.is_empty());

        // Save empty registry
        registry.save_to_file(&file_path).unwrap();

        // Load
        let loaded = PatternRegistry::from_file(&file_path).unwrap();

        // The loaded registry should be empty
        assert!(loaded.is_empty());
    }

    #[test]
    fn test_whitespace_only_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("whitespace_patterns.json");

        // Create file with only whitespace
        fs::write(&file_path, "   \n\t  \n  ").unwrap();

        let registry = PatternRegistry::from_file(&file_path).unwrap();

        // Should be empty (empty file handling)
        assert!(registry.is_empty());
    }

    #[test]
    fn test_save_to_default_file_path() {
        // Create a unique temp path to avoid conflicts
        let temp_dir = tempfile::tempdir().unwrap();
        let custom_path = temp_dir.path().join(".matrix").join("patterns.json");

        // Manually create and save
        let mut registry = PatternRegistry::new();
        let pattern = ConversationPattern::manual(PatternType::Code, "default-file-test")
            .with_description("Test save_to_file with custom path");
        registry.add_pattern(pattern);

        registry.save_to_file(&custom_path).unwrap();

        // Verify file exists
        assert!(custom_path.exists());

        // Load and verify
        let loaded = PatternRegistry::from_file(&custom_path).unwrap();
        assert!(loaded.patterns.iter().any(|p| p.pattern == "default-file-test"));
    }

    #[test]
    fn test_atomic_write_rollback_safety() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("atomic_test.json");

        // Create initial valid file
        let mut registry = PatternRegistry::new();
        registry.add_pattern(ConversationPattern::manual(PatternType::Code, "initial-pattern"));
        registry.save_to_file(&file_path).unwrap();

        // Verify temp file is cleaned up
        let tmp_path = file_path.with_extension("json.tmp");
        assert!(!tmp_path.exists(), "Temp file should not exist after save");

        // Verify the main file exists
        assert!(file_path.exists());

        // Load to verify content is valid
        let loaded = PatternRegistry::from_file(&file_path).unwrap();
        assert!(loaded.patterns.iter().any(|p| p.pattern == "initial-pattern"));
    }

    #[test]
    fn test_long_description_and_tags() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("long_content.json");

        // Create patterns with very long content
        let long_desc = "x".repeat(10000);
        let many_tags: Vec<String> = (0..100).map(|i| format!("tag-{}", i)).collect();

        let mut registry = PatternRegistry::new();
        let mut pattern = ConversationPattern::manual(PatternType::Code, "long-pattern");
        pattern.description = Some(long_desc.clone());
        pattern.tags = many_tags.clone();

        registry.add_pattern(pattern);

        // Save
        registry.save_to_file(&file_path).unwrap();

        // Load
        let loaded = PatternRegistry::from_file(&file_path).unwrap();

        // Verify long content preserved
        let found = loaded.patterns.iter().find(|p| p.pattern == "long-pattern").unwrap();
        assert_eq!(found.description, Some(long_desc));
        assert_eq!(found.tags.len(), 100);
        for i in 0..100 {
            assert!(found.tags.contains(&format!("tag-{}", i)));
        }
    }
}