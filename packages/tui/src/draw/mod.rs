//! TUI drawing module.

mod helpers;
mod hint;  // New hint bar module
mod input;
mod messages;
mod status;

use crate::app::TuiApp;
use crate::types::Activity;
use ratatui::layout::Rect;

impl TuiApp {
    pub(crate) fn draw(&self, f: &mut ratatui::Frame) {
        let total_height = f.area().height;
        let width = f.area().width;

        // Fixed heights for bottom components
        let status_height: u16 = 1;
        let hint_height: u16 = if self.should_show_hint() { 1 } else { 0 };
        let gap_height: u16 = 1;
        let queue_height: u16 = if self.pending_messages.is_empty() { 0 } else { 1 };

        // Activity indicator height: 1 line when thinking/running, 0 otherwise
        let activity_height: u16 = if matches!(self.activity, Activity::Thinking)
            || (self.is_tool_activity() && self.streaming.is_empty() && self.thinking.is_empty()) {
            1
        } else {
            0
        };

        // Dynamic input height based on content
        let input_height: u16 = self.calculate_input_height();

        // Calculate reserved height from bottom
        let reserved = status_height + input_height + hint_height + gap_height + queue_height + activity_height;

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

        // Create areas - messages area starts from top (y=0)
        let messages_area = Rect::new(0, 0, width, messages_height);
        let queue_area = Rect::new(0, queue_y, width, queue_height);
        let activity_area = Rect::new(0, activity_y, width, activity_height);
        let hint_area = Rect::new(0, hint_y, width, hint_height);
        let input_area = Rect::new(0, input_y, width, input_height);
        let status_area = Rect::new(0, status_y, width, status_height);

        // Render in order: messages first (top), then bottom components
        self.draw_messages(f, messages_area);
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

        let elapsed = self
            .request_start
            .map(|s| format!(" {:.1}s", s.elapsed().as_secs_f64()))
            .unwrap_or_default();
        let spinner_frame = self.frame % SPINNER.len();

        let line = if self.activity == Activity::Thinking {
            Line::from(vec![
                Span::styled(
                    format!("{} ", SPINNER[spinner_frame]),
                    Style::default().fg(Color::LightGreen),
                ),
                Span::styled("💭 ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("Thinking{}", elapsed),
                    Style::default().fg(Color::DarkGray),
                ),
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
}
