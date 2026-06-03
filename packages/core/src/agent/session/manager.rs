//! Agent session management.
//!
//! This module manages the session lifecycle, including:
//! - Event emission
//! - Cancellation tokens
//! - Ask response channel
//! - Pending input channel (for real-time message appending)
//!
//! By extracting session management into a dedicated struct, we enable:
//! - Clear separation between session and state
//! - Easier testing of event handling
//! - Better control over session lifecycle

use anyhow::Result;
use tokio::sync::mpsc;

use crate::cancel::CancellationToken;
use crate::event::AgentEvent;

/// Agent session manager.
///
/// Handles session lifecycle and communication channels.
pub struct SessionManager {
    /// Cancellation token for stopping the session.
    cancel_token: Option<CancellationToken>,

    /// Event sender for emitting agent events.
    event_tx: mpsc::Sender<AgentEvent>,

    /// Ask response receiver (for TUI mode).
    ask_rx: Option<mpsc::Receiver<String>>,

    /// Pending input receiver (for real-time message appending during streaming).
    pending_input_rx: Option<mpsc::Receiver<String>>,
}

impl SessionManager {
    /// Create a new session manager with event sender.
    pub fn new(event_tx: mpsc::Sender<AgentEvent>) -> Self {
        Self {
            cancel_token: None,
            event_tx,
            ask_rx: None,
            pending_input_rx: None,
        }
    }

    /// Create a session manager with all channels.
    pub fn with_channels(
        event_tx: mpsc::Sender<AgentEvent>,
        ask_rx: Option<mpsc::Receiver<String>>,
    ) -> Self {
        Self {
            cancel_token: None,
            event_tx,
            ask_rx,
            pending_input_rx: None,
        }
    }

    /// Create a session manager with pending input channel.
    pub fn with_pending_input(
        event_tx: mpsc::Sender<AgentEvent>,
        pending_input_rx: Option<mpsc::Receiver<String>>,
    ) -> Self {
        Self {
            cancel_token: None,
            event_tx,
            ask_rx: None,
            pending_input_rx,
        }
    }

    /// Create a session manager with all channels including pending input.
    pub fn with_all_channels(
        event_tx: mpsc::Sender<AgentEvent>,
        ask_rx: Option<mpsc::Receiver<String>>,
        pending_input_rx: Option<mpsc::Receiver<String>>,
    ) -> Self {
        Self {
            cancel_token: None,
            event_tx,
            ask_rx,
            pending_input_rx,
        }
    }

    /// Emit an agent event.
    pub fn emit(&self, event: AgentEvent) -> Result<()> {
        self.event_tx
            .try_send(event)
            .map_err(|e| anyhow::anyhow!("Failed to emit event: {}", e))?;
        Ok(())
    }

    /// Check if session is cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancel_token
            .as_ref()
            .map(|t| t.is_cancelled())
            .unwrap_or(false)
    }

    /// Set cancellation token.
    pub fn set_cancel_token(&mut self, token: CancellationToken) {
        self.cancel_token = Some(token);
    }

    /// Get cancellation token.
    pub fn cancel_token(&self) -> Option<&CancellationToken> {
        self.cancel_token.as_ref()
    }

    /// Set ask response channel.
    pub fn set_ask_channel(&mut self, rx: mpsc::Receiver<String>) {
        self.ask_rx = Some(rx);
    }

    /// Check if ask response channel is available (no mutable borrow needed).
    pub fn has_ask_channel(&self) -> bool {
        self.ask_rx.is_some()
    }

    /// Get ask response channel.
    pub fn ask_rx(&mut self) -> Option<&mut mpsc::Receiver<String>> {
        self.ask_rx.as_mut()
    }

    /// Set pending input channel.
    pub fn set_pending_input_channel(&mut self, rx: mpsc::Receiver<String>) {
        self.pending_input_rx = Some(rx);
    }

    /// Get pending input channel.
    pub fn pending_input_rx(&mut self) -> Option<&mut mpsc::Receiver<String>> {
        self.pending_input_rx.as_mut()
    }

    /// Drain pending inputs from the channel.
    pub fn drain_pending_inputs(&mut self) -> Vec<String> {
        let mut inputs = Vec::new();
        if let Some(rx) = &mut self.pending_input_rx {
            while let Ok(msg) = rx.try_recv() {
                inputs.push(msg);
            }
        }
        inputs
    }

    /// Get event sender (for cloning).
    pub fn event_sender(&self) -> mpsc::Sender<AgentEvent> {
        self.event_tx.clone()
    }

    /// Clear session state (reset cancellation).
    pub fn clear(&mut self) {
        self.cancel_token = None;
        // Note: ask_rx, pending_input_rx and event_tx are not cleared as they are channels
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_new_creates_empty_session() {
        let (tx, _rx) = mpsc::channel(100);
        let session = SessionManager::new(tx);

        assert!(session.cancel_token.is_none());
        assert!(session.ask_rx.is_none());
        assert!(!session.is_cancelled());
    }

    #[test]
    fn test_session_emit_sends_event() {
        let (tx, mut rx) = mpsc::channel(100);
        let session = SessionManager::new(tx);

        let event = AgentEvent::session_started();
        session.emit(event).unwrap();

        let received = rx.try_recv();
        assert!(received.is_ok(), "should receive emitted event");
    }

    #[test]
    fn test_session_set_cancel_token() {
        let (tx, _rx) = mpsc::channel(100);
        let mut session = SessionManager::new(tx);

        let token = CancellationToken::new();
        session.set_cancel_token(token.clone());

        assert!(session.cancel_token().is_some());
        assert!(!session.is_cancelled());

        // Cancel the token
        token.cancel();
        assert!(session.is_cancelled());
    }

    #[test]
    fn test_session_set_ask_channel() {
        let (tx, _rx) = mpsc::channel(100);
        let (ask_tx, ask_rx) = mpsc::channel(10);
        let mut session = SessionManager::new(tx);

        session.set_ask_channel(ask_rx);

        assert!(session.ask_rx().is_some());

        // Send a message through ask channel
        ask_tx.try_send("test response".to_string()).unwrap();

        let response = session.ask_rx().unwrap().try_recv();
        assert!(response.is_ok(), "should receive ask response");
    }

    #[test]
    fn test_session_event_sender_can_clone() {
        let (tx, _rx) = mpsc::channel(100);
        let session = SessionManager::new(tx);

        let sender = session.event_sender();
        // Should be able to send through cloned sender
        sender.try_send(AgentEvent::session_started()).unwrap();
    }

    #[test]
    fn test_session_clear_reset_cancellation() {
        let (tx, _rx) = mpsc::channel(100);
        let mut session = SessionManager::new(tx);

        // Set cancellation token
        let token = CancellationToken::new();
        session.set_cancel_token(token);

        // Clear
        session.clear();

        // Token should be None
        assert!(session.cancel_token().is_none());
        assert!(!session.is_cancelled());
    }
}