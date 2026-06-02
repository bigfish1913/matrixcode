//! Read history tracker for edit/write precondition checks.
//!
//! Ensures files are read before being modified to prevent accidental overwrites.

use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};

/// Tracker for files that have been read in the current session.
///
/// This tracker is used to enforce the "read before edit/write" rule:
/// - Files must be read with the `read` tool before they can be edited or written
/// - This prevents accidental overwrites and ensures context awareness
#[derive(Debug, Clone, Default)]
pub struct ReadHistoryTracker {
    /// Set of file paths that have been read (normalized paths)
    read_files: HashSet<String>,

    /// Timestamps of when each file was read (for debugging/auditing)
    read_timestamps: HashMap<String, DateTime<Utc>>,
}

impl ReadHistoryTracker {
    /// Create a new empty tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark a file as having been read.
    ///
    /// # Arguments
    /// * `file_path` - The path to the file that was read
    ///
    /// # Note
    /// Paths are normalized to handle different path representations:
    /// - Relative paths are converted to absolute paths where possible
    /// - Path separators are normalized
    pub fn mark_read(&mut self, file_path: &str) {
        let normalized = self.normalize_path(file_path);
        self.read_files.insert(normalized.clone());
        self.read_timestamps.insert(normalized, Utc::now());
    }

    /// Check if a file has been read in this session.
    ///
    /// # Arguments
    /// * `file_path` - The path to check
    ///
    /// # Returns
    /// `true` if the file has been read, `false` otherwise
    pub fn has_read(&self, file_path: &str) -> bool {
        let normalized = self.normalize_path(file_path);
        self.read_files.contains(&normalized)
    }

    /// Clear all read history (for new session).
    pub fn clear(&mut self) {
        self.read_files.clear();
        self.read_timestamps.clear();
    }

    /// Get the count of files that have been read.
    pub fn count(&self) -> usize {
        self.read_files.len()
    }

    /// Get the list of files that have been read.
    pub fn read_files(&self) -> Vec<&String> {
        self.read_files.iter().collect()
    }

    /// Get the timestamp when a file was read.
    pub fn read_timestamp(&self, file_path: &str) -> Option<DateTime<Utc>> {
        let normalized = self.normalize_path(file_path);
        self.read_timestamps.get(&normalized).copied()
    }

    /// Normalize a file path for consistent comparison.
    ///
    /// This handles:
    /// - Path separator normalization (Windows vs Unix)
    /// - Trailing slashes
    fn normalize_path(&self, path: &str) -> String {
        // Convert backslashes to forward slashes for consistency
        let normalized = path.replace('\\', "/");
        // Remove trailing slash (except for root paths like "/")
        if normalized.len() > 1 && normalized.ends_with('/') {
            normalized.trim_end_matches('/').to_string()
        } else {
            normalized
        }
    }
}

/// Error type for "must read first" violations.
///
/// This error is returned when attempting to edit or write a file
/// that has not been read in the current session.
#[derive(Debug, Clone)]
pub struct MustReadFirstError {
    /// The file path that needs to be read
    pub file: String,
    /// Human-readable error message
    pub message: String,
}

impl MustReadFirstError {
    /// Create a new MustReadFirstError.
    pub fn new(file: impl Into<String>) -> Self {
        let file_path = file.into();
        Self {
            file: file_path.clone(),
            message: format!(
                "File '{}' has not been read in this session. \
                 Please use the 'read' tool to read the file first before editing or writing. \
                 This ensures you understand the current file content and context.",
                file_path
            ),
        }
    }

    /// Get the file path.
    pub fn file(&self) -> &str {
        &self.file
    }

    /// Get the error message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for MustReadFirstError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for MustReadFirstError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mark_and_check_read() {
        let mut tracker = ReadHistoryTracker::new();

        // Initially no files are read
        assert!(!tracker.has_read("test.txt"));

        // Mark as read
        tracker.mark_read("test.txt");

        // Now it should be marked as read
        assert!(tracker.has_read("test.txt"));
    }

    #[test]
    fn test_path_normalization() {
        let mut tracker = ReadHistoryTracker::new();

        // Windows-style path
        tracker.mark_read("C:\\Users\\test\\file.txt");

        // Should recognize Unix-style equivalent
        assert!(tracker.has_read("C:/Users/test/file.txt"));

        // And vice versa
        tracker.mark_read("/home/user/file.txt");
        assert!(tracker.has_read("/home/user/file.txt"));
    }

    #[test]
    fn test_trailing_slash() {
        let mut tracker = ReadHistoryTracker::new();

        tracker.mark_read("path/to/file");

        // Should recognize path with trailing slash
        assert!(tracker.has_read("path/to/file/"));
    }

    #[test]
    fn test_clear() {
        let mut tracker = ReadHistoryTracker::new();

        tracker.mark_read("file1.txt");
        tracker.mark_read("file2.txt");

        assert_eq!(tracker.count(), 2);

        tracker.clear();

        assert_eq!(tracker.count(), 0);
        assert!(!tracker.has_read("file1.txt"));
    }

    #[test]
    fn test_error_message() {
        let error = MustReadFirstError::new("test.rs");

        assert_eq!(error.file(), "test.rs");
        assert!(error.message().contains("test.rs"));
        assert!(error.message().contains("read"));
    }
}