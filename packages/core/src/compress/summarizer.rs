//! Content summarization for large tool results.
//!
//! Provides light and deep summarization modes to compress large
//! content while preserving key information.

use anyhow::Result;

use crate::providers::{ChatRequest, ContentBlock, Message, MessageContent, Provider, Role};
use crate::truncate::truncate_with_suffix;

/// Summarizer for large content.
pub struct Summarizer {
    /// Fast model for light summarization.
    fast_model: Box<dyn Provider>,
    /// Optional main model for deep summarization.
    main_model: Option<Box<dyn Provider>>,
}

impl Summarizer {
    /// Create a new summarizer with fast model only.
    pub fn new(fast_model: Box<dyn Provider>) -> Self {
        Self {
            fast_model,
            main_model: None,
        }
    }

    /// Create a new summarizer with both models.
    pub fn new_with_main(fast_model: Box<dyn Provider>, main_model: Box<dyn Provider>) -> Self {
        Self {
            fast_model,
            main_model: Some(main_model),
        }
    }

    /// Generate a light summary (quick, key points only).
    pub async fn summarize_light(&self, content: &str) -> Result<String> {
        let truncated = truncate_with_suffix(content, 3000);
        let prompt = format!(
            "将以下内容压缩为简洁摘要（保留关键信息，200字以内）：\n{}",
            truncated
        );

        let response = self.fast_model.chat(build_summary_request(prompt)).await?;
        let summary = extract_summary_text(&response);

        Ok(summary)
    }

    /// Generate a deep summary (more detailed).
    pub async fn summarize_deep(&self, content: &str) -> Result<String> {
        let truncated = truncate_with_suffix(content, 5000);
        let prompt = format!(
            "将以下内容压缩为详细摘要（保留所有重要细节，500字以内）：\n{}",
            truncated
        );

        let model = self.main_model.as_ref().unwrap_or(&self.fast_model);
        let response = model.chat(build_summary_request(prompt)).await?;
        let summary = extract_summary_text(&response);

        Ok(summary)
    }

    /// Smart truncation preserving structure.
    pub fn smart_truncate(content: &str, target_tokens: u32) -> String {
        // Estimate: 4 chars per token for ASCII, 1.5 for Chinese
        let estimated_chars = (target_tokens as f64 * 3.0) as usize;
        truncate_with_suffix(content, estimated_chars)
    }

    /// Truncate preserving beginning and end.
    pub fn truncate_preserve_ends(content: &str, target_tokens: u32) -> String {
        let estimated_chars = (target_tokens as f64 * 3.0) as usize;

        if content.len() <= estimated_chars {
            return content.to_string();
        }

        // Split: 60% beginning, 40% end
        let begin_len = (estimated_chars as f64 * 0.6) as usize;
        let end_len = estimated_chars.saturating_sub(begin_len).saturating_sub(20); // Leave room for "..."

        if end_len == 0 {
            // Too short, just truncate
            return truncate_with_suffix(content, estimated_chars);
        }

        let begin = truncate_with_suffix(content, begin_len);
        let end_start = content.len().saturating_sub(end_len);

        // Find safe UTF-8 boundary
        let end_start = find_char_boundary(content, end_start);
        let end = &content[end_start..];

        format!("{}...\n[内容截断]\n...{}", begin, end)
    }

    /// Check if content needs summarization.
    pub fn needs_summary(content: &str, threshold_tokens: u32) -> bool {
        estimate_tokens_str(content) >= threshold_tokens
    }
}

/// Build a summary request.
fn build_summary_request(prompt: String) -> ChatRequest {
    ChatRequest {
        messages: vec![Message {
            role: Role::User,
            content: MessageContent::Text(prompt),
        }],
        tools: vec![],
        system: Some(SUMMARY_SYSTEM_PROMPT.to_string()),
        think: false,
        max_tokens: 512,
        server_tools: vec![],
        enable_caching: false,
    }
}

/// Extract summary text from response.
fn extract_summary_text(response: &crate::providers::ChatResponse) -> String {
    response
        .content
        .iter()
        .filter_map(|b| {
            if let ContentBlock::Text { text } = b {
                Some(text.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Find a safe UTF-8 character boundary.
fn find_char_boundary(s: &str, max: usize) -> usize {
    let max = max.min(s.len());
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    end
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

const SUMMARY_SYSTEM_PROMPT: &str = r#"CRITICAL: 仅用文本响应。不要调用任何工具。

你是一个内容摘要助手。将长内容压缩为结构化摘要。

输出要求：
- 结构化：使用关键信息列表格式
- 关键：保留重要操作、决策、结果
- 简洁：控制在指定字数以内

输出格式：
【操作】执行的主要操作
【结果】关键输出或结果
【要点】重要发现或注意事项

输出摘要后立即停止。
请直接输出摘要内容。"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smart_truncate() {
        let content = "这是一段很长的内容需要截断处理";
        let result = Summarizer::smart_truncate(content, 5);
        assert!(result.len() <= 20);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_truncate_preserve_ends() {
        // Need longer content to trigger truncation (500 tokens ~ 1500 chars)
        let content = "开头内容中间很长的部分结尾内容".repeat(50);
        let result = Summarizer::truncate_preserve_ends(&content, 100);
        assert!(result.contains("开头"));
        assert!(result.contains("结尾"));
        assert!(result.contains("[内容截断]"));
    }

    #[test]
    fn test_needs_summary() {
        let short = "短内容";
        assert!(!Summarizer::needs_summary(short, 100));

        let long = "这是一段很长的内容...".repeat(100);
        assert!(Summarizer::needs_summary(&long, 100));
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
