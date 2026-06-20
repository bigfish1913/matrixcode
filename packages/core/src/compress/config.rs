//! Compression configuration and bias settings.

use anyhow::Result;

// ============================================================================
// Constants
// ============================================================================

/// Compression trigger threshold (percentage of context window).
/// Lowered to 0.5 to compress earlier for long conversations (128K context -> 64K threshold)
pub const DEFAULT_COMPRESSION_THRESHOLD: f64 = 0.5;

/// Minimum messages to keep after compression.
/// Increased to preserve more recent context for continuity
pub const MIN_MESSAGES_TO_KEEP: usize = 20;

/// Target ratio after compression (keep this fraction of tokens).
pub const DEFAULT_TARGET_RATIO: f64 = 0.4;

/// Default model for summarization.
pub const DEFAULT_COMPRESSOR_MODEL: &str = "claude-3-5-haiku-20241022";

// ============================================================================
// Circuit Breaker (NEW - from Claude Code)
// ============================================================================

/// Maximum consecutive compression failures before stopping retries.
/// Claude Code: "1,279 sessions had 50+ consecutive failures, wasting ~250K API calls/day"
pub const MAX_CONSECUTIVE_FAILURES: u32 = 3;

/// Token buffers for threshold levels (from Claude Code).
pub const AUTOCOMPACT_BUFFER_TOKENS: u32 = 13_000;
pub const WARNING_THRESHOLD_BUFFER_TOKENS: u32 = 20_000;
pub const ERROR_THRESHOLD_BUFFER_TOKENS: u32 = 20_000;
pub const MANUAL_COMPACT_BUFFER_TOKENS: u32 = 3_000;

/// Time-based microcompact threshold (minutes since last assistant message).
/// When gap exceeds this, server cache has expired - clear old tool results.
pub const TIME_BASED_MC_GAP_THRESHOLD_MINUTES: u32 = 5;

/// Message to replace cleared tool result content (from Claude Code).
pub const TIME_BASED_MC_CLEARED_MESSAGE: &str = "[Old tool result content cleared]";

// ============================================================================
// Threshold Levels (NEW)
// ============================================================================

/// Threshold level for compression warnings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThresholdLevel {
    /// Normal - no action needed
    Normal,
    /// Warning - approaching limit, warn user
    Warning,
    /// Error - near limit, strongly suggest compact
    Error,
    /// Blocking - must compact before continuing
    Blocking,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Format token count for display.
pub fn format_tokens(n: u32) -> String {
    if n < 1_000 {
        n.to_string()
    } else if n < 10_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{:.0}K", n as f64 / 1_000.0)
    }
}

// ============================================================================
// Compression Bias
// ============================================================================

/// Compression bias - controls what to prioritize during compression.
#[derive(Debug, Clone, Default)]
pub struct CompressionBias {
    /// Preserve tool calls and their results.
    pub preserve_tools: bool,
    /// Preserve thinking blocks.
    pub preserve_thinking: bool,
    /// Preserve user questions.
    pub preserve_user_questions: bool,
    /// Compact long outputs instead of removing.
    pub compact_long_outputs: bool,
    /// Aggressive mode - remove more content.
    pub aggressive: bool,
    /// Custom keywords to preserve.
    pub preserve_keywords: Vec<String>,
}

impl CompressionBias {
    /// Default bias - balanced preservation.
    pub fn balanced() -> Self {
        Self {
            preserve_tools: true,
            preserve_thinking: false,
            preserve_user_questions: true,
            compact_long_outputs: false,
            aggressive: false,
            preserve_keywords: vec![
                "决定".to_string(),
                "decision".to_string(),
                "重要".to_string(),
                "important".to_string(),
                "关键".to_string(),
                "key".to_string(),
            ],
        }
    }

    /// Preserve all important content.
    pub fn preserve_important() -> Self {
        Self {
            preserve_tools: true,
            preserve_thinking: true,
            preserve_user_questions: true,
            compact_long_outputs: true,
            aggressive: false,
            preserve_keywords: vec![
                "决定".to_string(),
                "decision".to_string(),
                "重要".to_string(),
                "important".to_string(),
                "关键".to_string(),
                "key".to_string(),
                "完成".to_string(),
                "done".to_string(),
                "成功".to_string(),
                "success".to_string(),
            ],
        }
    }

    /// Aggressive compression.
    pub fn aggressive() -> Self {
        Self {
            preserve_tools: false,
            preserve_thinking: false,
            preserve_user_questions: false,
            compact_long_outputs: false,
            aggressive: true,
            preserve_keywords: vec![],
        }
    }

    /// Focus on preserving tool operations.
    pub fn tool_focused() -> Self {
        Self {
            preserve_tools: true,
            preserve_thinking: false,
            preserve_user_questions: false,
            compact_long_outputs: false,
            aggressive: false,
            preserve_keywords: vec![
                "工具".to_string(),
                "tool".to_string(),
                "执行".to_string(),
                "execute".to_string(),
                "文件".to_string(),
                "file".to_string(),
            ],
        }
    }

    /// Parse bias from a string specification.
    pub fn parse(spec: &str) -> Result<Self> {
        let spec = spec.trim().to_lowercase();

        if spec == "balanced" || spec == "default" || spec.is_empty() {
            return Ok(Self::balanced());
        }
        if spec == "aggressive" {
            return Ok(Self::aggressive());
        }
        if spec == "preserve_important" || spec == "important" {
            return Ok(Self::preserve_important());
        }
        if spec == "tool_focused" || spec == "tools" {
            return Ok(Self::tool_focused());
        }

        // Parse custom specification
        let mut bias = Self::default();

        for part in spec.split_whitespace() {
            if let Some(preserve_list) = part.strip_prefix("preserve:") {
                for item in preserve_list.split(',') {
                    match item.trim() {
                        "tools" | "tool" => bias.preserve_tools = true,
                        "thinking" | "think" => bias.preserve_thinking = true,
                        "user" | "questions" => bias.preserve_user_questions = true,
                        "compact" | "long" => bias.compact_long_outputs = true,
                        _ => {}
                    }
                }
            } else if let Some(keyword_list) = part.strip_prefix("keywords:") {
                bias.preserve_keywords = keyword_list
                    .split(',')
                    .map(|k| k.trim().to_string())
                    .filter(|k| !k.is_empty())
                    .collect();
            } else if part == "aggressive" {
                bias.aggressive = true;
            }
        }

        Ok(bias)
    }

    /// Format bias for display.
    pub fn format(&self) -> String {
        let mut parts: Vec<String> = Vec::new();

        if self.preserve_tools {
            parts.push("tools".to_string());
        }
        if self.preserve_thinking {
            parts.push("thinking".to_string());
        }
        if self.preserve_user_questions {
            parts.push("user".to_string());
        }
        if self.compact_long_outputs {
            parts.push("compact".to_string());
        }
        if self.aggressive {
            parts.push("aggressive".to_string());
        }

        if !self.preserve_keywords.is_empty() {
            parts.push(format!("keywords:{}", self.preserve_keywords.join(",")));
        }

        if parts.is_empty() {
            "default".to_string()
        } else {
            parts.join(", ")
        }
    }
}

// ============================================================================
// Compression Configuration
// ============================================================================

/// Configuration for context compression.
#[derive(Debug, Clone)]
pub struct CompressionConfig {
    /// Threshold (0.0-1.0) at which to trigger compression.
    pub threshold: f64,
    /// Maximum tokens to target after compression.
    pub target_ratio: f64,
    /// Minimum recent messages to always preserve.
    pub min_preserve_messages: usize,
    /// Whether to use AI summarization.
    pub use_summarization: bool,
    /// Optional model name for summarization.
    pub compressor_model: Option<String>,
    /// Compression bias.
    pub bias: CompressionBias,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            threshold: DEFAULT_COMPRESSION_THRESHOLD,
            target_ratio: DEFAULT_TARGET_RATIO,
            min_preserve_messages: MIN_MESSAGES_TO_KEEP,
            use_summarization: true,
            compressor_model: None,
            bias: CompressionBias::balanced(),
        }
    }
}

impl CompressionConfig {
    /// Get the compressor model name.
    pub fn compressor_model_name(&self) -> &str {
        self.compressor_model
            .as_deref()
            .unwrap_or(DEFAULT_COMPRESSOR_MODEL)
    }

    /// Calculate threshold level based on token usage.
    /// Returns the level and percentage of context remaining.
    pub fn calculate_threshold_level(
        token_usage: u32,
        context_window: u32,
    ) -> (ThresholdLevel, u32) {
        let percent_left = if context_window > 0 {
            // 使用 saturating_sub 防止下溢，确保百分比在 0-100 范围内
            let remaining = context_window.saturating_sub(token_usage);
            ((remaining as f64 / context_window as f64 * 100.0) as u32).min(100)
        } else {
            0
        };

        // Calculate thresholds
        let auto_threshold = context_window.saturating_sub(AUTOCOMPACT_BUFFER_TOKENS);
        let warning_threshold = auto_threshold.saturating_sub(WARNING_THRESHOLD_BUFFER_TOKENS);
        let error_threshold = auto_threshold.saturating_sub(ERROR_THRESHOLD_BUFFER_TOKENS);
        let blocking_threshold = context_window.saturating_sub(MANUAL_COMPACT_BUFFER_TOKENS);

        let level = if token_usage >= blocking_threshold {
            ThresholdLevel::Blocking
        } else if token_usage >= error_threshold {
            ThresholdLevel::Error
        } else if token_usage >= warning_threshold {
            ThresholdLevel::Warning
        } else {
            ThresholdLevel::Normal
        };

        (level, percent_left)
    }
}

// ============================================================================
// Circuit Breaker State (NEW)
// ============================================================================

/// State for circuit breaker to prevent infinite retry loops.
#[derive(Debug, Clone, Default)]
pub struct CircuitBreakerState {
    /// Number of consecutive compression failures.
    pub consecutive_failures: u32,
    /// Whether circuit breaker has tripped.
    pub is_tripped: bool,
    /// Last failure timestamp (for reset timeout).
    pub last_failure_time: Option<u64>,
}

impl CircuitBreakerState {
    /// Create a new circuit breaker state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a failure. Returns true if circuit breaker should trip.
    pub fn record_failure(&mut self) -> bool {
        self.consecutive_failures += 1;
        self.last_failure_time = Some(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs());

        if self.consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
            self.is_tripped = true;
            return true;
        }
        false
    }

    /// Record a success. Resets failure count.
    pub fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.is_tripped = false;
        self.last_failure_time = None;
    }

    /// Check if compression should be skipped due to circuit breaker.
    pub fn should_skip(&self) -> bool {
        self.is_tripped
    }

    /// Reset the circuit breaker (manual override).
    pub fn reset(&mut self) {
        self.consecutive_failures = 0;
        self.is_tripped = false;
        self.last_failure_time = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_threshold_level_normal() {
        // 正常情况：使用了 50% 的上下文
        let (level, percent) = CompressionConfig::calculate_threshold_level(50_000, 100_000);
        assert_eq!(level, ThresholdLevel::Normal);
        assert_eq!(percent, 50);
    }

    #[test]
    fn test_calculate_threshold_level_exceeds_window() {
        // 关键测试：token_usage 超过 context_window
        // 修复前会因 u32 下溢产生巨大值，修复后应为 0
        let (level, percent) = CompressionConfig::calculate_threshold_level(120_000, 100_000);
        assert_eq!(level, ThresholdLevel::Blocking);
        assert_eq!(percent, 0, "百分比应为 0，不应该超过 100%");
    }

    #[test]
    fn test_calculate_threshold_level_full_usage() {
        // 完全用满上下文
        let (level, percent) = CompressionConfig::calculate_threshold_level(100_000, 100_000);
        assert_eq!(level, ThresholdLevel::Blocking);
        assert_eq!(percent, 0);
    }

    #[test]
    fn test_calculate_threshold_level_zero_window() {
        // 边界情况：context_window 为 0，意味着没有可用空间
        // 所有阈值都会变成 0，因此任何 token_usage > 0 都触发 Blocking
        let (level, percent) = CompressionConfig::calculate_threshold_level(1000, 0);
        assert_eq!(level, ThresholdLevel::Blocking);  // 0 空间时应该阻止
        assert_eq!(percent, 0);
    }

    #[test]
    fn test_calculate_threshold_level_small_remaining() {
        // 接近上限但未超过
        let (_level, percent) = CompressionConfig::calculate_threshold_level(99_000, 100_000);
        assert_eq!(percent, 1);
    }
}
