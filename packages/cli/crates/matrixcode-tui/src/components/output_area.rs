//! Output Area Component
//!
//! Displays conversation history with scrolling support.

use ratatui::{
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::{AppState, OutputBlock, Role};

/// Output area component
pub struct OutputArea {
    /// Current scroll position
    scroll: u16,
}

impl OutputArea {
    /// Create new output area
    pub fn new() -> Self {
        Self { scroll: 0 }
    }

    /// Render the output area
    pub fn render(&mut self, f: &mut Frame, state: &AppState, area: ratatui::layout::Rect) {
        // Build lines from messages
        let mut lines: Vec<Line> = Vec::new();

        for msg in &state.messages {
            let role_style = match msg.role {
                Role::User => Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                Role::Assistant => Style::default().fg(Color::Blue),
                Role::System => Style::default().fg(Color::Yellow),
            };

            let role_prefix = match msg.role {
                Role::User => "You: ",
                Role::Assistant => "Assistant: ",
                Role::System => "System: ",
            };

            // Add role prefix line
            lines.push(Line::styled(role_prefix, role_style));

            // Add content blocks
            for block in &msg.content {
                match block {
                    OutputBlock::Text(text) => {
                        // Split text into lines for proper wrapping
                        for line in text.lines() {
                            lines.push(Line::styled(line.to_string(), Style::default()));
                        }
                    }
                    OutputBlock::Thinking(thought) => {
                        lines.push(Line::styled(
                            format!("  [Thinking: {}]", thought),
                            Style::default().fg(Color::DarkGray),
                        ));
                    }
                    OutputBlock::ToolUse { name, result, is_error, .. } => {
                        let style = if *is_error {
                            Style::default().fg(Color::Red)
                        } else {
                            Style::default().fg(Color::Cyan)
                        };
                        lines.push(Line::styled(format!("  [Tool: {}]", name), style));
                        // Truncate result for display
                        let display_result = if result.len() > 200 {
                            format!("{}...", &result[..200])
                        } else {
                            result.clone()
                        };
                        for line in display_result.lines() {
                            lines.push(Line::styled(format!("    {}", line), style));
                        }
                    }
                }
            }

            // Add blank line between messages
            lines.push(Line::raw(""));
        }

        // Calculate scroll based on state
        let total_lines = lines.len() as u16;
        let visible_lines = area.height.saturating_sub(2); // Account for borders

        // Auto-scroll to bottom if we're near the end
        if state.scroll_offset == 0 || total_lines <= visible_lines {
            self.scroll = 0;
        } else {
            self.scroll = (state.scroll_offset as u16).min(total_lines.saturating_sub(visible_lines));
        }

        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Conversation ")
                    .title_style(Style::default().fg(Color::White)),
            )
            .wrap(Wrap { trim: false })
            .scroll((self.scroll, 0));

        f.render_widget(paragraph, area);
    }
}

impl Default for OutputArea {
    fn default() -> Self {
        Self::new()
    }
}