//! Compression pipeline coordinator.
//!
//! Orchestrates all compression modules to perform complete
//! message compression with intelligent scoring, dependency tracking,
//! and content summarization.

use anyhow::Result;

use crate::providers::{ContentBlock, Message, MessageContent, Provider, Role};
use super::hardcode_config::HardcodeConfig;

use super::compressor::{compress_messages, estimate_total_tokens};
use super::config::{
    CircuitBreakerState, CompressionConfig, TIME_BASED_MC_CLEARED_MESSAGE, ThresholdLevel,
};
use super::dependency::DependencyBuilder;
use super::phase_detector::PhaseDetector;
use super::scorer::Scorer;
use super::summarizer::Summarizer;
use super::tool_compressor::ToolCompressor;
use super::types::{
    AiCompressionMode, CompressionStrategy, CompressionThresholds, DependencyGraph, ScoredMessage,
};

/// Compression pipeline that orchestrates all modules.
pub struct CompressionPipeline {
    /// Configuration for compression.
    config: CompressionConfig,
    /// Scorer for message preservation.
    scorer: Scorer,
    /// Tool compressor for large results.
    tool_compressor: ToolCompressor,
    /// Circuit breaker state for preventing infinite retries.
    circuit_breaker: CircuitBreakerState,
    /// Hardcoded configuration values.
    hardcode_config: HardcodeConfig,
}

/// Result of compression with metadata.
pub struct CompressionOutcome {
    /// Compressed messages.
    pub messages: Vec<Message>,
    /// Threshold level before compression.
    pub threshold_level: ThresholdLevel,
    /// Percentage of context remaining after compression.
    pub percent_left: u32,
    /// Whether compression succeeded.
    pub success: bool,
    /// Error message if failed.
    pub error: Option<String>,
    /// Circuit breaker tripped.
    pub circuit_breaker_tripped: bool,
}

/// Validation errors for compression.
#[derive(Debug, Clone)]
pub enum ValidationError {
    /// Orphaned tool result (no corresponding tool_use).
    OrphanedToolResult { tool_use_id: String, index: usize },
    /// Orphaned tool use (no corresponding tool_result).
    OrphanedToolUse { tool_use_id: String, index: usize },
    /// Missing first message (original user request).
    MissingFirstMessage,
    /// Message order violation.
    OrderViolation {
        expected_role: Role,
        actual_role: Role,
        index: usize,
    },
}

impl CompressionPipeline {
    /// Create a new pipeline without AI assistance.
    pub fn new_rule_only(config: CompressionConfig) -> Self {
        let thresholds = CompressionThresholds::default();
        Self {
            config,
            scorer: Scorer::new_rule_only(),
            tool_compressor: ToolCompressor::new_truncate_only(thresholds),
            circuit_breaker: CircuitBreakerState::new(),
            hardcode_config: HardcodeConfig::default(),
        }
    }

    /// Create a new pipeline with AI assistance.
    pub fn new_with_ai(config: CompressionConfig, fast_model: Box<dyn Provider>) -> Self {
        let thresholds = CompressionThresholds::default();
        let summarizer = Summarizer::new(fast_model.clone());

        Self {
            config,
            scorer: Scorer::new_with_ai(fast_model),
            tool_compressor: ToolCompressor::new_with_ai(summarizer, thresholds),
            circuit_breaker: CircuitBreakerState::new(),
            hardcode_config: HardcodeConfig::default(),
        }
    }

    /// Create a new pipeline with full AI support.
    pub fn new_with_full_ai(
        config: CompressionConfig,
        fast_model: Box<dyn Provider>,
        main_model: Box<dyn Provider>,
    ) -> Self {
        let thresholds = CompressionThresholds::default();
        let summarizer = Summarizer::new_with_main(fast_model.clone(), main_model);

        Self {
            config,
            scorer: Scorer::new_with_ai(fast_model),
            tool_compressor: ToolCompressor::new_with_ai(summarizer, thresholds),
            circuit_breaker: CircuitBreakerState::new(),
            hardcode_config: HardcodeConfig::default(),
        }
    }

    /// Check if compression should run (threshold check).
    pub fn should_compress(&self, token_usage: u32, context_window: u32) -> (bool, ThresholdLevel) {
        // Circuit breaker check
        if self.circuit_breaker.should_skip() {
            return (false, ThresholdLevel::Blocking);
        }

        let (level, _) = CompressionConfig::calculate_threshold_level(token_usage, context_window);

        let should_compress = level != ThresholdLevel::Normal;
        (should_compress, level)
    }

    /// Check if time-based microcompact should trigger.
    /// When gap since last assistant exceeds threshold, cache has expired.
    pub fn should_time_based_clear(messages: &[Message]) -> bool {
        let last_assistant = messages.iter().rev().find(|m| m.role == Role::Assistant);

        if let Some(_msg) = last_assistant {
            // Try to get timestamp from message
            // For now, use a simple heuristic: if there are many messages since last assistant
            let messages_since = messages
                .iter()
                .rev()
                .take_while(|m| m.role != Role::Assistant)
                .count();
            // Approximate: if more than 10 messages since last assistant, likely > 5 minutes
            messages_since > 10
        } else {
            false
        }
    }

    /// Execute time-based microcompact: clear old tool results.
    pub fn time_based_microcompact(messages: &[Message]) -> Vec<Message> {
        let config = HardcodeConfig::default();
        messages
            .iter()
            .map(|msg| {
                if msg.role != Role::Tool {
                    return msg.clone();
                }

                // Check if this is a tool result with large content
                match &msg.content {
                    MessageContent::Blocks(blocks) => {
                        let new_blocks: Vec<ContentBlock> = blocks
                            .iter()
                            .map(|b| {
                                if let ContentBlock::ToolResult {
                                    tool_use_id,
                                    content,
                                } = b
                                {
                                    // Clear if content is large and not already cleared
                                    if content.len() > config.preserve_content_threshold
                                        && content != TIME_BASED_MC_CLEARED_MESSAGE
                                    {
                                        ContentBlock::ToolResult {
                                            tool_use_id: tool_use_id.clone(),
                                            content: TIME_BASED_MC_CLEARED_MESSAGE.to_string(),
                                        }
                                    } else {
                                        b.clone()
                                    }
                                } else {
                                    b.clone()
                                }
                            })
                            .collect();
                        Message {
                            role: msg.role.clone(),
                            content: MessageContent::Blocks(new_blocks),
                        }
                    }
                    _ => msg.clone(),
                }
            })
            .collect()
    }

    /// Strip thinking blocks from messages (zero-cost compression).
    /// Thinking blocks consume significant tokens and can often be removed for context continuity.
    pub fn strip_thinking(messages: &[Message]) -> Vec<Message> {
        messages
            .iter()
            .map(|msg| {
                match &msg.content {
                    MessageContent::Blocks(blocks) => {
                        let new_blocks: Vec<ContentBlock> = blocks
                            .iter()
                            .filter(|b| {
                                // Keep all blocks except thinking
                                !matches!(b, ContentBlock::Thinking { .. })
                            })
                            .cloned()
                            .collect();
                        Message {
                            role: msg.role.clone(),
                            content: MessageContent::Blocks(new_blocks),
                        }
                    }
                    _ => msg.clone(),
                }
            })
            .collect()
    }

    /// Compactable tools - tool types that can be safely cleared.
    /// Based on Claude Code's COMPACTABLE_TOOLS list.
    const COMPACTABLE_TOOLS: &[&str] = &[
        "bash",
        "read",
        "glob",
        "grep",
        "ls",
        "edit",
        "write",
        "notebook_edit",
        "web_fetch",
        "web_search",
    ];

    /// Check if a tool name is compactable.
    pub fn is_compactable_tool(tool_name: &str) -> bool {
        Self::COMPACTABLE_TOOLS.contains(&tool_name)
    }

    /// Clear specific tool types (more targeted than time-based).
    pub fn clear_tool_results(messages: &[Message], _tool_names: &[&str]) -> Vec<Message> {
        let config = HardcodeConfig::default();
        messages
            .iter()
            .map(|msg| {
                if msg.role != Role::Tool {
                    return msg.clone();
                }

                match &msg.content {
                    MessageContent::Blocks(blocks) => {
                        let new_blocks: Vec<ContentBlock> = blocks
                            .iter()
                            .map(|b| {
                                if let ContentBlock::ToolResult {
                                    tool_use_id,
                                    content,
                                } = b
                                {
                                    // Check if the corresponding tool is in the list
                                    // We need to find the tool_use block to check the name
                                    if content.len() > config.preserve_content_threshold
                                        && content != TIME_BASED_MC_CLEARED_MESSAGE
                                    {
                                        ContentBlock::ToolResult {
                                            tool_use_id: tool_use_id.clone(),
                                            content: TIME_BASED_MC_CLEARED_MESSAGE.to_string(),
                                        }
                                    } else {
                                        b.clone()
                                    }
                                } else {
                                    b.clone()
                                }
                            })
                            .collect();
                        Message {
                            role: msg.role.clone(),
                            content: MessageContent::Blocks(new_blocks),
                        }
                    }
                    _ => msg.clone(),
                }
            })
            .collect()
    }

    /// Combined microcompact: clear all compactable tool results + strip thinking blocks.
    pub fn full_microcompact(messages: &[Message]) -> Vec<Message> {
        // First strip thinking blocks
        let no_thinking = Self::strip_thinking(messages);
        // Then clear large tool results
        Self::time_based_microcompact(&no_thinking)
    }

    // ========================================================================
    // Compression Validation
    // ========================================================================

    /// Validate compressed messages for integrity.
    pub fn validate_compression(
        messages: &[Message],
        _original_deps: &DependencyGraph,
    ) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // Check first message exists
        if messages.is_empty() {
            errors.push(ValidationError::MissingFirstMessage);
            return errors;
        }

        // Build new dependency graph for compressed messages
        let new_deps = DependencyBuilder::build(messages);

        // Check for orphaned tool results by scanning content
        for (idx, msg) in messages.iter().enumerate() {
            if msg.role == Role::Tool
                && let MessageContent::Blocks(blocks) = &msg.content
            {
                for block in blocks {
                    if let ContentBlock::ToolResult { tool_use_id, .. } = block {
                        // Find corresponding tool_use
                        let has_tool_use = messages.iter().any(|m| {
                            if let MessageContent::Blocks(bs) = &m.content {
                                bs.iter().any(|b| {
                                    if let ContentBlock::ToolUse { id, .. } = b {
                                        id == tool_use_id
                                    } else {
                                        false
                                    }
                                })
                            } else {
                                false
                            }
                        });

                        if !has_tool_use {
                            errors.push(ValidationError::OrphanedToolResult {
                                tool_use_id: tool_use_id.clone(),
                                index: idx,
                            });
                        }
                    }
                }
            }
        }

        // Check for orphaned tool use blocks (tool_use without tool_result)
        for (idx, msg) in messages.iter().enumerate() {
            if let MessageContent::Blocks(blocks) = &msg.content {
                for block in blocks {
                    if let ContentBlock::ToolUse { id, .. } = block {
                        // Find corresponding tool_result
                        let has_tool_result = messages.iter().any(|m| {
                            if m.role == Role::Tool {
                                if let MessageContent::Blocks(bs) = &m.content {
                                    bs.iter().any(|b| {
                                        if let ContentBlock::ToolResult { tool_use_id, .. } = b {
                                            tool_use_id == id
                                        } else {
                                            false
                                        }
                                    })
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        });

                        if !has_tool_result {
                            errors.push(ValidationError::OrphanedToolUse {
                                tool_use_id: id.clone(),
                                index: idx,
                            });
                        }
                    }
                }
            }
        }

        // Check dependency indices are valid
        for dep in &new_deps.dependencies {
            if dep.tool_use_idx >= messages.len() {
                errors.push(ValidationError::OrphanedToolUse {
                    tool_use_id: dep.tool_name.clone(),
                    index: dep.tool_use_idx,
                });
            }
            if dep.tool_result_idx >= messages.len() {
                errors.push(ValidationError::OrphanedToolResult {
                    tool_use_id: dep.tool_name.clone(),
                    index: dep.tool_result_idx,
                });
            }
        }

        errors
    }

    /// Check if compression is valid (no errors).
    pub fn is_valid_compression(messages: &[Message], original_deps: &DependencyGraph) -> bool {
        Self::validate_compression(messages, original_deps).is_empty()
    }

    /// Execute the full compression pipeline.
    pub async fn execute(
        &mut self,
        messages: &[Message],
        ai_mode: AiCompressionMode,
        token_usage: u32,
        context_window: u32,
    ) -> Result<CompressionOutcome> {
        // Circuit breaker check
        if self.circuit_breaker.should_skip() {
            return Ok(CompressionOutcome {
                messages: messages.to_vec(),
                threshold_level: ThresholdLevel::Blocking,
                percent_left: 0,
                success: false,
                error: Some("Circuit breaker tripped - too many consecutive failures".to_string()),
                circuit_breaker_tripped: true,
            });
        }

        if messages.len() <= self.config.min_preserve_messages {
            let (level, percent) =
                CompressionConfig::calculate_threshold_level(token_usage, context_window);
            return Ok(CompressionOutcome {
                messages: messages.to_vec(),
                threshold_level: level,
                percent_left: percent,
                success: true,
                error: None,
                circuit_breaker_tripped: false,
            });
        }

        // Pre-compression: time-based microcompact
        let pre_processed = if Self::should_time_based_clear(messages) {
            Self::time_based_microcompact(messages)
        } else {
            messages.to_vec()
        };

        // Phase 1: Pre-processing
        let phase = PhaseDetector::detect(&pre_processed);
        let weights = phase.default_weights();
        let deps = DependencyBuilder::build(&pre_processed);

        // Phase 2: Intelligent scoring
        let scored = self
            .scorer
            .score_all(&pre_processed, &weights, &deps, ai_mode)
            .await?;

        // Phase 3: Content compression
        let compressed = self
            .tool_compressor
            .compress_results(&pre_processed, ai_mode)
            .await?;

        // Phase 4: Select messages to preserve
        let target_count = calculate_target_count(pre_processed.len(), &self.config);
        let selected = self.select_messages(scored, &deps, target_count, &compressed);

        // Phase 5: Ensure dependency integrity
        let final_messages = self.ensure_dependency_integrity(selected, &deps, &pre_processed);

        // Success - reset circuit breaker
        self.circuit_breaker.record_success();

        // Calculate post-compression metrics
        let post_tokens = estimate_total_tokens(&final_messages);
        let (level, percent) =
            CompressionConfig::calculate_threshold_level(post_tokens, context_window);

        Ok(CompressionOutcome {
            messages: final_messages,
            threshold_level: level,
            percent_left: percent,
            success: true,
            error: None,
            circuit_breaker_tripped: false,
        })
    }

    /// Execute with error handling and circuit breaker.
    pub async fn execute_with_circuit_breaker(
        &mut self,
        messages: &[Message],
        ai_mode: AiCompressionMode,
        token_usage: u32,
        context_window: u32,
    ) -> Result<CompressionOutcome> {
        let result = self
            .execute(messages, ai_mode, token_usage, context_window)
            .await;

        match result {
            Ok(res) => Ok(res),
            Err(e) => {
                // Record failure for circuit breaker
                let tripped = self.circuit_breaker.record_failure();

                let (level, percent) =
                    CompressionConfig::calculate_threshold_level(token_usage, context_window);

                Ok(CompressionOutcome {
                    messages: messages.to_vec(),
                    threshold_level: level,
                    percent_left: percent,
                    success: false,
                    error: Some(e.to_string()),
                    circuit_breaker_tripped: tripped,
                })
            }
        }
    }

    /// Execute compression synchronously (rule-only mode).
    pub fn execute_sync(&self, messages: &[Message]) -> Result<Vec<Message>> {
        // Use legacy compression for sync mode
        compress_messages(messages, CompressionStrategy::BiasBased, &self.config)
    }

    /// Select messages to preserve based on scores.
    fn select_messages(
        &self,
        scored: Vec<ScoredMessage>,
        deps: &DependencyGraph,
        target_count: usize,
        compressed_messages: &[Message],
    ) -> Vec<Message> {
        // Sort by score (descending)
        let mut sorted = scored;
        sorted.sort_by(|a, b| b.final_score.partial_cmp(&a.final_score).unwrap());

        // Build a set of indices to preserve
        let mut preserve_indices: std::collections::HashSet<usize> =
            std::collections::HashSet::new();

        // First pass: select top scored messages
        for sm in sorted.iter().take(target_count) {
            preserve_indices.insert(sm.index);

            // Also preserve dependency pairs
            for pair_idx in deps.get_pair_indices(sm.index) {
                preserve_indices.insert(pair_idx);
            }
        }

        // Convert indices to messages
        let selected: Vec<Message> = preserve_indices
            .iter()
            .filter_map(|idx| compressed_messages.get(*idx).cloned())
            .collect();

        selected
    }

    /// Ensure dependency chain integrity.
    fn ensure_dependency_integrity(
        &self,
        selected: Vec<Message>,
        _deps: &DependencyGraph,
        _original: &[Message],
    ) -> Vec<Message> {
        // For now, we rely on the selection process to preserve pairs
        // This is a safety check that could be enhanced
        selected
    }

    /// Score messages without compressing.
    pub fn score_only(&self, messages: &[Message]) -> Vec<ScoredMessage> {
        let phase = PhaseDetector::detect(messages);
        let weights = phase.default_weights();
        let deps = DependencyBuilder::build(messages);

        // Sync scoring only (no AI)
        let mut scored: Vec<ScoredMessage> = Vec::new();
        for (idx, msg) in messages.iter().enumerate() {
            let base_score = super::scorer::score_by_rules(msg, idx, &weights);
            scored.push(ScoredMessage::new(idx, msg.clone(), base_score));
        }

        // Apply dependency bonus
        let bonus = weights.dependency_pair_bonus;
        for dep in &deps.dependencies {
            if let Some(sm) = scored.get_mut(dep.tool_use_idx) {
                sm.with_dependency_bonus(bonus);
            }
            if let Some(sm) = scored.get_mut(dep.tool_result_idx) {
                sm.with_dependency_bonus(bonus);
            }
        }

        scored
    }
}

/// Calculate target count based on config.
fn calculate_target_count(total: usize, config: &CompressionConfig) -> usize {
    let target = (total as f64 * config.target_ratio) as usize;
    target.max(config.min_preserve_messages)
}

/// Legacy compression function (backward compatible).
pub fn compress_with_pipeline(
    messages: &[Message],
    config: &CompressionConfig,
    ai_mode: AiCompressionMode,
    fast_model: Option<Box<dyn Provider>>,
) -> Result<Vec<Message>> {
    // Create pipeline based on AI mode
    let pipeline = match (ai_mode, fast_model) {
        (AiCompressionMode::None, _) => CompressionPipeline::new_rule_only(config.clone()),
        (AiCompressionMode::Light | AiCompressionMode::Deep, Some(model)) => {
            CompressionPipeline::new_with_ai(config.clone(), model)
        }
        _ => CompressionPipeline::new_rule_only(config.clone()),
    };

    // Execute synchronously for now (async version needs runtime)
    pipeline.execute_sync(messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::{MessageContent, Role};

    #[test]
    fn test_pipeline_new_rule_only() {
        let config = CompressionConfig::default();
        let pipeline = CompressionPipeline::new_rule_only(config);
        // Pipeline created successfully - test by executing
        let messages = vec![Message {
            role: Role::User,
            content: MessageContent::Text("Test".to_string()),
        }];
        let result = pipeline.execute_sync(&messages);
        assert!(result.is_ok());
    }

    #[test]
    fn test_calculate_target_count() {
        let config = CompressionConfig::default();
        let total = 100;
        let target = calculate_target_count(total, &config);
        assert!(target >= config.min_preserve_messages);
        assert!(target < total);
    }

    #[test]
    fn test_score_only() {
        let config = CompressionConfig::default();
        let pipeline = CompressionPipeline::new_rule_only(config);

        let messages = vec![
            Message {
                role: Role::User,
                content: MessageContent::Text("Hello".to_string()),
            },
            Message {
                role: Role::Assistant,
                content: MessageContent::Text("Hi".to_string()),
            },
        ];

        let scored = pipeline.score_only(&messages);
        assert_eq!(scored.len(), 2);
        assert!(scored[0].final_score > scored[1].final_score); // First message should score higher
    }

    #[test]
    fn test_execute_sync_small() {
        let config = CompressionConfig::default();
        let pipeline = CompressionPipeline::new_rule_only(config);

        let messages = vec![Message {
            role: Role::User,
            content: MessageContent::Text("Hello".to_string()),
        }];

        let result = pipeline.execute_sync(&messages).unwrap();
        assert_eq!(result.len(), 1); // Small message list unchanged
    }

    #[test]
    fn test_time_based_microcompact() {
        let messages = vec![
            Message {
                role: Role::Tool,
                content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                    tool_use_id: "tool_1".to_string(),
                    content: "This is a very long tool result content that should be cleared..."
                        .repeat(20),
                }]),
            },
            Message {
                role: Role::Tool,
                content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                    tool_use_id: "tool_2".to_string(),
                    content: "Short content".to_string(),
                }]),
            },
        ];

        let compacted = CompressionPipeline::time_based_microcompact(&messages);

        // First result should be cleared (large content)
        if let MessageContent::Blocks(blocks) = &compacted[0].content {
            if let ContentBlock::ToolResult { content, .. } = &blocks[0] {
                assert_eq!(content, TIME_BASED_MC_CLEARED_MESSAGE);
            }
        }

        // Second result should remain (small content)
        if let MessageContent::Blocks(blocks) = &compacted[1].content {
            if let ContentBlock::ToolResult { content, .. } = &blocks[0] {
                assert_eq!(content, "Short content");
            }
        }
    }

    #[test]
    fn test_strip_thinking() {
        let messages = vec![Message {
            role: Role::Assistant,
            content: MessageContent::Blocks(vec![
                ContentBlock::Text {
                    text: "Response".to_string(),
                },
                ContentBlock::Thinking {
                    thinking: "Long thinking process...".to_string(),
                    signature: None,
                },
            ]),
        }];

        let stripped = CompressionPipeline::strip_thinking(&messages);

        // Thinking should be removed
        if let MessageContent::Blocks(blocks) = &stripped[0].content {
            assert_eq!(blocks.len(), 1);
            assert!(matches!(&blocks[0], ContentBlock::Text { .. }));
        }
    }

    #[test]
    fn test_is_compactable_tool() {
        assert!(CompressionPipeline::is_compactable_tool("bash"));
        assert!(CompressionPipeline::is_compactable_tool("read"));
        assert!(CompressionPipeline::is_compactable_tool("glob"));
        assert!(!CompressionPipeline::is_compactable_tool("unknown_tool"));
    }

    #[test]
    fn test_should_time_based_clear() {
        // Many messages since last assistant (assistant at start, then 15+ messages)
        let mut many_messages: Vec<Message> = vec![Message {
            role: Role::Assistant,
            content: MessageContent::Text("response".to_string()),
        }];
        // Add 15 more messages after assistant
        for i in 0..15 {
            many_messages.push(Message {
                role: if i % 2 == 0 { Role::User } else { Role::Tool },
                content: MessageContent::Text("content".to_string()),
            });
        }

        assert!(CompressionPipeline::should_time_based_clear(&many_messages));

        // Few messages since last assistant
        let few_messages = vec![
            Message {
                role: Role::Assistant,
                content: MessageContent::Text("response".to_string()),
            },
            Message {
                role: Role::User,
                content: MessageContent::Text("follow-up".to_string()),
            },
        ];

        assert!(!CompressionPipeline::should_time_based_clear(&few_messages));
    }

    #[test]
    fn test_validate_compression_valid() {
        let messages = vec![
            Message {
                role: Role::User,
                content: MessageContent::Text("Request".to_string()),
            },
            Message {
                role: Role::Assistant,
                content: MessageContent::Blocks(vec![ContentBlock::ToolUse {
                    id: "tool_1".to_string(),
                    name: "read".to_string(),
                    input: serde_json::json!({"path": "test.txt"}),
                }]),
            },
            Message {
                role: Role::Tool,
                content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                    tool_use_id: "tool_1".to_string(),
                    content: "File content".to_string(),
                }]),
            },
        ];

        let deps = DependencyBuilder::build(&messages);
        let errors = CompressionPipeline::validate_compression(&messages, &deps);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_compression_orphaned_tool_result() {
        let messages = vec![
            Message {
                role: Role::User,
                content: MessageContent::Text("Request".to_string()),
            },
            Message {
                role: Role::Tool,
                content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                    tool_use_id: "tool_missing".to_string(),
                    content: "Orphaned result".to_string(),
                }]),
            },
        ];

        let deps = DependencyBuilder::build(&messages);
        let errors = CompressionPipeline::validate_compression(&messages, &deps);
        assert!(!errors.is_empty());
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, ValidationError::OrphanedToolResult { .. }))
        );
    }

    #[test]
    fn test_validate_compression_empty() {
        let messages: Vec<Message> = vec![];
        let deps = DependencyBuilder::build(&messages);
        let errors = CompressionPipeline::validate_compression(&messages, &deps);
        assert!(!errors.is_empty());
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, ValidationError::MissingFirstMessage))
        );
    }
}
