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

#[cfg(test)]
mod tests {
    use super::*;

    // ===== mode_bg_color Tests =====

    #[test]
    fn test_mode_bg_color_idle() {
        let mode = AppMode::Idle;
        assert_eq!(StatusBar::mode_bg_color(&mode), Color::Blue);
    }

    #[test]
    fn test_mode_bg_color_thinking() {
        let mode = AppMode::Thinking;
        assert_eq!(StatusBar::mode_bg_color(&mode), Color::Yellow);
    }

    #[test]
    fn test_mode_bg_color_tool_executing() {
        let mode = AppMode::ToolExecuting {
            name: "Read".to_string(),
            id: "tool-123".to_string(),
        };
        assert_eq!(StatusBar::mode_bg_color(&mode), Color::Cyan);
    }

    #[test]
    fn test_mode_bg_color_tool_executing_various_names() {
        // Color should be the same regardless of tool name
        let mode1 = AppMode::ToolExecuting {
            name: "Bash".to_string(),
            id: "tool-1".to_string(),
        };
        let mode2 = AppMode::ToolExecuting {
            name: "Read".to_string(),
            id: "tool-2".to_string(),
        };
        assert_eq!(StatusBar::mode_bg_color(&mode1), StatusBar::mode_bg_color(&mode2));
    }

    // ===== mode_fg_color Tests =====

    #[test]
    fn test_mode_fg_color_idle() {
        let mode = AppMode::Idle;
        assert_eq!(StatusBar::mode_fg_color(&mode), Color::White);
    }

    #[test]
    fn test_mode_fg_color_thinking() {
        let mode = AppMode::Thinking;
        assert_eq!(StatusBar::mode_fg_color(&mode), Color::Black);
    }

    #[test]
    fn test_mode_fg_color_tool_executing() {
        let mode = AppMode::ToolExecuting {
            name: "Read".to_string(),
            id: "tool-123".to_string(),
        };
        assert_eq!(StatusBar::mode_fg_color(&mode), Color::Black);
    }

    #[test]
    fn test_mode_fg_color_consistency() {
        // Thinking and ToolExecuting should have same foreground
        let mode_thinking = AppMode::Thinking;
        let mode_tool = AppMode::ToolExecuting {
            name: "Test".to_string(),
            id: "tool-1".to_string(),
        };
        assert_eq!(StatusBar::mode_fg_color(&mode_thinking), StatusBar::mode_fg_color(&mode_tool));
    }

    // ===== status_indicator Tests =====

    #[test]
    fn test_status_indicator_idle() {
        let mode = AppMode::Idle;
        assert_eq!(StatusBar::status_indicator(&mode), "●");
    }

    #[test]
    fn test_status_indicator_thinking() {
        let mode = AppMode::Thinking;
        assert_eq!(StatusBar::status_indicator(&mode), "◉");
    }

    #[test]
    fn test_status_indicator_tool_executing() {
        let mode = AppMode::ToolExecuting {
            name: "Read".to_string(),
            id: "tool-123".to_string(),
        };
        assert_eq!(StatusBar::status_indicator(&mode), "⚙");
    }

    #[test]
    fn test_status_indicator_tool_executing_various_names() {
        // Indicator should be the same regardless of tool name
        let mode1 = AppMode::ToolExecuting {
            name: "Bash".to_string(),
            id: "tool-1".to_string(),
        };
        let mode2 = AppMode::ToolExecuting {
            name: "VeryLongToolName".to_string(),
            id: "tool-2".to_string(),
        };
        assert_eq!(StatusBar::status_indicator(&mode1), StatusBar::status_indicator(&mode2));
    }

    #[test]
    fn test_status_indicator_distinct_for_each_mode() {
        // Each mode should have a distinct indicator
        let idle_indicator = StatusBar::status_indicator(&AppMode::Idle);
        let thinking_indicator = StatusBar::status_indicator(&AppMode::Thinking);
        let tool_indicator = StatusBar::status_indicator(&AppMode::ToolExecuting {
            name: "Test".to_string(),
            id: "tool-1".to_string(),
        });

        assert_ne!(idle_indicator, thinking_indicator);
        assert_ne!(thinking_indicator, tool_indicator);
        assert_ne!(idle_indicator, tool_indicator);
    }

    // ===== Color Contrast Tests =====

    #[test]
    fn test_idle_colors_contrast() {
        // Idle: Blue bg + White fg should be readable
        let bg = StatusBar::mode_bg_color(&AppMode::Idle);
        let fg = StatusBar::mode_fg_color(&AppMode::Idle);
        assert_eq!(bg, Color::Blue);
        assert_eq!(fg, Color::White);
    }

    #[test]
    fn test_thinking_colors_contrast() {
        // Thinking: Yellow bg + Black fg should be readable
        let bg = StatusBar::mode_bg_color(&AppMode::Thinking);
        let fg = StatusBar::mode_fg_color(&AppMode::Thinking);
        assert_eq!(bg, Color::Yellow);
        assert_eq!(fg, Color::Black);
    }

    #[test]
    fn test_tool_executing_colors_contrast() {
        // ToolExecuting: Cyan bg + Black fg should be readable
        let bg = StatusBar::mode_bg_color(&AppMode::ToolExecuting {
            name: "Test".to_string(),
            id: "tool-1".to_string(),
        });
        let fg = StatusBar::mode_fg_color(&AppMode::ToolExecuting {
            name: "Test".to_string(),
            id: "tool-1".to_string(),
        });
        assert_eq!(bg, Color::Cyan);
        assert_eq!(fg, Color::Black);
    }

    // ===== StatusBar Construction Tests =====

    #[test]
    fn test_status_bar_new() {
        let status_bar = StatusBar::new();
        let _ = status_bar;
    }

    #[test]
    fn test_status_bar_default() {
        let status_bar = StatusBar::default();
        let _ = status_bar;
    }
}