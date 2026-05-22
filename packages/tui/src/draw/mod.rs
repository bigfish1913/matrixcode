//! TUI drawing module.

mod helpers;
mod hint;  // New hint bar module
mod input;
mod messages;
mod status;

use crate::app::TuiApp;
use ratatui::layout::Rect;

impl TuiApp {
    pub(crate) fn draw(&self, f: &mut ratatui::Frame) {
        // Calculate heights for each component
        let total_height = f.area().height;

        // Hint bar: 1 line above input (shows command hints, tool status, etc.)
        let hint_height = if self.should_show_hint() { 1u16 } else { 0u16 };

        // Input: minimum 2 lines, expand for multiline content
        let input_lines = self.input.lines().count().max(1);
        let input_height = if input_lines <= 2 {
            2u16  // Default 2 lines
        } else {
            input_lines.min(5) as u16 + 1
        };

        // Status bar: 1 line at bottom
        let status_height = 1u16;

        // Queue indicator: 1 line if pending messages exist
        let queue_height = if self.pending_messages.is_empty() { 0u16 } else { 1u16 };

        // Gap between messages and input area (empty line for spacing)
        let gap_height = 1u16;

        // Calculate reserved space from bottom
        let reserved_height = status_height + input_height + hint_height + gap_height + queue_height;

        // Messages area gets remaining space, minimum 3 lines
        let messages_height = total_height.saturating_sub(reserved_height).max(3);

        // Calculate Y positions
        let status_y = total_height.saturating_sub(status_height);
        let input_y = status_y.saturating_sub(input_height);
        let hint_y = input_y.saturating_sub(hint_height);
        let gap_y = hint_y.saturating_sub(gap_height);
        let queue_y = gap_y.saturating_sub(queue_height);

        // Create areas with proper bounds
        let messages_area = Rect::new(f.area().x, f.area().y, f.area().width, messages_height);
        let queue_area = Rect::new(f.area().x, queue_y, f.area().width, queue_height);
        let hint_area = Rect::new(f.area().x, hint_y, f.area().width, hint_height);
        let input_area = Rect::new(f.area().x, input_y, f.area().width, input_height);
        let status_area = Rect::new(f.area().x, status_y, f.area().width, status_height);

        // Render components in order (bottom to top ensures proper layering)
        self.draw_status(f, status_area);
        self.draw_input(f, input_area);
        if hint_height > 0 {
            self.draw_hint(f, hint_area);
        }
        if !self.pending_messages.is_empty() && queue_height > 0 {
            self.draw_queue(f, queue_area);
        }
        // Messages area last - largest area
        self.draw_messages(f, messages_area);
    }
}
