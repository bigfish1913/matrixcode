use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::types::{Activity, ApproveMode, Role};
use crate::utils::{truncate, truncate_visual, truncate_visual_end, fmt_tokens, progress_bar, word_wrap};
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
            // Max 5 lines for input area
            Constraint::Length(input_lines.min(5) as u16 + 1)  // +1 for prompt
        };
        
        let constraints = vec![
            Constraint::Length(1),           // Status (MatrixCode + Model + mode)
            Constraint::Min(3),              // Messages (弹性高度，最大化)
            queue_height,                    // Queue (pending messages preview)
            Constraint::Length(1),           // Usage + Hints
            input_height,                    // Input (dynamic height)
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
        self.draw_usage(f, chunks[3]);
        self.draw_input(f, chunks[4]);
    }

    fn draw_status(&self, f: &mut ratatui::Frame, area: Rect) {
        // Status indicator on the right
        let status_text = if self.activity == Activity::Idle {
            " Ready "
        } else {
            " ... "
        };
        let status_color = if self.activity == Activity::Idle {
            Color::Green
        } else {
            Color::Yellow
        };
        
        let spans = vec![
            Span::styled(" MatrixCode ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled("│", Style::default().fg(Color::DarkGray)),
            Span::styled(format!(" {} ", self.model), Style::default().fg(Color::White)),
            Span::styled("│", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!(" mode:{} ", self.approve_mode.label()),
                Style::default().fg(match self.approve_mode {
                    ApproveMode::Ask => Color::Yellow,
                    ApproveMode::Auto => Color::Green,
                    ApproveMode::Strict => Color::Red,
                })
            ),
            Span::styled("│", Style::default().fg(Color::DarkGray)),
            Span::styled(status_text, Style::default().fg(status_color)),
        ];
        f.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    fn draw_usage(&self, f: &mut ratatui::Frame, area: Rect) {
        if self.tokens_in == 0 && self.tokens_out == 0 {
            f.render_widget(Paragraph::new(Line::styled(
                " /help │ PgUp/PgDn: scroll │ Home/End: top/bot │ Use terminal for text selection",
                Style::default().fg(Color::DarkGray)
            )), area);
            return;
        }
        
        let context_pct = if self.context_size > 0 {
            (self.tokens_in as f64 / self.context_size as f64 * 100.0).min(100.0)
        } else { 0.0 };
        
        let ctx_color = if context_pct < 50.0 { Color::Green }
                       else if context_pct < 75.0 { Color::Yellow }
                       else { Color::Red };
        
        let bar = progress_bar(context_pct, 20);
        
        let mut parts: Vec<Span> = vec![
            Span::styled(
                format!("in {} / out {} (session: {})", 
                    fmt_tokens(self.tokens_in), 
                    fmt_tokens(self.tokens_out),
                    fmt_tokens(self.session_total_out)
                ),
                Style::default().fg(Color::Gray)
            ),
        ];
        
        // Cache info: always show
        parts.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
        parts.push(Span::styled(
            format!("cache r/w {}/{}", 
                fmt_tokens(self.cache_read), 
                fmt_tokens(self.cache_created)
            ),
            Style::default().fg(Color::Cyan)
        ));
        
        // Debug mode: show api/tools/compress counts
        if self.debug_mode {
            parts.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
            parts.push(Span::styled(
                format!("api:{} ", self.api_calls),
                Style::default().fg(Color::Magenta)
            ));
            if self.tool_calls > 0 {
                parts.push(Span::styled(
                    format!("tools:{} ", self.tool_calls),
                    Style::default().fg(Color::Blue)
                ));
            }
            if self.compressions > 0 {
                parts.push(Span::styled(
                    format!("compress:{} ", self.compressions),
                    Style::default().fg(Color::Yellow)
                ));
            }
        }
        
        parts.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
        
        parts.push(Span::styled(
            format!("ctx {} / {} ({:.1}%) {}", 
                fmt_tokens(self.tokens_in), 
                fmt_tokens(self.context_size),
                context_pct,
                bar
            ),
            Style::default().fg(ctx_color)
        ));
        
        f.render_widget(Paragraph::new(Line::from(parts)), area);
    }

    fn draw_messages(&self, f: &mut ratatui::Frame, area: Rect) {
        let mut lines: Vec<Line> = Vec::new();
        let max_w = area.width.saturating_sub(5) as usize;

        // Get selection range for highlighting
        let selection = self.selection.map(|s| s.normalized());

        // Welcome
        if self.show_welcome && self.messages.is_empty() {
            lines.push(Line::styled(
                "╭─────────────────────────────────────────────────────────────╮",
                Style::default().fg(Color::Cyan)
            ));
            lines.push(Line::styled(
                "│                     🤖 MatrixCode                           │",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            ));
            lines.push(Line::styled(
                "│   AI-powered coding assistant with extended thinking       │",
                Style::default().fg(Color::DarkGray)
            ));
            lines.push(Line::raw("│                                                             │"));
            lines.push(Line::styled(
                "│   Commands: /help /clear /history /mode /new /exit         │",
                Style::default().fg(Color::Gray)
            ));
            lines.push(Line::styled(
                "│   Shortcuts: Enter=send │ PgUp/PgDn=scroll │ Alt+T=thinking │",
                Style::default().fg(Color::Gray)
            ));
            lines.push(Line::styled(
                "╰─────────────────────────────────────────────────────────────╯",
                Style::default().fg(Color::Cyan)
            ));
            lines.push(Line::raw(""));
        }

        // Render all messages
        for msg in &self.messages {
            let icon = msg.role.icon();
            let label = msg.role.label();
            let color = msg.role.color();
            
            // Check if this is an approval request message
            let is_approval = msg.content.contains("APPROVAL REQUIRED");
            
            // Thinking uses dim header style (appears smaller)
            if matches!(msg.role, Role::Thinking) {
                let fold_icon = if self.thinking_collapsed { "▶" } else { "▼" };
                lines.push(Line::from(vec![
                    Span::styled(format!("    💭 {} ", fold_icon), Style::default().fg(Color::DarkGray)),
                    Span::styled("Thinking", Style::default().fg(Color::DarkGray)),
                    Span::styled(" (Alt+T)", Style::default().fg(Color::DarkGray)),
                ]));
            } else {
                let header_color = if is_approval { Color::Red } else { color };
                lines.push(Line::from(vec![
                    Span::styled(icon, Style::default().fg(header_color)),
                    Span::raw(" "),
                    Span::styled(label, Style::default().fg(header_color).add_modifier(Modifier::BOLD)),
                ]));
            }
            
            if matches!(msg.role, Role::Thinking) {
                // Thinking uses markdown rendering with dim style and indent (appears smaller)
                let md_lines = render_markdown(&msg.content, max_w.saturating_sub(4));  // Leave room for indent
                if self.thinking_collapsed {
                    // Show only first 2 lines when collapsed
                    for line in md_lines.iter().take(2) {
                        // Add indent to make it look "smaller"
                        let indented = Line::styled(
                            format!("    {}", line.spans.iter().map(|s| s.content.as_ref()).collect::<String>()),
                            Style::default().fg(Color::DarkGray)
                        );
                        lines.push(indented);
                    }
                    if md_lines.len() > 2 {
                        lines.push(Line::styled(
                            format!("    ... ({} more lines)", md_lines.len() - 2),
                            Style::default().fg(Color::DarkGray)
                        ));
                    }
                } else {
                    for line in md_lines {
                        // Add indent to make it look "smaller"
                        let indented = Line::styled(
                            format!("    {}", line.spans.iter().map(|s| s.content.as_ref()).collect::<String>()),
                            Style::default().fg(Color::DarkGray)
                        );
                        lines.push(indented);
                    }
                }
            } else {
                if msg.role == Role::Assistant {
                    let md_lines = render_markdown(&msg.content, max_w);
                    lines.extend(md_lines);
                } else if let Role::Tool { name, is_error } = &msg.role {
                    // Tool results show abbreviated summary
                    let summary = summarize_tool_result(name, &msg.content, is_error, max_w);
                    for line in summary {
                        lines.push(line);
                    }
                } else {
                    // Use different style for approval messages
                    let content_color = if is_approval { Color::Yellow } else { Color::White };
                    // Word wrap for non-markdown content (User, System)
                    let wrapped = word_wrap(&msg.content, max_w);
                    for line in wrapped {
                        lines.push(Line::styled(
                            format!("  {}", line),
                            Style::default().fg(content_color)
                        ));
                    }
                }
            }
            
            lines.push(Line::raw(""));
        }

        // Current thinking (streaming) - markdown rendered with indent (appears smaller)
        if !self.thinking.is_empty() {
            let fold_icon = if self.thinking_collapsed { "▶" } else { "▼" };
            lines.push(Line::from(vec![
                Span::styled(format!("💭 {} ", fold_icon), Style::default().fg(Color::DarkGray)),
                Span::styled("Thinking", Style::default().fg(Color::DarkGray)),
                Span::styled(" (Alt+T)", Style::default().fg(Color::DarkGray)),
            ]));
            
            let md_lines = render_markdown(&self.thinking, max_w.saturating_sub(4));
            if self.thinking_collapsed {
                // Show only first line when collapsed
                for line in md_lines.iter().take(1) {
                    let indented = Line::styled(
                        format!("    {}", line.spans.iter().map(|s| s.content.as_ref()).collect::<String>()),
                        Style::default().fg(Color::DarkGray)
                    );
                    lines.push(indented);
                }
                if md_lines.len() > 1 {
                    lines.push(Line::styled(
                        format!("    ... ({} more lines)", md_lines.len() - 1),
                        Style::default().fg(Color::DarkGray)
                    ));
                }
            } else {
                for line in md_lines {
                    let indented = Line::styled(
                        format!("    {}", line.spans.iter().map(|s| s.content.as_ref()).collect::<String>()),
                        Style::default().fg(Color::DarkGray)
                    );
                    lines.push(indented);
                }
            }
            lines.push(Line::raw(""));
        }

        // Streaming text - markdown rendered (only if not empty)
        if !self.streaming.is_empty() {
            let spinner = SPINNER[self.frame];
            lines.push(Line::from(vec![
                Span::styled("🤖", Style::default().fg(Color::Blue)),
                Span::raw(" "),
                Span::styled("Assistant", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
                Span::styled(format!(" {} ", spinner), Style::default().fg(self.activity.color())),
            ]));
            let md_lines = render_markdown(&self.streaming, max_w);
            lines.extend(md_lines);
            lines.push(Line::styled("  ▌", Style::default().fg(Color::Cyan)));
        }
        
        // Activity indicator (tool execution progress)
        // Show when activity is a tool operation (not Thinking/Idle/Asking) and no streaming content
        let is_tool_activity = matches!(self.activity, 
            Activity::Reading | Activity::Writing | Activity::Editing | 
            Activity::Searching | Activity::Running | Activity::WebSearch | 
            Activity::WebFetch | Activity::Tool(_)
        );
        
        // Show spinner for Thinking state when waiting for AI response (empty content)
        if self.activity == Activity::Thinking && self.streaming.is_empty() && self.thinking.is_empty() {
            let spinner = SPINNER[self.frame];
            lines.push(Line::from(vec![
                Span::styled(spinner, Style::default().fg(self.activity.color())),
                Span::raw(" "),
                Span::styled(self.activity.label(), Style::default().fg(self.activity.color())),
                Span::styled("  Waiting for AI response...", Style::default().fg(Color::DarkGray)),
            ]));
        }
        
        if is_tool_activity && self.streaming.is_empty() && self.thinking.is_empty() {
            let tool_label = if !self.activity_detail.is_empty() {
                format!("{}({})", self.activity.label(), self.activity_detail)
            } else {
                self.activity.label()
            };
            let spans = vec![
                Span::styled(SPINNER[self.frame], Style::default().fg(self.activity.color())),
                Span::raw(" "),
                Span::styled(tool_label, Style::default().fg(self.activity.color())),
            ];
            lines.push(Line::from(spans));
        }

        // Scroll
        let total_lines = lines.len() as u16;
        let visible_height = area.height;
        let max_scroll = if total_lines > visible_height {
            total_lines.saturating_sub(visible_height)
        } else {
            0
        };
        
        // Store max_scroll for scroll detection in on_mouse
        self.max_scroll.set(max_scroll);
        
        // Force auto_scroll when AI is actively streaming/thinking (but not when user is selecting)
        let force_auto_scroll = (!self.streaming.is_empty() 
            || !self.thinking.is_empty() 
            || self.activity == Activity::Thinking)
            && !self.selecting;
        
        let scroll_offset = if self.auto_scroll || force_auto_scroll {
            max_scroll  // Scroll to bottom when auto_scroll or AI is active
        } else {
            self.scroll_offset.min(max_scroll)
        };

        // Apply selection highlight to lines (preserve original span styles, just add bg)
        let highlighted_lines = if let Some(sel) = selection {
            let sel_start = sel.start_line;
            let sel_end = sel.end_line;
            lines.into_iter().enumerate().map(|(i, line)| {
                if i >= sel_start && i <= sel_end {
                    // Preserve span styles, add background
                    let new_spans: Vec<Span> = line.spans.iter().map(|s| {
                        Span::styled(
                            s.content.to_string(),
                            s.style.bg(Color::DarkGray)
                        )
                    }).collect();
                    Line::from(new_spans)
                } else {
                    line
                }
            }).collect()
        } else {
            lines
        };

        // Scroll position indicator (when not at bottom)
        if !self.auto_scroll && max_scroll > 0 {
            let pct = (scroll_offset as f64 / max_scroll as f64 * 100.0) as u16;
            let indicator = Line::styled(
                format!("  ↑ scroll {}/{} ({:.0}%) — End to jump to bottom", 
                    scroll_offset, max_scroll, pct),
                Style::default().fg(Color::DarkGray)
            );
            // Render indicator at the top of the area
            let indicator_area = Rect::new(area.x, area.y, area.width, 1);
            f.render_widget(Paragraph::new(indicator), indicator_area);
            
            // Render messages below indicator
            let msg_area = Rect::new(area.x, area.y + 1, area.width, area.height.saturating_sub(1));
            f.render_widget(
                Paragraph::new(highlighted_lines)
                    .scroll((scroll_offset, 0)),
                msg_area
            );
        } else {
            f.render_widget(
                Paragraph::new(highlighted_lines)
                    .scroll((scroll_offset, 0)),
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
                // Show blinking cursor placeholder
                spans.push(Span::styled("▌", Style::default().fg(Color::Cyan)));
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

/// Summarize tool result for display (abbreviated format)
fn summarize_tool_result(name: &str, content: &str, is_error: &bool, max_w: usize) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let color = if *is_error { Color::Red } else { Color::Cyan };
    let error_prefix = if *is_error { "❌ " } else { "" };
    
    // Parse tool name from label
    let tool_type = name.to_lowercase();
    
    match tool_type.as_str() {
        // Read: show file preview with line count
        "read" => {
            let line_count = content.lines().count();
            if line_count <= 3 {
                for line in content.lines().take(3) {
                    lines.push(Line::styled(
                        format!("  {}{}", error_prefix, truncate(line, max_w - 4)),
                        Style::default().fg(color)
                    ));
                }
            } else {
                for line in content.lines().take(2) {
                    lines.push(Line::styled(
                        format!("  {}{}", error_prefix, truncate(line, max_w - 4)),
                        Style::default().fg(color)
                    ));
                }
                lines.push(Line::styled(
                    format!("  {}... ({}) lines total", error_prefix, line_count),
                    Style::default().fg(Color::DarkGray)
                ));
            }
        }
        
        // Edit: show what was changed
        "edit" | "multi_edit" => {
            if *is_error {
                lines.push(Line::styled(
                    format!("  {}{}", error_prefix, truncate(content, max_w - 4)),
                    Style::default().fg(Color::Red)
                ));
            } else {
                lines.push(Line::styled(
                    "  ✓ Applied changes",
                    Style::default().fg(Color::Green)
                ));
                // Show first few lines of diff preview
                for line in content.lines().take(3) {
                    let prefix = if line.starts_with('+') { "+" } 
                                 else if line.starts_with('-') { "-" }
                                 else { " " };
                    let line_color = if line.starts_with('+') { Color::Green }
                                     else if line.starts_with('-') { Color::Red }
                                     else { Color::DarkGray };
                    lines.push(Line::styled(
                        format!("  {}{}", prefix, truncate(line, max_w - 4)),
                        Style::default().fg(line_color)
                    ));
                }
            }
        }
        
        // Search/Glob: show match count + preview
        "search" | "glob" | "ls" => {
            let matches: Vec<&str> = content.lines().filter(|l| !l.is_empty()).take(10).collect();
            let total = content.lines().filter(|l| !l.is_empty()).count();
            
            lines.push(Line::styled(
                format!("  {}{} matches{}", error_prefix, total, if total > 10 { format!(" (showing {})", matches.len()) } else { String::new() }),
                Style::default().fg(color)
            ));
            for m in matches.iter().take(5) {
                lines.push(Line::styled(
                    format!("    {}", truncate(m, max_w - 6)),
                    Style::default().fg(Color::DarkGray)
                ));
            }
        }
        
        // Bash: show command output preview
        "bash" => {
            let line_count = content.lines().count();
            if line_count <= 5 {
                for line in content.lines() {
                    lines.push(Line::styled(
                        format!("  {}{}", error_prefix, truncate(line, max_w - 4)),
                        Style::default().fg(color)
                    ));
                }
            } else {
                // Show first 2 and last 2 lines
                for line in content.lines().take(2) {
                    lines.push(Line::styled(
                        format!("  {}{}", error_prefix, truncate(line, max_w - 4)),
                        Style::default().fg(color)
                    ));
                }
                lines.push(Line::styled(
                    format!("  {}... ({}) lines ...", error_prefix, line_count - 4),
                    Style::default().fg(Color::DarkGray)
                ));
                let last_lines: Vec<&str> = content.lines().rev().take(2).collect();
                for line in last_lines.iter().rev() {
                    lines.push(Line::styled(
                        format!("  {}{}", error_prefix, truncate(line, max_w - 4)),
                        Style::default().fg(color)
                    ));
                }
            }
        }
        
        // Write: show file created
        "write" => {
            if *is_error {
                lines.push(Line::styled(
                    format!("  {}{}", error_prefix, truncate(content, max_w - 4)),
                    Style::default().fg(Color::Red)
                ));
            } else {
                lines.push(Line::styled(
                    "  ✓ File written successfully",
                    Style::default().fg(Color::Green)
                ));
            }
        }
        
        // Todo_write: show full todo list with colored status markers
        "todo_write" => {
            // Show full todo list
            for line in content.lines() {
                let trimmed = line.trim();
                let (marker_color, content_color) = if trimmed.starts_with("[~]") {
                    // in_progress - yellow
                    (Color::Yellow, Color::Yellow)
                } else if trimmed.starts_with("[x]") {
                    // completed - green
                    (Color::Green, Color::Green)
                } else if trimmed.starts_with("[ ]") {
                    // pending - dark gray
                    (Color::DarkGray, Color::Gray)
                } else if trimmed.starts_with("Todos") {
                    // header - cyan bold
                    (Color::Cyan, Color::Cyan)
                } else {
                    (color, color)
                };
                
                // Format with proper indentation and colors
                if trimmed.starts_with("[") {
                    lines.push(Line::styled(
                        format!("  {}", truncate(line, max_w - 4)),
                        Style::default().fg(content_color)
                    ));
                } else {
                    // Header or other text
                    lines.push(Line::styled(
                        format!("  {}", truncate(line, max_w - 4)),
                        Style::default().fg(marker_color)
                    ));
                }
            }
        }
        
        // WebSearch/WebFetch: show results preview
        "websearch" | "webfetch" => {
            let line_count = content.lines().count();
            for line in content.lines().take(5) {
                lines.push(Line::styled(
                    format!("  {}{}", error_prefix, truncate(line, max_w - 4)),
                    Style::default().fg(color)
                ));
            }
            if line_count > 5 {
                lines.push(Line::styled(
                    format!("  {}... {} more results", error_prefix, line_count - 5),
                    Style::default().fg(Color::DarkGray)
                ));
            }
        }
        
        // Default: truncate and show
        _ => {
            let line_count = content.lines().count();
            if line_count <= 3 {
                for line in content.lines() {
                    lines.push(Line::styled(
                        format!("  {}{}", error_prefix, truncate(line, max_w - 4)),
                        Style::default().fg(color)
                    ));
                }
            } else {
                for line in content.lines().take(2) {
                    lines.push(Line::styled(
                        format!("  {}{}", error_prefix, truncate(line, max_w - 4)),
                        Style::default().fg(color)
                    ));
                }
                lines.push(Line::styled(
                    format!("  {}... ({}) lines total", error_prefix, line_count),
                    Style::default().fg(Color::DarkGray)
                ));
            }
        }
    }
    
    lines
}
