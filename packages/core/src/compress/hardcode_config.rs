//! Unified hardcoded value configuration - eliminates all magic numbers.
//!
//! This module centralizes all hardcoded thresholds, limits, and constants
//! used across the compression system for easy tuning and consistency.

use serde::{Deserialize, Serialize};

/// Unified configuration for all hardcoded values in compression system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardcodeConfig {
    // ============================================================================
    // Text Length Thresholds
    // ============================================================================
    
    /// Minimum word length to consider as meaningful (for coherence, focus)
    pub min_word_length: usize,
    
    /// Minimum text length to consider as substantial message
    pub min_substantial_text_length: usize,
    
    /// Threshold for "long text" that needs compression
    pub long_text_threshold: usize,
    
    /// Threshold for "very long text" (needs aggressive compression)
    pub very_long_text_threshold: usize,
    
    /// Maximum text length before truncation in simple mode
    pub max_simple_truncation_length: usize,
    
    /// Threshold for considering content as "code-heavy"
    pub code_content_threshold: usize,
    
    /// Maximum context length before aggressive truncation
    pub max_context_length: usize,
    
    /// Threshold for message content to be preserved
    pub preserve_content_threshold: usize,
    
    // ============================================================================
    // Extraction Limits
    // ============================================================================
    
    /// Number of words to extract as fallback topic
    pub fallback_topic_word_count: usize,
    
    /// Number of sentences to extract in brief summary
    pub brief_summary_sentence_count: usize,
    
    /// Number of sentences to extract in detailed summary
    pub detailed_summary_sentence_count: usize,
    
    /// Maximum characters in extracted question
    pub max_question_extract_length: usize,
    
    /// Maximum sentences to keep in compressed output
    pub max_compressed_sentence_count: usize,
    
    /// Number of words to keep in short summary
    pub short_summary_word_count: usize,
    
    // ============================================================================
    // Message Count Thresholds
    // ============================================================================
    
    /// Minimum messages to trigger compression
    pub min_messages_for_compression: usize,
    
    /// Large conversation threshold (many messages)
    pub large_conversation_threshold: usize,
    
    /// Medium conversation threshold
    pub medium_conversation_threshold: usize,
    
    /// Maximum recent context snippets to keep
    pub max_recent_context_count: usize,
    
    /// Minimum focus history size to consider
    pub min_focus_history_size: usize,
    
    // ============================================================================
    // Question/Query Thresholds
    // ============================================================================
    
    /// Minimum question length (chars)
    pub min_question_length: usize,
    
    /// Maximum question length (chars)
    pub max_question_length: usize,
    
    /// Minimum sentence length to keep
    pub min_sentence_length: usize,
    
    /// Maximum compressed output length
    pub max_compressed_output_length: usize,
    
    // ============================================================================
    // Special Thresholds
    // ============================================================================
    
    /// Threshold for detecting code blocks (combined with keyword check)
    pub code_detection_length_threshold: usize,
    
    /// Maximum truncated context length
    pub max_truncated_context_length: usize,
    
    /// Maximum trimmed content length
    pub max_trimmed_content_length: usize,
    
    /// Summary length threshold
    pub summary_length_threshold: usize,
    
    /// Cache capacity for focus tracking
    pub focus_cache_capacity: usize,
}

impl Default for HardcodeConfig {
    fn default() -> Self {
        Self {
            // Text Length Thresholds
            min_word_length: 3,
            min_substantial_text_length: 20,
            long_text_threshold: 200,
            very_long_text_threshold: 500,
            max_simple_truncation_length: 200,
            code_content_threshold: 1000,
            max_context_length: 3000,
            preserve_content_threshold: 500,
            
            // Extraction Limits
            fallback_topic_word_count: 3,
            brief_summary_sentence_count: 2,
            detailed_summary_sentence_count: 5,
            max_question_extract_length: 100,
            max_compressed_sentence_count: 30,
            short_summary_word_count: 10,
            
            // Message Count Thresholds
            min_messages_for_compression: 1,
            large_conversation_threshold: 30,
            medium_conversation_threshold: 20,
            max_recent_context_count: 5,
            min_focus_history_size: 1,
            
            // Question/Query Thresholds
            min_question_length: 2,
            max_question_length: 30,
            min_sentence_length: 20,
            max_compressed_output_length: 30,
            
            // Special Thresholds
            code_detection_length_threshold: 1000,
            max_truncated_context_length: 3000,
            max_trimmed_content_length: 300,
            summary_length_threshold: 200,
            focus_cache_capacity: 100,
        }
    }
}

impl HardcodeConfig {
    /// Create config for simple conversations (more aggressive compression)
    pub fn simple_conversation() -> Self {
        Self {
            // Lower thresholds for early compression
            min_substantial_text_length: 10,
            long_text_threshold: 100,
            very_long_text_threshold: 300,
            
            // Fewer items to extract
            max_recent_context_count: 3,
            detailed_summary_sentence_count: 3,
            
            // Smaller limits
            max_question_extract_length: 80,
            fallback_topic_word_count: 2,
            
            ..Self::default()
        }
    }
    
    /// Create config for complex technical discussions (conservative compression)
    pub fn complex_technical() -> Self {
        Self {
            // Higher thresholds for preserving more content
            min_substantial_text_length: 30,
            long_text_threshold: 300,
            very_long_text_threshold: 800,
            
            // More items to extract
            max_recent_context_count: 7,
            detailed_summary_sentence_count: 8,
            
            // Larger limits
            max_question_extract_length: 150,
            fallback_topic_word_count: 5,
            max_compressed_sentence_count: 50,
            
            // Higher message thresholds
            large_conversation_threshold: 50,
            medium_conversation_threshold: 30,
            
            ..Self::default()
        }
    }
    
    /// Create config from complexity level
    pub fn from_complexity(level: crate::compress::complexity::ComplexityLevel) -> Self {
        match level {
            crate::compress::complexity::ComplexityLevel::High => Self::complex_technical(),
            crate::compress::complexity::ComplexityLevel::Medium => Self::default(),
            crate::compress::complexity::ComplexityLevel::Low => Self::simple_conversation(),
        }
    }
    
    /// Validate configuration
    pub fn validate(&self) -> bool {
        // All thresholds should be positive
        self.min_word_length > 0 &&
        self.min_substantial_text_length > 0 &&
        self.long_text_threshold > 0 &&
        self.fallback_topic_word_count > 0 &&
        self.max_question_extract_length > 0 &&
        
        // Logical constraints
        self.min_substantial_text_length < self.long_text_threshold &&
        self.long_text_threshold < self.very_long_text_threshold &&
        self.min_question_length < self.max_question_length &&
        self.brief_summary_sentence_count < self.detailed_summary_sentence_count
    }
    
    /// Check if text is "short" (below substantial threshold)
    pub fn is_short_text(&self, len: usize) -> bool {
        len < self.min_substantial_text_length
    }
    
    /// Check if text is "long" (needs compression)
    pub fn is_long_text(&self, len: usize) -> bool {
        len > self.long_text_threshold
    }
    
    /// Check if text is "very long" (needs aggressive compression)
    pub fn is_very_long_text(&self, len: usize) -> bool {
        len > self.very_long_text_threshold
    }
    
    /// Check if conversation is "large"
    pub fn is_large_conversation(&self, message_count: usize) -> bool {
        message_count > self.large_conversation_threshold
    }
    
    /// Check if conversation is "medium"
    pub fn is_medium_conversation(&self, message_count: usize) -> bool {
        message_count > self.medium_conversation_threshold
    }
    
    /// Check if word is meaningful
    pub fn is_meaningful_word(&self, word_len: usize) -> bool {
        word_len > self.min_word_length
    }
    
    /// Check if question is valid length
    pub fn is_valid_question_length(&self, len: usize) -> bool {
        len > self.min_question_length && len < self.max_question_length
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config() {
        let config = HardcodeConfig::default();
        assert!(config.validate());
        assert_eq!(config.min_word_length, 3);
        assert_eq!(config.long_text_threshold, 200);
    }
    
    #[test]
    fn test_simple_conversation_config() {
        let config = HardcodeConfig::simple_conversation();
        assert_eq!(config.min_substantial_text_length, 10);
        assert_eq!(config.max_recent_context_count, 3);
    }
    
    #[test]
    fn test_complex_technical_config() {
        let config = HardcodeConfig::complex_technical();
        assert_eq!(config.min_substantial_text_length, 30);
        assert_eq!(config.max_recent_context_count, 7);
    }
    
    #[test]
    fn test_helper_methods() {
        let config = HardcodeConfig::default();
        
        assert!(config.is_short_text(10));
        assert!(!config.is_short_text(30));
        
        assert!(config.is_long_text(300));
        assert!(!config.is_long_text(100));
        
        assert!(config.is_very_long_text(1000));
        assert!(!config.is_very_long_text(300));
        
        assert!(config.is_meaningful_word(5));
        assert!(!config.is_meaningful_word(2));
        
        assert!(config.is_valid_question_length(10));
        assert!(!config.is_valid_question_length(1));
    }
    
    #[test]
    fn test_conversation_size_checks() {
        let config = HardcodeConfig::default();
        
        assert!(config.is_large_conversation(50));
        assert!(!config.is_large_conversation(20));
        
        assert!(config.is_medium_conversation(25));
        assert!(!config.is_medium_conversation(15));
    }
}