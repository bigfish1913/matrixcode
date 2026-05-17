//! TUI Application - Main App structure and event loop
//!
//! This module contains the main ratatui application that manages
//! state, components, and event handling.

use anyhow::Result;
use ratatui::{
    backend::CrosstermBackend,
    crossterm::{
        event::{self, Event},
        terminal::{disable_raw_mode, enable_raw_mode},
    },
    layout::{Constraint, Direction, Layout},
    Terminal,
};
use std::io::Stdout;
use std::time::Duration;

use matrixcode_core::{AgentEvent, cancel::CancellationToken};

use crate::bridge::EventBridge;
use crate::components::{InputBox, OutputArea, SidePanel, StatusBar};
use crate::handler::{InputAction, InputHandler};

/// Application version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Application mode
#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    /// Idle, waiting for user input
    Idle,
    /// Agent is thinking/processing
    Thinking,
    /// Tool is executing
    ToolExecuting { name: String, id: String },
}

impl AppMode {
    /// Get display label for current mode
    pub fn label(&self) -> String {
        match self {
            AppMode::Idle => "Ready".to_string(),
            AppMode::Thinking => "Thinking...".to_string(),
            AppMode::ToolExecuting { name, .. } => format!("Tool: {}", name),
        }
    }
}

/// Message role
#[derive(Debug, Clone, PartialEq)]
pub enum Role {
    User,
    Assistant,
    System,
}

/// Output block for rendering
#[derive(Debug, Clone)]
pub enum OutputBlock {
    /// Plain text
    Text(String),
    /// Thinking block
    Thinking(String),
    /// Tool use result
    ToolUse {
        name: String,
        id: String,
        result: String,
        is_error: bool,
    },
}

/// Output message
#[derive(Debug, Clone)]
pub struct OutputMessage {
    /// Message role
    pub role: Role,
    /// Message content blocks
    pub content: Vec<OutputBlock>,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl OutputMessage {
    /// Create new message
    pub fn new(role: Role, content: Vec<OutputBlock>) -> Self {
        Self {
            role,
            content,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Create user message
    pub fn user(text: String) -> Self {
        Self::new(Role::User, vec![OutputBlock::Text(text)])
    }

    /// Create assistant text message
    pub fn assistant(text: String) -> Self {
        Self::new(Role::Assistant, vec![OutputBlock::Text(text)])
    }
}

/// Application state
#[derive(Debug)]
pub struct AppState {
    /// Current mode
    pub mode: AppMode,
    /// Current model name
    pub model: String,
    /// Total tokens used
    pub tokens_used: u64,
    /// Output messages
    pub messages: Vec<OutputMessage>,
    /// Current input buffer
    pub input_buffer: String,
    /// Input history
    pub input_history: Vec<String>,
    /// Current history navigation index
    pub history_index: usize,
    /// Scroll offset for output area
    pub scroll_offset: usize,
    /// Whether sidebar is visible
    pub show_panel: bool,
    /// Current session ID
    pub session_id: Option<String>,
    /// Exit flag
    pub should_exit: bool,
    /// Status message
    pub status_message: Option<String>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            mode: AppMode::Idle,
            model: "claude-sonnet-4.6".to_string(),
            tokens_used: 0,
            messages: Vec::new(),
            input_buffer: String::new(),
            input_history: Vec::new(),
            history_index: 0,
            scroll_offset: 0,
            show_panel: false,
            session_id: None,
            should_exit: false,
            status_message: None,
        }
    }
}

impl AppState {
    /// Create new app state
    pub fn new() -> Self {
        Self::default()
    }

    /// Append text to output
    pub fn append_output(&mut self, text: &str) {
        if let Some(last) = self.messages.last_mut() {
            if let Some(OutputBlock::Text(existing)) = last.content.last_mut() {
                existing.push_str(text);
                return;
            }
        }
        // Create new message if no existing one
        self.messages.push(OutputMessage::assistant(text.to_string()));
    }

    /// Add thinking block
    pub fn append_thinking(&mut self, text: &str) {
        self.messages.push(OutputMessage::new(
            Role::Assistant,
            vec![OutputBlock::Thinking(text.to_string())],
        ));
    }

    /// Add tool result
    pub fn append_tool_result(&mut self, id: &str, name: &str, result: &str, is_error: bool) {
        self.messages.push(OutputMessage::new(
            Role::Assistant,
            vec![OutputBlock::ToolUse {
                id: id.to_string(),
                name: name.to_string(),
                result: result.to_string(),
                is_error,
            }],
        ));
    }

    /// Show error message
    pub fn show_error(&mut self, message: &str) {
        self.status_message = Some(format!("Error: {}", message));
    }

    /// Clear input buffer
    pub fn clear_input(&mut self) {
        self.input_buffer.clear();
        self.history_index = 0;
    }

    /// Navigate history up
    pub fn history_up(&mut self) {
        if !self.input_history.is_empty() && self.history_index < self.input_history.len() - 1 {
            self.history_index += 1;
            if let Some(text) = self.input_history.iter().rev().nth(self.history_index) {
                self.input_buffer = text.clone();
            }
        }
    }

    /// Navigate history down
    pub fn history_down(&mut self) {
        if self.history_index > 0 {
            self.history_index -= 1;
            if let Some(text) = self.input_history.iter().rev().nth(self.history_index) {
                self.input_buffer = text.clone();
            }
        } else if self.history_index == 0 {
            self.history_index = 0;
            self.input_buffer.clear();
        }
    }

    /// Add to input history
    pub fn add_to_history(&mut self, input: String) {
        if !input.trim().is_empty() {
            self.input_history.push(input);
            // Keep only last 100 entries
            if self.input_history.len() > 100 {
                self.input_history.remove(0);
            }
        }
        self.history_index = 0;
    }
}

/// UI Components container
pub struct Components {
    status_bar: StatusBar,
    output_area: OutputArea,
    input_box: InputBox,
    side_panel: SidePanel,
}

impl Default for Components {
    fn default() -> Self {
        Self::new()
    }
}

impl Components {
    pub fn new() -> Self {
        Self {
            status_bar: StatusBar::new(),
            output_area: OutputArea::new(),
            input_box: InputBox::new(),
            side_panel: SidePanel::new(),
        }
    }
}

/// Main TUI Application
pub struct App {
    /// Application state
    state: AppState,
    /// UI components
    components: Components,
    /// Input handler
    handler: InputHandler,
    /// Event bridge for Agent events
    bridge: EventBridge,
    /// Channel to send user input to Agent task
    agent_tx: tokio::sync::mpsc::Sender<String>,
    /// Channel to receive Agent events
    event_rx: tokio::sync::mpsc::Receiver<AgentEvent>,
    /// Cancellation token
    cancel_token: CancellationToken,
}

impl App {
    /// Create new App
    pub fn new(
        agent_tx: tokio::sync::mpsc::Sender<String>,
        event_rx: tokio::sync::mpsc::Receiver<AgentEvent>,
        cancel_token: CancellationToken,
    ) -> Self {
        Self {
            state: AppState::new(),
            components: Components::new(),
            handler: InputHandler::new(),
            bridge: EventBridge::new(),
            agent_tx,
            event_rx,
            cancel_token,
        }
    }

    /// Run the application
    pub fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        loop {
            // 1. Draw UI
            terminal.draw(|f| self.render(f))?;

            // 2. Handle input events (with timeout for agent events)
            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    if let Some(action) = self.handler.handle(key, &self.state) {
                        self.apply_action(action)?;
                    }
                }
            }

            // 3. Process Agent events (non-blocking)
            while let Ok(event) = self.event_rx.try_recv() {
                self.bridge.apply(event, &mut self.state);
            }

            // 4. Check exit condition
            if self.state.should_exit {
                break;
            }
        }
        Ok(())
    }

    /// Apply input action
    fn apply_action(&mut self, action: InputAction) -> Result<()> {
        match action {
            InputAction::Send(msg) => {
                // Add to history
                self.state.add_to_history(msg.clone());
                // Add user message to output
                self.state.messages.push(OutputMessage::user(msg.clone()));
                // Send to agent
                let _ = self.agent_tx.blocking_send(msg);
                // Update mode
                self.state.mode = AppMode::Thinking;
                self.state.clear_input();
            }
            InputAction::Command(cmd) => {
                self.handle_command(cmd)?;
            }
            InputAction::HistoryUp => {
                self.state.history_up();
            }
            InputAction::HistoryDown => {
                self.state.history_down();
            }
            InputAction::Interrupt => {
                self.cancel_token.cancel();
                self.state.mode = AppMode::Idle;
                self.state.status_message = Some("Interrupted".to_string());
            }
            InputAction::TogglePanel => {
                self.state.show_panel = !self.state.show_panel;
            }
            InputAction::ScrollUp => {
                if self.state.scroll_offset > 0 {
                    self.state.scroll_offset -= 1;
                }
            }
            InputAction::ScrollDown => {
                self.state.scroll_offset += 1;
            }
            InputAction::TypeChar(c) => {
                self.state.input_buffer.push(c);
            }
            InputAction::Backspace => {
                self.state.input_buffer.pop();
            }
            InputAction::ClearInput => {
                self.state.clear_input();
            }
            InputAction::Quit => {
                self.state.should_exit = true;
            }
        }
        Ok(())
    }

    /// Handle command
    fn handle_command(&mut self, cmd: crate::handler::Command) -> Result<()> {
        use crate::handler::Command;
        match cmd {
            Command::Help => {
                self.state.status_message = Some(
                    "Commands: /help, /exit, /clear, /model <name>, /session <cmd>".to_string()
                );
            }
            Command::Exit => {
                self.state.should_exit = true;
            }
            Command::Clear => {
                self.state.messages.clear();
                self.state.status_message = Some("Screen cleared".to_string());
            }
            Command::Model(name) => {
                self.state.model = name.clone();
                self.state.status_message = Some(format!("Model set to: {}", name));
            }
            Command::Session(_) => {
                self.state.status_message = Some("Session commands not yet implemented".to_string());
            }
        }
        Ok(())
    }

    /// Render the UI
    fn render(&mut self, f: &mut ratatui::Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),  // Status bar
                Constraint::Min(10),    // Output area
                Constraint::Length(3),  // Input box
            ])
            .split(f.area());

        // Render status bar
        self.components.status_bar.render(f, &self.state, chunks[0]);

        // Render output area
        self.components.output_area.render(f, &self.state, chunks[1]);

        // Render input box
        self.components.input_box.render(f, &self.state, chunks[2]);
    }
}

/// Setup terminal for TUI
pub fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let stdout = std::io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// Restore terminal to normal mode
pub fn restore_terminal() -> Result<()> {
    disable_raw_mode()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== AppMode Tests =====

    #[test]
    fn test_app_mode_idle_label() {
        let mode = AppMode::Idle;
        assert_eq!(mode.label(), "Ready");
    }

    #[test]
    fn test_app_mode_thinking_label() {
        let mode = AppMode::Thinking;
        assert_eq!(mode.label(), "Thinking...");
    }

    #[test]
    fn test_app_mode_tool_executing_label() {
        let mode = AppMode::ToolExecuting {
            name: "ReadFile".to_string(),
            id: "tool-123".to_string(),
        };
        assert_eq!(mode.label(), "Tool: ReadFile");
    }

    #[test]
    fn test_app_mode_equality() {
        let mode1 = AppMode::Idle;
        let mode2 = AppMode::Idle;
        let mode3 = AppMode::Thinking;
        assert_eq!(mode1, mode2);
        assert_ne!(mode1, mode3);
    }

    // ===== Role Tests =====

    #[test]
    fn test_role_equality() {
        assert_eq!(Role::User, Role::User);
        assert_eq!(Role::Assistant, Role::Assistant);
        assert_eq!(Role::System, Role::System);
        assert_ne!(Role::User, Role::Assistant);
        assert_ne!(Role::Assistant, Role::System);
    }

    // ===== OutputMessage Tests =====

    #[test]
    fn test_output_message_new() {
        let msg = OutputMessage::new(Role::User, vec![OutputBlock::Text("Hello".to_string())]);
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content.len(), 1);
    }

    #[test]
    fn test_output_message_user() {
        let msg = OutputMessage::user("Test message".to_string());
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content.len(), 1);
        if let OutputBlock::Text(text) = &msg.content[0] {
            assert_eq!(text, "Test message");
        } else {
            panic!("Expected Text block");
        }
    }

    #[test]
    fn test_output_message_assistant() {
        let msg = OutputMessage::assistant("Response".to_string());
        assert_eq!(msg.role, Role::Assistant);
        assert_eq!(msg.content.len(), 1);
        if let OutputBlock::Text(text) = &msg.content[0] {
            assert_eq!(text, "Response");
        } else {
            panic!("Expected Text block");
        }
    }

    #[test]
    fn test_output_message_timestamp() {
        let before = chrono::Utc::now();
        let msg = OutputMessage::user("Test".to_string());
        let after = chrono::Utc::now();
        assert!(msg.timestamp >= before);
        assert!(msg.timestamp <= after);
    }

    // ===== AppState Tests =====

    #[test]
    fn test_app_state_default() {
        let state = AppState::default();
        assert_eq!(state.mode, AppMode::Idle);
        assert_eq!(state.model, "claude-sonnet-4.6");
        assert_eq!(state.tokens_used, 0);
        assert!(state.messages.is_empty());
        assert!(state.input_buffer.is_empty());
        assert!(state.input_history.is_empty());
        assert_eq!(state.history_index, 0);
        assert_eq!(state.scroll_offset, 0);
        assert!(!state.show_panel);
        assert!(state.session_id.is_none());
        assert!(!state.should_exit);
        assert!(state.status_message.is_none());
    }

    #[test]
    fn test_app_state_new() {
        let state = AppState::new();
        assert_eq!(state.mode, AppMode::Idle);
        assert!(state.messages.is_empty());
    }

    #[test]
    fn test_append_output_new_message() {
        let mut state = AppState::new();
        state.append_output("Hello");
        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0].role, Role::Assistant);
    }

    #[test]
    fn test_append_output_existing_message() {
        let mut state = AppState::new();
        state.append_output("Hello");
        state.append_output(" World");
        assert_eq!(state.messages.len(), 1);
        if let OutputBlock::Text(text) = &state.messages[0].content[0] {
            assert_eq!(text, "Hello World");
        } else {
            panic!("Expected Text block");
        }
    }

    #[test]
    fn test_append_output_after_thinking_block() {
        let mut state = AppState::new();
        state.append_output("First");
        state.append_thinking("Thinking...");
        state.append_output("Second");
        // Should create a new message since last block is Thinking
        assert_eq!(state.messages.len(), 2);
    }

    #[test]
    fn test_append_thinking() {
        let mut state = AppState::new();
        state.append_thinking("Thinking about this...");
        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0].role, Role::Assistant);
        if let OutputBlock::Thinking(text) = &state.messages[0].content[0] {
            assert_eq!(text, "Thinking about this...");
        } else {
            panic!("Expected Thinking block");
        }
    }

    #[test]
    fn test_append_tool_result() {
        let mut state = AppState::new();
        state.append_tool_result("tool-123", "ReadFile", "File contents", false);
        assert_eq!(state.messages.len(), 1);
        if let OutputBlock::ToolUse { id, name, result, is_error } = &state.messages[0].content[0] {
            assert_eq!(id, "tool-123");
            assert_eq!(name, "ReadFile");
            assert_eq!(result, "File contents");
            assert!(!is_error);
        } else {
            panic!("Expected ToolUse block");
        }
    }

    #[test]
    fn test_append_tool_result_error() {
        let mut state = AppState::new();
        state.append_tool_result("tool-456", "WriteFile", "Permission denied", true);
        if let OutputBlock::ToolUse { is_error, .. } = &state.messages[0].content[0] {
            assert!(is_error);
        } else {
            panic!("Expected ToolUse block");
        }
    }

    #[test]
    fn test_show_error() {
        let mut state = AppState::new();
        state.show_error("Something went wrong");
        assert_eq!(state.status_message, Some("Error: Something went wrong".to_string()));
    }

    #[test]
    fn test_clear_input() {
        let mut state = AppState::new();
        state.input_buffer = "test input".to_string();
        state.history_index = 5;
        state.clear_input();
        assert!(state.input_buffer.is_empty());
        assert_eq!(state.history_index, 0);
    }

    #[test]
    fn test_history_up_empty_history() {
        let mut state = AppState::new();
        state.history_up();
        assert_eq!(state.history_index, 0);
        assert!(state.input_buffer.is_empty());
    }

    #[test]
    fn test_history_up_with_history() {
        let mut state = AppState::new();
        state.input_history = vec!["cmd1".to_string(), "cmd2".to_string(), "cmd3".to_string()];
        state.history_up();
        assert_eq!(state.history_index, 1);
        assert_eq!(state.input_buffer, "cmd3");
        state.history_up();
        assert_eq!(state.history_index, 2);
        assert_eq!(state.input_buffer, "cmd2");
    }

    #[test]
    fn test_history_up_boundary() {
        let mut state = AppState::new();
        state.input_history = vec!["cmd1".to_string()];
        state.history_up();
        assert_eq!(state.history_index, 1);
        // Should not go beyond the oldest entry
        state.history_up();
        assert_eq!(state.history_index, 1);
    }

    #[test]
    fn test_history_down() {
        let mut state = AppState::new();
        state.input_history = vec!["cmd1".to_string(), "cmd2".to_string(), "cmd3".to_string()];
        state.history_index = 2;
        state.history_down();
        assert_eq!(state.history_index, 1);
        assert_eq!(state.input_buffer, "cmd2");
    }

    #[test]
    fn test_history_down_to_bottom() {
        let mut state = AppState::new();
        state.input_history = vec!["cmd1".to_string(), "cmd2".to_string()];
        state.history_index = 1;
        state.history_down();
        assert_eq!(state.history_index, 0);
        assert!(state.input_buffer.is_empty());
    }

    #[test]
    fn test_history_down_already_at_bottom() {
        let mut state = AppState::new();
        state.history_down();
        assert_eq!(state.history_index, 0);
        assert!(state.input_buffer.is_empty());
    }

    #[test]
    fn test_add_to_history() {
        let mut state = AppState::new();
        state.add_to_history("first command".to_string());
        assert_eq!(state.input_history.len(), 1);
        assert_eq!(state.input_history[0], "first command");
        assert_eq!(state.history_index, 0);
    }

    #[test]
    fn test_add_to_history_empty_skipped() {
        let mut state = AppState::new();
        state.add_to_history("".to_string());
        state.add_to_history("   ".to_string());
        assert!(state.input_history.is_empty());
    }

    #[test]
    fn test_add_to_history_trims_whitespace() {
        let mut state = AppState::new();
        state.add_to_history("  test  ".to_string());
        // Note: add_to_history doesn't trim, it stores as-is
        // The trimming happens in the send action
        assert_eq!(state.input_history.len(), 1);
    }

    #[test]
    fn test_add_to_history_limit_100() {
        let mut state = AppState::new();
        for i in 0..105 {
            state.add_to_history(format!("cmd{}", i));
        }
        assert_eq!(state.input_history.len(), 100);
        // The oldest entries should be removed
        assert_eq!(state.input_history[0], "cmd5");
        assert_eq!(state.input_history[99], "cmd104");
    }

    // ===== Components Tests =====

    #[test]
    fn test_components_default() {
        let components = Components::default();
        // Just verify it can be created
        assert!(true);
    }

    #[test]
    fn test_components_new() {
        let components = Components::new();
        // Just verify it can be created
        assert!(true);
    }
}