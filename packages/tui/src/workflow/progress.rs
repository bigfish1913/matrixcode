//! Compact Progress View
//!
//! Alternative compact display for workflow execution progress

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Style, Color},
};
use crate::workflow::types::{WorkflowViewState, node_type_icon};

/// Render compact progress view
pub fn render_progress_view(state: &WorkflowViewState, area: Rect, buf: &mut Buffer) {
    if state.workflow_def.is_none() {
        buf.set_string(area.x, area.y, "No workflow running", Style::default());
        return;
    }

    let def = state.workflow_def.as_ref().unwrap();

    // Title line
    let status_label = if let Some(ctx) = &state.context {
        match ctx.status {
            matrixcode_core::workflow::WorkflowStatus::Running => "⟳ running",
            matrixcode_core::workflow::WorkflowStatus::Completed => "✓ completed",
            matrixcode_core::workflow::WorkflowStatus::Failed => "✗ failed",
            matrixcode_core::workflow::WorkflowStatus::Paused => "⏸ paused",
            _ => "○ pending",
        }
    } else {
        "○ pending"
    };

    let title = format!("{} [{}]", def.name, status_label);
    let title_len = title.chars().count() as u16;
    buf.set_string(area.x, area.y, &title, Style::default().fg(Color::White).bold());

    // Progress bar (if we have width)
    if area.width > 30 {
        let (completed, total) = state.progress();
        let bar_start = area.x + title_len + 2;
        let bar_width = area.width.saturating_sub(bar_start - area.x).saturating_sub(10);

        if bar_width > 0 {
            let filled = if total > 0 {
                ((bar_width as usize) * completed) / total
            } else {
                0
            };

            buf.set_string(bar_start, area.y, " [", Style::default().fg(Color::Gray));
            for i in 0..bar_width as usize {
                let ch = if i < filled { "█" } else { "░" };
                let color = if i < filled { Color::Green } else { Color::Gray };
                buf.set_string(bar_start + 1 + i as u16, area.y, ch, Style::default().fg(color));
            }
            buf.set_string(bar_start + 1 + bar_width, area.y, "] ", Style::default().fg(Color::Gray));
            buf.set_string(bar_start + 3 + bar_width, area.y, format!("{}/{}", completed, total), Style::default().fg(Color::Gray));
        }
    }

    // Node status line
    if area.height > 1 {
        let node_y = area.y + 1;
        let mut x = area.x;

        buf.set_string(x, node_y, "Nodes: ", Style::default().fg(Color::Gray));
        x += 7;

        for node in &def.nodes {
            if x >= area.right() {
                break;
            }
            let status = state.get_node_status(&node.id);
            let type_icon = node_type_icon(&node.node_type);
            let status_icon = status.icon();

            let color = match status.color() {
                "gray" => Color::Gray,
                "yellow" => Color::Yellow,
                "green" => Color::Green,
                "red" => Color::Red,
                "blue" => Color::Blue,
                _ => Color::Reset,
            };

            buf.set_string(x, node_y, format!("{}{}", type_icon, status_icon), Style::default().fg(color));
            x += 3;
        }
    }
}

/// Render inline progress indicator (for status line)
pub fn render_inline_progress(state: &WorkflowViewState, buf: &mut Buffer, x: u16, y: u16) -> u16 {
    if state.workflow_def.is_none() {
        return x;
    }

    let (completed, total) = state.progress();
    let text = format!(" wf:{}/{:02}", completed, total);
    let text_len = text.len() as u16;

    let color = if completed == total && total > 0 {
        Color::Green
    } else if let Some(ctx) = &state.context {
        match ctx.status {
            matrixcode_core::workflow::WorkflowStatus::Failed => Color::Red,
            matrixcode_core::workflow::WorkflowStatus::Running => Color::Yellow,
            _ => Color::Gray,
        }
    } else {
        Color::Gray
    };

    buf.set_string(x, y, &text, Style::default().fg(color));
    x + text_len
}