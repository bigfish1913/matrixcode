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
    use ratatui::crossterm::event::{KeyEventKind, KeyEventState};

    // ===== Command Parsing Tests =====

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
    fn test_parse_clear_command() {
        assert!(matches!(parse_command("/clear"), Some(Command::Clear)));
        assert!(matches!(parse_command("/cls"), Some(Command::Clear)));
    }

    #[test]
    fn test_parse_model_command() {
        let result = parse_command("/model gpt-4");
        assert!(matches!(result, Some(Command::Model(m)) if m == "gpt-4"));

        let result = parse_command("/model");
        assert!(matches!(result, Some(Command::Model(m)) if m == "claude-sonnet-4.6"));
    }

    #[test]
    fn test_parse_session_list_command() {
        assert!(matches!(parse_command("/session list"), Some(Command::Session(SessionCmd::List))));
        assert!(matches!(parse_command("/session ls"), Some(Command::Session(SessionCmd::List))));
        assert!(matches!(parse_command("/session"), Some(Command::Session(SessionCmd::List))));
    }

    #[test]
    fn test_parse_session_save_command() {
        assert!(matches!(parse_command("/session save"), Some(Command::Session(SessionCmd::Save))));
    }

    #[test]
    fn test_parse_session_new_command() {
        assert!(matches!(parse_command("/session new"), Some(Command::Session(SessionCmd::New))));
    }

    #[test]
    fn test_parse_session_load_command() {
        let result = parse_command("/session load abc123");
        assert!(matches!(result, Some(Command::Session(SessionCmd::Load(id))) if id == "abc123"));

        let result = parse_command("/session load");
        assert!(matches!(result, Some(Command::Session(SessionCmd::Load(id))) if id.is_empty()));
    }

    #[test]
    fn test_parse_session_delete_command() {
        let result = parse_command("/session delete xyz789");
        assert!(matches!(result, Some(Command::Session(SessionCmd::Delete(id))) if id == "xyz789"));

        let result = parse_command("/session del test-id");
        assert!(matches!(result, Some(Command::Session(SessionCmd::Delete(id))) if id == "test-id"));

        let result = parse_command("/session rm another-id");
        assert!(matches!(result, Some(Command::Session(SessionCmd::Delete(id))) if id == "another-id"));
    }

    #[test]
    fn test_parse_unknown_command() {
        assert!(parse_command("/unknown").is_none());
        assert!(parse_command("/invalid-cmd").is_none());
        assert!(parse_command("/foo").is_none());
    }

    #[test]
    fn test_parse_command_with_extra_whitespace() {
        // Split whitespace should handle this
        let result = parse_command("/help  ");
        // Extra whitespace after command is trimmed by split_whitespace
        assert!(matches!(result, Some(Command::Help)));
    }

    // ===== InputHandler Tests =====

    fn create_key_event(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent { code, modifiers, kind: KeyEventKind::Press, state: KeyEventState::empty() }
    }

    #[test]
    fn test_input_handler_new() {
        let handler = InputHandler::new();
        // Just verify it can be created
        assert!(true);
    }

    #[test]
    fn test_input_handler_default() {
        let handler = InputHandler::default();
        // Just verify it can be created
        assert!(true);
    }

    #[test]
    fn test_handle_enter_send_message() {
        let handler = InputHandler::new();
        let mut state = AppState::new();
        state.input_buffer = "Hello world".to_string();

        let event = create_key_event(KeyCode::Enter, KeyModifiers::empty());
        let result = handler.handle(event, &state);

        assert!(matches!(result, Some(InputAction::Send(msg)) if msg == "Hello world"));
    }

    #[test]
    fn test_handle_enter_empty_input() {
        let handler = InputHandler::new();
        let state = AppState::new();

        let event = create_key_event(KeyCode::Enter, KeyModifiers::empty());
        let result = handler.handle(event, &state);

        assert!(result.is_none());
    }

    #[test]
    fn test_handle_enter_whitespace_only() {
        let handler = InputHandler::new();
        let mut state = AppState::new();
        state.input_buffer = "   ".to_string();

        let event = create_key_event(KeyCode::Enter, KeyModifiers::empty());
        let result = handler.handle(event, &state);

        assert!(result.is_none());
    }

    #[test]
    fn test_handle_enter_command() {
        let handler = InputHandler::new();
        let mut state = AppState::new();
        state.input_buffer = "/help".to_string();

        let event = create_key_event(KeyCode::Enter, KeyModifiers::empty());
        let result = handler.handle(event, &state);

        assert!(matches!(result, Some(InputAction::Command(Command::Help))));
    }

    #[test]
    fn test_handle_up_arrow_empty_input() {
        let handler = InputHandler::new();
        let state = AppState::new();

        let event = create_key_event(KeyCode::Up, KeyModifiers::empty());
        let result = handler.handle(event, &state);

        assert!(matches!(result, Some(InputAction::HistoryUp)));
    }

    #[test]
    fn test_handle_up_arrow_with_input_ignored() {
        let handler = InputHandler::new();
        let mut state = AppState::new();
        state.input_buffer = "test".to_string();

        let event = create_key_event(KeyCode::Up, KeyModifiers::empty());
        let result = handler.handle(event, &state);

        assert!(result.is_none());
    }

    #[test]
    fn test_handle_down_arrow_empty_input() {
        let handler = InputHandler::new();
        let state = AppState::new();

        let event = create_key_event(KeyCode::Down, KeyModifiers::empty());
        let result = handler.handle(event, &state);

        assert!(matches!(result, Some(InputAction::HistoryDown)));
    }

    #[test]
    fn test_handle_down_arrow_with_input_ignored() {
        let handler = InputHandler::new();
        let mut state = AppState::new();
        state.input_buffer = "test".to_string();

        let event = create_key_event(KeyCode::Down, KeyModifiers::empty());
        let result = handler.handle(event, &state);

        assert!(result.is_none());
    }

    #[test]
    fn test_handle_tab() {
        let handler = InputHandler::new();
        let state = AppState::new();

        let event = create_key_event(KeyCode::Tab, KeyModifiers::empty());
        let result = handler.handle(event, &state);

        assert!(matches!(result, Some(InputAction::TogglePanel)));
    }

    #[test]
    fn test_handle_escape() {
        let handler = InputHandler::new();
        let state = AppState::new();

        let event = create_key_event(KeyCode::Esc, KeyModifiers::empty());
        let result = handler.handle(event, &state);

        assert!(matches!(result, Some(InputAction::ClearInput)));
    }

    #[test]
    fn test_handle_backspace() {
        let handler = InputHandler::new();
        let state = AppState::new();

        let event = create_key_event(KeyCode::Backspace, KeyModifiers::empty());
        let result = handler.handle(event, &state);

        assert!(matches!(result, Some(InputAction::Backspace)));
    }

    #[test]
    fn test_handle_ctrl_c() {
        let handler = InputHandler::new();
        let state = AppState::new();

        let event = create_key_event(KeyCode::Char('c'), KeyModifiers::CONTROL);
        let result = handler.handle(event, &state);

        assert!(matches!(result, Some(InputAction::Interrupt)));
    }

    #[test]
    fn test_handle_ctrl_d() {
        let handler = InputHandler::new();
        let state = AppState::new();

        let event = create_key_event(KeyCode::Char('d'), KeyModifiers::CONTROL);
        let result = handler.handle(event, &state);

        assert!(matches!(result, Some(InputAction::Quit)));
    }

    #[test]
    fn test_handle_char() {
        let handler = InputHandler::new();
        let state = AppState::new();

        let event = create_key_event(KeyCode::Char('a'), KeyModifiers::empty());
        let result = handler.handle(event, &state);

        assert!(matches!(result, Some(InputAction::TypeChar('a'))));

        let event = create_key_event(KeyCode::Char('Z'), KeyModifiers::empty());
        let result = handler.handle(event, &state);

        assert!(matches!(result, Some(InputAction::TypeChar('Z'))));
    }

    #[test]
    fn test_handle_page_up() {
        let handler = InputHandler::new();
        let state = AppState::new();

        let event = create_key_event(KeyCode::PageUp, KeyModifiers::empty());
        let result = handler.handle(event, &state);

        assert!(matches!(result, Some(InputAction::ScrollUp)));
    }

    #[test]
    fn test_handle_page_down() {
        let handler = InputHandler::new();
        let state = AppState::new();

        let event = create_key_event(KeyCode::PageDown, KeyModifiers::empty());
        let result = handler.handle(event, &state);

        assert!(matches!(result, Some(InputAction::ScrollDown)));
    }

    #[test]
    fn test_handle_unsupported_key() {
        let handler = InputHandler::new();
        let state = AppState::new();

        let event = create_key_event(KeyCode::F(1), KeyModifiers::empty());
        let result = handler.handle(event, &state);

        assert!(result.is_none());
    }

    // ===== InputAction Debug/Clone Tests =====

    #[test]
    fn test_input_action_clone() {
        let action = InputAction::Send("test".to_string());
        let cloned = action.clone();
        assert!(matches!(cloned, InputAction::Send(msg) if msg == "test"));
    }

    #[test]
    fn test_command_clone() {
        let cmd = Command::Model("gpt-4".to_string());
        let cloned = cmd.clone();
        assert!(matches!(cloned, Command::Model(m) if m == "gpt-4"));
    }

    #[test]
    fn test_session_cmd_clone() {
        let cmd = SessionCmd::Load("session-123".to_string());
        let cloned = cmd.clone();
        assert!(matches!(cloned, SessionCmd::Load(id) if id == "session-123"));
    }
}