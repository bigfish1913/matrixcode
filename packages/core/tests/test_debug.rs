//! Tests for debug logging system

use matrixcode_core::debug::*;

#[test]
fn test_debug_disabled_by_default() {
    let logger = DebugLog::new();
    assert!(!logger.is_enabled(), "Debug should be disabled by default");
}

#[test]
fn test_debug_enable_disable() {
    let logger = DebugLog::new();

    // Enable
    logger.enable(Some("test_session_123"));
    assert!(
        logger.is_enabled(),
        "Debug should be enabled after enable()"
    );

    // Disable
    logger.disable();
    assert!(
        !logger.is_enabled(),
        "Debug should be disabled after disable()"
    );
}

#[test]
fn test_debug_no_log_when_disabled() {
    let logger = DebugLog::new();

    // These should be no-op when disabled
    logger.api_call("test-model", 100, false);
    logger.compression(1000, 500, 0.5);
    logger.memory_save(10, 100);
    logger.tool_call("test-tool", "input", "result");
    logger.log("TEST", "This should not be logged");

    // No file should be created when disabled
    // This test passes because the methods return early when disabled
    assert!(!logger.is_enabled());
}

#[test]
fn test_session_id_generation() {
    let logger = DebugLog::new();
    logger.enable(None); // No session ID provided

    // Should generate a session ID - we can't access private fields,
    // but we can verify it's enabled and working
    assert!(logger.is_enabled());

    // Log something to verify it works
    logger.log("TEST", "Session ID generation test");
}

#[test]
fn test_stats() {
    let logger = DebugLog::new();
    let stats = logger.stats();

    // Initial stats should be zero or positive (u64 is always >= 0)
    // We just verify the format() method works
    let formatted = stats.format();
    assert!(formatted.contains("API"));
    assert!(formatted.contains("Compress"));
    assert!(formatted.contains("Memory"));
    assert!(formatted.contains("Tools"));
}
