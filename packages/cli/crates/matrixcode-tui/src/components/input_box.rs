//! Input Box Component
//!
//! Multi-line input with history navigation, command hints, and visual feedback.

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::{AppMode, AppState};

/// Maximum visible input lines
const MAX_INPUT_DISPLAY_LINES: usize = 5;

/// Input box component
pub struct InputBox {
    /// Cursor position in the input buffer
    cursor_pos: usize,
}

impl InputBox {
    /// Create new input box
    pub fn new() -> Self {
        Self { cursor_pos: 0 }
    }

    /// Get input prompt based on mode
    fn get_prompt(mode: &AppMode) -> &'static str {
        match mode {
            AppMode::Idle => ">",
            AppMode::Thinking => "⏳",
            AppMode::ToolExecuting { name, .. } => {
                // Show abbreviated tool name
                if name.len() > 8 {
                    "🔧"
                } else {
                    "⚙"
                }
            }
        }
    }

    /// Get input color based on mode
    fn get_input_color(mode: &AppMode) -> Color {
        match mode {
            AppMode::Idle => Color::Yellow,
            AppMode::Thinking => Color::DarkGray,
            AppMode::ToolExecuting { .. } => Color::Cyan,
        }
    }

    /// Check if input is a command
    fn is_command(input: &str) -> bool {
        input.trim().starts_with('/')
    }

    /// Get command suggestions based on partial input
    fn get_command_suggestion(input: &str) -> Option<&'static str> {
        let trimmed = input.trim();
        if !trimmed.starts_with('/') {
            return None;
        }

        // Partial command matching
        let commands = [
            ("/help", "Show available commands"),
            ("/exit", "Exit the terminal"),
            ("/clear", "Clear the screen"),
            ("/model", "Change model (e.g., /model claude-sonnet-4.6)"),
            ("/session", "Session management (list, save, load, new)"),
        ];

        for (cmd, desc) in commands {
            if cmd.starts_with(trimmed) && cmd != trimmed {
                return Some(desc);
            }
        }
        None
    }

    /// Render the input box
    pub fn render(&self, f: &mut Frame, state: &AppState, area: ratatui::layout::Rect) {
        let prompt = Self::get_prompt(&state.mode);
        let input_color = Self::get_input_color(&state.mode);
        let is_command = Self::is_command(&state.input_buffer);

        // Build input display with prompt
        let input_text: Line = if state.input_buffer.is_empty() {
            // Placeholder text when empty
            let placeholder = match state.mode {
                AppMode::Idle => "Type a message or command...",
                AppMode::Thinking => "Processing...",
                AppMode::ToolExecuting { name: _, .. } => "Executing...",
            };
            Line::from(Span::styled(placeholder, Style::default().fg(Color::DarkGray)))
        } else {
            // Show actual input with appropriate styling
            if is_command {
                // Command styling - highlight the / prefix
                Line::from(vec![
                    Span::styled("/", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                    Span::styled(
                        state.input_buffer.trim_start_matches('/'),
                        Style::default().fg(Color::Green),
                    ),
                ])
            } else {
                Line::from(Span::styled(&state.input_buffer, Style::default().fg(input_color)))
            }
        };

        // Main input field
        let title_style = if is_command {
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(input_color)
        };

        let input_paragraph = Paragraph::new(Line::from(vec![
            Span::styled(format!("{} ", prompt), Style::default().fg(input_color).add_modifier(Modifier::BOLD)),
        ].into_iter().chain(input_text.into_iter()).collect::<Vec<_>>()))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Input ")
                .title_style(title_style)
        );

        f.render_widget(input_paragraph, area);

        // Render hints line below input (using remaining space if available)
        let hints_area_height = area.height as usize;
        if hints_area_height >= 3 {
            // We have space for hints
            let hints_y = area.y + area.height - 1;

            // Build hints based on state
            let hints_spans = Self::build_hints(state);

            let hints_line = Line::from(hints_spans);

            // Render hints at the bottom of the area
            f.render_widget(
                Paragraph::new(hints_line).style(Style::default().fg(Color::DarkGray)),
                ratatui::layout::Rect {
                    x: area.x + 1,
                    y: hints_y,
                    width: area.width - 2,
                    height: 1,
                },
            );
        }

        // Show command suggestion if applicable
        if let Some(suggestion) = Self::get_command_suggestion(&state.input_buffer) {
            let suggestion_y = area.y + area.height.saturating_sub(2);
            f.render_widget(
                Paragraph::new(Line::styled(
                    format!("  💡 {}", suggestion),
                    Style::default().fg(Color::LightGreen),
                )),
                ratatui::layout::Rect {
                    x: area.x + 1,
                    y: suggestion_y,
                    width: area.width - 2,
                    height: 1,
                },
            );
        }

        // Show history navigation indicator if navigating
        if state.history_index > 0 {
            let total = state.input_history.len();
            let current = state.history_index;
            f.render_widget(
                Paragraph::new(Line::styled(
                    format!("  History: {}/{}", current, total),
                    Style::default().fg(Color::Magenta),
                )),
                ratatui::layout::Rect {
                    x: area.x + area.width.saturating_sub(15),
                    y: area.y + 1,
                    width: 14,
                    height: 1,
                },
            );
        }
    }

    /// Build hint spans based on current state
    fn build_hints(state: &AppState) -> Vec<Span> {
        let mut hints = Vec::new();

        // Primary actions
        if state.mode == AppMode::Idle {
            hints.push(Span::styled("[", Style::default().fg(Color::DarkGray)));
            hints.push(Span::styled("Enter", Style::default().fg(Color::Yellow)));
            hints.push(Span::styled(": Send]", Style::default().fg(Color::DarkGray)));
        } else {
            hints.push(Span::styled("[", Style::default().fg(Color::DarkGray)));
            hints.push(Span::styled("Ctrl+C", Style::default().fg(Color::Red)));
            hints.push(Span::styled(": Interrupt]", Style::default().fg(Color::DarkGray)));
        }

        // History navigation (only when input is empty)
        if state.input_buffer.is_empty() && !state.input_history.is_empty() {
            hints.push(Span::raw(" "));
            hints.push(Span::styled("[", Style::default().fg(Color::DarkGray)));
            hints.push(Span::styled("↑↓", Style::default().fg(Color::Magenta)));
            hints.push(Span::styled(": History]", Style::default().fg(Color::DarkGray)));
        }

        // Scroll hints
        hints.push(Span::raw(" "));
        hints.push(Span::styled("[", Style::default().fg(Color::DarkGray)));
        hints.push(Span::styled("PgUp/PgDn", Style::default().fg(Color::Cyan)));
        hints.push(Span::styled(": Scroll]", Style::default().fg(Color::DarkGray)));

        // Exit hint
        hints.push(Span::raw(" "));
        hints.push(Span::styled("[", Style::default().fg(Color::DarkGray)));
        hints.push(Span::styled("Ctrl+D", Style::default().fg(Color::Red)));
        hints.push(Span::styled(": Quit]", Style::default().fg(Color::DarkGray)));

        hints
    }

    /// Update cursor position (for future multi-line support)
    pub fn update_cursor(&mut self, pos: usize) {
        self.cursor_pos = pos;
    }

    /// Get current cursor position
    pub fn cursor_position(&self) -> usize {
        self.cursor_pos
    }
}

impl Default for InputBox {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== get_prompt Tests =====

    #[test]
    fn test_get_prompt_idle() {
        let mode = AppMode::Idle;
        assert_eq!(InputBox::get_prompt(&mode), ">");
    }

    #[test]
    fn test_get_prompt_thinking() {
        let mode = AppMode::Thinking;
        assert_eq!(InputBox::get_prompt(&mode), "⏳");
    }

    #[test]
    fn test_get_prompt_tool_executing_short_name() {
        let mode = AppMode::ToolExecuting {
            name: "Read".to_string(),
            id: "tool-123".to_string(),
        };
        assert_eq!(InputBox::get_prompt(&mode), "⚙");
    }

    #[test]
    fn test_get_prompt_tool_executing_long_name() {
        let mode = AppMode::ToolExecuting {
            name: "VeryLongToolName".to_string(),
            id: "tool-456".to_string(),
        };
        assert_eq!(InputBox::get_prompt(&mode), "🔧");
    }

    #[test]
    fn test_get_prompt_tool_executing_boundary_eight_chars() {
        let mode = AppMode::ToolExecuting {
            name: "12345678".to_string(), // Exactly 8 chars
            id: "tool-789".to_string(),
        };
        assert_eq!(InputBox::get_prompt(&mode), "⚙");
    }

    #[test]
    fn test_get_prompt_tool_executing_nine_chars() {
        let mode = AppMode::ToolExecuting {
            name: "123456789".to_string(), // 9 chars, > 8
            id: "tool-000".to_string(),
        };
        assert_eq!(InputBox::get_prompt(&mode), "🔧");
    }

    // ===== get_input_color Tests =====

    #[test]
    fn test_get_input_color_idle() {
        let mode = AppMode::Idle;
        assert_eq!(InputBox::get_input_color(&mode), Color::Yellow);
    }

    #[test]
    fn test_get_input_color_thinking() {
        let mode = AppMode::Thinking;
        assert_eq!(InputBox::get_input_color(&mode), Color::DarkGray);
    }

    #[test]
    fn test_get_input_color_tool_executing() {
        let mode = AppMode::ToolExecuting {
            name: "Read".to_string(),
            id: "tool-123".to_string(),
        };
        assert_eq!(InputBox::get_input_color(&mode), Color::Cyan);
    }

    // ===== is_command Tests =====

    #[test]
    fn test_is_command_true() {
        assert!(InputBox::is_command("/help"));
        assert!(InputBox::is_command("/exit"));
        assert!(InputBox::is_command("/model claude-sonnet"));
    }

    #[test]
    fn test_is_command_false() {
        assert!(!InputBox::is_command("hello"));
        assert!(!InputBox::is_command("help"));
        assert!(!InputBox::is_command(""));
    }

    #[test]
    fn test_is_command_whitespace() {
        assert!(InputBox::is_command("  /help"));
        assert!(InputBox::is_command("\t/exit"));
        assert!(InputBox::is_command("   /clear   "));
    }

    #[test]
    fn test_is_command_empty_string() {
        assert!(!InputBox::is_command(""));
    }

    #[test]
    fn test_is_command_whitespace_only() {
        assert!(!InputBox::is_command("   "));
        assert!(!InputBox::is_command("\t\n"));
    }

    // ===== get_command_suggestion Tests =====

    #[test]
    fn test_get_command_suggestion_partial_help() {
        let suggestion = InputBox::get_command_suggestion("/hel");
        assert_eq!(suggestion, Some("Show available commands"));
    }

    #[test]
    fn test_get_command_suggestion_partial_exit() {
        let suggestion = InputBox::get_command_suggestion("/ex");
        assert_eq!(suggestion, Some("Exit the terminal"));
    }

    #[test]
    fn test_get_command_suggestion_partial_clear() {
        let suggestion = InputBox::get_command_suggestion("/cle");
        assert_eq!(suggestion, Some("Clear the screen"));
    }

    #[test]
    fn test_get_command_suggestion_partial_model() {
        let suggestion = InputBox::get_command_suggestion("/mod");
        assert_eq!(suggestion, Some("Change model (e.g., /model claude-sonnet-4.6)"));
    }

    #[test]
    fn test_get_command_suggestion_partial_session() {
        let suggestion = InputBox::get_command_suggestion("/ses");
        assert_eq!(suggestion, Some("Session management (list, save, load, new)"));
    }

    #[test]
    fn test_get_command_suggestion_complete() {
        // Complete command should have no suggestion
        assert_eq!(InputBox::get_command_suggestion("/help"), None);
        assert_eq!(InputBox::get_command_suggestion("/exit"), None);
        assert_eq!(InputBox::get_command_suggestion("/clear"), None);
        assert_eq!(InputBox::get_command_suggestion("/model"), None);
        assert_eq!(InputBox::get_command_suggestion("/session"), None);
    }

    #[test]
    fn test_get_command_suggestion_non_command() {
        assert_eq!(InputBox::get_command_suggestion("hello"), None);
        assert_eq!(InputBox::get_command_suggestion("world"), None);
    }

    #[test]
    fn test_get_command_suggestion_invalid_prefix() {
        assert_eq!(InputBox::get_command_suggestion("help"), None);
        assert_eq!(InputBox::get_command_suggestion(" /help"), None);
    }

    #[test]
    fn test_get_command_suggestion_empty_string() {
        assert_eq!(InputBox::get_command_suggestion(""), None);
    }

    #[test]
    fn test_get_command_suggestion_slash_only() {
        // "/" alone should match all commands but only returns first suggestion
        let suggestion = InputBox::get_command_suggestion("/");
        assert!(suggestion.is_some());
    }

    // ===== InputBox Construction Tests =====

    #[test]
    fn test_input_box_new() {
        let input_box = InputBox::new();
        assert_eq!(input_box.cursor_position(), 0);
    }

    #[test]
    fn test_input_box_default() {
        let input_box = InputBox::default();
        assert_eq!(input_box.cursor_position(), 0);
    }

    #[test]
    fn test_input_box_update_cursor() {
        let mut input_box = InputBox::new();
        input_box.update_cursor(5);
        assert_eq!(input_box.cursor_position(), 5);
    }
}