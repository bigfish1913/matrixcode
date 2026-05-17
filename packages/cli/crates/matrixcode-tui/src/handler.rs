//! Input Handler
//!
//! Parses keyboard events and converts to InputAction.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::AppState;

/// Input action from user
#[derive(Debug, Clone)]
pub enum InputAction {
    /// Send message to agent
    Send(String),
    /// Execute command
    Command(Command),
    /// Navigate history up
    HistoryUp,
    /// Navigate history down
    HistoryDown,
    /// Interrupt current operation
    Interrupt,
    /// Toggle side panel
    TogglePanel,
    /// Scroll output up
    ScrollUp,
    /// Scroll output down
    ScrollDown,
    /// Type character
    TypeChar(char),
    /// Delete last character
    Backspace,
    /// Clear input buffer
    ClearInput,
    /// Quit application
    Quit,
}

/// Command types
#[derive(Debug, Clone)]
pub enum Command {
    /// Show help
    Help,
    /// Exit application
    Exit,
    /// Clear screen
    Clear,
    /// Change model
    Model(String),
    /// Session management
    Session(SessionCmd),
}

/// Session commands
#[derive(Debug, Clone)]
pub enum SessionCmd {
    /// List sessions
    List,
    /// Save current session
    Save,
    /// Load session by ID
    Load(String),
    /// Delete session by ID
    Delete(String),
    /// Create new session
    New,
}

/// Input handler
pub struct InputHandler;

impl InputHandler {
    /// Create new input handler
    pub fn new() -> Self {
        Self
    }

    /// Handle keyboard event
    pub fn handle(&self, event: KeyEvent, state: &AppState) -> Option<InputAction> {
        match event.code {
            KeyCode::Enter => {
                let input = state.input_buffer.trim();
                if input.is_empty() {
                    None
                } else if input.starts_with('/') {
                    parse_command(input).map(InputAction::Command)
                } else {
                    Some(InputAction::Send(input.to_string()))
                }
            }
            KeyCode::Up => {
                // If input is empty, navigate history; otherwise ignore
                if state.input_buffer.is_empty() {
                    Some(InputAction::HistoryUp)
                } else {
                    None
                }
            }
            KeyCode::Down => {
                if state.input_buffer.is_empty() {
                    Some(InputAction::HistoryDown)
                } else {
                    None
                }
            }
            KeyCode::Tab => Some(InputAction::TogglePanel),
            KeyCode::Esc => Some(InputAction::ClearInput),
            KeyCode::Backspace => Some(InputAction::Backspace),
            KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(InputAction::Interrupt)
            }
            KeyCode::Char('d') if event.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(InputAction::Quit)
            }
            KeyCode::Char(c) => Some(InputAction::TypeChar(c)),
            KeyCode::PageUp => Some(InputAction::ScrollUp),
            KeyCode::PageDown => Some(InputAction::ScrollDown),
            _ => None,
        }
    }
}

impl Default for InputHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse command from input string
fn parse_command(input: &str) -> Option<Command> {
    let parts: Vec<&str> = input.split_whitespace().collect();
    let cmd = parts.first()?;

    match *cmd {
        "/help" | "/h" | "/?" => Some(Command::Help),
        "/exit" | "/quit" | "/q" => Some(Command::Exit),
        "/clear" | "/cls" => Some(Command::Clear),
        "/model" => {
            let model = parts.get(1).unwrap_or(&"claude-sonnet-4.6");
            Some(Command::Model(model.to_string()))
        }
        "/session" => {
            let sub = parts.get(1).copied().unwrap_or("list");
            match sub {
                "list" | "ls" => Some(Command::Session(SessionCmd::List)),
                "save" => Some(Command::Session(SessionCmd::Save)),
                "new" => Some(Command::Session(SessionCmd::New)),
                "load" => {
                    let id = parts.get(2).unwrap_or(&"");
                    Some(Command::Session(SessionCmd::Load(id.to_string())))
                }
                "delete" | "del" | "rm" => {
                    let id = parts.get(2).unwrap_or(&"");
                    Some(Command::Session(SessionCmd::Delete(id.to_string())))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_help_command() {
        assert!(matches!(parse_command("/help"), Some(Command::Help)));
        assert!(matches!(parse_command("/h"), Some(Command::Help)));
        assert!(matches!(parse_command("/?"), Some(Command::Help)));
    }

    #[test]
    fn test_parse_exit_command() {
        assert!(matches!(parse_command("/exit"), Some(Command::Exit)));
        assert!(matches!(parse_command("/quit"), Some(Command::Exit)));
        assert!(matches!(parse_command("/q"), Some(Command::Exit)));
    }

    #[test]
    fn test_parse_model_command() {
        let result = parse_command("/model gpt-4");
        assert!(matches!(result, Some(Command::Model(m)) if m == "gpt-4"));
    }

    #[test]
    fn test_parse_unknown_command() {
        assert!(parse_command("/unknown").is_none());
    }
}