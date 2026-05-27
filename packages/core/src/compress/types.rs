//! Compression types and result structures.

use crate::providers::{Message, MessageContent, Role};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::config::format_tokens;

// ============================================================================
// Conversation Phase (NEW)
// ============================================================================

/// Conversation phase detected from message history.
/// Determines which weights to apply during scoring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ConversationPhase {
    /// User just made initial request - first message gets highest priority.
    InitialRequest,
    /// Agent is actively executing tools - tool results get higher priority.
    #[default]
    ActiveDevelopment,
    /// Task is nearing completion - final decisions get higher priority.
    Finalizing,
}

impl ConversationPhase {
    /// Get default weights for this phase.
    pub fn default_weights(&self) -> PhaseWeights {
        match self {
            Self::InitialRequest => PhaseWeights {
                first_msg_bonus: 200.0,   // First message extremely important
                user_msg_bonus: 50.0,
                tool_use_bonus: 30.0,
                tool_result_bonus: 20.0,
                critical_tool_bonus: 40.0,
                dependency_pair_bonus: 60.0,
            },
            Self::ActiveDevelopment => PhaseWeights {
                first_msg_bonus: 100.0,
                user_msg_bonus: 30.0,
                tool_use_bonus: 60.0,     // Tool calls more important during active work
                tool_result_bonus: 50.0,
                critical_tool_bonus: 80.0,
                dependency_pair_bonus: 100.0,  // Pairs extremely important for coherence
            },
            Self::Finalizing => PhaseWeights {
                first_msg_bonus: 80.0,
                user_msg_bonus: 40.0,
                tool_use_bonus: 40.0,
                tool_result_bonus: 30.0,
                critical_tool_bonus: 60.0,
                dependency_pair_bonus: 50.0,
            },
        }
    }
}


// ============================================================================
// AI Compression Mode (NEW)
// ============================================================================

/// Mode for AI-assisted compression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum AiCompressionMode {
    /// No AI assistance - pure rule-based scoring.
    None,
    /// Light AI assistance using fast_model for quick judgments.
    #[default]
    Light,
    /// Deep AI analysis for complex content.
    Deep,
}


// ============================================================================
// Message Dependency (NEW)
// ============================================================================

/// Dependency relationship between ToolUse and ToolResult.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDependency {
    /// Index of the message containing ToolUse.
    pub tool_use_idx: usize,
    /// Index of the message containing ToolResult.
    pub tool_result_idx: usize,
    /// Name of the tool (e.g., "read", "write", "bash").
    pub tool_name: String,
    /// Whether this is a critical tool (write/edit/bash).
    pub is_critical: bool,
}

// ============================================================================
// Dependency Graph (NEW)
// ============================================================================

/// Graph of message dependencies for preserving conversation coherence.
#[derive(Debug, Clone, Default)]
pub struct DependencyGraph {
    /// All dependency pairs.
    pub dependencies: Vec<MessageDependency>,
    /// Reverse index: message idx -> related dependency indices.
    pub message_to_deps: HashMap<usize, Vec<usize>>,
}

impl DependencyGraph {
    /// Create an empty dependency graph.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Check if a message index is part of any dependency.
    pub fn has_dependency(&self, idx: usize) -> bool {
        self.message_to_deps.contains_key(&idx)
    }

    /// Get all messages that must be kept together with the given message.
    pub fn get_pair_indices(&self, idx: usize) -> Vec<usize> {
        self.message_to_deps
            .get(&idx)
            .map(|dep_indices| {
                dep_indices
                    .iter()
                    .filter_map(|di| {
                        let dep = &self.dependencies[*di];
                        if dep.tool_use_idx == idx {
                            Some(dep.tool_result_idx)
                        } else if dep.tool_result_idx == idx {
                            Some(dep.tool_use_idx)
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

// ============================================================================
// Phase Weights (NEW)
// ============================================================================

/// Weight configuration for scoring based on conversation phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseWeights {
    /// Bonus for the first message (user's original request).
    pub first_msg_bonus: f64,
    /// Bonus for user messages.
    pub user_msg_bonus: f64,
    /// Bonus for tool use blocks.
    pub tool_use_bonus: f64,
    /// Bonus for tool result blocks.
    pub tool_result_bonus: f64,
    /// Additional bonus for critical tools (write/edit/bash).
    pub critical_tool_bonus: f64,
    /// Bonus for messages that are part of a dependency pair.
    pub dependency_pair_bonus: f64,
}

impl Default for PhaseWeights {
    fn default() -> Self {
        Self::balanced()
    }
}

impl PhaseWeights {
    /// Balanced weights for general use.
    pub fn balanced() -> Self {
        Self {
            first_msg_bonus: 100.0,
            user_msg_bonus: 30.0,
            tool_use_bonus: 25.0,
            tool_result_bonus: 20.0,
            critical_tool_bonus: 40.0,
            dependency_pair_bonus: 50.0,
        }
    }
}

// ============================================================================
// Scored Message (NEW)
// ============================================================================

/// Message with its preservation score.
#[derive(Debug, Clone)]
pub struct ScoredMessage {
    /// Original message index.
    pub index: usize,
    /// The message itself.
    pub message: Message,
    /// Score from rule-based evaluation.
    pub base_score: f64,
    /// Score from AI assistance (optional).
    pub ai_score: Option<f64>,
    /// Bonus from dependency relationships.
    pub dependency_bonus: f64,
    /// Final combined score.
    pub final_score: f64,
    /// Compressed content if applicable.
    pub compressed_content: Option<MessageContent>,
}

impl ScoredMessage {
    /// Create a new scored message with base score only.
    pub fn new(index: usize, message: Message, base_score: f64) -> Self {
        Self {
            index,
            message,
            base_score,
            ai_score: None,
            dependency_bonus: 0.0,
            final_score: base_score,
            compressed_content: None,
        }
    }

    /// Apply AI score bonus.
    pub fn with_ai_score(&mut self, score: f64) {
        self.ai_score = Some(score);
        self.final_score = self.base_score + score + self.dependency_bonus;
    }

    /// Apply dependency bonus.
    pub fn with_dependency_bonus(&mut self, bonus: f64) {
        self.dependency_bonus = bonus;
        self.final_score = self.base_score + self.ai_score.unwrap_or(0.0) + bonus;
    }
}

// ============================================================================
// Compression Thresholds (NEW)
// ============================================================================

/// Thresholds for content compression.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionThresholds {
    /// Content below this token count is kept unchanged.
    pub small_content: u32,
    /// Content below this gets light summarization.
    pub medium_content: u32,
    /// Content above medium gets deep summarization (if ai_mode=Deep).
    pub large_content_threshold: u32,
}

impl Default for CompressionThresholds {
    fn default() -> Self {
        Self {
            small_content: 500,
            medium_content: 2000,
            large_content_threshold: 5000,
        }
    }
}

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
            self.key_points
                .iter()
                .map(|p| format!("• {}", p))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let content = format!(
            "[对话摘要 - 原 {} 条消息]\n\n{}\n\n关键要点：\n{}",
            self.original_count, self.summary, key_points_text
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
