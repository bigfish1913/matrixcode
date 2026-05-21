//! Compression types and result structures.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::providers::{Message, MessageContent, Role};

use super::config::format_tokens;

// ============================================================================
// Compression Strategy
// ============================================================================

/// Strategy for compressing conversation history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompressionStrategy {
    /// Remove oldest messages, keep recent ones.
    Truncate,
    /// Use sliding window.
    SlidingWindow,
    /// Summarize old messages.
    Summarize,
    /// Use bias-based scoring.
    BiasBased,
}

// ============================================================================
// Compression Result
// ============================================================================

/// Result of a compression operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionResult {
    /// Original message count.
    pub original_count: usize,
    /// New message count.
    pub new_count: usize,
    /// Estimated token reduction.
    pub tokens_saved: u32,
    /// Summary of removed content.
    pub summary: Option<String>,
    /// Strategy used.
    pub strategy: CompressionStrategy,
    /// When the compression occurred.
    pub timestamp: DateTime<Utc>,
}

impl CompressionResult {
    /// Create a new compression result.
    pub fn new(
        original_count: usize,
        new_count: usize,
        tokens_saved: u32,
        summary: Option<String>,
        strategy: CompressionStrategy,
    ) -> Self {
        Self {
            original_count,
            new_count,
            tokens_saved,
            summary,
            strategy,
            timestamp: Utc::now(),
        }
    }

    /// Format for display.
    pub fn format_summary(&self) -> String {
        let strategy_name = match self.strategy {
            CompressionStrategy::Truncate => "truncate",
            CompressionStrategy::SlidingWindow => "sliding window",
            CompressionStrategy::Summarize => "AI summarize",
            CompressionStrategy::BiasBased => "bias-based",
        };
        format!(
            "{} messages → {} messages (saved ~{} tokens, {})",
            self.original_count,
            self.new_count,
            format_tokens(self.tokens_saved),
            strategy_name
        )
    }
}

// ============================================================================
// Summarized Segment
// ============================================================================

/// A segment of conversation history that has been summarized.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummarizedSegment {
    /// Timestamp range.
    pub time_range: (DateTime<Utc>, DateTime<Utc>),
    /// Number of original messages.
    pub original_count: usize,
    /// The summary text.
    pub summary: String,
    /// Key points extracted.
    pub key_points: Vec<String>,
}

impl SummarizedSegment {
    /// Render as a system message.
    pub fn to_message(&self) -> Message {
        let key_points_text = if self.key_points.is_empty() {
            "无".to_string()
        } else {
            self.key_points.iter().map(|p| format!("• {}", p)).collect::<Vec<_>>().join("\n")
        };

        let content = format!(
            "[对话摘要 - 原 {} 条消息]\n\n{}\n\n关键要点：\n{}",
            self.original_count,
            self.summary,
            key_points_text
        );

        Message {
            role: Role::User,
            content: MessageContent::Text(content),
        }
    }
}

// ============================================================================
// Compression History Entry
// ============================================================================

/// Compression history entry for session metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionHistoryEntry {
    /// When the compression occurred.
    pub timestamp: DateTime<Utc>,
    /// Strategy used.
    pub strategy: CompressionStrategy,
    /// Original message count.
    pub original_count: usize,
    /// New message count.
    pub new_count: usize,
    /// Estimated tokens saved.
    pub tokens_saved: u32,
    /// Whether summary was generated.
    pub has_summary: bool,
}

impl CompressionHistoryEntry {
    /// Create from a CompressionResult.
    pub fn from_result(result: &CompressionResult) -> Self {
        Self {
            timestamp: result.timestamp,
            strategy: result.strategy,
            original_count: result.original_count,
            new_count: result.new_count,
            tokens_saved: result.tokens_saved,
            has_summary: result.summary.is_some(),
        }
    }

    /// Format for display.
    pub fn format_line(&self) -> String {
        let strategy_name = match self.strategy {
            CompressionStrategy::Truncate => "truncate",
            CompressionStrategy::SlidingWindow => "sliding window",
            CompressionStrategy::Summarize => "AI summarize",
            CompressionStrategy::BiasBased => "bias-based",
        };
        let summary_marker = if self.has_summary { "📝" } else { "✂️" };
        format!(
            "{} {} - {} msgs → {} msgs (~{} tokens saved) {}",
            self.timestamp.format("%Y-%m-%d %H:%M"),
            strategy_name,
            self.original_count,
            self.new_count,
            format_tokens(self.tokens_saved),
            summary_marker
        )
    }
}