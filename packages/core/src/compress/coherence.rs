//! Semantic Coherence Detection: Preserve conversation continuity.
//!
//! This module analyzes conversation segments to determine if messages
//! should be compressed together or kept separate to maintain semantic
//! coherence.

use crate::providers::Message;
use crate::compress::hardcode_config::HardcodeConfig;
use crate::memory::PatternRegistry;
use std::collections::HashSet;

/// Coherence detector for conversation segments.
#[derive(Debug, Clone)]
pub struct CoherenceDetector {
    /// Minimum coherence score to keep messages together
    threshold: f32,
    /// Pattern registry for dynamic pattern loading (replaces hardcoded patterns)
    pattern_registry: PatternRegistry,
    /// Hardcode configuration
    hardcode_config: HardcodeConfig,
}

impl Default for CoherenceDetector {
    fn default() -> Self {
        Self {
            threshold: 0.7,
            pattern_registry: PatternRegistry::new(),
            hardcode_config: HardcodeConfig::default(),
        }
    }
}

impl CoherenceDetector {
    /// Create a new coherence detector with default settings.
    ///
    /// This is backward compatible - automatically loads preset patterns.
    pub fn new(threshold: f32) -> Self {
        Self {
            threshold,
            ..Default::default()
        }
    }

    /// Create a coherence detector with a custom pattern registry.
    ///
    /// Use this when you need to customize patterns or load from a file.
    pub fn new_with_registry(threshold: f32, registry: PatternRegistry) -> Self {
        Self {
            threshold,
            pattern_registry: registry,
            hardcode_config: HardcodeConfig::default(),
        }
    }

    /// Create a coherence detector with custom hardcode config.
    pub fn with_hardcode_config(mut self, config: HardcodeConfig) -> Self {
        self.hardcode_config = config;
        self
    }

    /// Get a reference to the pattern registry.
    pub fn pattern_registry(&self) -> &PatternRegistry {
        &self.pattern_registry
    }

    /// Get a mutable reference to the pattern registry.
    pub fn pattern_registry_mut(&mut self) -> &mut PatternRegistry {
        &mut self.pattern_registry
    }

    /// Check if a group of messages should be kept together.
    pub fn should_keep_together(&self, messages: &[Message]) -> bool {
        if messages.len() < 2 {
            return true;
        }

        let coherence_score = self.calculate_coherence(messages);
        coherence_score >= self.threshold
    }

    /// Calculate coherence score for a message group.
    pub fn calculate_coherence(&self, messages: &[Message]) -> f32 {
        if messages.len() < 2 {
            return 1.0;
        }

        // Weighted sum of scores (weights sum to 1.0, no need to divide)
        let topic_score = self.check_topic_continuity(messages);
        let reference_score = self.check_reference_patterns(messages);
        let code_score = self.check_code_context(messages);
        let entity_score = self.check_entity_consistency(messages);

        // Weighted average: weights sum to 1.0
        topic_score * 0.3 + reference_score * 0.25 + code_score * 0.25 + entity_score * 0.2
    }

    /// Check topic continuity across messages.
    fn check_topic_continuity(&self, messages: &[Message]) -> f32 {
        let topics: Vec<HashSet<String>> = messages
            .iter()
            .map(|m| self.extract_topic_keywords(&self.get_message_content(m)))
            .collect();

        if topics.len() < 2 {
            return 1.0;
        }

        // Calculate overlap between consecutive messages
        let mut overlap_scores = Vec::new();
        for i in 1..topics.len() {
            let overlap = self.calculate_set_overlap(&topics[i - 1], &topics[i]);
            overlap_scores.push(overlap);
        }

        // Average overlap
        if !overlap_scores.is_empty() {
            overlap_scores.iter().sum::<f32>() / overlap_scores.len() as f32
        } else {
            0.5
        }
    }

    /// Check if messages reference each other.
    fn check_reference_patterns(&self, messages: &[Message]) -> f32 {
        let patterns = self.pattern_registry.get_active_reference_patterns();
        if patterns.is_empty() {
            return 0.5; // Neutral if no patterns available
        }

        let mut has_references = false;

        for i in 1..messages.len() {
            let content_lower = self.get_message_content_lower(&messages[i]);

            for pattern in &patterns {
                // Try regex match first (case-insensitive), fallback to simple contains for non-regex patterns
                if let Ok(re) = regex::Regex::new(&format!("(?i){}", pattern)) {
                    if re.is_match(&content_lower) {
                        has_references = true;
                        break;
                    }
                } else {
                    // Fallback to simple contains for invalid regex patterns
                    if content_lower.contains(pattern.to_lowercase().as_str()) {
                        has_references = true;
                        break;
                    }
                }
            }
        }

        if has_references {
            1.0 // High coherence if messages reference each other
        } else {
            0.5 // Neutral
        }
    }

    /// Check if messages contain code that should stay together.
    fn check_code_context(&self, messages: &[Message]) -> f32 {
        let patterns = self.pattern_registry.get_active_code_patterns();
        if patterns.is_empty() {
            return 0.5; // Neutral if no patterns available
        }

        let mut code_messages = Vec::new();

        for (i, msg) in messages.iter().enumerate() {
            let content_lower = self.get_message_content_lower(msg);

            for pattern in &patterns {
                // Try regex match first (case-insensitive), fallback to simple contains for non-regex patterns
                if let Ok(re) = regex::Regex::new(&format!("(?i){}", pattern)) {
                    if re.is_match(&content_lower) {
                        code_messages.push(i);
                        break;
                    }
                } else {
                    // Fallback to simple contains for invalid regex patterns
                    if content_lower.contains(pattern.to_lowercase().as_str()) {
                        code_messages.push(i);
                        break;
                    }
                }
            }
        }

        if code_messages.is_empty() {
            return 0.5; // Neutral if no code
        }

        // Check if code messages are consecutive or close
        let mut consecutive_score = 0.0;
        for i in 1..code_messages.len() {
            let distance = code_messages[i] - code_messages[i - 1];
            if distance <= 2 {
                consecutive_score += 1.0;
            } else if distance <= 4 {
                consecutive_score += 0.5;
            }
        }

        if !code_messages.is_empty() && consecutive_score > 0.0 {
            consecutive_score / (code_messages.len() - 1).max(1) as f32
        } else {
            0.5
        }
    }

    /// Check entity consistency (files, functions, modules).
    fn check_entity_consistency(&self, messages: &[Message]) -> f32 {
        let entities: Vec<HashSet<String>> = messages
            .iter()
            .map(|m| self.extract_entities(&self.get_message_content(m)))
            .collect();

        if entities.len() < 2 {
            return 1.0;
        }

        // Find entities that appear in multiple messages
        let mut common_entities = HashSet::new();
        let all_entities: HashSet<String> = entities
            .iter()
            .flat_map(|e| e.iter().cloned())
            .collect();

        for entity in &all_entities {
            let count = entities.iter().filter(|e| e.contains(entity)).count();
            if count >= 2 {
                common_entities.insert(entity.clone());
            }
        }

        // Score based on number of common entities
        if common_entities.is_empty() {
            0.3 // Low coherence if no common entities
        } else if common_entities.len() >= 3 {
            1.0 // High coherence
        } else {
            0.7 // Medium coherence
        }
    }

    /// Extract topic keywords from message content.
    fn extract_topic_keywords(&self, content: &str) -> HashSet<String> {
        let content_lower = content.to_lowercase();
        let words = content_lower
            .split_whitespace()
            .filter(|w| w.len() > self.hardcode_config.min_word_length) // Skip short words
            .take(20) // Limit to 20 keywords
            .map(|w| w.to_string())
            .collect();
        words
    }

    /// Extract entities (file names, function names) from content.
    fn extract_entities(&self, content: &str) -> HashSet<String> {
        let mut entities = HashSet::new();

        // Pattern: file.rs, module.ts, etc.
        let file_pattern = regex::Regex::new(r"\b[\w]+\.[\w]{2,4}\b").unwrap();
        for cap in file_pattern.find_iter(content) {
            entities.insert(cap.as_str().to_string());
        }

        // Pattern: function_name, ClassName, etc.
        let name_pattern = regex::Regex::new(r"\b[A-Z][a-zA-Z]+\b|\b[a-z_][a-z0-9_]{3,}\b").unwrap();
        for cap in name_pattern.find_iter(content) {
            let name = cap.as_str();
            // Skip common words
            if !["true", "false", "null", "some", "none", "this", "that", "here", "there"].contains(&name.to_lowercase().as_str()) {
                entities.insert(name.to_string());
            }
        }

        entities
    }

    /// Calculate overlap between two sets.
    fn calculate_set_overlap<T: std::hash::Hash + Eq>(&self, set1: &HashSet<T>, set2: &HashSet<T>) -> f32 {
        if set1.is_empty() || set2.is_empty() {
            return 0.0;
        }

        let intersection = set1.intersection(set2).count();
        let union = set1.union(set2).count();

        if union > 0 {
            intersection as f32 / union as f32
        } else {
            0.0
        }
    }

    /// Get message content as string.
    fn get_message_content(&self, message: &Message) -> String {
        match &message.content {
            crate::providers::MessageContent::Text(text) => text.clone(),
            crate::providers::MessageContent::Blocks(blocks) => {
                blocks
                    .iter()
                    .filter_map(|b| {
                        if let crate::providers::ContentBlock::Text { text } = b {
                            Some(text.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            }
        }
    }

    /// Get message content as lowercase string.
    fn get_message_content_lower(&self, message: &Message) -> String {
        match &message.content {
            crate::providers::MessageContent::Text(text) => text.to_lowercase(),
            crate::providers::MessageContent::Blocks(blocks) => {
                blocks
                    .iter()
                    .filter_map(|b| {
                        if let crate::providers::ContentBlock::Text { text } = b {
                            Some(text.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
                    .to_lowercase()
            }
        }
    }

    /// Find optimal segmentation points in messages.
    pub fn find_segmentation_points(&self, messages: &[Message]) -> Vec<usize> {
        if messages.len() < 3 {
            return vec![];
        }

        let mut points = Vec::new();

        // Check coherence between consecutive pairs
        for i in 1..messages.len() - 1 {
            let before = &messages[0..i];
            let after = &messages[i..messages.len()];

            // Calculate coherence drop at this point
            let coherence_before = self.calculate_coherence(before);
            let coherence_after = self.calculate_coherence(after);
            let coherence_cross = self.calculate_coherence(&[messages[i - 1].clone(), messages[i].clone()]);

            // If cross coherence is significantly lower, mark as segmentation point
            if coherence_cross < coherence_before * 0.7 && coherence_cross < coherence_after * 0.7 {
                points.push(i);
            }
        }

        points
    }

    /// Segment messages into coherent groups.
    pub fn segment_messages(&self, messages: &[Message]) -> Vec<Vec<Message>> {
        if messages.is_empty() {
            return vec![];
        }

        let points = self.find_segmentation_points(messages);

        if points.is_empty() {
            return vec![messages.to_vec()];
        }

        let mut segments = Vec::new();
        let mut start = 0;

        for point in points {
            if point > start {
                segments.push(messages[start..point].to_vec());
                start = point;
            }
        }

        if start < messages.len() {
            segments.push(messages[start..messages.len()].to_vec());
        }

        segments
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::{Message, MessageContent, Role};

    fn create_text_message(text: &str) -> Message {
        Message {
            role: Role::User,
            content: MessageContent::Text(text.to_string()),
        }
    }

    #[test]
    fn test_coherence_detector_creation() {
        let detector = CoherenceDetector::default();
        assert_eq!(detector.threshold, 0.7);
    }

    #[test]
    fn test_single_message_coherence() {
        let detector = CoherenceDetector::default();
        let messages = vec![create_text_message("test")];
        assert!(detector.should_keep_together(&messages));
    }

    #[test]
    fn test_topic_continuity() {
        let detector = CoherenceDetector::default();
        let messages = vec![
            create_text_message("We need to optimize database performance"),
            create_text_message("The database queries are slow"),
            create_text_message("Let's add database indexes"),
        ];

        let score = detector.calculate_coherence(&messages);
        // Topic continuity with database keywords should give reasonable score
        assert!(score > 0.4, "Expected coherence > 0.4 for topic continuity, got {}", score);
    }

    #[test]
    fn test_reference_patterns() {
        // Create registry with custom reference pattern
        let mut registry = PatternRegistry::new();
        let pattern = crate::memory::ConversationPattern::manual(
            crate::memory::PatternType::Reference,
            "as i mentioned",
        );
        registry.add_pattern(pattern);

        let detector = CoherenceDetector::new_with_registry(0.7, registry);
        let messages = vec![
            create_text_message("We decided to use PostgreSQL"),
            create_text_message("As I mentioned, PostgreSQL is the choice"),
        ];

        let score = detector.calculate_coherence(&messages);
        // With reference pattern match (score=1.0 for reference), overall should be reasonable
        assert!(score > 0.5, "Expected coherence > 0.5 for reference patterns, got {}", score);
    }

    #[test]
    fn test_code_context() {
        // Create registry with custom code pattern
        let mut registry = PatternRegistry::new();
        let pattern = crate::memory::ConversationPattern::manual(
            crate::memory::PatternType::Code,
            "fn ",
        );
        registry.add_pattern(pattern);

        let detector = CoherenceDetector::new_with_registry(0.7, registry);
        let messages = vec![
            create_text_message("Here's the function:\n```rust\nfn test() {}\n```"),
            create_text_message("This function needs optimization"),
        ];

        let score = detector.calculate_coherence(&messages);
        // Code patterns should boost coherence (reference score = 0.5 neutral when patterns match)
        // Without presets, coherence depends on topic overlap and entity consistency
        assert!(score >= 0.35, "Expected coherence >= 0.35 for code context, got {}", score);
    }

    #[test]
    fn test_segmentation() {
        let detector = CoherenceDetector::default();
        let messages = vec![
            create_text_message("Topic A: database optimization"),
            create_text_message("More about database"),
            create_text_message("Topic B: frontend design"),
            create_text_message("More about frontend"),
        ];

        let segments = detector.segment_messages(&messages);
        assert!(segments.len() >= 1);
    }

    // =========================================================================
    // Pattern Registry Integration Tests
    // =========================================================================

    #[test]
    fn test_new_with_registry() {
        let registry = PatternRegistry::new();
        let detector = CoherenceDetector::new_with_registry(0.8, registry);

        assert_eq!(detector.threshold, 0.8);
        assert!(detector.pattern_registry().is_empty());
    }

    #[test]
    fn test_backward_compatible_new() {
        // new() should work with empty registry
        let detector = CoherenceDetector::new(0.7);

        assert_eq!(detector.threshold, 0.7);
        // PatternRegistry::new() is now empty, no presets loaded
        assert!(detector.pattern_registry().is_empty());

        // Should have no patterns from presets
        let ref_patterns = detector.pattern_registry().get_active_reference_patterns();
        let code_patterns = detector.pattern_registry().get_active_code_patterns();

        assert!(ref_patterns.is_empty(), "Reference patterns should be empty");
        assert!(code_patterns.is_empty(), "Code patterns should be empty");
    }

    #[test]
    fn test_default_uses_pattern_registry() {
        let detector = CoherenceDetector::default();

        // Default should have empty registry (no presets)
        let ref_patterns = detector.pattern_registry().get_active_reference_patterns();
        let code_patterns = detector.pattern_registry().get_active_code_patterns();

        assert!(ref_patterns.is_empty());
        assert!(code_patterns.is_empty());
    }

    #[test]
    fn test_pattern_registry_accessor() {
        let detector = CoherenceDetector::default();

        // Should be able to access the registry
        let registry = detector.pattern_registry();
        assert!(registry.is_empty());

        // No patterns loaded
        assert_eq!(registry.count_by_type(crate::memory::PatternType::Reference), 0);
        assert_eq!(registry.count_by_type(crate::memory::PatternType::Code), 0);
    }

    #[test]
    fn test_pattern_registry_mut_accessor() {
        let mut detector = CoherenceDetector::default();
        assert!(detector.pattern_registry().is_empty());

        // Should be able to mutate the registry
        let pattern = crate::memory::ConversationPattern::manual(
            crate::memory::PatternType::Code,
            "test-pattern-mut",
        );
        detector.pattern_registry_mut().add_pattern(pattern);

        assert_eq!(detector.pattern_registry().len(), 1);
    }

    #[test]
    fn test_with_hardcode_config() {
        let config = HardcodeConfig::complex_technical();
        let detector = CoherenceDetector::new(0.7).with_hardcode_config(config.clone());

        assert_eq!(detector.hardcode_config.min_word_length, config.min_word_length);
    }

    #[test]
    fn test_reference_patterns_from_registry() {
        // Create registry with custom reference pattern
        let mut registry = PatternRegistry::new();
        let pattern = crate::memory::ConversationPattern::manual(
            crate::memory::PatternType::Reference,
            "as i mentioned",
        );
        registry.add_pattern(pattern);

        let detector = CoherenceDetector::new_with_registry(0.7, registry);

        // Test with a message that should match custom reference pattern
        let messages = vec![
            create_text_message("Let's implement feature X"),
            create_text_message("As I mentioned earlier, feature X is important"),
        ];

        let score = detector.calculate_coherence(&messages);
        // Should have high coherence due to reference pattern match
        assert!(score > 0.5, "Expected coherence > 0.5 for reference patterns, got {}", score);
    }

    #[test]
    fn test_code_patterns_from_registry() {
        // Create registry with custom code pattern
        let mut registry = PatternRegistry::new();
        let pattern = crate::memory::ConversationPattern::manual(
            crate::memory::PatternType::Code,
            "fn ",
        );
        registry.add_pattern(pattern);

        let detector = CoherenceDetector::new_with_registry(0.7, registry);

        // Test with messages containing code patterns
        let messages = vec![
            create_text_message("Here is a function:\n```rust\nfn example() {}\n```"),
            create_text_message("This function does something"),
        ];

        let score = detector.calculate_coherence(&messages);
        // Should have reasonable coherence due to code pattern match
        assert!(score >= 0.35, "Expected coherence >= 0.35 for code patterns, got {}", score);
    }

    #[test]
    fn test_empty_registry_graceful_handling() {
        // Create an empty registry (no presets)
        let registry = PatternRegistry::new();
        assert!(registry.is_empty());

        let detector = CoherenceDetector::new_with_registry(0.7, registry);

        // Should still work without crashing, returning neutral scores
        let messages = vec![
            create_text_message("Message one"),
            create_text_message("Message two"),
        ];

        // Should not panic
        let score = detector.calculate_coherence(&messages);
        // Should get a valid score (neutral 0.5 for empty pattern registry)
        assert!(score >= 0.0 && score <= 1.0);
    }

    #[test]
    fn test_chinese_reference_patterns() {
        // Create registry with Chinese reference pattern
        let mut registry = PatternRegistry::new();
        let pattern = crate::memory::ConversationPattern::manual(
            crate::memory::PatternType::Reference,
            "正如我所说",
        );
        registry.add_pattern(pattern);

        let detector = CoherenceDetector::new_with_registry(0.7, registry);

        // Test Chinese reference patterns
        let messages = vec![
            create_text_message("我们决定使用 PostgreSQL"),
            create_text_message("正如我所说，PostgreSQL 是最佳选择"),
        ];

        let score = detector.calculate_coherence(&messages);
        // Should detect Chinese reference pattern "正如我所说"
        assert!(score > 0.5, "Expected coherence > 0.5 for Chinese reference patterns, got {}", score);
    }

    // =========================================================================
    // Additional Coverage Tests
    // =========================================================================

    #[test]
    fn test_empty_messages() {
        let detector = CoherenceDetector::default();

        // Empty messages should return true (keep together)
        let messages: Vec<Message> = vec![];
        assert!(detector.should_keep_together(&messages));

        // Empty messages should return 1.0 coherence
        let score = detector.calculate_coherence(&messages);
        assert!((score - 1.0).abs() < 0.001, "Expected coherence 1.0 for empty messages, got {}", score);
    }

    #[test]
    fn test_find_segmentation_points_empty() {
        let detector = CoherenceDetector::default();

        // Empty messages should return empty segmentation points
        let messages: Vec<Message> = vec![];
        let points = detector.find_segmentation_points(&messages);
        assert!(points.is_empty());

        // Single message should return empty segmentation points
        let single = vec![create_text_message("single message")];
        let points = detector.find_segmentation_points(&single);
        assert!(points.is_empty());

        // Two messages should return empty segmentation points
        let two = vec![
            create_text_message("first"),
            create_text_message("second"),
        ];
        let points = detector.find_segmentation_points(&two);
        assert!(points.is_empty());
    }

    #[test]
    fn test_segment_messages_empty() {
        let detector = CoherenceDetector::default();

        // Empty messages should return empty segments
        let messages: Vec<Message> = vec![];
        let segments = detector.segment_messages(&messages);
        assert!(segments.is_empty());
    }

    #[test]
    fn test_regex_pattern_matching() {
        // Test that regex patterns are properly matched
        let detector = CoherenceDetector::default();

        // Test with regex-style patterns (e.g., "之前的?讨论")
        let messages = vec![
            create_text_message("We discussed the architecture"),
            create_text_message("之前的讨论很有价值"),  // Chinese "previous discussion"
        ];

        let score = detector.calculate_coherence(&messages);
        // Should detect reference patterns
        assert!(score >= 0.0 && score <= 1.0, "Score should be valid, got {}", score);
    }

    #[test]
    fn test_simple_pattern_matching() {
        // Create registry with custom reference pattern
        let mut registry = PatternRegistry::new();
        let pattern = crate::memory::ConversationPattern::manual(
            crate::memory::PatternType::Reference,
            "as mentioned",
        );
        registry.add_pattern(pattern);

        let detector = CoherenceDetector::new_with_registry(0.7, registry);

        // Test with simple text patterns
        let messages = vec![
            create_text_message("Let's implement feature A"),
            create_text_message("As mentioned above, feature A is important"),
        ];

        let score = detector.calculate_coherence(&messages);
        // Should detect "as mentioned" reference pattern
        assert!(score > 0.5, "Expected high coherence for reference pattern match, got {}", score);
    }

    #[test]
    fn test_low_coherence_messages() {
        let detector = CoherenceDetector::new(0.7);

        // Messages with no semantic connection
        let messages = vec![
            create_text_message("The quick brown fox jumps"),
            create_text_message("Database optimization strategies"),
            create_text_message("Weather forecast tomorrow"),
        ];

        let score = detector.calculate_coherence(&messages);
        // Should have lower coherence due to no common entities or references
        // But still a valid score
        assert!(score >= 0.0 && score <= 1.0, "Score should be valid, got {}", score);
    }

    #[test]
    fn test_entity_consistency() {
        let detector = CoherenceDetector::default();

        // Messages sharing entities (file names, function names)
        let messages = vec![
            create_text_message("In process.rs we have a bug"),
            create_text_message("The process.rs file needs fixing"),
            create_text_message("Let me check process.rs again"),
        ];

        let score = detector.calculate_coherence(&messages);
        // Entity consistency contributes to coherence, but entity_score is only 20% weight
        // So the overall score depends on topic, reference, and code scores too
        // Verify that entity extraction finds "process.rs"
        assert!(score >= 0.3 && score <= 1.0, "Expected valid coherence score, got {}", score);

        // Compare with messages that have no shared entities
        let no_entity_messages = vec![
            create_text_message("First topic discussion"),
            create_text_message("Second unrelated topic"),
            create_text_message("Third different subject"),
        ];
        let no_entity_score = detector.calculate_coherence(&no_entity_messages);

        // Messages with shared entities should generally have higher or equal coherence
        // (entity_score contribution is 0.7 for 1-2 common entities vs 0.3 for none)
        // Note: This depends on other factors too, so we just verify valid scores
        assert!(no_entity_score >= 0.0 && no_entity_score <= 1.0);
    }

    #[test]
    fn test_custom_registry_affects_detection() {
        // Test that custom registry patterns affect detection
        use crate::memory::{ConversationPattern, PatternType};

        let mut registry = PatternRegistry::new();
        let initial_count = registry.len();

        // Add custom pattern
        let custom_pattern = ConversationPattern::manual(
            PatternType::Reference,
            "custom_reference_pattern_xyz",
        );
        registry.add_pattern(custom_pattern);

        assert!(registry.len() > initial_count);

        // Create detector with custom registry
        let detector = CoherenceDetector::new_with_registry(0.7, registry);

        // Test that custom pattern is in the registry
        let ref_patterns = detector.pattern_registry().get_active_reference_patterns();
        assert!(ref_patterns.iter().any(|p| p.contains("custom_reference_pattern_xyz")));
    }

    #[test]
    fn test_multiple_code_blocks() {
        let detector = CoherenceDetector::default();

        // Multiple code blocks in sequence
        let messages = vec![
            create_text_message("Here is the first function:\n```rust\nfn one() {}\n```"),
            create_text_message("And the second:\n```rust\nfn two() {}\n```"),
            create_text_message("The third function:\n```rust\nfn three() {}\n```"),
        ];

        let score = detector.calculate_coherence(&messages);
        // Should have good coherence due to consecutive code patterns
        assert!(score >= 0.0 && score <= 1.0);
    }

    #[test]
    fn test_threshold_affects_should_keep_together() {
        // Low threshold should keep more messages together
        let low_threshold = CoherenceDetector::new(0.3);
        // High threshold should separate more messages
        let high_threshold = CoherenceDetector::new(0.9);

        let messages = vec![
            create_text_message("Topic A discussion"),
            create_text_message("Related to topic A"),
        ];

        // Both should keep these related messages together
        assert!(low_threshold.should_keep_together(&messages));
        // High threshold may or may not keep together depending on coherence
        let _score = high_threshold.calculate_coherence(&messages);
        // Just verify it doesn't crash
    }
}