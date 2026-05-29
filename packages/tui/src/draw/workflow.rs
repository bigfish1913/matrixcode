//! Workflow Panel Drawing
//!
//! Renders workflow DAG visualization panel

use crate::app::TuiApp;
use crate::types::Activity;
use crate::workflow::{DagWidget, render_progress_view};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

/// Convert status color string to ratatui Color
fn status_to_color(status_color: &str) -> Color {
    match status_color {
        "gray" => Color::Gray,
        "yellow" => Color::Yellow,
        "green" => Color::Green,
        "red" => Color::Red,
        "blue" => Color::Blue,
        _ => Color::Reset,
    }
}

impl TuiApp {
    /// Draw workflow visualization panel (overlay on right side)
    /// The panel should fill from top to bottom, excluding bottom components
    pub(crate) fn draw_workflow_panel(&self, f: &mut ratatui::Frame) {
        if !self.workflow_state.visible {
            return;
        }

        let area = f.area();
        let panel_width = 40u16.min(area.width / 3);

        // Calculate layout heights (same as main draw)
        let status_height: u16 = 1;
        let hint_height: u16 = if matches!(
            self.approve_mode,
            crate::types::ApproveMode::Ask | crate::types::ApproveMode::Auto
        ) {
            1
        } else {
            0
        };
        let gap_height: u16 = 1;
        let queue_height: u16 = if self.pending_messages.is_empty() {
            0
        } else {
            1
        };
        let activity_height: u16 = if matches!(self.activity, Activity::Thinking)
            || (self.is_tool_activity() && self.streaming.is_empty() && self.thinking.is_empty())
        {
            1
        } else {
            0
        };
        let input_height: u16 = self.calculate_input_content_height() + 2;

        // Panel height: full height minus bottom components
        let bottom_reserved = status_height
            + input_height
            + hint_height
            + gap_height
            + queue_height
            + activity_height;
        let panel_height = area.height.saturating_sub(bottom_reserved);

        // Panel starts from top (y=0), ends before bottom components
        let panel_area = Rect::new(
            area.width.saturating_sub(panel_width),
            0,
            panel_width,
            panel_height,
        );

        // Border
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(Span::styled(
                " ⚙ Workflow (Alt+W) ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));

        let inner_area = block.inner(panel_area);
        f.render_widget(block, panel_area);

        // Render DAG or progress view based on mode
        match self.workflow_state.view_mode {
            crate::workflow::WorkflowViewMode::Dag => {
                let dag = DagWidget::new(&self.workflow_state);
                f.render_widget(dag, inner_area);
            }
            crate::workflow::WorkflowViewMode::Progress => {
                // Render progress view directly to buffer
                let mut buf = ratatui::buffer::Buffer::empty(inner_area);
                render_progress_view(&self.workflow_state, inner_area, &mut buf);
                // Copy buffer content to frame
                for y in inner_area.top()..inner_area.bottom() {
                    for x in inner_area.left()..inner_area.right() {
                        if let Some(cell) = buf.cell((x, y))
                            && let Some(c) = f.buffer_mut().cell_mut((x, y))
                        {
                            *c = cell.clone();
                        }
                    }
                }
            }
            crate::workflow::WorkflowViewMode::Detail => {
                self.draw_workflow_detail(f, inner_area);
            }
        }
    }

    /// Draw workflow node detail view
    fn draw_workflow_detail(&self, f: &mut ratatui::Frame, area: Rect) {
        if self.workflow_state.selected_node.is_none() {
            let text = Paragraph::new("No node selected\n\nUse ↑↓ to select node");
            f.render_widget(text, area);
            return;
        }

        let node_id = self.workflow_state.selected_node.as_ref().unwrap();
        let def = self.workflow_state.workflow_def.as_ref();

        if let Some(def) = def {
            let node = def.nodes.iter().find(|n| n.id == *node_id);
            if let Some(node) = node {
                let status = self.workflow_state.get_node_status(node_id);

                // Build detail info
                let lines = vec![
                    Line::from(Span::styled(
                        format!("Node: {}", node.name),
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(Span::styled(
                        format!("ID: {}", node.id),
                        Style::default().fg(Color::Gray),
                    )),
                    Line::from(Span::styled(
                        format!("Type: {:?}", node.node_type),
                        Style::default().fg(Color::Gray),
                    )),
                    Line::from(Span::styled(
                        format!("Status: {}", status.icon()),
                        Style::default().fg(status_to_color(status.color())),
                    )),
                ];

                let paragraph = Paragraph::new(lines);
                f.render_widget(paragraph, area);
            }
        }
    }
}
