//! TUI drawing module.

mod helpers;
mod hint;  // New hint bar module
mod input;
mod messages;
mod status;
mod workflow;

use crate::app::TuiApp;
use crate::types::Activity;
use ratatui::layout::Rect;

impl TuiApp {
    pub(crate) fn draw(&self, f: &mut ratatui::Frame) {
        let total_height = f.area().height;
        let width = f.area().width;

        // Workflow panel width (when visible)
        let workflow_width: u16 = if self.workflow_state.visible {
            40u16.min(width / 3)
        } else {
            0
        };

        // Adjust content width to avoid overlap with workflow panel
        let content_width = width.saturating_sub(workflow_width);

        // Debug panel height (when visible)
        let debug_height: u16 = if self.show_debug_panel {
            // Show debug panel at bottom, take up to 10 lines
            10.min(total_height / 3)
        } else {
            0
        };

        // Fixed heights for bottom components
        let status_height: u16 = 1;
        let hint_height: u16 = if self.should_show_hint() { 1 } else { 0 };
        let gap_height: u16 = 1;
        let queue_height: u16 = if self.pending_messages.is_empty() { 0 } else { 1 };

        // Activity indicator height: 1 line when thinking or tool activity
        // Tool activities show animation in message area when auto_scroll is on,
        // and in fixed bottom area when user scrolls up (auto_scroll off)
        let activity_height: u16 = if matches!(self.activity, Activity::Thinking)
            || (self.is_tool_activity() && self.streaming.is_empty() && self.thinking.is_empty()) {
            1
        } else {
            0
        };

        // Dynamic input height based on content
        let input_height: u16 = self.calculate_input_height();

        // Calculate reserved height from bottom
        let reserved = status_height + input_height + hint_height + gap_height + queue_height + activity_height + debug_height;

        // Messages height: what's left, minimum 5 lines
        let messages_height = total_height.saturating_sub(reserved).max(5);

        // Ensure messages_height doesn't exceed available space
        let messages_height = messages_height.min(total_height.saturating_sub(reserved));

        // Calculate Y positions from bottom up
        let status_y = total_height - status_height;
        let input_y = status_y - input_height;
        let hint_y = input_y - hint_height;
        let gap_y = hint_y - gap_height;
        let queue_y = gap_y - queue_height;
        let activity_y = queue_y - activity_height;
        let debug_y = activity_y - debug_height;

        // Create areas - messages area starts from top (y=0)
        // Use content_width to avoid overlap with workflow panel
        let messages_area = Rect::new(0, 0, content_width, messages_height);
        let debug_area = Rect::new(0, debug_y, content_width, debug_height);
        let queue_area = Rect::new(0, queue_y, content_width, queue_height);
        let activity_area = Rect::new(0, activity_y, content_width, activity_height);
        let hint_area = Rect::new(0, hint_y, content_width, hint_height);
        let input_area = Rect::new(0, input_y, content_width, input_height);
        let status_area = Rect::new(0, status_y, content_width, status_height);

        // Render in order: messages first (top), then bottom components
        self.draw_messages(f, messages_area);
        if debug_height > 0 {
            self.draw_debug_panel(f, debug_area);
        }
        if queue_height > 0 {
            self.draw_queue(f, queue_area);
        }
        if activity_height > 0 {
            self.draw_activity_indicator(f, activity_area);
        }
        if hint_height > 0 {
            self.draw_hint(f, hint_area);
        }
        self.draw_input(f, input_area);
        self.draw_status(f, status_area);

        // Workflow panel overlay (right side)
        self.draw_workflow_panel(f);
    }

    /// Check if current activity is a tool activity
    fn is_tool_activity(&self) -> bool {
        matches!(
            self.activity,
            Activity::Reading
                | Activity::Writing
                | Activity::Editing
                | Activity::Searching
                | Activity::Running
                | Activity::WebSearch
                | Activity::WebFetch
                | Activity::Tool(_)
        )
    }

    /// Draw activity indicator (spinner + label) - fixed at bottom
    fn draw_activity_indicator(&self, f: &mut ratatui::Frame, area: Rect) {
        use ratatui::{
            style::{Color, Modifier, Style},
            text::{Line, Span},
            widgets::Paragraph,
        };
        use crate::SPINNER;
        use crate::utils::fmt_tokens;

        let elapsed = self
            .request_start
            .map(|s| format!(" {:.1}s", s.elapsed().as_secs_f64()))
            .unwrap_or_default();
        let spinner_frame = self.frame % SPINNER.len();

        // For tool activities: only show in fixed area when user has scrolled up
        // (auto_scroll is false). When auto_scroll is on, the animation is visible
        // in the message area, so we skip rendering here to avoid duplication.
        if self.is_tool_activity() && self.auto_scroll {
            return;
        }

        let line = if self.activity == Activity::Thinking {
            // Show real-time token count during thinking
            let token_display = if self.current_request_tokens > 0 {
                format!(" {}tok", fmt_tokens(self.current_request_tokens))
            } else {
                String::new()
            };
            Line::from(vec![
                Span::styled(
                    format!("{} ", SPINNER[spinner_frame]),
                    Style::default().fg(Color::LightGreen),
                ),
                Span::styled("💭 ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("Thinking{}", token_display),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(elapsed, Style::default().fg(Color::DarkGray)),
            ])
        } else if self.is_tool_activity() {
            let tool_icon = match self.activity {
                Activity::Reading => "📖",
                Activity::Writing => "📝",
                Activity::Editing => "✏️",
                Activity::Searching => "🔍",
                Activity::Running => "⚡",
                Activity::WebSearch => "🌐",
                Activity::WebFetch => "🔗",
                Activity::Tool(ref name) => match name.as_str() {
                    "task" => "🚀",
                    "plan" => "📋",
                    _ => "🔧",
                },
                _ => "⚙️",
            };
            Line::from(vec![
                Span::styled(
                    format!("{} ", SPINNER[spinner_frame]),
                    Style::default().fg(Color::LightGreen),
                ),
                Span::styled(
                    format!("{} ", tool_icon),
                    Style::default().fg(self.activity.color()),
                ),
                Span::styled(
                    self.activity.label(),
                    Style::default()
                        .fg(self.activity.color())
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(elapsed, Style::default().fg(Color::DarkGray)),
            ])
        } else {
            return; // No indicator for other activities
        };

        f.render_widget(Paragraph::new(line), area);
    }

    /// Calculate required input area height based on current state
    pub(crate) fn calculate_input_height(&self) -> u16 {
        let base_height: u16 = 2; // Default for single line input

        // Ask mode with options needs 2 lines (prompt + options)
        if self.activity == crate::types::Activity::Asking && self.waiting_for_ask && !self.ask_options.is_empty() {
            return 2;
        }

        // Multiline input: calculate based on line count
        if self.input.contains('\n') {
            let line_count = self.input.lines().count() as u16;
            // Add 1 for char count line if needed
            let extra = if self.input.chars().count() > 50 || line_count > 1 {
                1
            } else {
                0
            };
            // Cap at reasonable max (leave room for messages)
            return (line_count + extra).min(6).max(base_height);
        }

        base_height
    }

    /// Draw debug panel (separate area for debug logs)
    pub(crate) fn draw_debug_panel(&self, f: &mut ratatui::Frame, area: Rect) {
        use ratatui::{
            style::{Color, Modifier, Style},
            text::{Line, Span},
            widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
        };

        // Border with title
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .title(Span::styled(
                " 🔍 Debug Logs (Shift+D to hide, Shift+C to clear) ",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ));

        let inner_area = block.inner(area);
        f.render_widget(block, area);

        if self.debug_logs.is_empty() {
            let empty_msg = Paragraph::new("No debug logs yet...")
                .style(Style::default().fg(Color::DarkGray));
            f.render_widget(empty_msg, inner_area);
            return;
        }

        // Calculate visible lines
        let visible_lines = inner_area.height as usize;
        let total_lines = self.debug_logs.len();

        // Apply scroll offset
        let scroll_offset = self.debug_scroll_offset as usize;
        let start = scroll_offset.min(total_lines.saturating_sub(visible_lines));
        let end = (start + visible_lines).min(total_lines);

        // Build lines from debug logs
        let lines: Vec<Line> = self.debug_logs[start..end]
            .iter()
            .map(|log| {
                // Color based on category
                let color = if log.contains("API") {
                    Color::Cyan
                } else if log.contains("MEMORY") {
                    Color::Green
                } else if log.contains("TOOL") {
                    Color::Magenta
                } else if log.contains("SESSION") {
                    Color::Blue
                } else if log.contains("KEYWORDS") {
                    Color::Yellow
                } else {
                    Color::Gray
                };
                Line::from(Span::styled(log.clone(), Style::default().fg(color)))
            })
            .collect();

        let paragraph = Paragraph::new(lines);
        f.render_widget(paragraph, inner_area);

        // Scrollbar if needed
        if total_lines > visible_lines {
            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"))
                .track_symbol(Some("│"))
                .thumb_symbol("█");

            let mut scrollbar_state = ScrollbarState::new(total_lines)
                .position(scroll_offset)
                .viewport_content_length(visible_lines);

            f.render_stateful_widget(
                scrollbar,
                Rect::new(inner_area.right() - 1, inner_area.top(), 1, inner_area.height),
                &mut scrollbar_state,
            );
        }
    }
}
