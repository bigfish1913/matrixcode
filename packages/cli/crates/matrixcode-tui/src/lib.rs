//! MatrixCode Terminal UI - Full TUI Implementation
//!
//! This crate handles all terminal rendering:
//! - TUI application with ratatui
//! - Components (StatusBar, OutputArea, InputBox)
//! - Input handling and event bridge
//! - Session persistence
//!
//! It receives AgentEvent from Core and renders to terminal.

pub mod app;
pub mod components;
pub mod handler;
pub mod bridge;
pub mod session;
pub mod ui;
pub mod markdown;

// Re-export main types
pub use app::{App, AppState, AppMode, OutputMessage, OutputBlock, Role};
pub use handler::{InputHandler, InputAction, Command, SessionCmd};
pub use bridge::EventBridge;
pub use session::{SessionStore, SessionData, SessionMessage};
pub use components::{StatusBar, OutputArea, InputBox};

use matrixcode_core::{AgentEvent, EventData, EventType};

/// TUI version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Legacy Terminal UI handler (for backward compatibility)
pub struct TerminalUI {
    // Will add spinner and markdown renderer after migration
}

impl TerminalUI {
    /// Create new Terminal UI
    pub fn new() -> Self {
        Self {}
    }

    /// Handle event and render
    pub fn handle_event(&mut self, event: &AgentEvent) {
        match event.event_type {
            EventType::TextStart => {
                // Stop spinner, prepare for text
            }
            EventType::TextDelta => {
                if let Some(EventData::Text { delta }) = &event.data {
                    print!("{}", delta);
                }
            }
            EventType::TextEnd => {
                println!();
            }
            EventType::ThinkingStart => {
                eprintln!("[Thinking...]");
            }
            EventType::ThinkingDelta => {
                if let Some(EventData::Thinking { delta, .. }) = &event.data {
                    eprint!("{}", delta);
                }
            }
            EventType::ThinkingEnd => {
                eprintln!();
            }
            EventType::ToolUseStart => {
                if let Some(EventData::ToolUse { id, name, .. }) = &event.data {
                    eprintln!("[Tool: {} (id: {})]", name, id);
                }
            }
            EventType::ToolResult => {
                if let Some(EventData::ToolResult { content, is_error, .. }) = &event.data {
                    if *is_error {
                        eprintln!("[Error: {}]", truncate(content, 100));
                    } else {
                        eprintln!("[Result: {}]", truncate(content, 100));
                    }
                }
            }
            EventType::Error => {
                if let Some(EventData::Error { message, .. }) = &event.data {
                    eprintln!("\nError: {}", message);
                }
            }
            EventType::SessionStarted => {
                eprintln!("--- Session Started ---");
            }
            EventType::SessionEnded => {
                eprintln!("--- Session Ended ---");
            }
            EventType::Usage => {
                if let Some(EventData::Usage { input_tokens, output_tokens, .. }) = &event.data {
                    eprintln!("\nTokens: {} in, {} out", input_tokens, output_tokens);
                }
            }
            EventType::Progress => {
                if let Some(EventData::Progress { message, percentage }) = &event.data {
                    if let Some(p) = percentage {
                        eprintln!("[{}%] {}", p, message);
                    } else {
                        eprintln!("[...] {}", message);
                    }
                }
            }
            _ => {}
        }
    }

    /// Handle JSON string (from Core output)
    pub fn handle_json(&mut self, json: &str) -> Result<(), serde_json::Error> {
        let event = AgentEvent::from_json(json)?;
        self.handle_event(&event);
        Ok(())
    }

    /// Handle event list
    pub fn handle_events(&mut self, events: &[AgentEvent]) {
        for event in events {
            self.handle_event(event);
        }
    }
}

impl Default for TerminalUI {
    fn default() -> Self {
        Self::new()
    }
}

/// Truncate string
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_text_event() {
        let mut ui = TerminalUI::new();
        let event = AgentEvent::text_delta("Hello");
        ui.handle_event(&event);
    }

    #[test]
    fn test_handle_json() {
        let mut ui = TerminalUI::new();
        let event = AgentEvent::session_started();
        let json = event.to_json().unwrap();
        ui.handle_json(&json).unwrap();
    }
}