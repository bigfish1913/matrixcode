//! Input area and queue rendering.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::BORDER_PADDING;
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
        let frame_style = Style::default().fg(Color::DarkGray);
        let border = "─".repeat(area.width as usize);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(border.clone(), frame_style))),
            Rect::new(area.x, area.y, area.width, 1),
        );
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(border, frame_style))),
            Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1),
        );

        let area = Rect::new(
            area.x,
            area.y.saturating_add(1),
            area.width,
            area.height.saturating_sub(2),
        );
        if area.height == 0 {
            return;
        }

        let (prompt, prompt_color) = match self.activity {
            Activity::Idle => ("❯ ", Color::Yellow),
            Activity::Asking => ("⚡ ", Color::Red),
            _ => ("⏳ ", Color::Gray),
        };

        let max_w = (area.width as usize).saturating_sub(BORDER_PADDING);

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
                // Show compact instruction
                spans.push(Span::styled(
                    if self.ask_multi_select {
                        "↑↓选择  Space切换  Enter确认"
                    } else {
                        "↑↓选择  Enter确认"
                    },
                    Style::default().fg(Color::DarkGray),
                ));
                lines.push(Line::from(spans));

                // Options display - use structured layout
                for (i, opt) in self.ask_options.iter().enumerate() {
                    let is_selected = self.ask_selected_index == i;
                    let is_checked = opt.selected;
                    let is_submit = opt.is_submit;
                    let is_other = opt.is_other;

                    // Build option line with visual indicators
                    let mut opt_spans: Vec<Span> = Vec::new();

                    // Selection arrow
                    opt_spans.push(Span::styled(
                        if is_selected { "▶ " } else { "  " },
                        Style::default().fg(if is_selected {
                            Color::Cyan
                        } else {
                            Color::DarkGray
                        }),
                    ));

                    // Checkbox/radio indicator
                    if self.ask_multi_select {
                        let box_char = if is_checked { "◆" } else { "◇" };
                        let box_color = if is_checked {
                            Color::Green
                        } else {
                            Color::Gray
                        };
                        opt_spans.push(Span::styled(
                            format!("{} ", box_char),
                            Style::default().fg(box_color),
                        ));
                    } else {
                        // Single select: use radio-style
                        let radio_char = if is_checked { "●" } else { "○" };
                        let radio_color = if is_checked { Color::Cyan } else { Color::Gray };
                        opt_spans.push(Span::styled(
                            format!("{} ", radio_char),
                            Style::default().fg(radio_color),
                        ));
                    }

                    // Option label
                    let label_style = if is_submit {
                        Style::default()
                            .fg(if is_checked {
                                Color::Yellow
                            } else {
                                Color::White
                            })
                            .add_modifier(Modifier::BOLD)
                    } else if is_checked {
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD)
                    } else if is_selected {
                        Style::default().fg(Color::White)
                    } else {
                        Style::default().fg(Color::Gray)
                    };
                    opt_spans.push(Span::styled(opt.label.clone(), label_style));

                    // Description (if available)
                    if let Some(desc) = &opt.description {
                        opt_spans.push(Span::styled(
                            format!(" {}", truncate(desc, 25)),
                            Style::default().fg(Color::DarkGray),
                        ));
                    }

                    // Other option hint
                    if is_other && is_selected && !is_checked {
                        opt_spans.push(Span::styled(
                            " ✏️自定义",
                            Style::default().fg(Color::Yellow),
                        ));
                    }

                    lines.push(Line::from(opt_spans));
                }
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

            // "Other" input mode: show input field
            if self.ask_other_input_active {
                lines.push(Line::styled(
                    "─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─",
                    Style::default().fg(Color::DarkGray),
                ));
                let input_line = if self.input.is_empty() {
                    Line::from(vec![
                        Span::styled("  ✏️ ", Style::default().fg(Color::Yellow)),
                        Span::styled("▌", Style::default().fg(Color::Cyan)),
                        Span::styled(" 输入自定义内容", Style::default().fg(Color::DarkGray)),
                    ])
                } else {
                    Line::from(vec![
                        Span::styled("  ✏️ ", Style::default().fg(Color::Yellow)),
                        Span::styled(&self.input, Style::default().fg(Color::White)),
                        Span::styled("▌", Style::default().fg(Color::Cyan)),
                    ])
                };
                lines.push(input_line);
                lines.push(Line::styled(
                    "  [Enter确认  Esc取消]",
                    Style::default().fg(Color::DarkGray),
                ));
            }

            f.render_widget(Paragraph::new(lines), area);
            return;
        }

        let is_multiline = self.input.contains('\n');

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
            // Use area height minus 1 for char count line if needed
            let show_char_count = self.input.chars().count() > 50 || total_lines_count > 1;
            let max_display_lines =
                (area.height as usize).saturating_sub(if show_char_count { 1 } else { 0 });

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
            if show_char_count {
                lines.push(Line::styled(
                    format!(
                        "  {} chars, {} lines",
                        self.input.chars().count(),
                        total_lines_count
                    ),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            f.render_widget(Paragraph::new(lines), area);
        }
    }
}
