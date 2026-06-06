//! LSP Progress Tracking Module
//!
//! Tracks LSP initialization progress and provides callbacks for UI updates.
//! Separated from core LSP logic to keep the library independent of UI frameworks.

use std::sync::Arc;
use std::time::Instant;

/// LSP initialization status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LspInitStatus {
    /// Not started yet
    NotStarted,
    /// Process is spawning
    Spawning,
    /// Server is initializing (building indexes, loading workspace)
    Initializing,
    /// Server is ready for requests
    Ready,
    /// Initialization failed
    Failed,
}

impl LspInitStatus {
    /// Get display label for the status
    pub fn label(&self) -> &'static str {
        match self {
            Self::NotStarted => "Not Started",
            Self::Spawning => "Starting Process",
            Self::Initializing => "Initializing",
            Self::Ready => "Ready",
            Self::Failed => "Failed",
        }
    }

    /// Check if initialization is complete (success or failure)
    pub fn is_complete(&self) -> bool {
        matches!(self, Self::Ready | Self::Failed)
    }

    /// Check if server is ready
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }

    /// Check if initialization is in progress
    pub fn is_in_progress(&self) -> bool {
        matches!(self, Self::Spawning | Self::Initializing)
    }
}

/// LSP progress tracker
///
/// Tracks the initialization progress of a single LSP server.
pub struct LspProgressTracker {
    /// Language name (e.g., "rust", "typescript")
    language: String,
    /// Server name (e.g., "rust-analyzer", "typescript-language-server")
    server_name: String,
    /// Current status
    status: LspInitStatus,
    /// Progress percentage (0.0 - 1.0)
    progress: f32,
    /// Human-readable progress message
    message: String,
    /// Timestamp when initialization started
    started_at: Option<Instant>,
}

impl LspProgressTracker {
    /// Create a new progress tracker
    pub fn new(language: impl Into<String>, server_name: impl Into<String>) -> Self {
        Self {
            language: language.into(),
            server_name: server_name.into(),
            status: LspInitStatus::NotStarted,
            progress: 0.0,
            message: "Waiting to start...".to_string(),
            started_at: None,
        }
    }

    /// Get language name
    pub fn language(&self) -> &str {
        &self.language
    }

    /// Get server name
    pub fn server_name(&self) -> &str {
        &self.server_name
    }

    /// Get current status
    pub fn status(&self) -> LspInitStatus {
        self.status
    }

    /// Get progress percentage (0.0 - 1.0)
    pub fn progress(&self) -> f32 {
        self.progress
    }

    /// Get progress message
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Get elapsed time since initialization started
    pub fn elapsed_secs(&self) -> Option<f64> {
        self.started_at.map(|t| t.elapsed().as_secs_f64())
    }

    /// Check if server is ready
    pub fn is_ready(&self) -> bool {
        self.status.is_ready()
    }

    /// Mark initialization as started
    pub fn start(&mut self) {
        self.status = LspInitStatus::Spawning;
        self.progress = 0.0;
        self.message = "Starting process...".to_string();
        self.started_at = Some(Instant::now());
    }

    /// Update progress during initialization
    pub fn update(&mut self, progress: f32, message: impl Into<String>) {
        if progress > 0.0 && progress <= 1.0 {
            self.progress = progress;
            self.message = message.into();
            
            // Auto-update status based on progress
            if progress < 0.3 {
                self.status = LspInitStatus::Spawning;
            } else if progress < 1.0 {
                self.status = LspInitStatus::Initializing;
            }
        }
    }

    /// Mark initialization as complete (success)
    pub fn complete(&mut self) {
        self.status = LspInitStatus::Ready;
        self.progress = 1.0;
        self.message = "Ready".to_string();
    }

    /// Mark initialization as failed
    pub fn fail(&mut self, error: impl Into<String>) {
        self.status = LspInitStatus::Failed;
        self.progress = 0.0;
        self.message = error.into();
    }
}

/// Progress callback trait for UI updates
///
/// Implement this trait to receive progress updates during LSP initialization.
/// The callback is called from async context, so implementations must be `Send + Sync`.
pub trait LspProgressCallback: Send + Sync {
    /// Called when progress updates
    ///
    /// - `progress`: Percentage (0.0 - 1.0)
    /// - `message`: Human-readable status message
    fn on_progress(&self, progress: f32, message: &str);

    /// Called when initialization completes successfully
    fn on_complete(&self);

    /// Called when initialization fails
    ///
    /// - `error`: Error message
    fn on_error(&self, error: &str);
}

/// No-op callback for testing or when progress updates are not needed
pub struct NoOpProgressCallback;

impl LspProgressCallback for NoOpProgressCallback {
    fn on_progress(&self, _progress: f32, _message: &str) {}
    fn on_complete(&self) {}
    fn on_error(&self, _error: &str) {}
}

/// Logging callback for CLI environments
///
/// Prints progress updates to the console.
pub struct LoggingProgressCallback;

impl LspProgressCallback for LoggingProgressCallback {
    fn on_progress(&self, progress: f32, message: &str) {
        log::info!("LSP Progress: {:.0}% - {}", progress * 100.0, message);
    }

    fn on_complete(&self) {
        log::info!("LSP Initialization complete");
    }

    fn on_error(&self, error: &str) {
        log::error!("LSP Initialization failed: {}", error);
    }
}

/// Multi-callback for combining multiple callbacks
///
/// Allows registering multiple callbacks (e.g., logging + UI update).
pub struct MultiProgressCallback {
    callbacks: Vec<Arc<dyn LspProgressCallback>>,
}

impl MultiProgressCallback {
    /// Create a new multi-callback
    pub fn new(callbacks: Vec<Arc<dyn LspProgressCallback>>) -> Self {
        Self { callbacks }
    }

    /// Add a callback
    pub fn add(&mut self, callback: Arc<dyn LspProgressCallback>) {
        self.callbacks.push(callback);
    }
}

impl LspProgressCallback for MultiProgressCallback {
    fn on_progress(&self, progress: f32, message: &str) {
        for callback in &self.callbacks {
            callback.on_progress(progress, message);
        }
    }

    fn on_complete(&self) {
        for callback in &self.callbacks {
            callback.on_complete();
        }
    }

    fn on_error(&self, error: &str) {
        for callback in &self.callbacks {
            callback.on_error(error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_labels() {
        assert_eq!(LspInitStatus::NotStarted.label(), "Not Started");
        assert_eq!(LspInitStatus::Spawning.label(), "Starting Process");
        assert_eq!(LspInitStatus::Initializing.label(), "Initializing");
        assert_eq!(LspInitStatus::Ready.label(), "Ready");
        assert_eq!(LspInitStatus::Failed.label(), "Failed");
    }

    #[test]
    fn test_status_checks() {
        assert!(LspInitStatus::Ready.is_complete());
        assert!(LspInitStatus::Failed.is_complete());
        assert!(!LspInitStatus::Initializing.is_complete());

        assert!(LspInitStatus::Ready.is_ready());
        assert!(!LspInitStatus::Failed.is_ready());

        assert!(LspInitStatus::Spawning.is_in_progress());
        assert!(LspInitStatus::Initializing.is_in_progress());
        assert!(!LspInitStatus::Ready.is_in_progress());
    }

    #[test]
    fn test_progress_tracker_lifecycle() {
        let mut tracker = LspProgressTracker::new("rust", "rust-analyzer");

        // Initial state
        assert_eq!(tracker.status(), LspInitStatus::NotStarted);
        assert_eq!(tracker.progress(), 0.0);
        assert_eq!(tracker.message(), "Waiting to start...");
        assert!(tracker.elapsed_secs().is_none());

        // Start
        tracker.start();
        assert_eq!(tracker.status(), LspInitStatus::Spawning);
        assert_eq!(tracker.progress(), 0.0);
        assert!(tracker.elapsed_secs().is_some());

        // Update
        tracker.update(0.5, "Loading workspace...");
        assert_eq!(tracker.status(), LspInitStatus::Initializing);
        assert_eq!(tracker.progress(), 0.5);
        assert_eq!(tracker.message(), "Loading workspace...");

        // Complete
        tracker.complete();
        assert_eq!(tracker.status(), LspInitStatus::Ready);
        assert_eq!(tracker.progress(), 1.0);
        assert_eq!(tracker.message(), "Ready");
    }

    #[test]
    fn test_progress_tracker_failure() {
        let mut tracker = LspProgressTracker::new("typescript", "typescript-language-server");
        tracker.start();
        tracker.update(0.3, "Initializing...");

        tracker.fail("Process spawn failed");
        assert_eq!(tracker.status(), LspInitStatus::Failed);
        assert_eq!(tracker.message(), "Process spawn failed");
        assert!(!tracker.is_ready());
    }

    #[test]
    fn test_no_op_callback() {
        let callback = NoOpProgressCallback;
        callback.on_progress(0.5, "test");
        callback.on_complete();
        callback.on_error("test error");
        // No assertions needed - just verify it doesn't crash
    }

    #[test]
    fn test_multi_callback() {
        use std::sync::Mutex;

        struct TestCallback {
            progress_calls: Mutex<Vec<(f32, String)>>,
        }

        impl LspProgressCallback for TestCallback {
            fn on_progress(&self, progress: f32, message: &str) {
                self.progress_calls.lock().unwrap().push((progress, message.to_string()));
            }
            fn on_complete(&self) {}
            fn on_error(&self, _error: &str) {}
        }

        let cb1 = Arc::new(TestCallback {
            progress_calls: Mutex::new(Vec::new()),
        });
        let cb2 = Arc::new(NoOpProgressCallback);

        let multi = MultiProgressCallback::new(vec![cb1.clone(), cb2]);
        multi.on_progress(0.5, "test");

        let calls = cb1.progress_calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], (0.5, "test".to_string()));
    }
}