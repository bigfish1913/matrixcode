//! Unicode DAG Renderer
//!
//! Renders workflow DAG using Unicode box-drawing characters

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Style, Color},
    text::Line,
    widgets::Widget,
};
use matrixcode_core::workflow::NodeType;
use crate::workflow::types::{WorkflowViewState, node_type_icon, NodeVisualStatus};

/// DAG Widget for rendering workflow graph
pub struct DagWidget<'a> {
    state: &'a WorkflowViewState,
}

impl<'a> DagWidget<'a> {
    pub fn new(state: &'a WorkflowViewState) -> Self {
        Self { state }
    }
}

impl<'a> Widget for DagWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.state.workflow_def.is_none() {
            // No workflow loaded
            let text = Line::from("No workflow running");
            text.render(area, buf);
            return;
        }

        // Render nodes and edges
        self.render_dag(area, buf);
    }
}

impl<'a> DagWidget<'a> {
    fn render_dag(&self, area: Rect, buf: &mut Buffer) {
        let def = self.state.workflow_def.as_ref().unwrap();

        // Calculate rendering positions
        let node_width = 10u16;  // Width of each node box
        let node_height = 3u16; // Height of each node box
        let spacing_x = 4u16;   // Horizontal spacing
        let spacing_y = 2u16;   // Vertical spacing

        // Center the DAG in available area
        let total_height = (self.state.layout.height as u16) * (node_height + spacing_y);
        let start_y = if area.height > total_height {
            (area.height - total_height) / 2
        } else {
            0
        };

        // Render each node
        for node in &def.nodes {
            let pos = self.state.layout.node_positions.get(&node.id);
            if let Some((row, col)) = pos {
                let x = area.x + 2 + (*col as u16) * (node_width + spacing_x);
                let y = start_y + (*row as u16) * (node_height + spacing_y);

                // Ensure within bounds
                if x < area.right() && y < area.bottom() {
                    let node_rect = Rect::new(x, y, node_width, node_height);
                    self.render_node(&node.id, &node.name, &node.node_type, node_rect, buf);
                }
            }
        }

        // Render edges
        for edge in &self.state.layout.edges {
            self.render_edge(&edge.from, &edge.to, area, buf);
        }

        // Render progress info at bottom
        let (completed, total) = self.state.progress();
        let progress_text = format!("Progress: {}/{} nodes", completed, total);
        let progress_y = area.bottom().saturating_sub(1);
        if progress_y > area.y {
            buf.set_string(area.x, progress_y, progress_text, Style::default().fg(Color::Gray));
        }
    }

    fn render_node(&self, id: &str, name: &str, node_type: &NodeType, rect: Rect, buf: &mut Buffer) {
        let status = self.state.get_node_status(id);

        // Determine colors based on status
        let (border_color, fill_color) = match &status {
            NodeVisualStatus::Pending => (Color::Gray, Color::Reset),
            NodeVisualStatus::Running => (Color::Yellow, Color::Reset),
            NodeVisualStatus::Completed => (Color::Green, Color::Reset),
            NodeVisualStatus::Failed { .. } => (Color::Red, Color::Reset),
            NodeVisualStatus::Skipped => (Color::Blue, Color::Reset),
        };

        // Draw box borders
        let box_chars = if matches!(status, NodeVisualStatus::Running) {
            // Animated border for running nodes
            ("╔", "╗", "╚", "╝", "║", "═")
        } else {
            ("┌", "┐", "└", "┘", "│", "─")
        };

        // Top border
        buf.set_string(rect.x, rect.y, box_chars.0, Style::default().fg(border_color));
        for x in rect.x + 1..rect.x + rect.width.saturating_sub(1) {
            buf.set_string(x, rect.y, box_chars.5, Style::default().fg(border_color));
        }
        buf.set_string(rect.x + rect.width.saturating_sub(1), rect.y, box_chars.1, Style::default().fg(border_color));

        // Middle content
        let _icon = node_type_icon(node_type);
        let status_icon = status.icon();
        let spinner = if matches!(status, NodeVisualStatus::Running) {
            self.state.spinner_char().to_string()
        } else {
            " ".to_string()
        };

        // Truncate name if too long
        let display_name = truncate(name, rect.width.saturating_sub(4) as usize);
        let content = format!("{}{} {}", status_icon, spinner, display_name);
        buf.set_string(rect.x + 1, rect.y + 1, content, Style::default().fg(fill_color));

        // Vertical borders
        buf.set_string(rect.x, rect.y + 1, box_chars.4, Style::default().fg(border_color));
        buf.set_string(rect.x + rect.width.saturating_sub(1), rect.y + 1, box_chars.4, Style::default().fg(border_color));

        // Bottom border
        buf.set_string(rect.x, rect.y + 2, box_chars.2, Style::default().fg(border_color));
        for x in rect.x + 1..rect.x + rect.width.saturating_sub(1) {
            buf.set_string(x, rect.y + 2, box_chars.5, Style::default().fg(border_color));
        }
        buf.set_string(rect.x + rect.width.saturating_sub(1), rect.y + 2, box_chars.3, Style::default().fg(border_color));
    }

    fn render_edge(&self, from_id: &str, to_id: &str, area: Rect, buf: &mut Buffer) {
        let from_pos = self.state.layout.node_positions.get(from_id);
        let to_pos = self.state.layout.node_positions.get(to_id);

        if let (Some((from_row, from_col)), Some((to_row, to_col))) = (from_pos, to_pos) {
            // Calculate pixel positions (center of nodes)
            let node_width = 10u16;
            let node_height = 3u16;
            let spacing_x = 4u16;
            let spacing_y = 2u16;

            let start_y = if area.height > (self.state.layout.height as u16) * (node_height + spacing_y) {
                (area.height - (self.state.layout.height as u16) * (node_height + spacing_y)) / 2
            } else {
                0
            };

            // From node bottom center
            let from_x = area.x + 2 + (*from_col as u16) * (node_width + spacing_x) + node_width / 2;
            let from_y = start_y + (*from_row as u16) * (node_height + spacing_y) + node_height;

            // To node top center
            let _to_x = area.x + 2 + (*to_col as u16) * (node_width + spacing_x) + node_width / 2;
            let to_y = start_y + (*to_row as u16) * (node_height + spacing_y);

            // Draw vertical line and arrow
            if from_row + 1 == *to_row {
                // Direct vertical connection
                buf.set_string(from_x, from_y, "│", Style::default().fg(Color::Gray));
                buf.set_string(from_x, from_y + 1, "↓", Style::default().fg(Color::Gray));
            } else {
                // Longer connection - draw line segments
                for y in from_y..to_y {
                    if y < area.bottom() && y >= area.y {
                        buf.set_string(from_x, y, "│", Style::default().fg(Color::Gray));
                    }
                }
                if to_y < area.bottom() {
                    buf.set_string(from_x, to_y, "↓", Style::default().fg(Color::Gray));
                }
            }
        }
    }
}

/// Truncate string to fit width
fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        s.chars().take(max_len.saturating_sub(1)).collect::<String>() + "…"
    }
}

/// Render compact progress view
pub fn render_progress(state: &WorkflowViewState, area: Rect, buf: &mut Buffer) {
    if state.workflow_def.is_none() {
        return;
    }

    let def = state.workflow_def.as_ref().unwrap();

    // Header
    let workflow_name = def.name.as_str();
    let (completed, total) = state.progress();
    let status_text = if let Some(ctx) = &state.context {
        match ctx.status {
            matrixcode_core::workflow::WorkflowStatus::Running => "running",
            matrixcode_core::workflow::WorkflowStatus::Completed => "completed",
            matrixcode_core::workflow::WorkflowStatus::Failed => "failed",
            matrixcode_core::workflow::WorkflowStatus::Paused => "paused",
            _ => "pending",
        }
    } else {
        "pending"
    };

    // Title line
    let title = format!("Workflow: {} [{}]", workflow_name, status_text);
    buf.set_string(area.x, area.y, title, Style::default().fg(Color::White));

    // Progress bar
    let bar_width = area.width.saturating_sub(20);
    let filled = if total > 0 {
        (bar_width as usize * completed) / total
    } else {
        0
    };

    let bar_y = area.y + 1;
    buf.set_string(area.x, bar_y, "[", Style::default().fg(Color::Gray));
    for i in 0..bar_width as usize {
        let ch = if i < filled { "█" } else { "░" };
        let color = if i < filled { Color::Green } else { Color::Gray };
        buf.set_string(area.x + 1 + i as u16, bar_y, ch, Style::default().fg(color));
    }
    buf.set_string(area.x + 1 + bar_width, bar_y, "]", Style::default().fg(Color::Gray));
    buf.set_string(area.x + 2 + bar_width, bar_y, format!(" {}%", if total > 0 { completed * 100 / total } else { 0 }), Style::default().fg(Color::Gray));

    // Node status strip
    let strip_y = area.y + 2;
    let mut x = area.x;
    for node in &def.nodes {
        if x >= area.right() {
            break;
        }
        let status = state.get_node_status(&node.id);
        let icon = node_type_icon(&node.node_type);
        let status_icon = status.icon();
        let color = match status.color() {
            "gray" => Color::Gray,
            "yellow" => Color::Yellow,
            "green" => Color::Green,
            "red" => Color::Red,
            "blue" => Color::Blue,
            _ => Color::Reset,
        };
        buf.set_string(x, strip_y, format!("{}{}", icon, status_icon), Style::default().fg(color));
        x += 4;
    }
}