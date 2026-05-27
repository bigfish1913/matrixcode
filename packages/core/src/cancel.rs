use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// A token that can be used to cancel ongoing operations.
/// Uses an atomic bool for reliable cancellation detection.
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    /// Create a new cancellation token.
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Cancel the operation. All clones will see this.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Check if the token has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Reset the cancellation state. Allows reuse after interrupt.
    /// All clones will see the reset state.
    pub fn reset(&self) {
        self.cancelled.store(false, Ordering::SeqCst);
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for CancellationToken {
    fn clone(&self) -> Self {
        Self {
            cancelled: self.cancelled.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cancel() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());
        token.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn test_clone_cancel() {
        let token = CancellationToken::new();
        let clone = token.clone();
        clone.cancel();
        assert!(token.is_cancelled());
        assert!(clone.is_cancelled());
    }

    #[test]
    fn test_multiple_clones() {
        let token = CancellationToken::new();
        let clone1 = token.clone();
        let clone2 = clone1.clone();

        assert!(!token.is_cancelled());
        assert!(!clone1.is_cancelled());
        assert!(!clone2.is_cancelled());

        token.cancel();

        assert!(token.is_cancelled());
        assert!(clone1.is_cancelled());
        assert!(clone2.is_cancelled());
    }

    #[test]
    fn test_reset() {
        let token = CancellationToken::new();
        token.cancel();
        assert!(token.is_cancelled());
        token.reset();
        assert!(!token.is_cancelled());
    }

    #[test]
    fn test_reset_visible_in_clones() {
        let token = CancellationToken::new();
        let clone = token.clone();

        token.cancel();
        assert!(clone.is_cancelled());

        clone.reset();
        assert!(!token.is_cancelled());
        assert!(!clone.is_cancelled());
    }

    #[test]
    fn test_cancel_reset_cycle() {
        let token = CancellationToken::new();

        // First cycle
        token.cancel();
        assert!(token.is_cancelled());
        token.reset();
        assert!(!token.is_cancelled());

        // Second cycle
        token.cancel();
        assert!(token.is_cancelled());
        token.reset();
        assert!(!token.is_cancelled());

        // Third cycle
        token.cancel();
        assert!(token.is_cancelled());
    }
}
