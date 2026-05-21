//! TUI drawing module.

mod helpers;
mod status;
mod input;
mod messages;

use ratatui::layout::Rect;
use crate::app::TuiApp;

impl TuiApp {
    pub(crate) fn draw(&self, f: &mut ratatui::Frame) {
        // Dynamic input height: expand for multiline content
        let input_lines = self.input.lines().count().max(1);
        let input_height = if input_lines <= 1 {
            1u16
        } else {
            input_lines.min(5) as u16 + 1
        };

        // Calculate fixed bottom elements first (from bottom to top)
        // This ensures input box position is stable for IME candidate window
        let total_height = f.area().height;
        let status_height = 1u16;
        let queue_height_actual = if self.pending_messages.is_empty() { 0u16 } else { 1u16 };

        // Input at bottom, fixed position
        let input_y = total_height.saturating_sub(input_height);
        // Status above input
        let status_y = input_y.saturating_sub(status_height);
        // Queue above status (if any)
        let queue_y = status_y.saturating_sub(queue_height_actual);
        // Messages fill remaining space from top
        let messages_height = queue_y;

        // Create fixed-position areas instead of dynamic layout
        let messages_area = Rect::new(f.area().x, f.area().y, f.area().width, messages_height);
        let queue_area = Rect::new(f.area().x, queue_y, f.area().width, queue_height_actual);
        let status_area = Rect::new(f.area().x, status_y, f.area().width, status_height);
        let input_area = Rect::new(f.area().x, input_y, f.area().width, input_height);

        self.draw_messages(f, messages_area);
        if !self.pending_messages.is_empty() {
            self.draw_queue(f, queue_area);
        }
        self.draw_status(f, status_area);
        self.draw_input(f, input_area);
    }
}