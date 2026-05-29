//! Section Cache System
//!
//! Provides caching for static sections to:
//! - Reduce token costs (cached content not re-computed)
//! - Enable prompt prefix caching for API efficiency
//! - Track cache statistics for optimization

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Cache key for a section
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct CacheKey {
    /// Section name
    pub name: String,
    /// Profile (default, safe, fast, review)
    pub profile: String,
    /// Optional hash of content for validation
    pub content_hash: Option<u64>,
}

impl CacheKey {
    pub fn new(name: impl Into<String>, profile: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            profile: profile.into(),
            content_hash: None,
        }
    }

    pub fn with_hash(self, hash: u64) -> Self {
        Self {
            content_hash: Some(hash),
            ..self
        }
    }
}

/// Cached entry with metadata
#[derive(Debug, Clone)]
pub struct CachedEntry {
    /// Cached content
    pub content: String,
    /// When it was cached
    pub cached_at: Instant,
    /// Estimated token count
    pub token_count: usize,
    /// Number of times used
    pub use_count: u64,
}

impl CachedEntry {
    pub fn new(content: String) -> Self {
        let token_count = estimate_tokens(&content);
        Self {
            content,
            cached_at: Instant::now(),
            token_count,
            use_count: 0,
        }
    }

    /// Check if entry is expired
    pub fn is_expired(&self, max_age: Duration) -> bool {
        self.cached_at.elapsed() > max_age
    }

    /// Mark as used
    pub fn mark_used(&mut self) {
        self.use_count += 1;
    }
}

/// Section cache with statistics
pub struct SectionCache {
    /// Cached entries
    entries: RwLock<HashMap<CacheKey, CachedEntry>>,
    /// Maximum cache age
    max_age: Duration,
    /// Statistics
    stats: RwLock<CacheStats>,
}

/// Cache statistics
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// Total cached entries
    pub total_entries: usize,
    /// Total hits
    pub total_hits: u64,
    /// Total misses
    pub total_misses: u64,
    /// Total evictions
    pub total_evictions: u64,
    /// Estimated tokens saved
    pub tokens_saved: u64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        if self.total_hits + self.total_misses == 0 {
            0.0
        } else {
            self.total_hits as f64 / (self.total_hits + self.total_misses) as f64
        }
    }
}

impl SectionCache {
    /// Create a new cache with default max age
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            max_age: Duration::from_secs(3600), // 1 hour default
            stats: RwLock::new(CacheStats::default()),
        }
    }

    /// Create cache with custom max age
    pub fn with_max_age(max_age: Duration) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            max_age,
            stats: RwLock::new(CacheStats::default()),
        }
    }

    /// Get a cached entry
    pub fn get(&self, key: &CacheKey) -> Option<String> {
        let mut entries = self.entries.write().unwrap();
        let mut stats = self.stats.write().unwrap();

        if let Some(entry) = entries.get_mut(key) {
            if entry.is_expired(self.max_age) {
                // Expired, remove and count as miss
                entries.remove(key);
                stats.total_misses += 1;
                stats.total_evictions += 1;
                None
            } else {
                // Valid, mark as used
                entry.mark_used();
                stats.total_hits += 1;
                stats.tokens_saved += entry.token_count as u64;
                Some(entry.content.clone())
            }
        } else {
            stats.total_misses += 1;
            None
        }
    }

    /// Set a cached entry
    pub fn set(&self, key: CacheKey, content: String) {
        let mut entries = self.entries.write().unwrap();
        let mut stats = self.stats.write().unwrap();

        let entry = CachedEntry::new(content);
        entries.insert(key, entry);
        stats.total_entries = entries.len();
    }

    /// Get or compute (cache miss pattern)
    pub fn get_or_compute<F>(&self, key: &CacheKey, compute: F) -> String
    where
        F: FnOnce() -> String,
    {
        if let Some(cached) = self.get(key) {
            cached
        } else {
            let content = compute();
            self.set(key.clone(), content.clone());
            content
        }
    }

    /// Clear all cache entries
    pub fn clear(&self) {
        let mut entries = self.entries.write().unwrap();
        let mut stats = self.stats.write().unwrap();

        let evicted = entries.len();
        entries.clear();
        stats.total_entries = 0;
        stats.total_evictions += evicted as u64;
    }

    /// Clear entries for a specific profile
    pub fn clear_profile(&self, profile: &str) {
        let mut entries = self.entries.write().unwrap();
        let mut stats = self.stats.write().unwrap();

        entries.retain(|k, _| k.profile != profile);
        stats.total_entries = entries.len();
    }

    /// Get statistics
    pub fn stats(&self) -> CacheStats {
        self.stats.read().unwrap().clone()
    }

    /// Get total cached token count
    pub fn cached_tokens(&self) -> usize {
        let entries = self.entries.read().unwrap();
        entries.values().map(|e| e.token_count).sum()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.entries.read().unwrap().is_empty()
    }

    /// Get cache size
    pub fn size(&self) -> usize {
        self.entries.read().unwrap().len()
    }
}

impl Default for SectionCache {
    fn default() -> Self {
        Self::new()
    }
}

// Note: Clone implementation removed - use Arc<SectionCache> for sharing
// Full cloning of potentially large cache entries is expensive and unnecessary
// when Arc provides cheap reference counting

/// Estimate token count for content
pub fn estimate_tokens(content: &str) -> usize {
    // Rough estimate:
    // - Chinese: ~3 chars per token (each Chinese char is ~1 token, but /3 for safety)
    // - English words: ~1 token per word
    // - Other ASCII chars: ~4 chars per token
    let chinese_chars = content
        .chars()
        .filter(|c| c.is_alphabetic() && c.len_utf8() > 1)
        .count();
    let english_words = content.split_whitespace().count();
    let non_whitespace: usize = content.chars().filter(|c| !c.is_whitespace()).count();

    // Fallback: if no words detected (no whitespace), use char count / 4
    let fallback_estimate = if english_words == 0 && non_whitespace > 0 {
        non_whitespace / 4
    } else {
        0
    };

    chinese_chars / 3 + english_words + fallback_estimate
}

/// Global cache instance
static GLOBAL_CACHE: std::sync::OnceLock<Arc<SectionCache>> = std::sync::OnceLock::new();

/// Get the global section cache
pub fn global_cache() -> Arc<SectionCache> {
    GLOBAL_CACHE
        .get_or_init(|| Arc::new(SectionCache::new()))
        .clone()
}

/// Clear the global cache (for /clear, /compact, worktree switch)
pub fn clear_global_cache() {
    global_cache().clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_basic() {
        let cache = SectionCache::new();
        let key = CacheKey::new("test", "default");

        // Miss
        assert!(cache.get(&key).is_none());

        // Set
        cache.set(key.clone(), "test content".to_string());

        // Hit
        assert_eq!(cache.get(&key), Some("test content".to_string()));

        // Stats
        let stats = cache.stats();
        assert_eq!(stats.total_hits, 1);
        assert_eq!(stats.total_misses, 1);
    }

    #[test]
    fn test_cache_expiry() {
        let cache = SectionCache::with_max_age(Duration::from_millis(10));
        let key = CacheKey::new("test", "default");

        cache.set(key.clone(), "test".to_string());

        // Wait for expiry
        std::thread::sleep(Duration::from_millis(20));

        // Should be expired
        assert!(cache.get(&key).is_none());
        let stats = cache.stats();
        assert_eq!(stats.total_evictions, 1);
    }

    #[test]
    fn test_get_or_compute() {
        let cache = SectionCache::new();
        let key = CacheKey::new("compute", "default");

        let result = cache.get_or_compute(&key, || "computed".to_string());
        assert_eq!(result, "computed");

        // Second call should use cache
        let result2 = cache.get_or_compute(&key, || "different".to_string());
        assert_eq!(result2, "computed"); // Still cached value
    }

    #[test]
    fn test_clear_profile() {
        let cache = SectionCache::new();

        cache.set(CacheKey::new("a", "default"), "a".to_string());
        cache.set(CacheKey::new("b", "safe"), "b".to_string());

        cache.clear_profile("default");

        assert!(cache.get(&CacheKey::new("a", "default")).is_none());
        assert_eq!(
            cache.get(&CacheKey::new("b", "safe")),
            Some("b".to_string())
        );
    }

    #[test]
    fn test_estimate_tokens() {
        let english = "Hello world this is a test";
        let chinese = "你好世界这是一个测试";

        // English: 5 words, should be roughly 5-7 tokens
        let eng_tokens = estimate_tokens(english);
        assert!(
            eng_tokens >= 5 && eng_tokens <= 10,
            "English tokens: {}",
            eng_tokens
        );

        // Chinese: 9 chars / 3 = 3 tokens
        let ch_tokens = estimate_tokens(chinese);
        assert!(
            ch_tokens >= 2 && ch_tokens <= 10,
            "Chinese tokens: {}",
            ch_tokens
        );
    }

    #[test]
    fn test_global_cache() {
        clear_global_cache();
        let cache = global_cache();

        let key = CacheKey::new("global_test", "default");
        cache.set(key.clone(), "global content".to_string());

        // Should persist across calls
        let cache2 = global_cache();
        assert_eq!(cache2.get(&key), Some("global content".to_string()));

        clear_global_cache();
        assert!(cache2.get(&key).is_none());
    }
}
