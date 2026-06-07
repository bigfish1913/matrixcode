//! Memory system constants and configuration.

use serde::{Deserialize, Serialize};

// ============================================================================
// Constants
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

/// Default fast model for AI memory extraction.
pub const DEFAULT_FAST_MODEL: &str = "claude-3-5-haiku-20241022";

/// Default importance scores by category.
/// Lower values allow for gradual importance growth through references.
pub const DEFAULT_IMPORTANCE_DECISION: f64 = 75.0;
pub const DEFAULT_IMPORTANCE_SOLUTION: f64 = 70.0;
pub const DEFAULT_IMPORTANCE_PREF: f64 = 65.0;
pub const DEFAULT_IMPORTANCE_FINDING: f64 = 55.0;
pub const DEFAULT_IMPORTANCE_TECH: f64 = 45.0;
pub const DEFAULT_IMPORTANCE_STRUCTURE: f64 = 35.0;

// ============================================================================
// AI Modes
// ============================================================================

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
                log::warn!(
                    "Unknown MEMORY_AI_KEYWORDS value: '{}', using 'auto'",
                    other
                );
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
                log::warn!(
                    "Unknown MEMORY_AI_DETECTION value: '{}', using 'auto'",
                    other
                );
                AiDetectionMode::Auto
            }
        }
    }

    /// Whether AI detection should be used.
    pub fn should_use_ai(&self) -> bool {
        match self {
            AiDetectionMode::Always => true,
            AiDetectionMode::Never => false,
            AiDetectionMode::Auto => false, // Default to rule-based for speed
        }
    }

    /// Whether AI detection should be used for given text length.
    /// Longer texts benefit more from AI detection.
    pub fn should_use_ai_for_text(&self, text_len: usize) -> bool {
        match self {
            AiDetectionMode::Always => true,
            AiDetectionMode::Never => false,
            AiDetectionMode::Auto => text_len > 500,
        }
    }
}

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
            reference_increment: 1.0,
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
