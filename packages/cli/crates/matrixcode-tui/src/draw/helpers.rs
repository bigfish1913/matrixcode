//! Drawing helper functions.

/// Estimate tokens for a text string.
/// ASCII: ~0.25 tokens/char, Non-ASCII (Chinese): ~0.67 tokens/char
pub fn estimate_text_tokens(text: &str) -> u32 {
    let (ascii, non_ascii) = count_chars(text);
    let ascii_tokens = (ascii as f64 * 0.25).ceil() as u32;
    let non_ascii_tokens = (non_ascii as f64 * 0.67).ceil() as u32;
    ascii_tokens + non_ascii_tokens
}

/// Estimate tokens for message content.
pub fn estimate_message_tokens(content: &str) -> u32 {
    estimate_text_tokens(content) + 10
}

/// Count ASCII and non-ASCII characters.
pub fn count_chars(s: &str) -> (u32, u32) {
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