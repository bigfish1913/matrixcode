//! Common spinner utilities for all tools.
//! 
//! Provides a RAII-based spinner that automatically clears on drop,
//! ensuring proper cleanup even when errors occur.

use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

/// Spinner animation frames (Braille patterns)
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// RAII guard that manages a progress spinner.
/// 
/// The spinner is created on construction and automatically cleared
/// when dropped, ensuring cleanup even on early returns or errors.
/// 
/// # Example
/// 
/// ```ignore
/// let spinner = ToolSpinner::new("reading file.txt");
/// let content = tokio::fs::read_to_string("file.txt").await?;
/// spinner.finish("✓ 100 lines");
/// // If the read fails, spinner is still cleared via Drop
/// ```
pub struct ToolSpinner {
    bar: ProgressBar,
}

impl ToolSpinner {
    /// Create a new spinner with the given message.
    /// The spinner starts ticking immediately.
    pub fn new(msg: &str) -> Self {
        let bar = ProgressBar::new_spinner();
        bar.set_style(
            ProgressStyle::with_template("{spinner:.cyan} {msg}")
                .unwrap_or_else(|_| ProgressStyle::default_spinner())
                .tick_strings(SPINNER_FRAMES),
        );
        bar.set_message(msg.to_string());
        bar.enable_steady_tick(Duration::from_millis(80));
        bar.tick(); // force immediate draw for fast operations
        Self { bar }
    }

    /// Finish the spinner with a final message (shown briefly before clearing).
    pub fn finish(&self, msg: &str) {
        self.bar.finish_with_message(msg.to_string());
    }

    /// Finish the spinner with a success message.
    pub fn finish_success(&self, msg: &str) {
        self.finish(&format!("✓ {}", msg));
    }

    /// Finish the spinner with an error message.
    pub fn finish_error(&self, msg: &str) {
        self.finish(&format!("✗ {}", msg));
    }

    /// Update the spinner message without stopping it.
    pub fn set_message(&self, msg: &str) {
        self.bar.set_message(msg.to_string());
    }

    /// Get the underlying ProgressBar for advanced usage.
    pub fn bar(&self) -> &ProgressBar {
        &self.bar
    }
}

impl Drop for ToolSpinner {
    fn drop(&mut self) {
        // Always clear the spinner line when dropped
        self.bar.finish_and_clear();
    }
}

/// Convenience function to run a synchronous operation with a spinner.
/// The spinner shows the message while the operation runs, then shows
/// the success message on completion (or clears on error).
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spinner_can_be_created_and_finished() {
        let spinner = ToolSpinner::new("test operation");
        spinner.finish("done");
        // Spinner should be cleared on drop
    }

    #[test]
    fn spinner_success_format() {
        let spinner = ToolSpinner::new("test");
        spinner.finish_success("completed");
    }

    #[test]
    fn spinner_error_format() {
        let spinner = ToolSpinner::new("test");
        spinner.finish_error("failed");
    }
}