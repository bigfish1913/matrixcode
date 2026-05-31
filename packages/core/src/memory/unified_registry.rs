//! Unified registry for learning from extraction results.
//!
//! This module provides a unified interface to learn from extraction
//! results, updating pattern registry for persistent learning.

use anyhow::Result;

use super::unified_extraction::UnifiedExtractionResult;
use super::pattern_registry::PatternRegistry;

/// Unified registry that manages pattern registry.
///
/// Provides a single interface to learn from unified extraction results,
/// updating pattern registry and saving them atomically.
/// Note: Keywords are now handled in real-time via FocusTrackerConfig,
/// not persisted in the registry.
pub struct UnifiedRegistry {
    /// Pattern registry for conversation patterns.
    pattern_registry: PatternRegistry,
}

impl Default for UnifiedRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl UnifiedRegistry {
    /// Create a new unified registry with default registries.
    pub fn new() -> Self {
        Self {
            pattern_registry: PatternRegistry::new(),
        }
    }

    /// Create a unified registry from existing pattern registry.
    pub fn from_pattern_registry(pattern_registry: PatternRegistry) -> Self {
        Self {
            pattern_registry,
        }
    }

    /// Load unified registry from default file paths.
    ///
    /// If files don't exist, creates registries with presets loaded.
    pub fn load_or_default() -> Result<Self> {
        let pattern_registry = PatternRegistry::from_default_file()?;
        Ok(Self {
            pattern_registry,
        })
    }

    /// Learn from a unified extraction result.
    ///
    /// Updates pattern registry with extracted data.
    /// Note: Keywords are no longer persisted; they are used in real-time.
    pub fn learn_from_extraction(&mut self, result: &UnifiedExtractionResult, _session_id: &str) {
        // Learn conversation patterns
        if !result.conversation_patterns.is_empty() {
            self.pattern_registry.learn_patterns(&result.conversation_patterns);
        }

        // Keywords are now handled in real-time via FocusTrackerConfig,
        // not persisted in the registry
    }

    /// Save all registries to their default file paths.
    pub fn save_all(&self) -> Result<()> {
        self.pattern_registry.save_to_default_file()?;
        Ok(())
    }

    /// Get reference to pattern registry.
    pub fn pattern_registry(&self) -> &PatternRegistry {
        &self.pattern_registry
    }

    /// Get mutable reference to pattern registry.
    pub fn pattern_registry_mut(&mut self) -> &mut PatternRegistry {
        &mut self.pattern_registry
    }

    /// Prune pattern registry (remove inactive/old entries).
    pub fn prune(&mut self) {
        self.pattern_registry.prune();
    }

    /// Get combined statistics.
    pub fn stats(&self) -> UnifiedRegistryStats {
        let pattern_stats = self.pattern_registry.stats();

        UnifiedRegistryStats {
            total_patterns: pattern_stats.total,
            active_patterns: pattern_stats.active,
        }
    }
}

/// Combined statistics for unified registry.
#[derive(Debug, Clone)]
pub struct UnifiedRegistryStats {
    /// Total pattern count.
    pub total_patterns: usize,
    /// Active pattern count.
    pub active_patterns: usize,
}

impl std::fmt::Display for UnifiedRegistryStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "Unified Registry Stats:"
        )?;
        writeln!(
            f,
            "  Patterns: {} (active: {})",
            self.total_patterns, self.active_patterns
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unified_registry_new() {
        let registry = UnifiedRegistry::new();
        // PatternRegistry is now empty - no presets loaded
        assert!(registry.pattern_registry().is_empty());
    }

    #[test]
    fn test_unified_registry_default() {
        let registry = UnifiedRegistry::default();
        // PatternRegistry is now empty - no presets loaded
        assert!(registry.pattern_registry().is_empty());
    }

    #[test]
    fn test_unified_registry_stats() {
        let registry = UnifiedRegistry::new();
        let stats = registry.stats();

        // PatternRegistry is empty, so total_patterns should be 0
        assert_eq!(stats.total_patterns, 0);
    }

    #[test]
    fn test_unified_registry_stats_display() {
        let registry = UnifiedRegistry::new();
        let stats = registry.stats();
        let display = format!("{}", stats);

        assert!(display.contains("Unified Registry Stats"));
        assert!(display.contains("Patterns:"));
    }

    #[test]
    fn test_unified_registry_prune() {
        let mut registry = UnifiedRegistry::new();
        // Add a pattern first
        registry.pattern_registry_mut().add_pattern(
            crate::memory::ConversationPattern::manual(
                crate::memory::PatternType::Code,
                "test-pattern",
            ),
        );

        registry.prune();

        // After prune, manually added patterns should still be present
        assert!(!registry.pattern_registry().is_empty());
    }

    #[test]
    fn test_unified_registry_mut_accessors() {
        let mut registry = UnifiedRegistry::new();

        // PatternRegistry is now empty - no presets loaded
        let patterns = registry.pattern_registry_mut();
        assert!(patterns.is_empty());

        // Test mutable accessor works - add a pattern
        registry.pattern_registry_mut().add_pattern(
            crate::memory::ConversationPattern::manual(
                crate::memory::PatternType::Code,
                "test-pattern",
            ),
        );
        assert!(!registry.pattern_registry().is_empty());
    }
}