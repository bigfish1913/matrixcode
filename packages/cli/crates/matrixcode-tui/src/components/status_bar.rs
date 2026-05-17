//! Status Bar Component
//!
//! Displays version, model, tokens, and current status.

use ratatui::{
    style::{Color, Style},
    widgets::Paragraph,
    Frame,
};

use crate::app::{AppMode, AppState};

/// Status bar component
pub struct StatusBar;

impl StatusBar {
    /// Create new status bar
    pub fn new() -> Self {
        Self
    }

    /// Render the status bar
    pub fn render(&self, f: &mut Frame, state: &AppState, area: ratatui::layout::Rect) {
        // Status color based on mode (for future use)
        let _mode_style = match &state.mode {
            AppMode::Idle => Style::default().fg(Color::Green),
            AppMode::Thinking => Style::default().fg(Color::Yellow),
            AppMode::ToolExecuting { .. } => Style::default().fg(Color::Cyan),
        };

        let status_text = format!(
            " MatrixCode {} | Model: {} | Tokens: {} | {} ",
            crate::app::VERSION,
            state.model,
            state.tokens_used,
            state.mode.label()
        );

        // Add status message if present
        let text = if let Some(msg) = &state.status_message {
            format!("{} | {}", status_text, msg)
        } else {
            status_text
        };

        let paragraph = Paragraph::new(text)
            .style(Style::default().bg(Color::Blue).fg(Color::White));

        f.render_widget(paragraph, area);
    }
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new()
    }
}