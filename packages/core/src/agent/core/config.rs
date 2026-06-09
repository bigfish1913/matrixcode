//! Agent configuration constants.
//!
//! This module defines configuration constants that were previously hardcoded
//! throughout the agent module. By extracting these into a configuration struct,
//! we enable:
//! - Dynamic adjustment based on task complexity
//! - User-configurable limits in config files
//! - Better testability (can test edge cases)
//! - Clearer documentation of defaults

use crate::compress::CompressionConfig;
use crate::tools::code_quality_hook::VerificationStrategy;

/// Maximum number of iterations in the main loop.
///
/// This constant is used as the default for [`AgentConfig::max_iterations`].
/// It is re-exported from the parent module for backward compatibility.
///
/// See [`AgentConfig::max_iterations`] for detailed documentation.
pub const MAX_ITERATIONS: usize = 200;

/// Agent runtime configuration.
///
/// Controls iteration limits, retry behavior, size thresholds, and compression settings.
/// All defaults match the original hardcoded values for backward compatibility.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Maximum number of iterations in the main loop.
    ///
    /// **Default**: 200
    ///
    /// **Why this value?**
    /// - Sufficient for most common tasks (file edits, code review, simple builds)
    /// - Prevents infinite loops and runaway operations
    /// - Balances task completion with resource efficiency
    ///
    /// **What happens when limit is reached?**
    /// - Agent stops execution gracefully
    /// - User receives detailed warning message
    /// - Can continue, break down task, or resume
    ///
    /// **Examples**:
    /// - Simple task (edit file): ~5-10 iterations
    /// - Medium task (refactor module): ~15-30 iterations
    /// - Complex task (build system): ~40-50 iterations (may hit limit)
    pub max_iterations: usize,

    /// Maximum retry attempts for API calls.
    ///
    /// **Default**: 5
    ///
    /// **Why this value?**
    /// - Handles transient network failures
    /// - Balances resilience with response time
    /// - Most failures resolve within 2-3 retries
    pub max_retries: u32,

    /// Delay between retry attempts (milliseconds).
    ///
    /// **Default**: 1000ms
    ///
    /// **Why this value?**
    /// - Gives API server time to recover
    /// - Not too long to frustrate users
    /// - Often used with exponential backoff
    pub retry_delay_ms: u64,

    /// Maximum size for tool execution results (bytes).
    ///
    /// **Default**: 50,000 (50KB)
    ///
    /// **Why this value?**
    /// - Prevents oversized responses from breaking API calls
    /// - Fits within typical context window limits
    /// - Most tool results are <10KB
    ///
    /// **Truncation behavior**:
    /// - Results larger than this are truncated
    /// - Truncation message added: "⚠️ Output truncated (X bytes total)"
    /// - User can request specific portions if needed
    pub max_tool_result_size: usize,

    /// Maximum number of todo reminders per todo item.
    ///
    /// **Default**: 2
    ///
    /// **Why this value?**
    /// - Prevents infinite loops when model doesn't update todo status
    /// - Gives model multiple chances to notice pending work
    /// - After limit, stops reminding (assumes model won't update)
    pub max_todo_reminders: usize,

    /// Iteration count threshold for warning user.
    ///
    /// **Default**: max_iterations - 10 (190)
    ///
    /// **Why this value?**
    /// - Warns user 10 iterations before limit
    /// - Gives time to decide: continue, break down task, or cancel
    /// - Prevents surprising cutoff at exactly max_iterations
    pub iteration_warning_threshold: usize,

    /// Context compression configuration.
    ///
    /// Controls when and how to compress long conversations.
    /// See [`CompressionConfig`] for details.
    pub compression: CompressionConfig,

    // === API Request Settings ===
    /// Maximum tokens for API responses.
    ///
    /// **Default**: 4096 (from QUICK_ACTION_MAX_TOKENS)
    pub max_tokens: u32,

    /// Override for context window size.
    ///
    /// **Default**: None (use provider's default)
    pub context_size_override: Option<u32>,

    /// Enable thinking mode for extended reasoning.
    ///
    /// **Default**: false
    pub think: bool,

    /// Code verification strategy for write operations.
    ///
    /// Controls whether and when code quality checks run on edit/write/multi_edit.
    /// - `None`: No verification
    /// - `Post`: Verify after write (default)
    /// - `Pre`: Verify before write, block if errors
    /// - `PreQuick`: Quick syntax check before, full check after
    pub verify_strategy: VerificationStrategy,

    /// Project root path for code verification.
    ///
    /// Used to detect project type (Rust/Node.js/Python/Go) and run
    /// appropriate verification commands (cargo check, tsc, etc.).
    pub project_path: Option<std::path::PathBuf>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_iterations: MAX_ITERATIONS,
            max_retries: 5,
            retry_delay_ms: 1000,
            max_tool_result_size: 50_000,
            max_todo_reminders: 2,
            iteration_warning_threshold: MAX_ITERATIONS.saturating_sub(10),
            compression: CompressionConfig::default(),
            max_tokens: crate::constants::QUICK_ACTION_MAX_TOKENS,
            context_size_override: None,
            think: false,
            verify_strategy: VerificationStrategy::default(),
            project_path: None,
        }
    }
}

impl AgentConfig {
    /// Create a new config with all settings.
    pub fn new(
        max_tokens: u32,
        context_size_override: Option<u32>,
        think: bool,
        compression: CompressionConfig,
    ) -> Self {
        Self {
            max_tokens,
            context_size_override,
            think,
            compression,
            ..Self::default()
        }
    }

    /// Create a new config with verification strategy.
    pub fn with_verify_strategy(
        verify_strategy: VerificationStrategy,
        project_path: Option<std::path::PathBuf>,
    ) -> Self {
        Self {
            verify_strategy,
            project_path,
            ..Self::default()
        }
    }

    /// Create a new config with custom max_iterations.
    ///
    /// Automatically adjusts iteration_warning_threshold to max_iterations - 10.
    pub fn with_max_iterations(max_iterations: usize) -> Self {
        Self {
            max_iterations,
            iteration_warning_threshold: max_iterations.saturating_sub(10),
            ..Self::default()
        }
    }

    /// Create a new config with custom compression settings.
    pub fn with_compression(compression: CompressionConfig) -> Self {
        Self {
            compression,
            ..Self::default()
        }
    }

    /// Create a config optimized for simple tasks.
    ///
    /// Use for: file edits, quick queries, simple tool calls.
    /// Characteristics: lower iteration limit, smaller result size.
    pub fn simple_task() -> Self {
        Self {
            max_iterations: 50,
            iteration_warning_threshold: 40,
            max_tool_result_size: 10_000,
            ..Self::default()
        }
    }

    /// Create a config optimized for complex tasks.
    ///
    /// Use for: refactoring, build systems, multi-file changes.
    /// Characteristics: higher iteration limit, larger result size.
    pub fn complex_task() -> Self {
        Self {
            max_iterations: 300,
            iteration_warning_threshold: 290,
            max_tool_result_size: 100_000,
            max_todo_reminders: 3,
            ..Self::default()
        }
    }

    // === Accessor methods ===

    /// Get max tokens for API responses.
    pub fn max_tokens(&self) -> u32 {
        self.max_tokens
    }

    /// Get context size override.
    pub fn context_size_override(&self) -> Option<u32> {
        self.context_size_override
    }

    /// Get thinking mode flag.
    pub fn think(&self) -> bool {
        self.think
    }

    /// Get compression config.
    pub fn compression_config(&self) -> &CompressionConfig {
        &self.compression
    }

    /// Get mutable compression config.
    pub fn compression_config_mut(&mut self) -> &mut CompressionConfig {
        &mut self.compression
    }

    /// Get verification strategy.
    pub fn verify_strategy(&self) -> VerificationStrategy {
        self.verify_strategy
    }

    /// Get project path for verification.
    pub fn project_path(&self) -> Option<&std::path::Path> {
        self.project_path.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_matches_hardcoded_values() {
        let config = AgentConfig::default();

        // Verify all defaults match original hardcoded constants
        assert_eq!(config.max_iterations, 200, "max_iterations default changed");
        assert_eq!(config.max_retries, 5, "max_retries default changed");
        assert_eq!(config.retry_delay_ms, 1000, "retry_delay_ms default changed");
        assert_eq!(config.max_tool_result_size, 50_000, "max_tool_result_size default changed");
        assert_eq!(config.max_todo_reminders, 2, "max_todo_reminders default changed");
        assert_eq!(config.iteration_warning_threshold, 190, "iteration_warning_threshold default changed");
    }

    #[test]
    fn test_with_max_iterations_adjusts_warning_threshold() {
        let config = AgentConfig::with_max_iterations(100);

        assert_eq!(config.max_iterations, 100);
        assert_eq!(config.iteration_warning_threshold, 90, "warning threshold should be max - 10");
    }

    #[test]
    fn test_with_max_iterations_handles_small_values() {
        let config = AgentConfig::with_max_iterations(5);

        // saturating_sub prevents negative threshold
        assert_eq!(config.iteration_warning_threshold, 0, "threshold should use saturating_sub");
    }

    #[test]
    fn test_simple_task_optimization() {
        let config = AgentConfig::simple_task();

        assert_eq!(config.max_iterations, 50, "simple tasks should have lower iteration limit");
        assert_eq!(config.max_tool_result_size, 10_000, "simple tasks should have smaller result size");
    }

    #[test]
    fn test_complex_task_optimization() {
        let config = AgentConfig::complex_task();

        assert_eq!(config.max_iterations, 300, "complex tasks should have higher iteration limit");
        assert_eq!(config.max_tool_result_size, 100_000, "complex tasks should have larger result size");
        assert_eq!(config.max_todo_reminders, 3, "complex tasks should allow more reminders");
    }
}