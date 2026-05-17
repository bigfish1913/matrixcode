//! Status Bar Component
//!
//! Displays version, model, tokens, and current status with visual indicators.

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::{AppMode, AppState};
use crate::ui::format_tokens;

/// Status bar component
pub struct StatusBar;

impl StatusBar {
    /// Create new status bar
    pub fn new() -> Self {
        Self
    }

    /// Get background color based on mode
    fn mode_bg_color(mode: &AppMode) -> Color {
        match mode {
            AppMode::Idle => Color::Blue,
            AppMode::Thinking => Color::Yellow,
            AppMode::ToolExecuting { .. } => Color::Cyan,
        }
    }

    /// Get foreground color based on mode
    fn mode_fg_color(mode: &AppMode) -> Color {
        match mode {
            AppMode::Idle => Color::White,
            AppMode::Thinking => Color::Black,
            AppMode::ToolExecuting { .. } => Color::Black,
        }
    }

    /// Get status indicator symbol
    fn status_indicator(mode: &AppMode) -> &'static str {
        match mode {
            AppMode::Idle => "●",
            AppMode::Thinking => "◉",
            AppMode::ToolExecuting { .. } => "⚙",
        }
    }

    /// Render the status bar
    pub fn render(&self, f: &mut Frame, state: &AppState, area: ratatui::layout::Rect) {
        let bg_color = Self::mode_bg_color(&state.mode);
        let fg_color = Self::mode_fg_color(&state.mode);
        let indicator = Self::status_indicator(&state.mode);

        // Build styled spans for the status bar
        let spans: Vec<Span> = vec![
            // Version with bold
            Span::styled(
                format!(" {} MatrixCode v{} ", indicator, crate::app::VERSION),
                Style::default().fg(fg_color).bg(bg_color).add_modifier(Modifier::BOLD),
            ),
            // Separator
            Span::styled("│", Style::default().fg(Color::DarkGray).bg(bg_color)),
            // Model
            Span::styled(
                format!(" {} ", state.model),
                Style::default().fg(fg_color).bg(bg_color),
            ),
            // Separator
            Span::styled("│", Style::default().fg(Color::DarkGray).bg(bg_color)),
            // Tokens (formatted nicely)
            Span::styled(
                format!(" {} ", format_tokens(state.tokens_used)),
                Style::default().fg(fg_color).bg(bg_color),
            ),
            // Separator
            Span::styled("│", Style::default().fg(Color::DarkGray).bg(bg_color)),
            // Mode label
            Span::styled(
                format!(" {} ", state.mode.label()),
                Style::default().fg(fg_color).bg(bg_color),
            ),
        ];

        // Add status message if present
        let mut all_spans = spans;
        if let Some(msg) = &state.status_message {
            all_spans.push(Span::styled("│", Style::default().fg(Color::DarkGray).bg(bg_color)));
            all_spans.push(Span::styled(
                format!(" {} ", msg),
                Style::default().fg(Color::LightYellow).bg(bg_color),
            ));
        }

        // Fill remaining space with background color
        let remaining = area.width as usize;
        let content_len = all_spans.iter().map(|s| s.content.len()).sum::<usize>();
        if content_len < remaining {
            all_spans.push(Span::styled(
                " ".repeat(remaining - content_len),
                Style::default().bg(bg_color),
            ));
        }

        let paragraph = Paragraph::new(Line::from(all_spans));

        f.render_widget(paragraph, area);
    }
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new()
    }
}