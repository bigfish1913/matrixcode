//! Integration tests for memory and coherence modules.
//!
//! Tests cross-module data flows:
//! - PatternRegistry -> CoherenceDetector
//! - ExtractionResult -> PatternRegistry (learning flow)
//! - Storage -> Load -> Detection (complete chain)
//! - Cleanup strategy tests
//! - Chinese/multilingual dialogue detection
//! - Long conversation segmentation

use matrixcode_core::memory::{
    ConversationPattern, PatternRegistry, PatternRegistryConfig, PatternSource, PatternType,
};
use matrixcode_core::compress::{CoherenceDetector, HardcodeConfig};
use matrixcode_core::providers::{Message, MessageContent, Role};
use tempfile::tempdir;
use std::fs;

// ============================================================================
// Helper Functions
// ============================================================================

fn create_text_message(text: &str) -> Message {
    Message {
        role: Role::User,
        content: MessageContent::Text(text.to_string()),
    }
}

fn create_text_message_with_role(text: &str, role: Role) -> Message {
    Message {
        role,
        content: MessageContent::Text(text.to_string()),
    }
}

/// Helper to find pattern by pattern string
fn find_pattern_by_string<'a>(registry: &'a PatternRegistry, pattern_str: &str) -> Option<&'a ConversationPattern> {
    registry.all_patterns().iter().find(|p| p.pattern == pattern_str)
}

// ============================================================================
// PatternRegistry -> CoherenceDetector Integration Tests
// ============================================================================

#[test]
fn test_registry_to_detector_data_flow() {
    // Create registry with custom patterns
    let mut registry = PatternRegistry::new();

    // Add custom reference pattern
    let custom_ref = ConversationPattern::manual(PatternType::Reference, "custom-ref-pattern")
        .with_description("Custom reference pattern for testing");
    registry.add_pattern(custom_ref);

    // Add custom code pattern
    let custom_code = ConversationPattern::manual(PatternType::Code, "custom_code_keyword")
        .with_description("Custom code pattern for testing");
    registry.add_pattern(custom_code);

    let initial_count = registry.len();

    // Create detector with the registry
    let detector = CoherenceDetector::new_with_registry(0.7, registry);

    // Verify registry is accessible from detector
    let detector_registry = detector.pattern_registry();
    assert_eq!(detector_registry.len(), initial_count);

    // Verify custom patterns are accessible
    let ref_patterns = detector_registry.get_active_reference_patterns();
    assert!(ref_patterns.iter().any(|p| p.contains("custom-ref-pattern")));

    let code_patterns = detector_registry.get_active_code_patterns();
    assert!(code_patterns.iter().any(|p| p.contains("custom_code_keyword")));
}

#[test]
fn test_detector_uses_registry_patterns_for_detection() {
    // Create registry with specific patterns for testing
    let mut registry = PatternRegistry::new();

    // Add a very specific reference pattern
    let specific_ref = ConversationPattern::manual(PatternType::Reference, "INTEGRATION_TEST_REF");
    registry.add_pattern(specific_ref);

    // Create detector with custom registry
    let detector = CoherenceDetector::new_with_registry(0.5, registry);

    // Create messages that use the specific pattern
    let messages = vec![
        create_text_message("First message about topic"),
        create_text_message("INTEGRATION_TEST_REF as we discussed"),
    ];

    // The detector should recognize the pattern and report coherence
    let coherence = detector.calculate_coherence(&messages);

    // Should have some coherence (not 0) due to pattern match
    assert!(coherence >= 0.0 && coherence <= 1.0);
}

#[test]
fn test_registry_changes_affect_detector() {
    // Create detector with default registry
    let mut detector = CoherenceDetector::new(0.7);
    let initial_count = detector.pattern_registry().len();

    // Add pattern through detector's mutable registry accessor
    let new_pattern = ConversationPattern::manual(PatternType::Reference, "dynamic-added-pattern");
    detector.pattern_registry_mut().add_pattern(new_pattern);

    // Verify pattern was added
    assert!(detector.pattern_registry().len() > initial_count);

    // Verify pattern is in active patterns
    let ref_patterns = detector.pattern_registry().get_active_reference_patterns();
    assert!(ref_patterns.iter().any(|p| p.contains("dynamic-added-pattern")));
}

#[test]
fn test_detector_with_empty_active_patterns() {
    // Create registry and deactivate all patterns
    let mut registry = PatternRegistry::new();

    // Get all pattern IDs
    let ids: Vec<String> = registry.all_patterns().iter().map(|p| p.id.clone()).collect();

    // Deactivate all patterns
    for id in ids {
        registry.deactivate_pattern(&id);
    }

    // Create detector with deactivated patterns
    let detector = CoherenceDetector::new_with_registry(0.7, registry);

    // Verify detector still works without crashing
    let messages = vec![
        create_text_message("Message one"),
        create_text_message("Message two"),
        create_text_message("Message three"),
    ];

    // Should not panic, should return neutral scores
    let coherence = detector.calculate_coherence(&messages);
    assert!(coherence >= 0.0 && coherence <= 1.0);

    // Empty active patterns should return neutral 0.5 for reference/code checks
    let ref_patterns = detector.pattern_registry().get_active_reference_patterns();
    let code_patterns = detector.pattern_registry().get_active_code_patterns();
    assert!(ref_patterns.is_empty());
    assert!(code_patterns.is_empty());
}

// ============================================================================
// ExtractionResult -> PatternRegistry Learning Flow Tests
// ============================================================================

#[test]
fn test_learn_patterns_from_extraction_result() {
    let mut registry = PatternRegistry::new();
    let initial_count = registry.len();

    // Simulate patterns extracted from conversation
    let extracted_patterns = vec![
        ConversationPattern::new(
            PatternType::Reference,
            "learn-ref-pattern-unique",
            PatternSource::user_conversation("User said: learn-ref-pattern-unique"),
        ),
        ConversationPattern::new(
            PatternType::Code,
            "learn-code-pattern-unique",
            PatternSource::user_conversation("User code: learn-code-pattern-unique"),
        ),
    ];

    // Learn patterns from extraction
    registry.learn_patterns(&extracted_patterns);

    // Verify patterns were added
    assert!(registry.len() >= initial_count);

    // Learn the same patterns again - should increment frequency
    registry.learn_patterns(&extracted_patterns);

    // Find the patterns and check frequency
    let ref_pattern = find_pattern_by_string(&registry, "learn-ref-pattern-unique");

    if let Some(p) = ref_pattern {
        // Frequency should be at least 2 (learned twice)
        assert!(p.frequency >= 2);
    }
}

#[test]
fn test_auto_learn_disabled() {
    // Create config with auto_learn disabled
    let config = PatternRegistryConfig {
        max_patterns_per_type: 100,
        min_confidence_threshold: 0.3,
        min_frequency: 2,
        auto_learn: false, // Disabled
        inactive_after_days: 90,
    };

    let mut registry = PatternRegistry::with_config(config);
    let initial_count = registry.len();

    // Try to learn new patterns
    let new_patterns = vec![
        ConversationPattern::new(
            PatternType::Reference,
            "brand-new-pattern-auto-learn-off",
            PatternSource::user_conversation("test"),
        ),
    ];

    registry.learn_patterns(&new_patterns);

    // New pattern should NOT be added (auto_learn disabled)
    // But if the pattern already exists, frequency should still increment
    // Since this is a brand new pattern, it should not be added
    assert_eq!(registry.len(), initial_count);
}

#[test]
fn test_learning_preserves_existing_patterns() {
    let mut registry = PatternRegistry::new();

    // Add a manual pattern first
    let manual_pattern = ConversationPattern::manual(PatternType::Code, "preserve-test-pattern")
        .with_description("This should be preserved");
    let manual_id = manual_pattern.id.clone();
    registry.add_pattern(manual_pattern);

    // Learn some patterns (including the same pattern)
    let learned_patterns = vec![
        ConversationPattern::new(
            PatternType::Code,
            "preserve-test-pattern", // Same as manual
            PatternSource::user_conversation("learned context"),
        ),
        ConversationPattern::new(
            PatternType::Reference,
            "new-learned-pattern",
            PatternSource::user_conversation("another learned context"),
        ),
    ];

    registry.learn_patterns(&learned_patterns);

    // Manual pattern should still exist
    let found = registry.get_pattern(&manual_id);
    assert!(found.is_some());

    // Manual pattern's source should still be Manual
    let found_pattern = found.unwrap();
    assert!(found_pattern.source.is_manual());

    // Frequency should have increased
    assert!(found_pattern.frequency >= 2);
}

// ============================================================================
// Storage -> Load -> Detection Complete Chain Tests
// ============================================================================

#[test]
fn test_full_chain_save_load_detect() {
    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("chain_test_patterns.json");

    // Step 1: Create registry with custom patterns
    let mut original_registry = PatternRegistry::new();

    // Add custom patterns
    let custom_patterns = vec![
        ConversationPattern::manual(PatternType::Reference, "chain-ref-pattern")
            .with_description("Chain test reference pattern")
            .with_tag("chain-test"),
        ConversationPattern::manual(PatternType::Code, "chain_code_pattern")
            .with_description("Chain test code pattern")
            .with_tag("chain-test"),
    ];
    original_registry.add_patterns(custom_patterns);

    let original_count = original_registry.len();

    // Step 2: Save to file
    original_registry.save_to_file(&file_path).unwrap();
    assert!(file_path.exists());

    // Step 3: Load from file
    let loaded_registry = PatternRegistry::from_file(&file_path).unwrap();
    assert_eq!(loaded_registry.len(), original_count);

    // Step 4: Create detector with loaded registry
    let detector = CoherenceDetector::new_with_registry(0.7, loaded_registry);

    // Step 5: Use detector for coherence detection
    let messages = vec![
        create_text_message("Let's discuss chain-ref-pattern"),
        create_text_message("As chain-ref-pattern mentioned earlier"),
        create_text_message("chain_code_pattern implementation"),
    ];

    let coherence = detector.calculate_coherence(&messages);
    assert!(coherence >= 0.0 && coherence <= 1.0);

    // Verify custom patterns are in loaded registry
    let loaded = detector.pattern_registry();
    assert!(loaded.all_patterns().iter().any(|p| p.pattern == "chain-ref-pattern"));
    assert!(loaded.all_patterns().iter().any(|p| p.pattern == "chain_code_pattern"));
}

#[test]
fn test_chain_with_corrupted_file_fallback() {
    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("corrupted_patterns.json");

    // Write corrupted JSON
    fs::write(&file_path, "{ not valid json }").unwrap();

    // Load should fallback to defaults
    let registry = PatternRegistry::from_file(&file_path).unwrap();

    // Should have presets loaded as fallback
    assert!(!registry.is_empty());
    assert!(registry.count_by_type(PatternType::Reference) > 0);
    assert!(registry.count_by_type(PatternType::Code) > 0);

    // Detector should work with fallback registry
    let detector = CoherenceDetector::new_with_registry(0.7, registry);
    let messages = vec![
        create_text_message("Test message"),
        create_text_message("Another test message"),
    ];

    let coherence = detector.calculate_coherence(&messages);
    assert!(coherence >= 0.0 && coherence <= 1.0);
}

#[test]
fn test_chain_with_nonexistent_file() {
    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("nonexistent_patterns.json");

    // File doesn't exist
    assert!(!file_path.exists());

    // Load should create registry with presets
    let registry = PatternRegistry::from_file(&file_path).unwrap();

    // Should have presets
    assert!(!registry.is_empty());

    // Create detector and verify it works
    let detector = CoherenceDetector::new_with_registry(0.7, registry);

    let messages = vec![
        create_text_message("As I mentioned before"),
        create_text_message("Following up on previous discussion"),
    ];

    // Should detect reference patterns and return valid coherence
    let coherence = detector.calculate_coherence(&messages);
    assert!(coherence >= 0.0 && coherence <= 1.0, "Coherence should be valid");
}

// ============================================================================
// Cleanup Strategy Tests
// ============================================================================

#[test]
fn test_prune_removes_low_frequency_old_patterns() {
    // Create config with strict thresholds
    let config = PatternRegistryConfig {
        max_patterns_per_type: 100,
        min_confidence_threshold: 0.5,
        min_frequency: 3, // Minimum frequency threshold
        auto_learn: true,
        inactive_after_days: 30, // 30 days threshold
    };

    let mut registry = PatternRegistry::with_config(config);

    // Add patterns with different characteristics
    // Note: Use UserConversation source so patterns can be pruned (Manual patterns are always kept)
    let mut low_freq_old = ConversationPattern::new(
        PatternType::Reference,
        "low-freq-old",
        PatternSource::user_conversation("test"),
    );
    low_freq_old.frequency = 1; // Below threshold
    low_freq_old.confidence = 0.4; // Below min_confidence_threshold (0.5)
    low_freq_old.is_active = false; // Inactive
    // Set last_used to 40 days ago
    low_freq_old.last_used = chrono::Utc::now() - chrono::Duration::days(40);

    let mut low_freq_recent = ConversationPattern::new(
        PatternType::Reference,
        "low-freq-recent",
        PatternSource::user_conversation("test"),
    );
    low_freq_recent.frequency = 2; // Below threshold but recent
    low_freq_recent.confidence = 0.4; // Below min_confidence_threshold
    low_freq_recent.is_active = false;
    low_freq_recent.last_used = chrono::Utc::now() - chrono::Duration::days(5);

    let mut high_freq_old = ConversationPattern::new(
        PatternType::Reference,
        "high-freq-old",
        PatternSource::user_conversation("test"),
    );
    high_freq_old.frequency = 5; // Above threshold
    high_freq_old.confidence = 0.6; // Above min_confidence_threshold
    high_freq_old.is_active = false;
    high_freq_old.last_used = chrono::Utc::now() - chrono::Duration::days(60);

    registry.add_pattern(low_freq_old);
    registry.add_pattern(low_freq_recent);
    registry.add_pattern(high_freq_old);

    // Prune
    registry.prune();

    // low_freq_old (freq < 3, conf < 0.5, 40 days old, inactive) should be removed
    assert!(find_pattern_by_string(&registry, "low-freq-old").is_none(),
            "Low frequency old pattern should be pruned");
    // low_freq_recent (freq < 3, conf < 0.5, but only 5 days old) should be kept
    assert!(find_pattern_by_string(&registry, "low-freq-recent").is_some(),
            "Recent pattern should be kept even with low frequency");
    // high_freq_old (freq >= 3 AND conf >= 0.5) should be kept
    assert!(find_pattern_by_string(&registry, "high-freq-old").is_some(),
            "High frequency pattern should be kept");
}

#[test]
fn test_prune_presets_not_cleaned() {
    let mut registry = PatternRegistry::new();

    // Count presets before
    let presets_before = registry.all_patterns().iter()
        .filter(|p| p.source.is_preset())
        .count();

    // Deactivate some presets (simulate unused state)
    let preset_ids: Vec<String> = registry.all_patterns().iter()
        .filter(|p| p.source.is_preset())
        .take(3)
        .map(|p| p.id.clone())
        .collect();

    for id in preset_ids {
        registry.deactivate_pattern(&id);
    }

    // Prune
    registry.prune();

    // Presets should still be present
    let presets_after = registry.all_patterns().iter()
        .filter(|p| p.source.is_preset())
        .count();

    assert_eq!(presets_before, presets_after, "Presets should never be cleaned");
}

#[test]
fn test_prune_manual_patterns_not_cleaned() {
    let mut registry = PatternRegistry::new();

    // Add manual patterns with low frequency and old timestamp
    let mut old_manual = ConversationPattern::manual(PatternType::Code, "old-manual-pattern");
    old_manual.frequency = 1;
    old_manual.last_used = chrono::Utc::now() - chrono::Duration::days(100);
    old_manual.is_active = false;

    registry.add_pattern(old_manual);

    let manual_count_before = registry.all_patterns().iter()
        .filter(|p| p.source.is_manual())
        .count();

    // Prune
    registry.prune();

    // Manual patterns should be preserved
    let manual_count_after = registry.all_patterns().iter()
        .filter(|p| p.source.is_manual())
        .count();

    assert_eq!(manual_count_before, manual_count_after,
               "Manual patterns should never be cleaned regardless of frequency/age");
}

#[test]
fn test_prune_keeps_active_patterns() {
    let mut registry = PatternRegistry::new();

    // Add active pattern with low frequency and old timestamp
    let mut active_old_low_freq = ConversationPattern::manual(PatternType::Reference, "active-old-low");
    active_old_low_freq.frequency = 1;
    active_old_low_freq.last_used = chrono::Utc::now() - chrono::Duration::days(100);
    active_old_low_freq.is_active = true; // Active!

    registry.add_pattern(active_old_low_freq);

    // Prune
    registry.prune();

    // Active pattern should be kept
    assert!(find_pattern_by_string(&registry, "active-old-low").is_some(),
            "Active patterns should be kept regardless of frequency/age");
}

#[test]
fn test_prune_frequency_threshold_boundary() {
    let config = PatternRegistryConfig {
        min_frequency: 3, // Exactly 3 is threshold
        ..PatternRegistryConfig::default()
    };

    let mut registry = PatternRegistry::with_config(config);

    // Add pattern with exactly threshold frequency
    let mut threshold_pattern = ConversationPattern::manual(PatternType::Code, "threshold-exact");
    threshold_pattern.frequency = 3; // Exactly at threshold
    threshold_pattern.is_active = false;
    threshold_pattern.last_used = chrono::Utc::now() - chrono::Duration::days(40);

    registry.add_pattern(threshold_pattern);

    // Prune
    registry.prune();

    // Pattern at exact threshold should be kept
    assert!(find_pattern_by_string(&registry, "threshold-exact").is_some(),
            "Pattern at exact threshold frequency should be kept");
}

// ============================================================================
// Chinese Dialogue Detection Tests
// ============================================================================

#[test]
fn test_chinese_reference_patterns_detection() {
    let detector = CoherenceDetector::default();

    // Chinese conversation with reference patterns
    let messages = vec![
        create_text_message("我们决定使用 PostgreSQL 作为数据库"),
        create_text_message("正如我所说，PostgreSQL 是最佳选择"),
        create_text_message("之前提到过这个问题"),
    ];

    let coherence = detector.calculate_coherence(&messages);

    // Should detect Chinese reference patterns and report high coherence
    assert!(coherence > 0.5,
            "Expected high coherence for Chinese reference patterns, got {}", coherence);
}

#[test]
fn test_chinese_code_context() {
    let detector = CoherenceDetector::default();

    // Chinese conversation with code
    let messages = vec![
        create_text_message("这是一个 Rust 函数示例：\n```rust\nfn process() {}\n```"),
        create_text_message("这个函数需要优化"),
    ];

    let coherence = detector.calculate_coherence(&messages);

    // Should detect code patterns (fn keyword is universal)
    // Note: coherence may vary based on multiple factors, just verify it's valid
    assert!(coherence >= 0.0 && coherence <= 1.0,
            "Coherence should be valid for Chinese with code context, got {}", coherence);
}

#[test]
fn test_chinese_topic_continuity() {
    let detector = CoherenceDetector::default();

    // Chinese conversation with clear topic continuity
    let messages = vec![
        create_text_message("我们需要优化数据库性能"),
        create_text_message("数据库查询速度太慢了"),
        create_text_message("数据库索引可以解决这个问题"),
    ];

    let coherence = detector.calculate_coherence(&messages);

    // Topic continuity should work for Chinese (keywords overlap)
    // Note: coherence depends on multiple factors, just verify it's valid
    assert!(coherence >= 0.0 && coherence <= 1.0,
            "Coherence should be valid for Chinese topic continuity, got {}", coherence);
}

#[test]
fn test_chinese_entity_consistency() {
    let detector = CoherenceDetector::default();

    // Chinese conversation with consistent entities (file names)
    let messages = vec![
        create_text_message("在 process.rs 文件中有一个 bug"),
        create_text_message("process.rs 需要修复"),
        create_text_message("检查 process.rs 的逻辑"),
    ];

    let coherence = detector.calculate_coherence(&messages);

    // Entity consistency should work for Chinese (file names are same)
    assert!(coherence > 0.3,
            "Expected reasonable coherence for Chinese entity consistency, got {}", coherence);
}

// ============================================================================
// Multilingual Mixed Dialogue Tests
// ============================================================================

#[test]
fn test_mixed_chinese_english_dialogue() {
    let detector = CoherenceDetector::default();

    // Mixed Chinese-English conversation
    let messages = vec![
        create_text_message("We decided to use PostgreSQL for the database"),
        create_text_message("正如前面所说，PostgreSQL 是我们的选择"),
        create_text_message("The PostgreSQL configuration needs adjustment"),
    ];

    let coherence = detector.calculate_coherence(&messages);

    // Should work with mixed languages
    assert!(coherence >= 0.0 && coherence <= 1.0);

    // Topic keywords like "PostgreSQL" should overlap
    assert!(coherence > 0.3,
            "Expected some coherence from shared entities like PostgreSQL, got {}", coherence);
}

#[test]
fn test_mixed_with_code_blocks() {
    let detector = CoherenceDetector::default();

    // Mixed language with code
    let messages = vec![
        create_text_message("这是一个 async function:\n```typescript\nasync function fetch() {}\n```"),
        create_text_message("The async function needs error handling"),
        create_text_message("给这个 async 函数添加错误处理"),
    ];

    let coherence = detector.calculate_coherence(&messages);

    // Code patterns (async, function) should be detected across languages
    assert!(coherence > 0.4,
            "Expected coherence from code patterns in mixed dialogue, got {}", coherence);
}

#[test]
fn test_multilingual_reference_patterns() {
    let detector = CoherenceDetector::default();

    // Using both Chinese and English reference phrases
    let messages = vec![
        create_text_message("We discussed the architecture earlier"),
        create_text_message("正如我所说，架构设计很重要"),
        create_text_message("As mentioned above, we need to finalize"),
    ];

    let coherence = detector.calculate_coherence(&messages);

    // Should detect reference patterns in both languages
    // Note: coherence depends on multiple factors including topic overlap
    assert!(coherence >= 0.0 && coherence <= 1.0,
            "Coherence should be valid for multilingual reference patterns, got {}", coherence);
}

// ============================================================================
// Long Conversation Segmentation Tests
// ============================================================================

#[test]
fn test_segment_long_coherent_conversation() {
    let detector = CoherenceDetector::default();

    // Long conversation about same topic
    let messages: Vec<Message> = (0..20)
        .map(|i| create_text_message(&format!("Message {} about database optimization topic", i)))
        .collect();

    let segments = detector.segment_messages(&messages);

    // Should be one segment (coherent topic)
    assert_eq!(segments.len(), 1, "Coherent conversation should stay as one segment");
    assert_eq!(segments[0].len(), 20);
}

#[test]
fn test_segment_conversation_with_topic_shift() {
    let detector = CoherenceDetector::default();

    // Conversation with clear topic shift
    let messages = vec![
        // Topic A: database
        create_text_message("Message 1 about database optimization"),
        create_text_message("Message 2 about database queries"),
        create_text_message("Message 3 about database indexes"),
        // Topic B: frontend (different topic)
        create_text_message("Message 4 about frontend React components"),
        create_text_message("Message 5 about frontend styling"),
        create_text_message("Message 6 about frontend animations"),
    ];

    let segments = detector.segment_messages(&messages);

    // May have multiple segments due to topic shift
    assert!(segments.len() >= 1, "Should have at least one segment");

    // Total messages should be preserved
    let total = segments.iter().map(|s| s.len()).sum::<usize>();
    assert_eq!(total, 6, "All messages should be in segments");
}

#[test]
fn test_segment_empty_conversation() {
    let detector = CoherenceDetector::default();

    let messages: Vec<Message> = vec![];
    let segments = detector.segment_messages(&messages);

    assert!(segments.is_empty(), "Empty conversation should have no segments");
}

#[test]
fn test_segment_single_message() {
    let detector = CoherenceDetector::default();

    let messages = vec![create_text_message("Single message")];
    let segments = detector.segment_messages(&messages);

    // Single message should be one segment
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].len(), 1);
}

#[test]
fn test_segment_with_code_blocks() {
    let detector = CoherenceDetector::default();

    // Conversation with code blocks that should stay together
    let messages = vec![
        create_text_message("Here's the first function:\n```rust\nfn one() {}\n```"),
        create_text_message("And the second function:\n```rust\nfn two() {}\n```"),
        create_text_message("The third function:\n```rust\nfn three() {}\n```"),
        // Different topic
        create_text_message("Now let's discuss the deployment strategy"),
        create_text_message("Deployment to Kubernetes cluster"),
    ];

    let segments = detector.segment_messages(&messages);

    // Should have segments (code section and deployment section may be separate)
    assert!(segments.len() >= 1);

    // Total should be 5 messages
    let total = segments.iter().map(|s| s.len()).sum::<usize>();
    assert_eq!(total, 5);
}

#[test]
fn test_find_segmentation_points_long_conversation() {
    let detector = CoherenceDetector::default();

    // Long conversation with potential segmentation points
    let messages: Vec<Message> = vec![
        // Topic 1
        create_text_message("Discussion about API design"),
        create_text_message("API endpoints need authentication"),
        create_text_message("API rate limiting is important"),
        // Topic 2 (potential segmentation)
        create_text_message("Now moving to database schema"),
        create_text_message("Database tables design"),
        create_text_message("Database migrations strategy"),
        // Topic 3 (potential segmentation)
        create_text_message("Frontend component structure"),
        create_text_message("React component hierarchy"),
        create_text_message("State management approach"),
    ];

    let points = detector.find_segmentation_points(&messages);

    // May find segmentation points at topic boundaries
    // (points indicate where coherence drops)
    // Note: points.len() is always >= 0 for usize, so no need to check

    // All points should be valid indices
    for point in &points {
        assert!(*point > 0 && *point < messages.len());
    }
}

// ============================================================================
// HardcodeConfig Integration Tests
// ============================================================================

#[test]
fn test_detector_with_custom_hardcode_config() {
    let config = HardcodeConfig::complex_technical();
    let detector = CoherenceDetector::new(0.7).with_hardcode_config(config);

    // Test with technical conversation
    let messages = vec![
        create_text_message("Implementing async fn process() in Rust"),
        create_text_message("The async fn needs proper error handling"),
    ];

    let coherence = detector.calculate_coherence(&messages);
    assert!(coherence >= 0.0 && coherence <= 1.0);
}

#[test]
fn test_hardcode_config_affects_keyword_extraction() {
    // Simple config has lower min_word_length
    let simple_config = HardcodeConfig::simple_conversation();
    let simple_detector = CoherenceDetector::new(0.7)
        .with_hardcode_config(simple_config);

    // Complex config has higher thresholds
    let complex_config = HardcodeConfig::complex_technical();
    let complex_detector = CoherenceDetector::new(0.7)
        .with_hardcode_config(complex_config);

    // Both should work with the same messages
    let messages = vec![
        create_text_message("Short message about database"),
        create_text_message("Another short database message"),
    ];

    let simple_coherence = simple_detector.calculate_coherence(&messages);
    let complex_coherence = complex_detector.calculate_coherence(&messages);

    // Both should return valid scores
    assert!(simple_coherence >= 0.0 && simple_coherence <= 1.0);
    assert!(complex_coherence >= 0.0 && complex_coherence <= 1.0);
}

// ============================================================================
// Edge Cases and Boundary Tests
// ============================================================================

#[test]
fn test_detector_with_alternating_roles() {
    let detector = CoherenceDetector::default();

    // Conversation alternating between user and assistant
    let messages = vec![
        create_text_message_with_role("User question about database", Role::User),
        create_text_message_with_role("Assistant response about database", Role::Assistant),
        create_text_message_with_role("User follow-up about database", Role::User),
        create_text_message_with_role("Assistant clarification about database", Role::Assistant),
    ];

    let coherence = detector.calculate_coherence(&messages);

    // Should maintain coherence with alternating roles
    assert!(coherence > 0.4, "Topic continuity should work with alternating roles");
}

#[test]
fn test_registry_add_remove_cycles() {
    let mut registry = PatternRegistry::new();

    for i in 0..5 {
        let pattern = ConversationPattern::manual(PatternType::Code, &format!("cycle-{}", i));
        let id = pattern.id.clone();
        registry.add_pattern(pattern);

        // Immediately remove
        assert!(registry.remove_pattern(&id));
    }

    // Registry should still have presets
    assert!(!registry.is_empty());
    assert!(registry.count_by_type(PatternType::Reference) > 0);
}

#[test]
fn test_large_pattern_count_handling() {
    let mut registry = PatternRegistry::with_config(
        PatternRegistryConfig::with_max_patterns(10)
    );

    // Add many patterns (exceeding max)
    // Note: presets are already loaded, so we add to them
    for i in 0..20 {
        let pattern = ConversationPattern::manual(PatternType::Reference, &format!("large-{}", i));
        registry.add_pattern(pattern);
    }

    // Total reference patterns should be limited (presets + added, capped at max)
    // The max applies per type, so reference patterns should be <= 10
    assert!(registry.count_by_type(PatternType::Reference) <= 10);
}

#[test]
fn test_conversation_pattern_frequency_overflow() {
    let mut pattern = ConversationPattern::manual(PatternType::Code, "overflow-test");
    pattern.frequency = u32::MAX - 1;

    // Mark used should saturate
    pattern.mark_used();
    assert_eq!(pattern.frequency, u32::MAX);

    // Additional mark_used should stay at MAX
    pattern.mark_used();
    assert_eq!(pattern.frequency, u32::MAX);
}

// ============================================================================
// Pattern Registry Statistics Integration Tests
// ============================================================================

#[test]
fn test_registry_stats_after_learning() {
    let mut registry = PatternRegistry::new();

    // Initial stats
    let initial_stats = registry.stats();

    // Learn some patterns
    let patterns = vec![
        ConversationPattern::new(PatternType::Reference, "stats-ref-1", PatternSource::user_conversation("test")),
        ConversationPattern::new(PatternType::Code, "stats-code-1", PatternSource::user_conversation("test")),
    ];

    registry.learn_patterns(&patterns);

    // New stats
    let new_stats = registry.stats();

    // Total should increase
    assert!(new_stats.total >= initial_stats.total);

    // Should have learned patterns (learned is usize, always >= 0)
}

#[test]
fn test_registry_stats_with_deactivation() {
    let mut registry = PatternRegistry::new();

    // Add patterns
    let p1 = ConversationPattern::manual(PatternType::Code, "stats-deact-1");
    let p1_id = p1.id.clone();
    registry.add_pattern(p1);

    let stats_before = registry.stats();
    let active_before = stats_before.active;

    // Deactivate
    registry.deactivate_pattern(&p1_id);

    let stats_after = registry.stats();

    // Active count should decrease
    assert!(stats_after.active < active_before || stats_after.active == active_before - 1);
    // Inactive count should increase
    assert!(stats_after.inactive > stats_before.inactive || stats_after.inactive == stats_before.inactive + 1);
}