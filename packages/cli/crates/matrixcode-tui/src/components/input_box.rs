//! Input Box Component
//!
//! Multi-line input with history navigation and command hints.

use ratatui::{
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::AppState;

/// Input box component
pub struct InputBox;

impl InputBox {
    /// Create new input box
    pub fn new() -> Self {
        Self
    }

    /// Render the input box
    pub fn render(&self, f: &mut Frame, state: &AppState, area: ratatui::layout::Rect) {
        // Build input display
        let input_text = if state.input_buffer.is_empty() {
            "Type a message or command..."
        } else {
            &state.input_buffer
        };

        // Input field
        let input_paragraph = Paragraph::new(input_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Input ")
                    .title_style(Style::default().fg(Color::Yellow))
            )
            .style(Style::default().fg(Color::White));

        f.render_widget(input_paragraph, area);

        // Hints displayed in area below input (for future use)
        let _hints = "[Enter: Send] [↑↓: History] [Tab: Panel] [Esc: Clear] [Ctrl+C: Interrupt] [Ctrl+D: Quit]";
    }
}

impl Default for InputBox {
    fn default() -> Self {
        Self::new()
    }
}