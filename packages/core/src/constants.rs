//! Core constants and configuration defaults
//!
//! Centralizes hardcoded values for easier maintenance.

/// Default max tokens for responses
pub const DEFAULT_MAX_TOKENS: u32 = 16384;

/// Max tokens for quick actions/compression
pub const QUICK_ACTION_MAX_TOKENS: u32 = 4096;

/// Max tokens for compress role (shorter output)
pub const COMPRESS_MAX_TOKENS: u32 = 1024;

/// Max tokens for fast role (quick actions)
pub const FAST_MAX_TOKENS: u32 = 2048;

/// Max tokens for memory extraction
pub const MEMORY_EXTRACTION_MAX_TOKENS: u32 = 512;

/// Max tokens for AI memory selection
pub const AI_MEMORY_SELECTION_MAX_TOKENS: u32 = 100;

/// Default connect timeout in seconds
pub const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 10;

/// Default total request timeout in seconds
pub const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 300;

/// Default read timeout for streaming chunks (seconds)
pub const DEFAULT_READ_TIMEOUT_SECS: u64 = 60;

/// Default content timeout in seconds (for slow APIs)
pub const DEFAULT_CONTENT_TIMEOUT_SECS: u64 = 300;

/// Thinking budget for new models (2025+)
pub const THINKING_BUDGET_NEW_MODELS: u32 = 10000;

/// Thinking budget for older models
pub const THINKING_BUDGET_OLD_MODELS: u32 = 5000;

/// Max tool result size (bytes)
pub const MAX_TOOL_RESULT_SIZE: usize = 50_000;

/// Max file size for read without warning (bytes)
pub const MAX_READ_FILE_SIZE: u64 = 5_000_000;

/// Default read limit (lines)
pub const DEFAULT_READ_LIMIT: usize = 500;

/// Default search timeout (seconds)
pub const DEFAULT_SEARCH_TIMEOUT_SECS: u64 = 30;

/// Max files for search
pub const MAX_SEARCH_FILES: usize = 500;

/// Default grep head limit
pub const DEFAULT_GREP_HEAD_LIMIT: usize = 100;

/// Max output for bash commands (bytes)
pub const MAX_BASH_OUTPUT: usize = 30_000;

/// Default bash timeout (milliseconds)
pub const DEFAULT_BASH_TIMEOUT_MS: u64 = 30_000;

/// Lock acquire timeout (milliseconds)
pub const LOCK_ACQUIRE_TIMEOUT_MS: u64 = 5000;

/// Retry delay for streaming (milliseconds)
pub const STREAMING_RETRY_DELAY_MS: u64 = 1000;

/// Max retries for streaming
pub const MAX_STREAMING_RETRIES: u32 = 5;

/// Anthropic API version header
pub const ANTHROPIC_API_VERSION: &str = "2025-04-15";

/// Default base URLs
pub const ANTHROPIC_DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
pub const OPENAI_DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

/// CodeGraph CLI command timeout (seconds)
pub const CODEGRAPH_CLI_TIMEOUT_SECS: u64 = 60;

/// CodeGraph auto-sync debounce interval (seconds)
pub const CODEGRAPH_SYNC_INTERVAL_SECS: u64 = 2;