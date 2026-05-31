//! Integration usage example for pattern registry and coherence detector.
//!
//! This example demonstrates how to use the new types from the memory and compress modules.

use matrixcode_core::memory::{
    ConversationPattern, PatternRegistry, PatternType,
};
use matrixcode_core::compress::{
    CoherenceDetector, HardcodeConfig,
};
use matrixcode_core::providers::{Message, MessageContent, Role};

fn main() {
    println!("=== Pattern Registry Integration Example ===\n");

    // 1. Create a PatternRegistry with presets
    let registry = PatternRegistry::new();
    println!("PatternRegistry created with {} patterns", registry.len());

    // 2. Check pattern counts by type
    let ref_count = registry.count_by_type(PatternType::Reference);
    let code_count = registry.count_by_type(PatternType::Code);
    println!("  Reference patterns: {}", ref_count);
    println!("  Code patterns: {}", code_count);

    // 3. Get active patterns
    let active_refs = registry.get_active_reference_patterns();
    let active_codes = registry.get_active_code_patterns();
    println!("  Active reference patterns: {}", active_refs.len());
    println!("  Active code patterns: {}", active_codes.len());

    // 4. Create a custom pattern
    let custom_pattern = ConversationPattern::manual(
        PatternType::Reference,
        r"\bCUSTOM-\d+\b",
    )
    .with_description("Custom reference pattern")
    .with_tag("custom");

    println!("\nCustom pattern created:");
    println!("  ID: {}", custom_pattern.id);
    println!("  Type: {}", custom_pattern.pattern_type.display_name());
    println!("  Pattern: {}", custom_pattern.pattern);
    println!("  Description: {:?}", custom_pattern.description);

    // 5. Create a CoherenceDetector with the registry
    let detector = CoherenceDetector::new(0.7);
    println!("\nCoherenceDetector created with threshold: 0.7");

    // 6. Test coherence detection
    let messages = vec![
        Message {
            role: Role::User,
            content: MessageContent::Text("Let's implement feature X".to_string()),
        },
        Message {
            role: Role::Assistant,
            content: MessageContent::Text("As I mentioned earlier, feature X is important".to_string()),
        },
    ];

    let coherence = detector.calculate_coherence(&messages);
    println!("  Coherence score for test messages: {:.2}", coherence);
    println!("  Should keep together: {}", detector.should_keep_together(&messages));

    // 7. Access the pattern registry from the detector
    let detector_registry = detector.pattern_registry();
    println!("\nDetector's pattern registry has {} patterns", detector_registry.len());

    // 8. Use HardcodeConfig
    let config = HardcodeConfig::complex_technical();
    let _detector_with_config = CoherenceDetector::new(0.7)
        .with_hardcode_config(config.clone());
    println!("\nCoherenceDetector created with HardcodeConfig (complex_technical)");

    // 9. Get registry statistics
    let stats = registry.stats();
    println!("\nPattern Registry Stats:");
    println!("  Total: {}", stats.total);
    println!("  Active: {}", stats.active);
    println!("  Presets: {}", stats.presets);
    println!("  Manual: {}", stats.manual);

    println!("\n=== Integration Example Complete ===");
}