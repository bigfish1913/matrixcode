//! Common spinner utilities for all tools.
//! 
//! Provides a RAII-based spinner that automatically clears on drop,
//! ensuring proper cleanup even when errors occur.

use indicatif::{ProgressBar, ProgressStyle};
use std::io::Write;
use std::time::{Duration, Instant};

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
/// let mut spinner = ToolSpinner::new("reading file.txt");
/// let content = tokio::fs::read_to_string("file.txt").await?;
/// spinner.finish("✓ 100 lines");
/// // If the read fails, spinner is still cleared via Drop
/// ```
pub struct ToolSpinner {
    bar: ProgressBar,
    created_at: Instant,
    finished: bool,  // Track if spinner has been cleared to prevent double-clear in Drop
}

/// Minimum time spinner should be visible (in milliseconds)
const MIN_DISPLAY_MS: u64 = 100;

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

        // Force stdout flush to ensure spinner appears immediately
        let _ = std::io::stdout().flush();

        Self { bar, created_at: Instant::now(), finished: false }
    }

    /// Finish the spinner and print a success message on a new line.
    /// This ensures minimum display time before clearing.
    /// Note: This method is synchronous and may block briefly.
    pub fn finish(&mut self, msg: &str) {
        // Ensure spinner has been visible for at least MIN_DISPLAY_MS
        let elapsed = self.created_at.elapsed();
        if elapsed < Duration::from_millis(MIN_DISPLAY_MS) {
            // Use a blocking wait - this is acceptable in tool execution context
            // where we want to ensure the user sees the feedback
            std::thread::sleep(Duration::from_millis(MIN_DISPLAY_MS) - elapsed);
        }

        self.bar.finish_and_clear();
        self.finished = true;
        println!("  ✓ {}", msg);
    }

    /// Finish the spinner with a success message.
    pub fn finish_success(&mut self, msg: &str) {
        self.finish(msg);
    }

    /// Finish the spinner with an error message.
    pub fn finish_error(&mut self, msg: &str) {
        // Ensure spinner has been visible for at least MIN_DISPLAY_MS
        let elapsed = self.created_at.elapsed();
        if elapsed < Duration::from_millis(MIN_DISPLAY_MS) {
            std::thread::sleep(Duration::from_millis(MIN_DISPLAY_MS) - elapsed);
        }

        self.bar.finish_and_clear();
        self.finished = true;
        println!("  ✗ {}", msg);
    }

    /// Clear the spinner without printing any message.
    /// Useful when another spinner or output will follow immediately.
    pub fn finish_clear(&mut self) {
        // Ensure spinner has been visible for at least MIN_DISPLAY_MS
        let elapsed = self.created_at.elapsed();
        if elapsed < Duration::from_millis(MIN_DISPLAY_MS) {
            std::thread::sleep(Duration::from_millis(MIN_DISPLAY_MS) - elapsed);
        }

        self.bar.finish_and_clear();
        self.finished = true;
    }

    /// Clear the spinner immediately without waiting for minimum display time.
    /// Useful for transitional spinners that hand off to another spinner.
    pub fn finish_clear_immediate(&mut self) {
        self.bar.finish_and_clear();
        self.finished = true;
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
        // Only clear if not already finished
        // This prevents double-clear when finish methods were called explicitly
        if !self.finished {
            self.bar.finish_and_clear();
        }
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
        let mut spinner = ToolSpinner::new("test operation");
        spinner.finish("done");
        // Spinner should be cleared on drop
    }

    #[test]
    fn spinner_success_format() {
        let mut spinner = ToolSpinner::new("test");
        spinner.finish_success("completed");
    }

    #[test]
    fn spinner_error_format() {
        let mut spinner = ToolSpinner::new("test");
        spinner.finish_error("failed");
    }
}