//! Unicode DAG Renderer
//!
//! Renders workflow DAG using Unicode box-drawing characters

use crate::workflow::types::{NodeVisualStatus, WorkflowViewState, node_type_icon};
use matrixcode_core::workflow::NodeType;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::Widget,
};

/// DAG rendering constants
const NODE_WIDTH: u16 = 16;
const NODE_HEIGHT: u16 = 5;
const SPACING_X: u16 = 2;
const SPACING_Y: u16 = 1;

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

        // Center the DAG in available area
        let total_height = (self.state.layout.height as u16) * (NODE_HEIGHT + SPACING_Y);
        let total_width = (self.state.layout.width as u16) * (NODE_WIDTH + SPACING_X);

        let start_y = if area.height > total_height {
            (area.height - total_height) / 2
        } else {
            0
        };

        let start_x = if area.width > total_width + 2 {
            (area.width - total_width) / 2
        } else {
            1
        };

        // Render each node
        for node in &def.nodes {
            let pos = self.state.layout.node_positions.get(&node.id);
            if let Some((row, col)) = pos {
                let x = area.x + start_x + (*col as u16) * (NODE_WIDTH + SPACING_X);
                let y = start_y + (*row as u16) * (NODE_HEIGHT + SPACING_Y);

                // Ensure within bounds
                if x < area.right() && y < area.bottom() {
                    let node_rect = Rect::new(x, y, NODE_WIDTH, NODE_HEIGHT);
                    self.render_node(&node.id, &node.name, &node.node_type, node_rect, buf);
                }
            }
        }

        // Render edges
        for edge in &self.state.layout.edges {
            self.render_edge(&edge.from, &edge.to, area, buf, start_y, start_x);
        }

        // Render progress info at bottom
        let (completed, total) = self.state.progress();
        let progress_text = format!("Progress: {}/{} nodes", completed, total);
        let progress_y = area.bottom().saturating_sub(1);
        if progress_y > area.y {
            buf.set_string(
                area.x,
                progress_y,
                progress_text,
                Style::default().fg(Color::Gray),
            );
        }
    }

    fn render_node(
        &self,
        id: &str,
        name: &str,
        node_type: &NodeType,
        rect: Rect,
        buf: &mut Buffer,
    ) {
        let status = self.state.get_node_status(id);

        // Determine colors based on status
        let (border_color, text_color) = match &status {
            NodeVisualStatus::Pending => (Color::Gray, Color::Gray),
            NodeVisualStatus::Running => (Color::Yellow, Color::Yellow),
            NodeVisualStatus::Completed => (Color::Green, Color::Green),
            NodeVisualStatus::Failed { .. } => (Color::Red, Color::Red),
            NodeVisualStatus::Skipped => (Color::Blue, Color::Blue),
        };

        // Draw box borders
        let box_chars = if matches!(status, NodeVisualStatus::Running) {
            ("╔", "╗", "╚", "╝", "║", "═")
        } else {
            ("┌", "┐", "└", "┘", "│", "─")
        };

        let width = rect.width.saturating_sub(1);
        let height = rect.height;

        // Top border
        buf.set_string(
            rect.x,
            rect.y,
            box_chars.0,
            Style::default().fg(border_color),
        );
        for x in rect.x + 1..rect.x + width {
            buf.set_string(x, rect.y, box_chars.5, Style::default().fg(border_color));
        }
        buf.set_string(
            rect.x + width,
            rect.y,
            box_chars.1,
            Style::default().fg(border_color),
        );

        // Content lines (support dynamic height)
        let icon = node_type_icon(node_type);
        let status_icon = status.icon();
        let spinner = if matches!(status, NodeVisualStatus::Running) {
            self.state.spinner_char().to_string()
        } else {
            " ".to_string()
        };

        // Line 1: Icon + Status + Spinner
        let line1 = format!("{} {}{}", icon, status_icon, spinner);
        let display_line1 = truncate(&line1, width.saturating_sub(2) as usize);
        buf.set_string(
            rect.x + 1,
            rect.y + 1,
            &display_line1,
            Style::default().fg(text_color),
        );
        buf.set_string(
            rect.x,
            rect.y + 1,
            box_chars.4,
            Style::default().fg(border_color),
        );
        buf.set_string(
            rect.x + width,
            rect.y + 1,
            box_chars.4,
            Style::default().fg(border_color),
        );

        // Line 2: Node name
        let display_name = truncate(name, width.saturating_sub(2) as usize);
        buf.set_string(
            rect.x + 1,
            rect.y + 2,
            &display_name,
            Style::default().fg(Color::White),
        );
        buf.set_string(
            rect.x,
            rect.y + 2,
            box_chars.4,
            Style::default().fg(border_color),
        );
        buf.set_string(
            rect.x + width,
            rect.y + 2,
            box_chars.4,
            Style::default().fg(border_color),
        );

        // Additional lines for larger boxes (empty with borders)
        for line in 3..height.saturating_sub(1) {
            buf.set_string(
                rect.x,
                rect.y + line,
                box_chars.4,
                Style::default().fg(border_color),
            );
            buf.set_string(
                rect.x + width,
                rect.y + line,
                box_chars.4,
                Style::default().fg(border_color),
            );
        }

        // Bottom border
        let bottom_y = rect.y + height.saturating_sub(1);
        buf.set_string(
            rect.x,
            bottom_y,
            box_chars.2,
            Style::default().fg(border_color),
        );
        for x in rect.x + 1..rect.x + width {
            buf.set_string(x, bottom_y, box_chars.5, Style::default().fg(border_color));
        }
        buf.set_string(
            rect.x + width,
            bottom_y,
            box_chars.3,
            Style::default().fg(border_color),
        );
    }

    fn render_edge(
        &self,
        from_id: &str,
        to_id: &str,
        area: Rect,
        buf: &mut Buffer,
        start_y: u16,
        start_x: u16,
    ) {
        let from_pos = self.state.layout.node_positions.get(from_id);
        let to_pos = self.state.layout.node_positions.get(to_id);

        if let (Some((from_row, from_col)), Some((to_row, to_col))) = (from_pos, to_pos) {
            // From node bottom center
            let from_x =
                area.x + start_x + (*from_col as u16) * (NODE_WIDTH + SPACING_X) + NODE_WIDTH / 2;
            let from_y = start_y + (*from_row as u16) * (NODE_HEIGHT + SPACING_Y) + NODE_HEIGHT;

            // To node top center
            let to_x =
                area.x + start_x + (*to_col as u16) * (NODE_WIDTH + SPACING_X) + NODE_WIDTH / 2;
            let to_y = start_y + (*to_row as u16) * (NODE_HEIGHT + SPACING_Y);

            // Draw edge based on relative positions
            if from_col == to_col {
                // Same column: direct vertical line
                if from_y < area.bottom() {
                    buf.set_string(from_x, from_y, "│", Style::default().fg(Color::Gray));
                }
                if from_y + 1 < area.bottom() && from_y < to_y {
                    buf.set_string(from_x, from_y + 1, "▼", Style::default().fg(Color::Gray));
                }
            } else {
                // Different columns: need horizontal segment
                // Draw from bottom of source node going down
                if from_y < area.bottom() {
                    buf.set_string(from_x, from_y, "│", Style::default().fg(Color::Gray));
                }

                // Draw horizontal line at the middle point
                let mid_y = from_y + 1;
                if mid_y < area.bottom() && mid_y < to_y {
                    // Draw horizontal line from from_x to to_x
                    let (start_x, end_x) = if from_x < to_x {
                        (from_x, to_x)
                    } else {
                        (to_x, from_x)
                    };
                    for x in start_x..=end_x {
                        if x < area.right() {
                            buf.set_string(x, mid_y, "─", Style::default().fg(Color::Gray));
                        }
                    }
                    // Corners
                    if from_x < to_x {
                        if from_x < area.right() {
                            buf.set_string(from_x, mid_y, "┐", Style::default().fg(Color::Gray));
                        }
                        if to_x < area.right() {
                            buf.set_string(to_x, mid_y, "┌", Style::default().fg(Color::Gray));
                        }
                    } else {
                        if from_x < area.right() {
                            buf.set_string(from_x, mid_y, "┘", Style::default().fg(Color::Gray));
                        }
                        if to_x < area.right() {
                            buf.set_string(to_x, mid_y, "└", Style::default().fg(Color::Gray));
                        }
                    }
                }

                // Draw vertical line to target node
                for y in (mid_y + 1)..to_y.min(area.bottom()) {
                    buf.set_string(to_x, y, "│", Style::default().fg(Color::Gray));
                }

                // Arrow at target
                if to_y < area.bottom() {
                    buf.set_string(to_x, to_y, "▼", Style::default().fg(Color::Gray));
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
        s.chars()
            .take(max_len.saturating_sub(1))
            .collect::<String>()
            + "…"
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
            matrixcode_core::workflow::WorkflowStatus::Running => "▶ running",
            matrixcode_core::workflow::WorkflowStatus::Completed => "✓ completed",
            matrixcode_core::workflow::WorkflowStatus::Failed => "✗ failed",
            matrixcode_core::workflow::WorkflowStatus::Paused => "⏸ paused",
            _ => "○ pending",
        }
    } else {
        "○ pending"
    };

    // Title line
    let title = format!("{} [{}]", workflow_name, status_text);
    buf.set_string(
        area.x,
        area.y,
        title,
        Style::default()
            .fg(Color::White)
            .add_modifier(ratatui::style::Modifier::BOLD),
    );

    // Progress bar
    let bar_width = area.width.saturating_sub(22);
    let filled = if total > 0 {
        (bar_width as usize * completed) / total
    } else {
        0
    };

    let bar_y = area.y + 1;
    buf.set_string(area.x, bar_y, "[", Style::default().fg(Color::Gray));
    for i in 0..bar_width as usize {
        let ch = if i < filled { "█" } else { "░" };
        let color = if i < filled {
            Color::Green
        } else {
            Color::Gray
        };
        buf.set_string(area.x + 1 + i as u16, bar_y, ch, Style::default().fg(color));
    }
    buf.set_string(
        area.x + 1 + bar_width,
        bar_y,
        "]",
        Style::default().fg(Color::Gray),
    );
    buf.set_string(
        area.x + 2 + bar_width,
        bar_y,
        format!(
            " {}%",
            if total > 0 {
                completed * 100 / total
            } else {
                0
            }
        ),
        Style::default().fg(Color::Gray),
    );

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
        buf.set_string(
            x,
            strip_y,
            format!("{}{}", icon, status_icon),
            Style::default().fg(color),
        );
        x += 4;
    }
}
