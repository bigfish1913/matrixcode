use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::types::{Activity, ApproveMode, Role};
use crate::utils::{truncate, fmt_tokens, progress_bar, word_wrap};
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
        
        let constraints = vec![
            Constraint::Length(1),           // Status (MatrixCode + Model + mode)
            Constraint::Min(3),              // Messages (弹性高度，最大化)
            queue_height,                    // Queue (pending messages preview)
            Constraint::Length(1),           // Usage + Hints
            Constraint::Length(1),           // Input
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
        
        let bar = progress_bar(context_pct, 10);
        
        let mut parts: Vec<Span> = vec![
            Span::styled(
                format!("in {} / out {}", 
                    fmt_tokens(self.tokens_in), 
                    fmt_tokens(self.session_total_out)
                ),
                Style::default().fg(Color::Gray)
            ),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        ];
        
        // Debug mode: show api/tools/compress/cache counts
        if self.debug_mode {
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
            if self.cache_read > 0 || self.cache_created > 0 {
                parts.push(Span::styled(
                    format!("cache r/c {}/{} ", fmt_tokens(self.cache_read), fmt_tokens(self.cache_created)),
                    Style::default().fg(Color::Cyan)
                ));
            }
            parts.push(Span::styled("│ ", Style::default().fg(Color::DarkGray)));
        }
        
        parts.push(Span::styled(
            format!("ctx {:.0}% {}", context_pct, bar),
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
                lines.push(Line::from(vec![
                    Span::styled("    💭 ", Style::default().fg(Color::DarkGray)),
                    Span::styled("Thinking", Style::default().fg(Color::DarkGray)),
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
            lines.push(Line::from(vec![
                Span::styled("💭 ", Style::default().fg(Color::DarkGray)),
                Span::styled("Thinking", Style::default().fg(Color::DarkGray)),
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
        
        if is_tool_activity && self.streaming.is_empty() && self.thinking.is_empty() {
            let mut spans = vec![
                Span::styled(SPINNER[self.frame], Style::default().fg(self.activity.color())),
                Span::raw(" "),
                Span::styled(self.activity.label(), Style::default().fg(self.activity.color())),
            ];
            if !self.activity_detail.is_empty() {
                spans.push(Span::styled(
                    format!(" {}", self.activity_detail),
                    Style::default().fg(Color::DarkGray)
                ));
            }
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
        
        let scroll_offset = if self.auto_scroll {
            max_scroll
        } else {
            self.scroll_offset.min(max_scroll)
        };

        // Apply selection highlight to lines
        let highlighted_lines = if let Some(sel) = selection {
            let sel_start = sel.start_line;
            let sel_end = sel.end_line;
            lines.into_iter().enumerate().map(|(i, line)| {
                if i >= sel_start && i <= sel_end {
                    // Add selection background color
                    let content = line.spans.iter().map(|s| s.content.as_ref()).collect::<String>();
                    Line::styled(content, Style::default().fg(Color::White).bg(Color::DarkGray))
                } else {
                    line
                }
            }).collect()
        } else {
            lines
        };

        f.render_widget(
            Paragraph::new(highlighted_lines)
                .scroll((scroll_offset, 0)),
            area
        );
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
        let mut spans: Vec<Span> = vec![];
        
        // Prompt indicator based on activity
        match self.activity {
            Activity::Idle => {
                spans.push(Span::styled("❯ ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
            }
            Activity::Asking => {
                spans.push(Span::styled("❓ ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
                spans.push(Span::styled("[reply: y/n or option] ", Style::default().fg(Color::Yellow)));
            }
            _ => {
                spans.push(Span::styled("❯ ", Style::default().fg(Color::Gray)));
            }
        }
        
        // Input content
        if self.input.is_empty() {
            spans.push(Span::styled("_", Style::default().fg(Color::Cyan)));
        } else {
            let display = if self.input.contains('\n') {
                let n = self.input.lines().count();
                let last = self.input.lines().last().unwrap_or("");
                if last.is_empty() { format!("{} lines", n) } else { format!("{} lines: {}", n, last) }
            } else {
                self.input.clone()
            };
            spans.push(Span::styled(truncate(&display, area.width as usize - 25), Style::default().fg(Color::White)));
        }
        
        // Hint
        spans.push(Span::styled(" Shift+Enter↵", Style::default().fg(Color::DarkGray)));
        
        f.render_widget(Paragraph::new(Line::from(spans)), area);
    }
}

/// Summarize tool result for display (abbreviated format)
fn summarize_tool_result(name: &str, content: &str, is_error: &bool, max_w: usize) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let color = if *is_error { Color::Red } else { Color::Cyan };
    let error_prefix = if *is_error { "❌ " } else { "" };
    
    // Parse tool name from label (e.g., "📖 Reading" -> "read")
    let tool_type = name.to_lowercase();
    
    match tool_type {
        // Read: show file preview with line count
        t if t.contains("read") || t.contains("reading") => {
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
        t if t.contains("edit") || t.contains("editing") => {
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
        t if t.contains("search") || t.contains("glob") => {
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
        t if t.contains("run") || t.contains("bash") => {
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
        t if t.contains("write") || t.contains("writing") => {
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
        t if t.contains("todo") => {
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
        t if t.contains("web") || t.contains("search") || t.contains("fetch") => {
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
