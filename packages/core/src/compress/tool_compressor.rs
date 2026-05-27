//! Tool result compression for large content.
//!
//! Compresses large tool results (e.g., file reads) using
//! summarization or truncation to reduce token usage.

use anyhow::Result;

use crate::providers::{ContentBlock, Message, MessageContent};
use super::summarizer::Summarizer;
use super::types::{AiCompressionMode, CompressionThresholds};

/// Compressor for tool results.
pub struct ToolCompressor {
    /// Summarizer for AI-based compression.
    summarizer: Option<Summarizer>,
    /// Thresholds for content size.
    thresholds: CompressionThresholds,
}

impl ToolCompressor {
    /// Create a new tool compressor without AI summarization.
    pub fn new_truncate_only(thresholds: CompressionThresholds) -> Self {
        Self {
            summarizer: None,
            thresholds,
        }
    }

    /// Create a new tool compressor with AI summarization.
    pub fn new_with_ai(summarizer: Summarizer, thresholds: CompressionThresholds) -> Self {
        Self {
            summarizer: Some(summarizer),
            thresholds,
        }
    }

    /// Compress large tool results in all messages.
    pub async fn compress_results(
        &self,
        messages: &[Message],
        ai_mode: AiCompressionMode,
    ) -> Result<Vec<Message>> {
        let mut result = messages.to_vec();

        for msg in &mut result {
            if let MessageContent::Blocks(blocks) = &mut msg.content {
                for block in blocks.iter_mut() {
                    if let ContentBlock::ToolResult { content, .. } = block {
                        let tokens = estimate_tokens_str(content);

                        if tokens < self.thresholds.small_content {
                            // Keep unchanged
                            continue;
                        }

                        // Compress based on mode
                        let compressed = self.compress_content(content, tokens, ai_mode).await?;
                        *content = compressed;
                    }
                }
            }
        }

        Ok(result)
    }

    /// Compress a single content string.
    async fn compress_content(
        &self,
        content: &str,
        tokens: u32,
        ai_mode: AiCompressionMode,
    ) -> Result<String> {
        // No AI mode: truncate
        if ai_mode == AiCompressionMode::None || self.summarizer.is_none() {
            return Ok(self.truncate_content(content));
        }

        let summarizer = self.summarizer.as_ref().unwrap();

        // Medium content: light summary
        if tokens < self.thresholds.medium_content {
            let summary = summarizer.summarize_light(content).await?;
            return Ok(format!("[摘要] {}", summary));
        }

        // Large content: choose based on ai_mode
        match ai_mode {
            AiCompressionMode::Light => {
                let summary = summarizer.summarize_light(content).await?;
                Ok(format!("[摘要] {}", summary))
            }
            AiCompressionMode::Deep => {
                let summary = summarizer.summarize_deep(content).await?;
                Ok(format!("[详细摘要] {}", summary))
            }
            AiCompressionMode::None => Ok(self.truncate_content(content)),
        }
    }

    /// Truncate content without AI.
    fn truncate_content(&self, content: &str) -> String {
        // Preserve ends for better context
        Summarizer::truncate_preserve_ends(content, self.thresholds.small_content)
    }

    /// Check if a tool result needs compression.
    pub fn needs_compression(content: &str, thresholds: &CompressionThresholds) -> bool {
        estimate_tokens_str(content) >= thresholds.small_content
    }
}

/// Estimate tokens from string (simplified).
fn estimate_tokens_str(s: &str) -> u32 {
    let (ascii, non_ascii) = count_chars(s);
    let ascii_tokens = (ascii as f64 * 0.25).ceil() as u32;
    let non_ascii_tokens = (non_ascii as f64 * 0.67).ceil() as u32;
    ascii_tokens + non_ascii_tokens
}

/// Count ASCII and non-ASCII characters.
fn count_chars(s: &str) -> (u32, u32) {
    let mut ascii = 0u32;
    let mut non_ascii = 0u32;
    for ch in s.chars() {
        if ch.is_ascii() {
            ascii += 1;
        } else {
            non_ascii += 1;
        }
    }
    (ascii, non_ascii)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_needs_compression() {
        let thresholds = CompressionThresholds::default();

        let short = "短内容";
        assert!(!ToolCompressor::needs_compression(short, &thresholds));

        // Need ~500+ tokens to trigger (about 2000 chars for ASCII)
        let long = "很长的内容...".repeat(200);
        assert!(ToolCompressor::needs_compression(&long, &thresholds));
    }

    #[test]
    fn test_truncate_content() {
        let thresholds = CompressionThresholds::default();
        let compressor = ToolCompressor::new_truncate_only(thresholds);

        // Need content longer than threshold
        let content = "开头内容中间很长的部分结尾内容".repeat(50);
        let result = compressor.truncate_content(&content);

        assert!(result.len() < content.len());
        assert!(result.contains("[内容截断]"));
    }

    #[test]
    fn test_estimate_tokens_str() {
        let ascii = "hello world";
        let tokens = estimate_tokens_str(ascii);
        assert!(tokens > 0 && tokens < 10);

        let chinese = "你好世界";
        let tokens = estimate_tokens_str(chinese);
        assert!(tokens > 0);
    }
}