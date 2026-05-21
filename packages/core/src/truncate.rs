//! Truncation utilities for safe string handling.
//!
//! Provides functions to truncate strings while respecting UTF-8 boundaries.

/// Find a safe UTF-8 boundary position.
/// Returns the largest position <= max that is a valid char boundary.
pub fn find_boundary(s: &str, max: usize) -> usize {
    let max = max.min(s.len());
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    end
}

/// Truncate string to max bytes, respecting UTF-8 boundaries.
/// Does not add any suffix.
pub fn truncate_bytes(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let end = find_boundary(s, max);
    &s[..end]
}

/// Truncate string to max bytes with "..." suffix.
/// Respects UTF-8 boundaries.
pub fn truncate_with_suffix(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let suffix = "...";
    let suffix_len = suffix.len();
    let end = find_boundary(s, max.saturating_sub(suffix_len));
    format!("{}{}", &s[..end], suffix)
}

/// Truncate string to max characters (not bytes).
/// Useful for display purposes where visual length matters.
pub fn truncate_chars(s: &str, max_chars: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_chars {
        return s.to_string();
    }
    let suffix = "...";
    let take_chars = max_chars.saturating_sub(suffix.chars().count());
    s.chars().take(take_chars).collect::<String>() + suffix
}

/// Truncate string and modify in place (for String types).
pub fn truncate_string_in_place(s: &mut String, max: usize) {
    if s.len() <= max {
        return;
    }
    let end = find_boundary(s, max);
    s.truncate(end);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_boundary_ascii() {
        let s = "hello world";
        assert_eq!(find_boundary(s, 5), 5);
        assert_eq!(find_boundary(s, 100), 11);
    }

    #[test]
    fn test_find_boundary_multibyte() {
        // Chinese: each char is 3 bytes
        let s = "你好世界";
        assert_eq!(find_boundary(s, 4), 3); // Falls in middle of '好', back to '你'
        assert_eq!(find_boundary(s, 6), 6); // Exactly at boundary of '世'
        assert_eq!(find_boundary(s, 7), 6); // Falls in '世', back to 6
    }

    #[test]
    fn test_truncate_bytes() {
        let s = "hello";
        assert_eq!(truncate_bytes(s, 10), "hello");
        assert_eq!(truncate_bytes(s, 3), "hel");
    }

    #[test]
    fn test_truncate_bytes_chinese() {
        let s = "你好世界";
        assert_eq!(truncate_bytes(s, 100), "你好世界");
        assert_eq!(truncate_bytes(s, 5), "你"); // 3 bytes, not 5 (boundary at 3)
    }

    #[test]
    fn test_truncate_with_suffix() {
        let s = "hello world";
        assert_eq!(truncate_with_suffix(s, 100), "hello world");
        assert_eq!(truncate_with_suffix(s, 8), "hello...");
        assert_eq!(truncate_with_suffix(s, 5), "he...");
    }

    #[test]
    fn test_truncate_chars() {
        let s = "你好世界hello";
        assert_eq!(truncate_chars(s, 10), "你好世界hello");
        // max_chars=4: take 4-3=1 char + "..." = "你..."
        assert_eq!(truncate_chars(s, 4), "你...");
        // max_chars=6: take 6-3=3 chars + "..." = "你好世..."
        assert_eq!(truncate_chars(s, 6), "你好世...");
        // max_chars=7: take 7-3=4 chars + "..." = "你好世界..."
        assert_eq!(truncate_chars(s, 7), "你好世界...");
    }

    #[test]
    fn test_truncate_in_place() {
        let mut s = "hello world".to_string();
        truncate_string_in_place(&mut s, 5);
        assert_eq!(s, "hello");

        let mut s2 = "你好世界".to_string();
        truncate_string_in_place(&mut s2, 5);
        assert_eq!(s2, "你"); // 3 bytes, not 5
    }
}
