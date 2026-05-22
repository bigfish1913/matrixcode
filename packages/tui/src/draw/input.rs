//! Input area and queue rendering.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::TuiApp;
use crate::types::Activity;
use crate::utils::{truncate, truncate_visual, truncate_visual_end};

impl TuiApp {
    pub(crate) fn draw_queue(&self, f: &mut ratatui::Frame, area: Rect) {
        let mut spans: Vec<Span> = vec![
            Span::styled("⏳ ", Style::default().fg(Color::Magenta)),
            Span::styled(
                format!("Queue ({}): ", self.pending_messages.len()),
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
        ];

        for (i, msg) in self.pending_messages.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
            }
            let preview = msg.lines().next().unwrap_or("");
            let truncated = truncate(preview, 30);
            spans.push(Span::styled(
                format!("\"{}\"", truncated),
                Style::default().fg(Color::Yellow),
            ));
        }

        f.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    pub(crate) fn draw_input(&self, f: &mut ratatui::Frame, area: Rect) {
        let (prompt, prompt_color) = match self.activity {
            Activity::Idle => ("❯ ", Color::Yellow),
            Activity::Asking => ("⚡ ", Color::Red),
            _ => ("⏳ ", Color::Gray),
        };

        let max_w = (area.width as usize).saturating_sub(4);

        // History mode indicator
        let history_indicator = if self.history_index.is_some() {
            "📜 "
        } else {
            ""
        };

        // Ask mode handling
        if self.activity == Activity::Asking && self.waiting_for_ask {
            let mut lines: Vec<Line> = Vec::new();

            // First line: prompt and instructions
            let mut spans: Vec<Span> = vec![Span::styled(
                prompt,
                Style::default()
                    .fg(prompt_color)
                    .add_modifier(Modifier::BOLD),
            )];

            if !self.ask_options.is_empty() {
                spans.push(Span::styled(
                    if self.ask_multi_select {
                        "↑↓ navigate  Space toggle  Enter confirm"
                    } else {
                        "↑↓ select  Enter confirm"
                    },
                    Style::default().fg(Color::DarkGray),
                ));
                lines.push(Line::from(spans));

                // Second line: show options with selection indicator
                let mut option_spans: Vec<Span> = Vec::new();
                option_spans.push(Span::styled("  ", Style::default()));

                for (i, opt) in self.ask_options.iter().enumerate() {
                    let is_selected = self.ask_selected_index == i;

                    let marker = if is_selected { "▸" } else { " " };
                    let color = if is_selected { Color::Cyan } else { Color::Gray };

                    if i > 0 {
                        option_spans.push(Span::styled("  ", Style::default()));
                    }
                    option_spans.push(Span::styled(
                        format!("{} ", marker),
                        Style::default().fg(color),
                    ));

                    // Show option label with description if available
                    let label_text = if let Some(desc) = &opt.description {
                        format!("{} - {}", opt.label, truncate(desc, 30))
                    } else {
                        opt.label.clone()
                    };
                    option_spans.push(Span::styled(
                        label_text,
                        Style::default().fg(if is_selected { Color::White } else { Color::Gray }),
                    ));
                }
                lines.push(Line::from(option_spans));
            } else {
                // Free text input mode - show user input with cursor
                if self.input.is_empty() {
                    spans.push(Span::styled("▌", Style::default().fg(Color::Cyan)));
                    spans.push(Span::styled(
                        "Type y/n, Enter to submit  ESC abort",
                        Style::default().fg(Color::DarkGray),
                    ));
                } else {
                    spans.push(Span::styled(
                        self.input.clone(),
                        Style::default().fg(Color::White),
                    ));
                    spans.push(Span::styled("▌", Style::default().fg(Color::Cyan)));
                    spans.push(Span::styled(
                        "  Enter to submit  ESC abort",
                        Style::default().fg(Color::DarkGray),
                    ));
                }
                lines.push(Line::from(spans));
            }

            f.render_widget(Paragraph::new(lines), area);
            return;
        }

        let is_multiline = self.input.contains('\n');

        // Show queue indicator when AI is processing and user is typing
        let queue_hint = if self.activity != Activity::Idle
            && self.activity != Activity::Asking
            && !self.input.is_empty()
        {
            let queue_count = self.pending_messages.len();
            if queue_count > 0 {
                format!(" [queue: {}]", queue_count + 1)
            } else {
                " [will queue]".to_string()
            }
        } else {
            String::new()
        };

        if !is_multiline {
            let mut spans: Vec<Span> = vec![Span::styled(
                prompt,
                Style::default()
                    .fg(prompt_color)
                    .add_modifier(Modifier::BOLD),
            )];

            // History mode indicator
            if self.history_index.is_some() {
                spans.push(Span::styled(
                    history_indicator,
                    Style::default().fg(Color::DarkGray),
                ));
            }

            if self.input.is_empty() {
                spans.push(Span::styled("▌", Style::default().fg(Color::Cyan)));
                // Show helpful shortcuts hints
                if self.history_index.is_some() {
                    spans.push(Span::styled(
                        "↑↓ navigate  Enter use  Esc back",
                        Style::default().fg(Color::DarkGray),
                    ));
                } else {
                    spans.push(Span::styled(
                        " Ask anything... ",
                        Style::default().fg(Color::DarkGray),
                    ));
                    spans.push(Span::styled(
                        "(Ctrl+V paste │ ↑↓ history │ Shift+Enter newline)",
                        Style::default().fg(Color::DarkGray),
                    ));
                }
            } else {
                let display_width = max_w.saturating_sub(15);
                let before_cursor = &self.input[..self.cursor_pos];
                let after_cursor = &self.input[self.cursor_pos..];

                let before_vis_width: usize = before_cursor
                    .chars()
                    .map(|c| if c > '\u{7F}' { 2 } else { 1 })
                    .sum();
                let after_vis_width: usize = after_cursor
                    .chars()
                    .map(|c| if c > '\u{7F}' { 2 } else { 1 })
                    .sum();

                if before_vis_width + after_vis_width <= display_width {
                    spans.push(Span::styled(
                        before_cursor.to_string(),
                        Style::default().fg(Color::White),
                    ));
                    spans.push(Span::styled("▌", Style::default().fg(Color::Cyan)));
                    spans.push(Span::styled(
                        after_cursor.to_string(),
                        Style::default().fg(Color::White),
                    ));
                    if !queue_hint.is_empty() {
                        spans.push(Span::styled(queue_hint, Style::default().fg(Color::Yellow)));
                    }
                } else if before_vis_width < display_width {
                    spans.push(Span::styled(
                        before_cursor.to_string(),
                        Style::default().fg(Color::White),
                    ));
                    spans.push(Span::styled("▌", Style::default().fg(Color::Cyan)));
                    let remaining = display_width.saturating_sub(before_vis_width);
                    let truncated_after = truncate_visual(after_cursor, remaining);
                    spans.push(Span::styled(
                        truncated_after,
                        Style::default().fg(Color::White),
                    ));
                } else {
                    let start_width = display_width.saturating_sub(10);
                    let truncated_before = truncate_visual_end(before_cursor, start_width);
                    spans.push(Span::styled(
                        format!("…{}", truncated_before),
                        Style::default().fg(Color::White),
                    ));
                    spans.push(Span::styled("▌", Style::default().fg(Color::Cyan)));
                    let remaining = display_width.saturating_sub(start_width + 1);
                    let truncated_after = truncate_visual(after_cursor, remaining);
                    spans.push(Span::styled(
                        truncated_after,
                        Style::default().fg(Color::White),
                    ));
                }
            }

            f.render_widget(Paragraph::new(Line::from(spans)), area);
        } else {
            // Multiline mode
            let mut lines: Vec<Line> = Vec::new();
            let input_lines: Vec<&str> = self.input.split('\n').collect();
            let cursor_line = self.input[..self.cursor_pos].matches('\n').count();
            let cursor_col_byte = self.input[..self.cursor_pos]
                .rfind('\n')
                .map(|i| self.cursor_pos - i - 1)
                .unwrap_or(self.cursor_pos);

            let total_lines_count = input_lines.len();
            let max_display_lines = (area.height as usize).saturating_sub(1);

            for (i, line) in input_lines.iter().enumerate().take(max_display_lines) {
                let line_prompt = if i == 0 { prompt } else { "  " };
                let line_prompt_color = if i == 0 {
                    prompt_color
                } else {
                    Color::DarkGray
                };

                // Add line number indicator for multiline
                let line_num_hint = if i == cursor_line && total_lines_count > 1 {
                    format!("({}/{}) ", i + 1, total_lines_count)
                } else {
                    String::new()
                };

                if i == cursor_line {
                    let before = &line[..cursor_col_byte.min(line.len())];
                    let after = &line[cursor_col_byte.min(line.len())..];
                    lines.push(Line::from(vec![
                        Span::styled(
                            line_prompt,
                            Style::default()
                                .fg(line_prompt_color)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(line_num_hint, Style::default().fg(Color::DarkGray)),
                        Span::styled(before.to_string(), Style::default().fg(Color::White)),
                        Span::styled("▌", Style::default().fg(Color::Cyan)),
                        Span::styled(after.to_string(), Style::default().fg(Color::White)),
                    ]));
                } else {
                    lines.push(Line::from(vec![
                        Span::styled(
                            line_prompt,
                            Style::default()
                                .fg(line_prompt_color)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(truncate(line, max_w), Style::default().fg(Color::White)),
                    ]));
                }
            }

            let total_lines = input_lines.len();
            if total_lines > max_display_lines {
                lines.push(Line::styled(
                    format!("  … ({}/{} lines)", max_display_lines, total_lines),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            // Show character count at the bottom for multiline input
            let char_count = self.input.chars().count();
            if char_count > 50 || total_lines_count > 1 {
                lines.push(Line::styled(
                    format!("  {} chars, {} lines", char_count, total_lines_count),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            f.render_widget(Paragraph::new(lines), area);
        }
    }
}
