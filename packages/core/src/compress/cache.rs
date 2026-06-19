//! Compression cache to avoid redundant compression operations.
//!
//! This module provides caching for compression results to improve
//! performance when dealing with repeated compression of similar content.
//!
//! # Extended Cache Features
//! - 焦点预测缓存
//! - 优先级分数缓存
//! - Coherence 分段缓存
//! - 复杂度分析缓存

use crate::providers::Message;
use crate::compress::complexity::ComplexityLevel;
use crate::compress::focus_point::FocusPoint;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use chrono::{DateTime, Utc};

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

/// Cached priority score with validity tracking
#[derive(Debug, Clone)]
pub struct CachedPriorityScore {
    /// Priority score value
    pub score: f32,
    /// When the score was calculated
    pub calculated_at: DateTime<Utc>,
    /// How long the score remains valid
    pub valid_for: Duration,
    /// Keywords that influenced this score
    pub keywords: Vec<String>,
}

impl CachedPriorityScore {
    pub fn new(score: f32, keywords: Vec<String>, valid_for: Duration) -> Self {
        Self {
            score,
            calculated_at: Utc::now(),
            valid_for,
            keywords,
        }
    }
    
    pub fn is_valid(&self) -> bool {
        let now = Utc::now();
        // Safe conversion: valid_for is typically seconds/minutes, well within chrono::Duration range
        let duration = chrono::Duration::from_std(self.valid_for)
            .unwrap_or_else(|_| chrono::Duration::zero());
        now - self.calculated_at < duration
    }
}

/// Cached focus prediction
#[derive(Debug, Clone)]
pub struct CachedFocusPrediction {
    /// Predicted focus point
    pub focus: FocusPoint,
    /// Prediction confidence
    pub confidence: f32,
    /// When the prediction was made
    pub predicted_at: DateTime<Utc>,
}

impl CachedFocusPrediction {
    pub fn new(focus: FocusPoint, confidence: f32) -> Self {
        Self {
            focus,
            confidence,
            predicted_at: Utc::now(),
        }
    }
}

/// Cached complexity level
#[derive(Debug, Clone)]
pub struct CachedComplexity {
    /// Complexity level
    pub level: ComplexityLevel,
    /// When the analysis was performed
    pub analyzed_at: DateTime<Utc>,
}

impl CachedComplexity {
    pub fn new(level: ComplexityLevel) -> Self {
        Self {
            level,
            analyzed_at: Utc::now(),
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

        // Check if entry exists and is not expired
        if let Some(entry) = self.entries.get(&hash) {
            if entry.created_at.elapsed() < self.config.ttl {
                self.stats.hits += 1;
                // Update hit count - safe unwrap since we just verified entry exists
                if let Some(mut_entry) = self.entries.get_mut(&hash) {
                    mut_entry.hit_count += 1;
                }
                // Return the entry
                return self.entries.get(&hash);
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

/// 扩展压缩缓存：支持多种类型的缓存
/// 
/// 包括：焦点预测、优先级分数、复杂度分析等
#[derive(Debug)]
pub struct ExtendedCompressionCache {
    /// 基础摘要缓存（现有）
    base_cache: CompressionCache,
    
    /// 焦点预测缓存：message_id -> predicted focus
    focus_predictions: HashMap<String, CachedFocusPrediction>,
    
    /// 优先级分数缓存：message_id -> priority score
    priority_scores: HashMap<String, CachedPriorityScore>,
    
    /// 复杂度分析缓存：conversation_id -> complexity
    complexity_cache: HashMap<String, CachedComplexity>,
    
    /// 配置
    config: ExtendedCacheConfig,
}

/// 扩展缓存配置
#[derive(Debug, Clone)]
pub struct ExtendedCacheConfig {
    /// 焦点预测缓存最大数量
    max_focus_predictions: usize,
    /// 复杂度缓存最大数量
    max_complexity_entries: usize,
}

impl Default for ExtendedCacheConfig {
    fn default() -> Self {
        Self {
            max_focus_predictions: 50,
            max_complexity_entries: 20,
        }
    }
}

impl Default for ExtendedCompressionCache {
    fn default() -> Self {
        Self::new(ExtendedCacheConfig::default())
    }
}

impl ExtendedCompressionCache {
    pub fn new(config: ExtendedCacheConfig) -> Self {
        Self {
            base_cache: CompressionCache::default(),
            focus_predictions: HashMap::new(),
            priority_scores: HashMap::new(),
            complexity_cache: HashMap::new(),
            config,
        }
    }
    
    /// 获取优先级分数（如果缓存有效）
    pub fn get_priority_score(&self, message_id: &str) -> Option<&CachedPriorityScore> {
        self.priority_scores.get(message_id)
            .filter(|cached| cached.is_valid())
    }
    
    /// 添加优先级分数缓存
    pub fn put_priority_score(&mut self, message_id: String, score: CachedPriorityScore) {
        // 如果超出容量，移除最旧的
        if self.priority_scores.len() >= self.config.max_focus_predictions {
            self.evict_oldest_priority();
        }
        
        self.priority_scores.insert(message_id, score);
    }
    
    /// 获取焦点预测
    pub fn get_focus_prediction(&self, message_id: &str) -> Option<&CachedFocusPrediction> {
        self.focus_predictions.get(message_id)
    }
    
    /// 添加焦点预测缓存
    pub fn put_focus_prediction(&mut self, message_id: String, prediction: CachedFocusPrediction) {
        if self.focus_predictions.len() >= self.config.max_focus_predictions {
            self.evict_oldest_focus();
        }
        
        self.focus_predictions.insert(message_id, prediction);
    }
    
    /// 获取复杂度分析
    pub fn get_complexity(&self, conversation_id: &str) -> Option<&CachedComplexity> {
        self.complexity_cache.get(conversation_id)
    }
    
    /// 添加复杂度缓存
    pub fn put_complexity(&mut self, conversation_id: String, complexity: CachedComplexity) {
        if self.complexity_cache.len() >= self.config.max_complexity_entries {
            self.evict_oldest_complexity();
        }
        
        self.complexity_cache.insert(conversation_id, complexity);
    }
    
    /// 增量更新优先级分数（基于新消息）
    pub fn update_priority_incremental(&mut self, new_keywords: &[String], _existing_messages: &[Message]) {
        let now = Utc::now();

        for cached in self.priority_scores.values_mut() {
            // 检查关键词重叠
            let overlap_count = cached.keywords.iter()
                .filter(|kw| new_keywords.contains(kw))
                .count();
            
            // 如果有重叠，更新相关性分数
            if overlap_count > 0 {
                cached.score += overlap_count as f32 * 0.1;
                cached.calculated_at = now;
            }
        }
    }
    
    /// 清理过期缓存
    pub fn cleanup_expired(&mut self) {
        // 清理过期的优先级分数
        self.priority_scores.retain(|_, cached| cached.is_valid());
        
        // 清理基础缓存过期项
        self.base_cache.evict_expired();
    }
    
    /// 移除最旧的优先级分数
    fn evict_oldest_priority(&mut self) {
        if let Some((oldest_id, _)) = self.priority_scores.iter()
            .min_by_key(|(_, cached)| cached.calculated_at)
        {
            let id = oldest_id.clone();
            self.priority_scores.remove(&id);
        }
    }
    
    /// 移除最旧的焦点预测
    fn evict_oldest_focus(&mut self) {
        if let Some((oldest_id, _)) = self.focus_predictions.iter()
            .min_by_key(|(_, cached)| cached.predicted_at)
        {
            let id = oldest_id.clone();
            self.focus_predictions.remove(&id);
        }
    }
    
    /// 移除最旧的复杂度缓存
    fn evict_oldest_complexity(&mut self) {
        if let Some((oldest_id, _)) = self.complexity_cache.iter()
            .min_by_key(|(_, cached)| cached.analyzed_at)
        {
            let id = oldest_id.clone();
            self.complexity_cache.remove(&id);
        }
    }
    
    /// 获取基础缓存
    pub fn base_cache(&self) -> &CompressionCache {
        &self.base_cache
    }
    
    /// 获取基础缓存（可变）
    pub fn base_cache_mut(&mut self) -> &mut CompressionCache {
        &mut self.base_cache
    }
    
    /// 清空所有缓存
    pub fn clear_all(&mut self) {
        self.base_cache.clear();
        self.focus_predictions.clear();
        self.priority_scores.clear();
        self.complexity_cache.clear();
    }
    
    /// 获取缓存统计
    pub fn extended_stats(&self) -> ExtendedCacheStats {
        ExtendedCacheStats {
            base_stats: self.base_cache.stats().clone(),
            focus_prediction_count: self.focus_predictions.len(),
            priority_score_count: self.priority_scores.len(),
            complexity_cache_count: self.complexity_cache.len(),
        }
    }
}

/// 扩展缓存统计
#[derive(Debug, Clone)]
pub struct ExtendedCacheStats {
    pub base_stats: CacheStats,
    pub focus_prediction_count: usize,
    pub priority_score_count: usize,
    pub complexity_cache_count: usize,
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
        let original = create_test_message("This is a test message that is long enough to be cached, it needs to be at least 100 characters to pass the minimum size threshold");
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
        let original = create_test_message("This is a longer test message for caching purposes, it needs to be at least 100 characters long to be cached properly");
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

        let msg1 = create_test_message("Message 1 - long enough for caching, needs at least 100 characters to be stored in the cache system properly");
        let msg2 = create_test_message("Message 2 - also long enough, needs at least 100 characters for caching in our compression cache system");
        let msg3 = create_test_message("Message 3 - this one too, needs at least 100 characters to be cached in the compression cache system");

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
        let msg = create_test_message("Long enough message for the cache system, needs at least 100 characters to be cached in our compression cache properly");

        cache.put(&msg, msg.clone());
        assert!(!cache.is_empty());

        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_stats() {
        let mut cache = CompressionCache::default();
        let msg = create_test_message("This is a test message for statistics tracking, needs at least 100 characters to be cached in our compression cache system");

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