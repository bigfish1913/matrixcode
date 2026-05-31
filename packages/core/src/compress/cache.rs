//! Compression cache to avoid redundant compression operations.
//!
//! This module provides caching for compression results to improve
//! performance when dealing with repeated compression of similar content.

use crate::providers::Message;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Cache entry for a compressed message.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// Compressed message
    pub compressed: Message,
    /// Original content hash
    pub hash: u64,
    /// When the entry was created
    pub created_at: Instant,
    /// Number of times this entry was used
    pub hit_count: usize,
}

/// Statistics for the compression cache.
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub hits: usize,
    pub misses: usize,
    pub entries: usize,
    pub total_saved_tokens: u32,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f32 {
        if self.hits + self.misses == 0 {
            0.0
        } else {
            self.hits as f32 / (self.hits + self.misses) as f32
        }
    }
}

/// Compression cache configuration.
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Maximum number of entries
    pub max_entries: usize,
    /// Time-to-live for cache entries
    pub ttl: Duration,
    /// Minimum message size to cache (in characters)
    pub min_size_to_cache: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 100,
            ttl: Duration::from_secs(300), // 5 minutes
            min_size_to_cache: 100,       // Only cache messages > 100 chars
        }
    }
}

/// Compression cache implementation.
#[derive(Debug)]
pub struct CompressionCache {
    entries: HashMap<u64, CacheEntry>,
    config: CacheConfig,
    stats: CacheStats,
}

impl Default for CompressionCache {
    fn default() -> Self {
        Self::new(CacheConfig::default())
    }
}

impl CompressionCache {
    pub fn new(config: CacheConfig) -> Self {
        Self {
            entries: HashMap::new(),
            config,
            stats: CacheStats::default(),
        }
    }

    /// Calculate hash for a message.
    fn hash_message(message: &Message) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        
        // Hash role as string
        let role_str = match message.role {
            crate::providers::Role::User => "user",
            crate::providers::Role::Assistant => "assistant",
            crate::providers::Role::System => "system",
            crate::providers::Role::Tool => "tool",
        };
        role_str.hash(&mut hasher);

        // Hash content
        match &message.content {
            crate::providers::MessageContent::Text(text) => {
                text.hash(&mut hasher);
            }
            crate::providers::MessageContent::Blocks(blocks) => {
                // Hash each block's string representation
                for block in blocks {
                    let block_str = format!("{:?}", block);
                    block_str.hash(&mut hasher);
                }
            }
        }

        hasher.finish()
    }

    /// Check if a message is in the cache.
    pub fn get(&mut self, message: &Message) -> Option<&CacheEntry> {
        let hash = Self::hash_message(message);

        if let Some(entry) = self.entries.get(&hash) {
            // Check TTL
            if entry.created_at.elapsed() < self.config.ttl {
                self.stats.hits += 1;
                let entry = self.entries.get_mut(&hash).unwrap();
                entry.hit_count += 1;
                return Some(entry);
            } else {
                // Expired, remove it
                self.entries.remove(&hash);
            }
        }

        self.stats.misses += 1;
        None
    }

    /// Add a compressed message to the cache.
    pub fn put(&mut self, original: &Message, compressed: Message) {
        let hash = Self::hash_message(original);

        // Check minimum size
        let size = match &original.content {
            crate::providers::MessageContent::Text(text) => text.len(),
            crate::providers::MessageContent::Blocks(blocks) => {
                blocks.iter().map(|b| format!("{:?}", b).len()).sum()
            }
        };

        if size < self.config.min_size_to_cache {
            return;
        }

        // Evict old entries if at capacity
        if self.entries.len() >= self.config.max_entries {
            self.evict_oldest();
        }

        self.entries.insert(
            hash,
            CacheEntry {
                compressed,
                hash,
                created_at: Instant::now(),
                hit_count: 0,
            },
        );
        self.stats.entries = self.entries.len();
    }

    /// Evict the oldest entry.
    fn evict_oldest(&mut self) {
        if let Some((&oldest_hash, _)) = self
            .entries
            .iter()
            .min_by_key(|(_, entry)| entry.created_at)
        {
            self.entries.remove(&oldest_hash);
        }
    }

    /// Evict expired entries.
    pub fn evict_expired(&mut self) {
        let now = Instant::now();
        self.entries.retain(|_, entry| {
            now.duration_since(entry.created_at) < self.config.ttl
        });
        self.stats.entries = self.entries.len();
    }

    /// Clear the cache.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.stats.entries = 0;
    }

    /// Get cache statistics.
    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }

    /// Get the number of cached entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Record token savings from cache hit.
    pub fn record_token_savings(&mut self, tokens: u32) {
        self.stats.total_saved_tokens += tokens;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::{MessageContent, Role};

    fn create_test_message(content: &str) -> Message {
        Message {
            role: Role::User,
            content: MessageContent::Text(content.to_string()),
        }
    }

    #[test]
    fn test_cache_put_and_get() {
        let mut cache = CompressionCache::default();
        let original = create_test_message("This is a test message that is long enough to be cached");
        let compressed = create_test_message("This is a test message...");

        // Put in cache
        cache.put(&original, compressed.clone());

        // Get from cache
        let entry = cache.get(&original);
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().hit_count, 1);
    }

    #[test]
    fn test_cache_miss() {
        let mut cache = CompressionCache::default();
        let msg = create_test_message("Test message");

        let entry = cache.get(&msg);
        assert!(entry.is_none());
        assert_eq!(cache.stats().misses, 1);
    }

    #[test]
    fn test_cache_hit_increments_counter() {
        let mut cache = CompressionCache::default();
        let original = create_test_message("This is a longer test message for caching purposes");
        let compressed = create_test_message("Longer test message...");

        cache.put(&original, compressed);

        // Get multiple times
        cache.get(&original);
        cache.get(&original);
        cache.get(&original);

        assert_eq!(cache.stats().hits, 3);
    }

    #[test]
    fn test_cache_minimum_size() {
        let config = CacheConfig {
            min_size_to_cache: 50,
            ..Default::default()
        };
        let mut cache = CompressionCache::new(config);

        let small_msg = create_test_message("Short");
        let compressed = create_test_message("...");

        cache.put(&small_msg, compressed);

        // Should not be cached (too small)
        assert!(cache.get(&small_msg).is_none());
    }

    #[test]
    fn test_cache_eviction() {
        let config = CacheConfig {
            max_entries: 2,
            ..Default::default()
        };
        let mut cache = CompressionCache::new(config);

        let msg1 = create_test_message("Message 1 - long enough for caching");
        let msg2 = create_test_message("Message 2 - also long enough");
        let msg3 = create_test_message("Message 3 - this one too");

        cache.put(&msg1, msg1.clone());
        cache.put(&msg2, msg2.clone());
        assert_eq!(cache.len(), 2);

        // Adding a third should evict the oldest
        cache.put(&msg3, msg3.clone());
        assert_eq!(cache.len(), 2);

        // msg1 should have been evicted
        assert!(cache.get(&msg1).is_none());
        assert!(cache.get(&msg2).is_some());
        assert!(cache.get(&msg3).is_some());
    }

    #[test]
    fn test_cache_clear() {
        let mut cache = CompressionCache::default();
        let msg = create_test_message("Long enough message for the cache system");

        cache.put(&msg, msg.clone());
        assert!(!cache.is_empty());

        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_stats() {
        let mut cache = CompressionCache::default();
        let msg = create_test_message("This is a test message for statistics tracking");

        // Miss
        cache.get(&msg);
        assert_eq!(cache.stats().misses, 1);
        assert_eq!(cache.stats().hits, 0);

        // Put and hit
        cache.put(&msg, msg.clone());
        cache.get(&msg);
        assert_eq!(cache.stats().hits, 1);

        // Hit rate
        assert_eq!(cache.stats().hit_rate(), 0.5);
    }

    #[test]
    fn test_message_hash_consistency() {
        let msg1 = create_test_message("Test message");
        let msg2 = create_test_message("Test message");
        let msg3 = create_test_message("Different message");

        let hash1 = CompressionCache::hash_message(&msg1);
        let hash2 = CompressionCache::hash_message(&msg2);
        let hash3 = CompressionCache::hash_message(&msg3);

        // Same content should have same hash
        assert_eq!(hash1, hash2);
        // Different content should have different hash
        assert_ne!(hash1, hash3);
    }
}