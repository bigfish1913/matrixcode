//! Unified registry for learning from extraction results.
//!
//! This module provides a unified interface to learn from extraction
//! results, updating multiple registries in a single operation.

use anyhow::Result;

use super::unified_extraction::UnifiedExtractionResult;
use super::pattern_registry::PatternRegistry;
use super::focus_keywords_registry::FocusKeywordsRegistry;

/// Unified registry that manages pattern and keyword registries together.
///
/// Provides a single interface to learn from unified extraction results,
/// updating multiple registries and saving them atomically.
pub struct UnifiedRegistry {
    /// Pattern registry for conversation patterns.
    pattern_registry: PatternRegistry,
    /// Keywords registry for focus tracking keywords.
    keywords_registry: FocusKeywordsRegistry,
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
            keywords_registry: FocusKeywordsRegistry::new(),
        }
    }

    /// Create a unified registry from existing registries.
    pub fn from_registries(
        pattern_registry: PatternRegistry,
        keywords_registry: FocusKeywordsRegistry,
    ) -> Self {
        Self {
            pattern_registry,
            keywords_registry,
        }
    }

    /// Load unified registry from default file paths.
    ///
    /// If files don't exist, creates registries with presets loaded.
    pub fn load_or_default() -> Result<Self> {
        let pattern_registry = PatternRegistry::from_default_file()?;
        let keywords_registry = FocusKeywordsRegistry::from_default_file()?;
        Ok(Self {
            pattern_registry,
            keywords_registry,
        })
    }

    /// Learn from a unified extraction result.
    ///
    /// Updates both pattern and keyword registries with extracted data.
    pub fn learn_from_extraction(&mut self, result: &UnifiedExtractionResult, session_id: &str) {
        // Learn conversation patterns
        if !result.conversation_patterns.is_empty() {
            self.pattern_registry.learn_patterns(&result.conversation_patterns);
        }

        // Learn focus keywords
        if !result.focus_keywords.is_empty() {
            let keyword_pairs = result.focus_keywords.to_keyword_pairs();
            let keywords: Vec<_> = keyword_pairs
                .iter()
                .map(|(k, c)| (k.as_str(), *c))
                .collect();
            self.keywords_registry.learn_keywords(&keywords, session_id);
        }
    }

    /// Save all registries to their default file paths.
    pub fn save_all(&self) -> Result<()> {
        self.pattern_registry.save_to_default_file()?;
        self.keywords_registry.save_to_default_file()?;
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

    /// Get reference to keywords registry.
    pub fn keywords_registry(&self) -> &FocusKeywordsRegistry {
        &self.keywords_registry
    }

    /// Get mutable reference to keywords registry.
    pub fn keywords_registry_mut(&mut self) -> &mut FocusKeywordsRegistry {
        &mut self.keywords_registry
    }

    /// Prune all registries (remove inactive/old entries).
    pub fn prune(&mut self) {
        self.pattern_registry.prune();
        self.keywords_registry.prune();
    }

    /// Get combined statistics.
    pub fn stats(&self) -> UnifiedRegistryStats {
        let pattern_stats = self.pattern_registry.stats();
        let keyword_stats = self.keywords_registry.stats();

        UnifiedRegistryStats {
            total_patterns: pattern_stats.total,
            active_patterns: pattern_stats.active,
            total_keywords: keyword_stats.total,
            active_keywords: keyword_stats.active,
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
    /// Total keyword count.
    pub total_keywords: usize,
    /// Active keyword count.
    pub active_keywords: usize,
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
        )?;
        writeln!(
            f,
            "  Keywords: {} (active: {})",
            self.total_keywords, self.active_keywords
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unified_registry_new() {
        let registry = UnifiedRegistry::new();
        assert!(!registry.pattern_registry().is_empty());
        assert!(!registry.keywords_registry().is_empty());
    }

    #[test]
    fn test_unified_registry_default() {
        let registry = UnifiedRegistry::default();
        assert!(!registry.pattern_registry().is_empty());
    }

    #[test]
    fn test_unified_registry_stats() {
        let registry = UnifiedRegistry::new();
        let stats = registry.stats();

        assert!(stats.total_patterns > 0);
        assert!(stats.total_keywords > 0);
    }

    #[test]
    fn test_unified_registry_stats_display() {
        let registry = UnifiedRegistry::new();
        let stats = registry.stats();
        let display = format!("{}", stats);

        assert!(display.contains("Unified Registry Stats"));
        assert!(display.contains("Patterns:"));
        assert!(display.contains("Keywords:"));
    }

    #[test]
    fn test_unified_registry_prune() {
        let mut registry = UnifiedRegistry::new();
        registry.prune();

        // After prune, presets should still be present
        assert!(!registry.pattern_registry().is_empty());
        assert!(!registry.keywords_registry().is_empty());
    }

    #[test]
    fn test_unified_registry_mut_accessors() {
        let mut registry = UnifiedRegistry::new();

        // Test mutable accessors work
        let patterns = registry.pattern_registry_mut();
        assert!(!patterns.is_empty());

        let keywords = registry.keywords_registry_mut();
        assert!(!keywords.is_empty());
    }
}