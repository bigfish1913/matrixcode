use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::types::{Activity, ApproveMode, Role};
use crate::utils::{truncate, truncate_visual, truncate_visual_end, fmt_tokens, word_wrap};
use crate::markdown::render_markdown;
use crate::app::TuiApp;
use crate::SPINNER;

impl TuiApp {
    pub(crate) fn draw(&self, f: &mut ratatui::Frame) {
        // Dynamic queue height: show if there are pending messages
        let queue_height = if self.pending_messages.is_empty() {
            Constraint::Length(0)
        } else {
            Constraint::Length(1)
        };
        
        // Dynamic input height: expand for multiline content
        let input_lines = self.input.lines().count().max(1);
        let input_height = if input_lines <= 1 {
            Constraint::Length(1)
        } else {
            Constraint::Length(input_lines.min(5) as u16 + 1)
        };
        
        let constraints = vec![
            Constraint::Length(1),           // Status bar (merged: model + mode + tokens)
            Constraint::Min(3),              // Messages
            queue_height,                    // Queue (pending messages preview)
            input_height,                    // Input
        ];

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(f.area());

        // Store messages area top for mouse selection
        self.msg_area_top.set(chunks[1].y);

        self.draw_status(f, chunks[0]);
        self.draw_messages(f, chunks[1]);
        if !self.pending_messages.is_empty() {
            self.draw_queue(f, chunks[2]);
        }
        self.draw_input(f, chunks[3]);
    }

    fn draw_status(&self, f: &mut ratatui::Frame, area: Rect) {
        let context_pct = if self.context_size > 0 {
            (self.tokens_in as f64 / self.context_size as f64 * 100.0).min(100.0)
        } else { 0.0 };
        let ctx_color = if context_pct < 50.0 { Color::DarkGray }
                       else if context_pct < 75.0 { Color::Yellow }
                       else { Color::Red };

        let mode_color = match self.approve_mode {
            ApproveMode::Ask => Color::DarkGray,
            ApproveMode::Auto => Color::DarkGray,
            ApproveMode::Strict => Color::Red,
        };

        let mut spans = vec![
            Span::styled(format!(" {} ", self.model), Style::default().fg(Color::DarkGray)),
            Span::styled("│", Style::default().fg(Color::DarkGray)),
            Span::styled(format!(" {} ", self.approve_mode.label()), Style::default().fg(mode_color)),
            Span::styled("│", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!(" ctx {:.0}% ", context_pct),
                Style::default().fg(ctx_color)
            ),
            Span::styled(
                format!("out {} ", fmt_tokens(self.session_total_out)),
                Style::default().fg(Color::DarkGray)
            ),
        ];

        // Cache info only when non-zero
        if self.cache_read > 0 || self.cache_created > 0 {
            spans.push(Span::styled(
                format!("cache {}/{} ", fmt_tokens(self.cache_read), fmt_tokens(self.cache_created)),
                Style::default().fg(Color::DarkGray)
            ));
        }

        // Debug stats
        if self.debug_mode {
            spans.push(Span::styled(
                format!("api:{} tools:{} ", self.api_calls, self.tool_calls),
                Style::default().fg(Color::DarkGray)
            ));
        }

        // Status on the right
        let status_text = if self.activity == Activity::Idle {
            "Ready".to_string()
        } else if !self.activity_detail.is_empty() {
            format!("{}({})", self.activity.label(), self.activity_detail)
        } else {
            self.activity.label()
        };
        let status_color = if self.activity == Activity::Idle { Color::DarkGray } else { Color::Yellow };
        
        spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
        spans.push(Span::styled(format!(" {} ", status_text), Style::default().fg(status_color)));

        f.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    fn draw_messages(&self, f: &mut ratatui::Frame, area: Rect) {
        let mut lines: Vec<Line> = Vec::new();
        let max_w = area.width.saturating_sub(4) as usize;

        let selection = self.selection.map(|s| s.normalized());

        // Welcome (responsive)
        if self.show_welcome && self.messages.is_empty() {
            let w = (area.width as usize).min(60);
            let border = "\u{2500}".repeat(w.saturating_sub(2));
            lines.push(Line::styled(format!("\u{256d}{}\u{256e}", border), Style::default().fg(Color::Cyan)));
            lines.push(Line::styled(
                format!("\u{2502}{:^width$}\u{2502}", "\u{1f916} MatrixCode", width = w.saturating_sub(2)),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            ));
            lines.push(Line::styled(
                format!("\u{2502}{:^width$}\u{2502}", "AI coding assistant", width = w.saturating_sub(2)),
                Style::default().fg(Color::DarkGray)
            ));
            lines.push(Line::styled(
                format!("\u{2502}{:^width$}\u{2502}", "/help for commands | Enter to send", width = w.saturating_sub(2)),
                Style::default().fg(Color::Gray)
            ));
            lines.push(Line::styled(format!("\u{2570}{}\u{256f}", border), Style::default().fg(Color::Cyan)));
            lines.push(Line::raw(""));
        }

        // Render all messages
        for msg in &self.messages {
            match &msg.role {
                Role::User => {
                    let wrapped = word_wrap(&msg.content, max_w.saturating_sub(2));
                    for line in wrapped {
                        lines.push(Line::from(vec![
                            Span::styled("\u{2502} ", Style::default().fg(Color::Green)),
                            Span::styled(line, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                        ]));
                    }
                    lines.push(Line::raw(""));
                }
                Role::Assistant => {
                    let md_lines = render_markdown(&msg.content, max_w);
                    lines.extend(md_lines);
                    lines.push(Line::raw(""));
                }
                Role::Thinking => {
                    let line_count = msg.content.lines().count();
                    if self.thinking_collapsed {
                        lines.push(Line::from(vec![
                            Span::styled("  \u{1f4ad} \u{25b6} ", Style::default().fg(Color::DarkGray)),
                            Span::styled(
                                format!("Thinking ({} lines)", line_count),
                                Style::default().fg(Color::DarkGray)
                            ),
                        ]));
                    } else {
                        lines.push(Line::from(vec![
                            Span::styled("  \u{1f4ad} \u{25bc} ", Style::default().fg(Color::DarkGray)),
                            Span::styled("Thinking", Style::default().fg(Color::DarkGray)),
                        ]));
                        let md_lines = render_markdown(&msg.content, max_w.saturating_sub(4));
                        for line in md_lines.iter().take(20) {
                            let text = line.spans.iter().map(|s| s.content.as_ref()).collect::<String>();
                            lines.push(Line::styled(format!("    {}", text), Style::default().fg(Color::DarkGray)));
                        }
                        if md_lines.len() > 20 {
                            lines.push(Line::styled(
                                format!("    ... ({} more lines)", md_lines.len() - 20),
                                Style::default().fg(Color::DarkGray)
                            ));
                        }
                    }
                }
                Role::Tool { name, is_error } => {
                    let icon = if *is_error { "\u{2717}" } else { "\u{2713}" };
                    let color = if *is_error { Color::Red } else { Color::DarkGray };
                    let line_count = msg.content.lines().count();
                    let preview = msg.content.lines().next().unwrap_or("");
                    let summary = if *is_error {
                        truncate(preview, max_w.saturating_sub(name.len() + 6))
                    } else {
                        match name.as_str() {
                            "read" => format!("{} lines", line_count),
                            "write" => "written".into(),
                            "edit" | "multi_edit" => "applied".into(),
                            "bash" => {
                                if line_count <= 1 {
                                    truncate(preview, max_w.saturating_sub(name.len() + 6))
                                } else {
                                    format!("{} lines output", line_count)
                                }
                            }
                            "search" | "glob" | "ls" => format!("{} results", line_count),
                            _ => truncate(preview, max_w.saturating_sub(name.len() + 6)),
                        }
                    };
                    lines.push(Line::from(vec![
                        Span::styled(format!("  {} ", icon), Style::default().fg(color)),
                        Span::styled(name.clone(), Style::default().fg(color)),
                        Span::styled(format!(" \u{2192} {}", summary), Style::default().fg(Color::DarkGray)),
                    ]));
                }
                Role::System => {
                    let content = &msg.content;
                    if content.contains("APPROVAL REQUIRED") || content.contains("requires approval") || content.contains("Allow?") {
                        let wrapped = word_wrap(content, max_w);
                        for line in wrapped {
                            lines.push(Line::styled(format!("  {}", line), Style::default().fg(Color::Yellow)));
                        }
                        lines.push(Line::raw(""));
                    } else {
                        let first_line = content.lines().next().unwrap_or("");
                        lines.push(Line::styled(
                            format!("  {}", truncate(first_line, max_w)),
                            Style::default().fg(Color::DarkGray)
                        ));
                    }
                }
            }
        }

        // Current thinking (streaming)
        if !self.thinking.is_empty() {
            if self.thinking_collapsed {
                lines.push(Line::from(vec![
                    Span::styled("  \u{1f4ad} \u{25b6} ", Style::default().fg(Color::DarkGray)),
                    Span::styled("Thinking...", Style::default().fg(Color::DarkGray)),
                ]));
            } else {
                lines.push(Line::from(vec![
                    Span::styled("  \u{1f4ad} \u{25bc} ", Style::default().fg(Color::DarkGray)),
                    Span::styled("Thinking...", Style::default().fg(Color::DarkGray)),
                ]));
                let md_lines = render_markdown(&self.thinking, max_w.saturating_sub(4));
                for line in md_lines.iter().take(10) {
                    let text = line.spans.iter().map(|s| s.content.as_ref()).collect::<String>();
                    lines.push(Line::styled(format!("    {}", text), Style::default().fg(Color::DarkGray)));
                }
                if md_lines.len() > 10 {
                    lines.push(Line::styled(
                        format!("    ... ({} more)", md_lines.len() - 10),
                        Style::default().fg(Color::DarkGray)
                    ));
                }
            }
        }

        // Streaming text
        if !self.streaming.is_empty() {
            let md_lines = render_markdown(&self.streaming, max_w);
            lines.extend(md_lines);
            lines.push(Line::styled("  \u{258c}", Style::default().fg(Color::Cyan)));
        }

        // Activity indicator
        let is_tool_activity = matches!(self.activity,
            Activity::Reading | Activity::Writing | Activity::Editing |
            Activity::Searching | Activity::Running | Activity::WebSearch |
            Activity::WebFetch | Activity::Tool(_)
        );

        if self.activity == Activity::Thinking && self.streaming.is_empty() && self.thinking.is_empty() {
            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", SPINNER[self.frame]), Style::default().fg(self.activity.color())),
                Span::styled("Waiting for response...", Style::default().fg(Color::DarkGray)),
            ]));
        }

        if is_tool_activity && self.streaming.is_empty() && self.thinking.is_empty() {
            let tool_label = if !self.activity_detail.is_empty() {
                format!("{}({})", self.activity.label(), self.activity_detail)
            } else {
                self.activity.label()
            };
            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", SPINNER[self.frame]), Style::default().fg(self.activity.color())),
                Span::styled(tool_label, Style::default().fg(self.activity.color())),
            ]));
        }

        // Scroll
        let total_lines = lines.len() as u16;
        let visible_height = area.height;
        let max_scroll = if total_lines > visible_height {
            total_lines.saturating_sub(visible_height)
        } else {
            0
        };
        self.max_scroll.set(max_scroll);

        let force_auto_scroll = (!self.streaming.is_empty()
            || !self.thinking.is_empty()
            || self.activity == Activity::Thinking)
            && !self.selecting;

        let scroll_offset = if self.auto_scroll || force_auto_scroll {
            max_scroll
        } else {
            self.scroll_offset.min(max_scroll)
        };

        // Apply selection highlight (preserve span styles, add bg)
        let highlighted_lines = if let Some(sel) = selection {
            let sel_start = sel.start_line;
            let sel_end = sel.end_line;
            lines.into_iter().enumerate().map(|(i, line)| {
                if i >= sel_start && i <= sel_end {
                    let new_spans: Vec<Span> = line.spans.iter().map(|s| {
                        Span::styled(s.content.to_string(), s.style.bg(Color::DarkGray))
                    }).collect();
                    Line::from(new_spans)
                } else {
                    line
                }
            }).collect()
        } else {
            lines
        };

        // Scroll position indicator
        if !self.auto_scroll && max_scroll > 0 {
            let pct = (scroll_offset as f64 / max_scroll as f64 * 100.0) as u16;
            let indicator = Line::styled(
                format!("  \u{2191} {}/{} ({:.0}%) \u{2014} End to jump to bottom", scroll_offset, max_scroll, pct),
                Style::default().fg(Color::DarkGray)
            );
            let indicator_area = Rect::new(area.x, area.y, area.width, 1);
            f.render_widget(Paragraph::new(indicator), indicator_area);
            let msg_area = Rect::new(area.x, area.y + 1, area.width, area.height.saturating_sub(1));
            f.render_widget(
                Paragraph::new(highlighted_lines).scroll((scroll_offset, 0)),
                msg_area
            );
        } else {
            f.render_widget(
                Paragraph::new(highlighted_lines).scroll((scroll_offset, 0)),
                area
            );
        }
    }

    fn draw_queue(&self, f: &mut ratatui::Frame, area: Rect) {
        let mut spans: Vec<Span> = vec![
            Span::styled("⏳ ", Style::default().fg(Color::Magenta)),
            Span::styled(
                format!("Queue ({}): ", self.pending_messages.len()),
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
            ),
        ];
        
        // Show preview of each queued message (truncated)
        for (i, msg) in self.pending_messages.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
            }
            let preview = msg.lines().next().unwrap_or("");
            let truncated = truncate(preview, 30);
            spans.push(Span::styled(
                format!("\"{}\"", truncated),
                Style::default().fg(Color::Yellow)
            ));
        }
        
        f.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    fn draw_input(&self, f: &mut ratatui::Frame, area: Rect) {
        // Prompt indicator based on activity
        let (prompt, prompt_color) = match self.activity {
            Activity::Idle => ("❯ ", Color::Yellow),
            Activity::Asking => ("❓ ", Color::Yellow),
            _ => ("⏳ ", Color::Gray),  // Show queuing indicator when busy
        };
        
        // Check if multiline content
        let is_multiline = self.input.contains('\n');
        let max_w = (area.width as usize).saturating_sub(4);  // Safe minimum margin
        
        if !is_multiline {
            // Single line mode with visible cursor
            let mut spans: Vec<Span> = vec![
                Span::styled(prompt, Style::default().fg(prompt_color).add_modifier(Modifier::BOLD)),
            ];
            
            if self.activity == Activity::Asking {
                spans.push(Span::styled("[reply: y/n or option] ", Style::default().fg(Color::Yellow)));
            }
            
            if self.input.is_empty() {
                // Show placeholder with cursor
                spans.push(Span::styled("▌", Style::default().fg(Color::Cyan)));
                spans.push(Span::styled(" Ask anything...", Style::default().fg(Color::DarkGray)));
            } else {
                // Split input at cursor position to show cursor
                let display_width = max_w.saturating_sub(15);  // Reserve space for hints
                let before_cursor = &self.input[..self.cursor_pos];
                let after_cursor = &self.input[self.cursor_pos..];
                
                // Calculate visual offset if input is too long
                let before_vis_width: usize = before_cursor.chars().map(|c| if c > '\u{7F}' { 2 } else { 1 }).sum();
                let after_vis_width: usize = after_cursor.chars().map(|c| if c > '\u{7F}' { 2 } else { 1 }).sum();
                
                if before_vis_width + after_vis_width <= display_width {
                    // Fits in display
                    spans.push(Span::styled(before_cursor.to_string(), Style::default().fg(Color::White)));
                    spans.push(Span::styled("▌", Style::default().fg(Color::Cyan)));
                    spans.push(Span::styled(after_cursor.to_string(), Style::default().fg(Color::White)));
                } else if before_vis_width < display_width {
                    // Cursor near start, truncate end
                    spans.push(Span::styled(before_cursor.to_string(), Style::default().fg(Color::White)));
                    spans.push(Span::styled("▌", Style::default().fg(Color::Cyan)));
                    let remaining = display_width.saturating_sub(before_vis_width);
                    let truncated_after = truncate_visual(after_cursor, remaining);
                    spans.push(Span::styled(truncated_after, Style::default().fg(Color::White)));
                } else {
                    // Cursor far right, truncate start
                    let start_width = display_width.saturating_sub(10);  // Show ~10 chars after cursor
                    let truncated_before = truncate_visual_end(before_cursor, start_width);
                    spans.push(Span::styled(format!("…{}", truncated_before), Style::default().fg(Color::White)));
                    spans.push(Span::styled("▌", Style::default().fg(Color::Cyan)));
                    let remaining = display_width.saturating_sub(start_width + 1);
                    let truncated_after = truncate_visual(after_cursor, remaining);
                    spans.push(Span::styled(truncated_after, Style::default().fg(Color::White)));
                }
            }
            
            // Show hint
            let hint = if self.activity != Activity::Idle && self.activity != Activity::Asking {
                " [queued]"
            } else {
                ""
            };
            if !hint.is_empty() {
                spans.push(Span::styled(hint, Style::default().fg(Color::Magenta)));
            }
            
            f.render_widget(Paragraph::new(Line::from(spans)), area);
        } else {
            // Multiline mode: show actual content with cursor
            let mut lines: Vec<Line> = Vec::new();
            let input_lines: Vec<&str> = self.input.split('\n').collect();
            let cursor_line = self.input[..self.cursor_pos].matches('\n').count();
            let cursor_col_byte = self.input[..self.cursor_pos].rfind('\n')
                .map(|i| self.cursor_pos - i - 1)
                .unwrap_or(self.cursor_pos);
            
            let max_display_lines = (area.height as usize).saturating_sub(1);
            
            for (i, line) in input_lines.iter().enumerate().take(max_display_lines) {
                let line_prompt = if i == 0 { prompt } else { "  " };
                let line_prompt_color = if i == 0 { prompt_color } else { Color::DarkGray };
                
                if i == cursor_line {
                    // This line has the cursor
                    let before = &line[..cursor_col_byte.min(line.len())];
                    let after = &line[cursor_col_byte.min(line.len())..];
                    lines.push(Line::from(vec![
                        Span::styled(line_prompt, Style::default().fg(line_prompt_color).add_modifier(Modifier::BOLD)),
                        Span::styled(before.to_string(), Style::default().fg(Color::White)),
                        Span::styled("▌", Style::default().fg(Color::Cyan)),
                        Span::styled(after.to_string(), Style::default().fg(Color::White)),
                    ]));
                } else {
                    lines.push(Line::from(vec![
                        Span::styled(line_prompt, Style::default().fg(line_prompt_color).add_modifier(Modifier::BOLD)),
                        Span::styled(truncate(line, max_w), Style::default().fg(Color::White)),
                    ]));
                }
            }
            
            // Show line count if truncated
            let total_lines = input_lines.len();
            if total_lines > max_display_lines {
                lines.push(Line::styled(
                    format!("  … ({}/{} lines)", max_display_lines, total_lines),
                    Style::default().fg(Color::DarkGray)
                ));
            }
            
            f.render_widget(Paragraph::new(lines), area);
        }
    }
}
