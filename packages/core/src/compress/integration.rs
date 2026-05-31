//! Integration example showing optimized compression workflow.
//!
//! This example demonstrates how to use the new semantic compression,
//! dynamic priority scoring, and caching together.

use crate::compress::{
    CompressionCache, CompressionConfig, CacheConfig, PriorityScorer, 
    SemanticCompressor, SemanticStrategy, PriorityScore, estimate_tokens,
};
use crate::providers::{Message, MessageContent, Role};
use anyhow::Result;

/// Optimized compressor with all enhancements.
pub struct OptimizedCompressor {
    config: CompressionConfig,
    cache: CompressionCache,
    scorer: PriorityScorer,
    semantic_strategy: SemanticStrategy,
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
        }
    }

    /// Compress messages with optimizations.
    pub async fn compress(&mut self, messages: Vec<Message>, context_size: Option<u32>) -> Result<Vec<Message>> {
        if messages.is_empty() {
            return Ok(messages);
        }

        // Step 1: Calculate current token usage (accurate with tiktoken)
        let current_tokens: u32 = messages.iter().map(|m| estimate_tokens(m)).sum();
        let context_limit = context_size.unwrap_or(100_000);

        log::info!(
            "Current tokens: {}, Context limit: {}, Threshold: {}",
            current_tokens,
            context_limit,
            (context_limit as f64 * self.config.threshold) as u32
        );

        // Step 2: Check if compression needed
        if current_tokens < (context_limit as f64 * self.config.threshold) as u32 {
            log::debug!("No compression needed");
            return Ok(messages);
        }

        log::info!("Starting optimized compression");

        // Step 3: Score messages by priority
        let scored_messages = self.score_messages(&messages);

        // Step 4: Compress with cache
        let compressed = self.compress_with_cache(scored_messages, context_limit)?;

        // Step 5: Log statistics
        self.log_stats();

        Ok(compressed)
    }

    /// Score all messages by priority.
    fn score_messages(&self, messages: &[Message]) -> Vec<(Message, PriorityScore)> {
        messages
            .iter()
            .enumerate()
            .map(|(idx, msg)| {
                let score = self.scorer.score(msg, idx, messages.len());
                log::trace!(
                    "Message {} priority: {:.2} ({})",
                    idx,
                    score.value(),
                    PriorityScorer::level(score)
                );
                (msg.clone(), score)
            })
            .collect()
    }

    /// Compress messages with cache optimization.
    fn compress_with_cache(
        &mut self,
        scored_messages: Vec<(Message, PriorityScore)>,
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

        // High priority messages next
        for (msg, score) in scored_messages.iter() {
            if score.is_high() && !matches!(msg.role, Role::System) {
                // Check cache first
                if let Some(entry) = self.cache.get(msg) {
                    log::debug!("Cache hit for high priority message");
                    compressed.push(entry.compressed.clone());
                    current_tokens += estimate_tokens(&entry.compressed);
                } else {
                    compressed.push(msg.clone());
                    current_tokens += estimate_tokens(msg);
                }
            }
        }

        // Medium and low priority with compression
        for (msg, score) in scored_messages.iter() {
            if score.is_medium() || score.is_low() {
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

    /// Compress a single message.
    fn compress_message(&self, message: &Message, _score: &PriorityScore) -> Result<Message> {
        match self.semantic_strategy {
            SemanticStrategy::None => {
                // Simple truncation
                self.truncate_message(message)
            }
            SemanticStrategy::OldOnly | SemanticStrategy::Aggressive => {
                // Check if semantic compression is suitable
                if SemanticCompressor::should_summarize(&[message.clone()]) {
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
                if text.len() > 200 {
                    let truncated = format!("{}...[compressed]", &text[..150]);
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
                                if text.len() > 200 {
                                    Some(crate::providers::ContentBlock::Text {
                                        text: format!("{}...[compressed]", &text[..150]),
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

/// Example usage showing all optimizations.
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

    // Create sample messages
    let messages = vec![
        Message {
            role: Role::System,
            content: MessageContent::Text("You are a helpful coding assistant.".to_string()),
        },
        Message {
            role: Role::User,
            content: MessageContent::Text("I decided to use Rust for this important project.".to_string()),
        },
        Message {
            role: Role::Assistant,
            content: MessageContent::Text("Great choice! Rust is excellent for performance-critical applications.".to_string()),
        },
        Message {
            role: Role::User,
            content: MessageContent::Text("Can you help me implement a compression algorithm?".to_string()),
        },
        Message {
            role: Role::Assistant,
            content: MessageContent::Text("Sure! Here's the code:\n```rust\nfn compress(data: &[u8]) -> Vec<u8> {\n    // compression logic\n}\n```".to_string()),
        },
    ];

    // Compress with optimizations
    let compressed = compressor.compress(messages.clone(), Some(50_000)).await?;

    println!("Original messages: {}", messages.len());
    println!("Compressed messages: {}", compressed.len());

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
    fn test_score_messages() {
        let compressor = OptimizedCompressor::new(
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

        let scored = compressor.score_messages(&messages);
        assert_eq!(scored.len(), 2);
        assert!(scored[0].1.value() >= 0.0 && scored[0].1.value() <= 1.0);
    }

    #[test]
    fn test_truncate_message() {
        let compressor = OptimizedCompressor::new(
            CompressionConfig::default(),
            CacheConfig::default(),
            SemanticStrategy::None,
        );

        let long_msg = Message {
            role: Role::User,
            content: MessageContent::Text("This is a very long message that should be truncated to fit within the compression budget".to_string()),
        };

        let score = PriorityScore::new(0.3);
        let truncated = compressor.truncate_message(&long_msg).unwrap();
        
        if let MessageContent::Text(text) = &truncated.content {
            assert!(text.contains("[compressed]"));
            assert!(text.len() < long_msg.content.to_string().len());
        }
    }
}