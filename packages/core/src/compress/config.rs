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
}
