//! Accurate token counting using tiktoken.
//!
//! Provides precise token counting for messages to enable better
//! context window management.

use once_cell::sync::Lazy;
use std::sync::Arc;
use tiktoken_rs::CoreBPE;

/// Global BPE encoder for token counting (cl100k_base for GPT-4/Claude).
static BPE: Lazy<Arc<CoreBPE>> = Lazy::new(|| {
    Arc::new(tiktoken_rs::cl100k_base().expect("Failed to initialize tokenizer"))
});

/// Count tokens in a text string.
pub fn count_tokens(text: &str) -> u32 {
    BPE.encode_with_special_tokens(text).len() as u32
}

/// Count tokens for a role prefix (e.g., "user: ", "assistant: ").
/// Each message has overhead for role markers and formatting.
pub fn message_overhead() -> u32 {
    // Approximate overhead per message:
    // - Role prefix: ~4 tokens
    // - Message separators: ~2 tokens
    // - Total overhead: ~6 tokens
    6
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_tokens_simple() {
        let text = "Hello, world!";
        let count = count_tokens(text);
        assert!(count > 0);
        // "Hello, world!" is typically 4 tokens
        assert!(count >= 3 && count <= 5);
    }

    #[test]
    fn test_count_tokens_chinese() {
        let text = "你好，世界！";
        let count = count_tokens(text);
        assert!(count > 0);
        // Chinese characters typically use more tokens
        assert!(count >= 5);
    }

    #[test]
    fn test_count_tokens_code() {
        let code = r#"
fn main() {
    println!("Hello");
}
"#;
        let count = count_tokens(code);
        assert!(count > 0);
        // Code typically uses more tokens due to symbols
        // Actual count is around 13 tokens
        assert!(count >= 10, "Code should use at least 10 tokens, got {}", count);
    }

    #[test]
    fn test_message_overhead() {
        let overhead = message_overhead();
        assert_eq!(overhead, 6);
    }

    #[test]
    fn test_token_counting_accuracy() {
        // Compare with known token counts
        // The phrase "Hello, world!" is 4 tokens in cl100k_base
        assert_eq!(count_tokens("Hello, world!"), 4);
        
        // Single word
        assert_eq!(count_tokens("Hello"), 1);
        
        // Numbers
        assert_eq!(count_tokens("12345"), 2); // "123" + "45"
        
        // Chinese characters (each typically 1-2 tokens)
        let chinese = "你好世界";
        let chinese_count = count_tokens(chinese);
        assert!(chinese_count >= 4, "Chinese text should use at least 4 tokens, got {}", chinese_count);
        
        // Empty string
        assert_eq!(count_tokens(""), 0);
        
        // Whitespace
        assert_eq!(count_tokens("   "), 1);
    }
}