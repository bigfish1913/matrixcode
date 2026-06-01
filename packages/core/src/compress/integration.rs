//! Integration example showing optimized compression workflow with focus tracking.
//!
//! This example demonstrates how to use the new semantic compression,
//! dynamic priority scoring, caching, and focus tracking together.

use crate::compress::{
    CompressionCache, CompressionConfig, CacheConfig, PriorityScorer,
    SemanticCompressor, SemanticStrategy, estimate_tokens,
    FocusTracker, ConversationFocus,
};
use crate::compress::hardcode_config::HardcodeConfig;
use crate::providers::{Message, MessageContent, Role};
use anyhow::Result;

/// Optimized compressor with all enhancements including focus tracking.
pub struct OptimizedCompressor {
    config: CompressionConfig,
    cache: CompressionCache,
    scorer: PriorityScorer,
    semantic_strategy: SemanticStrategy,
    focus_tracker: FocusTracker,
    hardcode_config: HardcodeConfig,
    semantic_compressor: SemanticCompressor,
}

impl OptimizedCompressor {
    pub fn new(
        compression_config: CompressionConfig,
        cache_config: CacheConfig,
        semantic_strategy: SemanticStrategy,
    ) -> Self {
        Self {
            config: compression_config,
            cache: CompressionCache::new(cache_config),
            scorer: PriorityScorer::default(),
            semantic_strategy,
            focus_tracker: FocusTracker::new(),
            hardcode_config: HardcodeConfig::default(),
            semantic_compressor: SemanticCompressor::default(),
        }
    }

    /// Compress messages with optimizations and focus preservation.
    pub async fn compress(&mut self, messages: Vec<Message>, context_size: Option<u32>) -> Result<Vec<Message>> {
        if messages.is_empty() {
            return Ok(messages);
        }

        // Step 1: Detect current conversation focus (NEW!)
        let focus = self.focus_tracker.detect_focus(&messages);
        log::info!(
            "Detected focus - Topic: {:?}, Question: {:?}",
            focus.current_topic,
            focus.current_question
        );

        // Step 2: Calculate current token usage (accurate with tiktoken)
        let current_tokens: u32 = messages.iter().map(|m| estimate_tokens(m)).sum();
        let context_limit = context_size.unwrap_or(100_000);

        log::info!(
            "Current tokens: {}, Context limit: {}, Threshold: {}",
            current_tokens,
            context_limit,
            (context_limit as f64 * self.config.threshold) as u32
        );

        // Step 3: Check if compression needed
        if current_tokens < (context_limit as f64 * self.config.threshold) as u32 {
            log::debug!("No compression needed");
            return Ok(messages);
        }

        log::info!("Starting optimized compression with focus preservation");

        // Step 4: Score messages by priority AND focus (NEW!)
        let scored_messages = self.score_messages_with_focus(&messages, &focus);

        // Step 5: Compress with cache and focus preservation
        let compressed = self.compress_with_cache_and_focus(scored_messages, &focus, context_limit)?;

        // Step 6: Inject focus message at the beginning (NEW!)
        let final_messages = self.inject_focus_message(compressed, &focus);

        // Step 7: Log statistics
        self.log_stats();

        Ok(final_messages)
    }

    /// Score messages by both priority and focus relevance.
    fn score_messages_with_focus(&self, messages: &[Message], focus: &ConversationFocus) -> Vec<(Message, f32)> {
        messages
            .iter()
            .enumerate()
            .map(|(idx, msg)| {
                // Combine priority score and focus score
                let priority_score = self.scorer.score(msg, idx, messages.len()).value();
                let focus_score = self.focus_tracker.focus_score(msg, focus);
                
                // Combined score: priority + focus boost
                // Focus score can boost priority by up to 0.3
                let combined_score = priority_score + focus_score;
                
                log::trace!(
                    "Message {} - Priority: {:.2}, Focus: {:.2}, Combined: {:.2}",
                    idx,
                    priority_score,
                    focus_score,
                    combined_score
                );
                
                (msg.clone(), combined_score.min(1.0)) // Cap at 1.0
            })
            .collect()
    }

    /// Compress messages with cache and focus preservation.
    fn compress_with_cache_and_focus(
        &mut self,
        scored_messages: Vec<(Message, f32)>,
        focus: &ConversationFocus,
        context_limit: u32,
    ) -> Result<Vec<Message>> {
        let target_tokens = (context_limit as f64 * self.config.target_ratio) as u32;
        let mut compressed = Vec::new();
        let mut current_tokens = 0u32;

        // System messages first (highest priority)
        for (msg, _score) in scored_messages.iter() {
            if matches!(msg.role, Role::System) {
                compressed.push(msg.clone());
                current_tokens += estimate_tokens(msg);
            }
        }

        // High score messages (priority + focus) next
        for (msg, score) in scored_messages.iter() {
            if *score >= 0.7 && !matches!(msg.role, Role::System) {
                // Check cache first
                if let Some(entry) = self.cache.get(msg) {
                    log::debug!("Cache hit for high score message");
                    compressed.push(entry.compressed.clone());
                    current_tokens += estimate_tokens(&entry.compressed);
                } else {
                    compressed.push(msg.clone());
                    current_tokens += estimate_tokens(msg);
                }
            }
        }

        // Always preserve recent context for focus (NEW!)
        for ctx_text in &focus.recent_context {
            // Find and preserve messages containing recent context
            for (msg, score) in scored_messages.iter() {
                if *score < 0.7 {
                    let msg_text = match &msg.content {
                        MessageContent::Text(t) => t.clone(),
                        MessageContent::Blocks(blocks) => {
                            blocks.iter()
                                .filter_map(|b| {
                                    if let crate::providers::ContentBlock::Text { text } = b {
                                        Some(text.clone())
                                    } else {
                                        None
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join(" ")
                        }
                    };

                    if msg_text.contains(ctx_text) && !compressed.contains(msg) {
                        compressed.push(msg.clone());
                        current_tokens += estimate_tokens(msg);
                        log::debug!("Preserved message for focus context: {}", ctx_text);
                    }
                }
            }
        }

        // Medium and low score with compression
        for (msg, score) in scored_messages.iter() {
            if *score < 0.7 && !compressed.contains(msg) {
                if current_tokens >= target_tokens {
                    // Need to compress
                    let compressed_msg = self.compress_message(msg, score)?;
                    
                    // Calculate tokens before moving
                    let msg_tokens = estimate_tokens(&compressed_msg);
                    
                    // Cache the result
                    self.cache.put(msg, compressed_msg.clone());
                    
                    compressed.push(compressed_msg);
                    current_tokens += msg_tokens;
                } else {
                    // Keep original if within budget
                    compressed.push(msg.clone());
                    current_tokens += estimate_tokens(msg);
                }
            }
        }

        Ok(compressed)
    }

    /// Inject focus message at the beginning of compressed messages.
    /// If a focus message already exists, it will be replaced (not duplicated).
    fn inject_focus_message(&self, mut compressed: Vec<Message>, focus: &ConversationFocus) -> Vec<Message> {
        // Create focus message
        let focus_msg = self.focus_tracker.create_focus_message(focus);

        // Check if a focus message already exists
        let existing_focus_pos = compressed.iter().position(|m| {
            if matches!(m.role, Role::System) {
                match &m.content {
                    MessageContent::Text(t) => {
                        t.contains("焦点") || t.contains("Focus") || t.contains("【焦点上下文】")
                    }
                    _ => false
                }
            } else {
                false
            }
        });

        if let Some(pos) = existing_focus_pos {
            // Replace existing focus message
            compressed[pos] = focus_msg;
            log::info!("Replaced existing focus message at position {}", pos);
        } else {
            // Insert after system messages but before other content
            let insert_pos = compressed.iter()
                .position(|m| !matches!(m.role, Role::System))
                .unwrap_or(1);

            compressed.insert(insert_pos, focus_msg);
            log::info!("Injected new focus message at position {}", insert_pos);
        }

        compressed
    }

    /// Compress a single message.
    fn compress_message(&self, message: &Message, _score: &f32) -> Result<Message> {
        match self.semantic_strategy {
            SemanticStrategy::None => {
                // Simple truncation
                self.truncate_message(message)
            }
            SemanticStrategy::OldOnly | SemanticStrategy::Aggressive => {
                // Check if semantic compression is suitable
                if self.semantic_compressor.should_summarize(&[message.clone()]) {
                    // Would use AI to summarize (not implemented in this example)
                    // For now, just truncate
                    self.truncate_message(message)
                } else {
                    self.truncate_message(message)
                }
            }
        }
    }

    /// Truncate a message to fit budget.
    fn truncate_message(&self, message: &Message) -> Result<Message> {
        // Simple truncation with suffix
        match &message.content {
            MessageContent::Text(text) => {
                if text.len() > self.hardcode_config.long_text_threshold {
                    let keep_len = (self.hardcode_config.long_text_threshold as f64 * 0.75) as usize;
                    let truncated = format!("{}...[compressed]", &text.chars().take(keep_len).collect::<String>());
                    Ok(Message {
                        role: message.role,
                        content: MessageContent::Text(truncated),
                    })
                } else {
                    Ok(message.clone())
                }
            }
            MessageContent::Blocks(blocks) => {
                // Compress blocks
                let compressed_blocks = blocks
                    .iter()
                    .filter_map(|block| {
                        match block {
                            crate::providers::ContentBlock::Text { text } => {
                                if text.len() > self.hardcode_config.long_text_threshold {
                                    let keep_len = (self.hardcode_config.long_text_threshold as f64 * 0.75) as usize;
                                    Some(crate::providers::ContentBlock::Text {
                                        text: format!("{}...[compressed]", &text.chars().take(keep_len).collect::<String>()),
                                    })
                                } else {
                                    Some(block.clone())
                                }
                            }
                            _ => Some(block.clone()),
                        }
                    })
                    .collect();

                Ok(Message {
                    role: message.role,
                    content: MessageContent::Blocks(compressed_blocks),
                })
            }
        }
    }

    /// Log compression statistics.
    fn log_stats(&self) {
        let stats = self.cache.stats();
        log::info!(
            "Compression stats - Hits: {}, Misses: {}, Hit rate: {:.2}%, Entries: {}",
            stats.hits,
            stats.misses,
            stats.hit_rate() * 100.0,
            stats.entries
        );
    }
}

/// Example usage showing all optimizations with focus tracking.
pub async fn example_optimized_compression() -> Result<()> {
    // Create optimized compressor
    let compression_config = CompressionConfig::default();

    let cache_config = CacheConfig {
        max_entries: 100,
        ttl: std::time::Duration::from_secs(300),
        min_size_to_cache: 100,
    };

    let mut compressor = OptimizedCompressor::new(
        compression_config,
        cache_config,
        SemanticStrategy::OldOnly,
    );

    // Create sample messages with topic transitions
    let messages = vec![
        Message {
            role: Role::System,
            content: MessageContent::Text("You are a helpful coding assistant.".to_string()),
        },
        Message {
            role: Role::User,
            content: MessageContent::Text("Let's discuss compression algorithms.".to_string()),
        },
        Message {
            role: Role::Assistant,
            content: MessageContent::Text("Compression algorithms reduce data size...".to_string()),
        },
        Message {
            role: Role::User,
            content: MessageContent::Text("How do I implement Huffman coding?".to_string()),
        },
        Message {
            role: Role::Assistant,
            content: MessageContent::Text("Huffman coding uses frequency-based encoding...".to_string()),
        },
        // Topic transition
        Message {
            role: Role::User,
            content: MessageContent::Text("Wait, switching to a different topic: how to optimize database queries?".to_string()),
        },
        Message {
            role: Role::Assistant,
            content: MessageContent::Text("Database optimization involves indexing...".to_string()),
        },
        Message {
            role: Role::User,
            content: MessageContent::Text("Can you help me fix this slow query in PostgreSQL?".to_string()),
        },
    ];

    // Compress with optimizations
    let compressed = compressor.compress(messages.clone(), Some(50_000)).await?;

    println!("Original messages: {}", messages.len());
    println!("Compressed messages: {}", compressed.len());

    // Verify focus is preserved
    for msg in compressed.iter() {
        if let MessageContent::Text(text) = &msg.content {
            if text.contains("Current Conversation Focus") {
                println!("\nFocus message found:\n{}", text);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimized_compressor_creation() {
        let compressor = OptimizedCompressor::new(
            CompressionConfig::default(),
            CacheConfig::default(),
            SemanticStrategy::OldOnly,
        );
        assert!(compressor.cache.is_empty());
    }

    #[test]
    fn test_focus_detection() {
        let mut compressor = OptimizedCompressor::new(
            CompressionConfig::default(),
            CacheConfig::default(),
            SemanticStrategy::None,
        );

        let messages = vec![
            Message {
                role: Role::User,
                content: MessageContent::Text("Test message".to_string()),
            },
            Message {
                role: Role::Assistant,
                content: MessageContent::Text("Response".to_string()),
            },
        ];

        let focus = compressor.focus_tracker.detect_focus(&messages);
        assert!(focus.recent_context.len() > 0);
    }

    #[test]
    fn test_combined_scoring() {
        let mut compressor = OptimizedCompressor::new(
            CompressionConfig::default(),
            CacheConfig::default(),
            SemanticStrategy::None,
        );

        let messages = vec![
            Message {
                role: Role::User,
                content: MessageContent::Text("Let's discuss database optimization".to_string()),
            },
            Message {
                role: Role::Assistant,
                content: MessageContent::Text("Database optimization is important...".to_string()),
            },
            Message {
                role: Role::User,
                content: MessageContent::Text("How to fix slow query?".to_string()),
            },
        ];

        let focus = compressor.focus_tracker.detect_focus(&messages);
        let scored = compressor.score_messages_with_focus(&messages, &focus);

        // Last user message should have highest score (recent + contains current question)
        assert!(scored[2].1 > scored[0].1);
    }

    #[test]
    fn test_focus_message_injection() {
        let compressor = OptimizedCompressor::new(
            CompressionConfig::default(),
            CacheConfig::default(),
            SemanticStrategy::None,
        );

        let focus = ConversationFocus {
            current_topic: Some("optimization".to_string()),
            current_question: Some("How to fix slow query?".to_string()),
            recent_context: vec!["Database discussion".to_string()],
            topic_transitions: vec![],
            detected_at: 2,
        };

        let messages = vec![
            Message {
                role: Role::System,
                content: MessageContent::Text("System prompt".to_string()),
            },
            Message {
                role: Role::User,
                content: MessageContent::Text("User question".to_string()),
            },
        ];

        let final_messages = compressor.inject_focus_message(messages, &focus);
        
        // Should have 3 messages now (system + focus + user)
        assert_eq!(final_messages.len(), 3);
        
        // Focus message should be at position 1
        if let MessageContent::Text(text) = &final_messages[1].content {
            assert!(text.contains("焦点上下文"));
        } else {
            panic!("Expected text content");
        }
    }
}