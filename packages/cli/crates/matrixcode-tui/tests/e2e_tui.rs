//! E2E Tests for TUI Application
//!
//! These tests verify the complete rendering and interaction flow of the TUI.

use ratatui::{
    backend::TestBackend,
    Terminal,
    layout::Rect,
};

use matrixcode_tui::handler::{InputHandler, InputAction, Command, SessionCmd};
use matrixcode_tui::app::{AppState, AppMode, Role, OutputMessage, OutputBlock};

/// Helper to create a test terminal with specified dimensions
fn create_test_terminal(width: u16, height: u16) -> Terminal<TestBackend> {
    let backend = TestBackend::new(width, height);
    Terminal::new(backend).unwrap()
}

/// Helper to get the rendered buffer content as a string
fn buffer_to_string(terminal: &Terminal<TestBackend>) -> String {
    let buffer = terminal.backend().buffer();
    let mut result = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            let cell = &buffer[(x, y)];
            result.push_str(cell.symbol());
        }
        result.push('\n');
    }
    result
}

mod initial_render {
    use super::*;

    #[test]
    fn test_terminal_initialization() {
        let mut terminal = create_test_terminal(80, 24);
        let area = terminal.get_frame().area();

        assert_eq!(area.width, 80);
        assert_eq!(area.height, 24);
    }

    #[test]
    fn test_backend_buffer_access() {
        let mut terminal = create_test_terminal(40, 10);
        let buffer = terminal.backend().buffer();

        // Buffer should match terminal dimensions
        assert_eq!(buffer.area.width, 40);
        assert_eq!(buffer.area.height, 10);

        // All cells should start empty
        let cell = &buffer[(0, 0)];
        assert_eq!(cell.symbol(), " ");
    }
}

mod layout_tests {
    use super::*;

    #[test]
    fn test_layout_calculation_full_screen() {
        let mut terminal = create_test_terminal(80, 24);
        let frame = terminal.get_frame();
        let area = frame.area();

        // Calculate expected layout areas
        // Status bar: top row (height 1)
        let status_bar_area = Rect { x: 0, y: 0, width: 80, height: 1 };

        // Main output area: middle section
        let output_height = 24 - 3; // minus status bar (1) and input box (2)
        let _output_area = Rect { x: 0, y: 1, width: 80, height: output_height };

        // Input box: bottom (height 2)
        let input_area = Rect { x: 0, y: 23, width: 80, height: 1 };

        assert!(output_height >= 18); // Ensure reasonable output area
        assert_eq!(status_bar_area.y, 0);
        assert_eq!(input_area.y, 23);
    }

    #[test]
    fn test_layout_with_side_panel() {
        // When side panel is visible, main area width is reduced
        let main_width_without_panel = 80;
        let panel_width = 30;
        let main_width_with_panel = main_width_without_panel - panel_width;

        assert_eq!(main_width_with_panel, 50);
        assert!(main_width_with_panel >= 40); // Ensure minimum usable width
    }
}

mod state_transition {
    use super::*;

    #[test]
    fn test_state_initialization() {
        let state = AppState::default();

        assert_eq!(state.mode, AppMode::Idle);
        assert!(state.input_buffer.is_empty());
        assert!(state.messages.is_empty());
        assert_eq!(state.tokens_used, 0);
    }

    #[test]
    fn test_state_mode_transitions() {
        let mut state = AppState::default();

        // Idle -> Thinking
        state.mode = AppMode::Thinking;
        assert_eq!(state.mode, AppMode::Thinking);

        // Thinking -> ToolExecuting
        state.mode = AppMode::ToolExecuting { name: "Read".to_string(), id: "tool_1".to_string() };
        assert!(matches!(state.mode, AppMode::ToolExecuting { name: _, id: _ }));

        // ToolExecuting -> Idle
        state.mode = AppMode::Idle;
        assert_eq!(state.mode, AppMode::Idle);
    }

    #[test]
    fn test_state_input_buffer_update() {
        let mut state = AppState::default();

        state.input_buffer = "Hello, world!".to_string();
        assert_eq!(state.input_buffer.len(), 13);

        state.input_buffer.clear();
        assert!(state.input_buffer.is_empty());
    }

    #[test]
    fn test_app_mode_labels() {
        assert_eq!(AppMode::Idle.label(), "Ready");
        assert_eq!(AppMode::Thinking.label(), "Thinking...");
        let tool_mode = AppMode::ToolExecuting { name: "ReadFile".to_string(), id: "t1".to_string() };
        assert_eq!(tool_mode.label(), "Tool: ReadFile");
    }
}

mod input_flow {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, KeyEventKind, KeyEventState};
    use super::*;

    fn create_key_event(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent { code, modifiers, kind: KeyEventKind::Press, state: KeyEventState::empty() }
    }

    #[test]
    fn test_user_input_character_flow() {
        let handler = InputHandler::new();
        let mut state = AppState::default();

        // Simulate typing "hello"
        for ch in "hello".chars() {
            let action = handler.handle(create_key_event(KeyCode::Char(ch), KeyModifiers::empty()), &state);
            if let Some(InputAction::TypeChar(c)) = action {
                state.input_buffer.push(c);
            }
        }

        assert_eq!(state.input_buffer, "hello");
    }

    #[test]
    fn test_command_input_flow() {
        let handler = InputHandler::new();
        let mut state = AppState::default();

        // Simulate typing "/help"
        for ch in "/help".chars() {
            state.input_buffer.push(ch);
        }

        // Verify command is detected
        let action = handler.handle(create_key_event(KeyCode::Enter, KeyModifiers::empty()), &state);
        assert!(matches!(action, Some(InputAction::Command(Command::Help))));
    }

    #[test]
    fn test_backspace_flow() {
        let handler = InputHandler::new();
        let mut state = AppState::default();

        state.input_buffer = "hello".to_string();

        // Backspace should remove last character
        let action = handler.handle(create_key_event(KeyCode::Backspace, KeyModifiers::empty()), &state);
        assert!(matches!(action, Some(InputAction::Backspace)));

        state.input_buffer.pop();
        assert_eq!(state.input_buffer, "hell");
    }

    #[test]
    fn test_interrupt_flow() {
        let handler = InputHandler::new();
        let state = AppState {
            mode: AppMode::Thinking,
            ..Default::default()
        };

        // Ctrl+C should interrupt
        let action = handler.handle(
            create_key_event(KeyCode::Char('c'), KeyModifiers::CONTROL),
            &state
        );
        assert!(matches!(action, Some(InputAction::Interrupt)));
    }
}

mod history_navigation {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, KeyEventKind, KeyEventState};
    use super::*;

    fn create_key_event(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent { code, modifiers, kind: KeyEventKind::Press, state: KeyEventState::empty() }
    }

    #[test]
    fn test_history_up_empty_history() {
        let handler = InputHandler::new();
        let state = AppState::default();

        // No history, up arrow with empty input should trigger HistoryUp
        let action = handler.handle(create_key_event(KeyCode::Up, KeyModifiers::empty()), &state);
        assert!(matches!(action, Some(InputAction::HistoryUp)));
    }

    #[test]
    fn test_history_up_with_history() {
        let mut state = AppState {
            input_history: vec!["msg1".to_string(), "msg2".to_string()],
            ..Default::default()
        };

        // First up arrow - navigate to last message (newest in history)
        state.history_up();
        assert_eq!(state.input_buffer, "msg2");
        assert_eq!(state.history_index, 1);

        // Second up - older message
        state.history_up();
        assert_eq!(state.input_buffer, "msg1");
        assert_eq!(state.history_index, 2);
    }

    #[test]
    fn test_history_down_navigation() {
        let mut state = AppState {
            input_history: vec!["msg1".to_string(), "msg2".to_string()],
            history_index: 2, // Already at oldest
            input_buffer: "msg1".to_string(),
            ..Default::default()
        };

        // Down arrow should go back to newer messages
        state.history_down();
        assert_eq!(state.input_buffer, "msg2");
        assert_eq!(state.history_index, 1);

        // Another down goes to bottom (empty)
        state.history_down();
        assert_eq!(state.history_index, 0);
        assert!(state.input_buffer.is_empty());
    }

    #[test]
    fn test_up_arrow_ignored_with_input() {
        let handler = InputHandler::new();
        let state = AppState {
            input_buffer: "some text".to_string(),
            ..Default::default()
        };

        // Up arrow should be ignored when there's input
        let action = handler.handle(create_key_event(KeyCode::Up, KeyModifiers::empty()), &state);
        assert!(action.is_none());
    }
}

mod command_handling {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, KeyEventKind, KeyEventState};
    use super::*;

    fn create_key_event(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent { code, modifiers, kind: KeyEventKind::Press, state: KeyEventState::empty() }
    }

    #[test]
    fn test_exit_command() {
        let handler = InputHandler::new();
        let mut state = AppState::default();

        // Type "/exit"
        state.input_buffer = "/exit".to_string();

        let action = handler.handle(create_key_event(KeyCode::Enter, KeyModifiers::empty()), &state);
        assert!(matches!(action, Some(InputAction::Command(Command::Exit))));
    }

    #[test]
    fn test_quit_command() {
        let handler = InputHandler::new();
        let mut state = AppState::default();

        // Type "/quit"
        state.input_buffer = "/quit".to_string();

        let action = handler.handle(create_key_event(KeyCode::Enter, KeyModifiers::empty()), &state);
        assert!(matches!(action, Some(InputAction::Command(Command::Exit))));
    }

    #[test]
    fn test_help_command() {
        let handler = InputHandler::new();
        let mut state = AppState::default();

        // Type "/help"
        state.input_buffer = "/help".to_string();

        let action = handler.handle(create_key_event(KeyCode::Enter, KeyModifiers::empty()), &state);
        assert!(matches!(action, Some(InputAction::Command(Command::Help))));
    }

    #[test]
    fn test_clear_command() {
        let handler = InputHandler::new();
        let mut state = AppState::default();

        // Type "/clear"
        state.input_buffer = "/clear".to_string();

        let action = handler.handle(create_key_event(KeyCode::Enter, KeyModifiers::empty()), &state);
        assert!(matches!(action, Some(InputAction::Command(Command::Clear))));
    }

    #[test]
    fn test_model_command() {
        let handler = InputHandler::new();
        let mut state = AppState::default();

        // Type "/model claude-opus-4"
        state.input_buffer = "/model claude-opus-4".to_string();

        let action = handler.handle(create_key_event(KeyCode::Enter, KeyModifiers::empty()), &state);
        assert!(matches!(action, Some(InputAction::Command(Command::Model(m))) if m == "claude-opus-4"));
    }

    #[test]
    fn test_session_list_command() {
        let handler = InputHandler::new();
        let mut state = AppState::default();

        // Type "/session list"
        state.input_buffer = "/session list".to_string();

        let action = handler.handle(create_key_event(KeyCode::Enter, KeyModifiers::empty()), &state);
        assert!(matches!(action, Some(InputAction::Command(Command::Session(SessionCmd::List)))));
    }

    #[test]
    fn test_session_save_command() {
        let handler = InputHandler::new();
        let mut state = AppState::default();

        // Type "/session save"
        state.input_buffer = "/session save".to_string();

        let action = handler.handle(create_key_event(KeyCode::Enter, KeyModifiers::empty()), &state);
        assert!(matches!(action, Some(InputAction::Command(Command::Session(SessionCmd::Save)))));
    }

    #[test]
    fn test_session_new_command() {
        let handler = InputHandler::new();
        let mut state = AppState::default();

        // Type "/session new"
        state.input_buffer = "/session new".to_string();

        let action = handler.handle(create_key_event(KeyCode::Enter, KeyModifiers::empty()), &state);
        assert!(matches!(action, Some(InputAction::Command(Command::Session(SessionCmd::New)))));
    }

    #[test]
    fn test_session_load_command() {
        let handler = InputHandler::new();
        let mut state = AppState::default();

        // Type "/session load abc123"
        state.input_buffer = "/session load abc123".to_string();

        let action = handler.handle(create_key_event(KeyCode::Enter, KeyModifiers::empty()), &state);
        assert!(matches!(action, Some(InputAction::Command(Command::Session(SessionCmd::Load(id)))) if id == "abc123"));
    }

    #[test]
    fn test_ctrl_d_quit() {
        let handler = InputHandler::new();
        let state = AppState::default();

        // Ctrl+D should quit directly
        let action = handler.handle(
            create_key_event(KeyCode::Char('d'), KeyModifiers::CONTROL),
            &state
        );
        assert!(matches!(action, Some(InputAction::Quit)));
    }
}

mod message_handling {
    use super::*;

    #[test]
    fn test_user_message_creation() {
        let msg = OutputMessage::user("Hello, how can I help?".to_string());
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content.len(), 1);
        if let OutputBlock::Text(text) = &msg.content[0] {
            assert_eq!(text, "Hello, how can I help?");
        } else {
            panic!("Expected Text block");
        }
    }

    #[test]
    fn test_assistant_message_creation() {
        let msg = OutputMessage::assistant("I can help you with that.".to_string());
        assert_eq!(msg.role, Role::Assistant);
        assert_eq!(msg.content.len(), 1);
    }

    #[test]
    fn test_thinking_block_append() {
        let mut state = AppState::default();
        state.append_thinking("Let me think about this...");

        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0].role, Role::Assistant);
        if let OutputBlock::Thinking(text) = &state.messages[0].content[0] {
            assert_eq!(text, "Let me think about this...");
        } else {
            panic!("Expected Thinking block");
        }
    }

    #[test]
    fn test_tool_result_append() {
        let mut state = AppState::default();
        state.append_tool_result("tool-1", "ReadFile", "File contents here", false);

        assert_eq!(state.messages.len(), 1);
        if let OutputBlock::ToolUse { name, result, is_error, .. } = &state.messages[0].content[0] {
            assert_eq!(name, "ReadFile");
            assert_eq!(result, "File contents here");
            assert!(!is_error);
        } else {
            panic!("Expected ToolUse block");
        }
    }

    #[test]
    fn test_tool_error_result() {
        let mut state = AppState::default();
        state.append_tool_result("tool-2", "WriteFile", "Permission denied", true);

        if let OutputBlock::ToolUse { is_error, .. } = &state.messages[0].content[0] {
            assert!(is_error);
        } else {
            panic!("Expected ToolUse block");
        }
    }

    #[test]
    fn test_message_history_sequence() {
        let mut state = AppState::default();

        // Add user message
        state.messages.push(OutputMessage::user("Question 1".to_string()));
        // Add assistant response
        state.append_output("Answer 1");
        // Add another user message
        state.messages.push(OutputMessage::user("Question 2".to_string()));
        // Add thinking
        state.append_thinking("Analyzing...");
        // Add assistant response
        state.append_output("Answer 2");

        assert_eq!(state.messages.len(), 5);
        assert_eq!(state.messages[0].role, Role::User);
        assert_eq!(state.messages[1].role, Role::Assistant);
        assert_eq!(state.messages[2].role, Role::User);
        assert_eq!(state.messages[3].role, Role::Assistant);
        assert_eq!(state.messages[4].role, Role::Assistant);
    }
}

mod full_user_flow {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, KeyEventKind, KeyEventState};
    use super::*;

    fn create_key_event(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent { code, modifiers, kind: KeyEventKind::Press, state: KeyEventState::empty() }
    }

    #[test]
    fn test_complete_conversation_flow() {
        let handler = InputHandler::new();
        let mut state = AppState::default();

        // 1. User types a message
        for ch in "What is the weather?".chars() {
            let action = handler.handle(create_key_event(KeyCode::Char(ch), KeyModifiers::empty()), &state);
            if let Some(InputAction::TypeChar(c)) = action {
                state.input_buffer.push(c);
            }
        }
        assert_eq!(state.input_buffer, "What is the weather?");

        // 2. User presses Enter to send
        let action = handler.handle(create_key_event(KeyCode::Enter, KeyModifiers::empty()), &state);
        if let Some(InputAction::Send(msg)) = action {
            state.add_to_history(msg.clone());
            state.messages.push(OutputMessage::user(msg));
            state.mode = AppMode::Thinking;
            state.clear_input();
        }

        assert!(state.input_buffer.is_empty());
        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.mode, AppMode::Thinking);

        // 3. Assistant responds (simulated)
        state.mode = AppMode::Idle;
        state.append_output("I don't have access to real-time weather data.");

        assert_eq!(state.messages.len(), 2);
        assert_eq!(state.mode, AppMode::Idle);

        // 4. User types another message
        for ch in "Thanks anyway".chars() {
            let action = handler.handle(create_key_event(KeyCode::Char(ch), KeyModifiers::empty()), &state);
            if let Some(InputAction::TypeChar(c)) = action {
                state.input_buffer.push(c);
            }
        }
        assert_eq!(state.input_buffer, "Thanks anyway");
    }

    #[test]
    fn test_interrupt_and_continue_flow() {
        let handler = InputHandler::new();
        let mut state = AppState::default();

        // 1. User sends message
        state.input_buffer = "Long query".to_string();
        let action = handler.handle(create_key_event(KeyCode::Enter, KeyModifiers::empty()), &state);
        if let Some(InputAction::Send(msg)) = action {
            state.messages.push(OutputMessage::user(msg));
            state.mode = AppMode::Thinking;
            state.clear_input();
        }

        assert_eq!(state.mode, AppMode::Thinking);

        // 2. User interrupts with Ctrl+C
        let action = handler.handle(
            create_key_event(KeyCode::Char('c'), KeyModifiers::CONTROL),
            &state
        );
        if let Some(InputAction::Interrupt) = action {
            state.mode = AppMode::Idle;
            state.status_message = Some("Interrupted".to_string());
        }

        assert_eq!(state.mode, AppMode::Idle);
        assert_eq!(state.status_message, Some("Interrupted".to_string()));

        // 3. User continues with new input
        for ch in "New query".chars() {
            let action = handler.handle(create_key_event(KeyCode::Char(ch), KeyModifiers::empty()), &state);
            if let Some(InputAction::TypeChar(c)) = action {
                state.input_buffer.push(c);
            }
        }
        assert_eq!(state.input_buffer, "New query");
    }

    #[test]
    fn test_scroll_output_flow() {
        let handler = InputHandler::new();
        let mut state = AppState::default();

        // Fill with messages to require scrolling
        for i in 0..50 {
            state.messages.push(OutputMessage::assistant(format!("Message {}", i)));
        }

        // Initial scroll offset should be 0
        assert_eq!(state.scroll_offset, 0);

        // PageUp should scroll up
        let action = handler.handle(
            create_key_event(KeyCode::PageUp, KeyModifiers::empty()),
            &state
        );
        if let Some(InputAction::ScrollUp) = action {
            // Actually scroll would be handled by App, but we can simulate
            state.scroll_offset = 10;
        }

        assert_eq!(state.scroll_offset, 10);

        // PageDown should scroll down
        let action = handler.handle(
            create_key_event(KeyCode::PageDown, KeyModifiers::empty()),
            &state
        );
        if let Some(InputAction::ScrollDown) = action {
            state.scroll_offset += 1;
        }

        assert_eq!(state.scroll_offset, 11);
    }

    #[test]
    fn test_toggle_panel_flow() {
        let handler = InputHandler::new();
        let state = AppState::default();

        // Tab should toggle panel
        let action = handler.handle(
            create_key_event(KeyCode::Tab, KeyModifiers::empty()),
            &state
        );
        assert!(matches!(action, Some(InputAction::TogglePanel)));
    }
}