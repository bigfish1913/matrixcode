//! CLI constants and configuration defaults
//!
//! Centralizes hardcoded values for easier maintenance.

/// Default model name
pub const DEFAULT_MODEL: &str = "claude-sonnet-4-20250514";

/// Default max tokens for responses
pub const DEFAULT_MAX_TOKENS: u32 = 16384;

/// Max tokens for quick actions (smaller for faster response)
pub const QUICK_ACTION_MAX_TOKENS: u32 = 4096;

/// Event channel buffer size
pub const EVENT_CHANNEL_BUFFER: usize = 100;

/// Task channel buffer size (increased for merged queue messages)
pub const TASK_CHANNEL_BUFFER: usize = 100;

/// Ask channel buffer size
pub const ASK_CHANNEL_BUFFER: usize = 1;

/// Cleanup grace period in milliseconds
pub const CLEANUP_TIMEOUT_MS: u64 = 500;

/// Event timeout in milliseconds
pub const EVENT_TIMEOUT_MS: u64 = 100;

/// Session cleanup age in days
pub const SESSION_CLEANUP_DAYS: u64 = 30;

/// Display limits
pub const DISPLAY_SESSIONS_LIMIT: usize = 10;
#[allow(dead_code)]
pub const DISPLAY_OVERVIEW_CHARS_LIMIT: usize = 2000;
#[allow(dead_code)]
pub const DISPLAY_MEMORY_SEARCH_LIMIT: usize = 10;
pub const DISPLAY_ERROR_CHARS_LIMIT: usize = 50;

/// Memory configuration
pub const MEMORY_MANIFEST_SIZE: usize = 50;
pub const MEMORY_SUMMARY_SIZE: usize = 20;
pub const MEMORY_INITIAL_SUMMARY_SIZE: usize = 10;
pub const MEMORY_TURN_CLEANUP_INTERVAL: usize = 10;
pub const MEMORY_EXTRACTION_INTERVAL: usize = 3;
pub const MEMORY_MIN_ENTRIES_FOR_AI_SELECTION: usize = 5;