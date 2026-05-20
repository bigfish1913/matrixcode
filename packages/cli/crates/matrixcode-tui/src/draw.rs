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
            Constraint::Length(input_lines.min(5) as u16 + 1)
        };
        
        let constraints = vec![
            Constraint::Min(3),              // Messages
            queue_height,                    // Queue (pending messages preview)
            Constraint::Length(1),           // Status bar (bottom)
            input_height,                    // Input
        ];

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(f.area());

        // Store messages area top for mouse selection
        self.msg_area_top.set(chunks[0].y);

        self.draw_messages(f, chunks[0]);
        if !self.pending_messages.is_empty() {
            self.draw_queue(f, chunks[1]);
        }
        self.draw_status(f, chunks[2]);
        self.draw_input(f, chunks[3]);
    }

    fn draw_status(&self, f: &mut ratatui::Frame, area: Rect) {
        // Estimate total context tokens from all messages (not just current request)
        let estimated_context_tokens = self.messages.iter().map(|m| {
            // Simple estimation: ~3 chars per token
            (m.content.len() / 3).max(1) as u64
        }).sum::<u64>();
        
        let context_pct = if self.context_size > 0 {
            (estimated_context_tokens as f64 / self.context_size as f64 * 100.0).min(100.0)
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
                format!(" {} {:.0}% ", progress_bar(context_pct, 10), context_pct),
                Style::default().fg(ctx_color)
            ),
            Span::styled(
                format!("{} tok ", fmt_tokens(self.session_total_out)),
                Style::default().fg(Color::DarkGray)
            ),
        ];

        // Cache info only when non-zero
        if self.cache_read > 0 || self.cache_created > 0 {
            spans.push(Span::styled(
                format!("c {}k/{}k ", self.cache_read / 1000, self.cache_created / 1000),
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

        // Status on the right - show real-time info
        let is_tool_activity = matches!(self.activity,
            Activity::Reading | Activity::Writing | Activity::Editing |
            Activity::Searching | Activity::Running | Activity::WebSearch |
            Activity::WebFetch | Activity::Tool(_)
        );
        
        let status_text = if self.activity == Activity::Idle {
            "Ready".to_string()
        } else if is_tool_activity {
            // Tool activity: show detail (e.g., "editing(file.rs)")
            if !self.activity_detail.is_empty() {
                format!("{}({})", self.activity.label(), self.activity_detail)
            } else {
                self.activity.label().to_string()
            }
        } else if !self.streaming.is_empty() {
            // Thinking with streaming text: show estimated tokens
            let estimated_tokens = self.streaming.chars().count() / 4;
            if estimated_tokens > 0 {
                fmt_tokens(estimated_tokens as u64)
            } else {
                "...".to_string()
            }
        } else if !self.thinking.is_empty() {
            // Thinking with thinking text: show estimated tokens
            let estimated_tokens = self.thinking.chars().count() / 4;
            if estimated_tokens > 0 {
                fmt_tokens(estimated_tokens as u64)
            } else {
                "...".to_string()
            }
        } else if self.activity == Activity::Thinking {
            // Thinking but no content yet: show 0 tokens
            "0".to_string()
        } else {
            // Default: show activity label
            self.activity.label().to_string()
        };
        let status_color = if self.activity == Activity::Idle { Color::Green } else { Color::Yellow };
        
        spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
        spans.push(Span::styled(format!(" {} ", status_text), Style::default().fg(status_color)));

        f.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    fn draw_messages(&self, f: &mut ratatui::Frame, area: Rect) {
        let mut lines: Vec<Line> = Vec::new();
        let max_w = area.width.saturating_sub(4) as usize;

        let selection = self.selection.map(|s| s.normalized());

        // Welcome (responsive) - MATRIX in solid █ (banner font) - Blue-purple gradient (tech style)
        if self.show_welcome && self.messages.is_empty() {
            // MATRIX (blue-purple gradient: deep blue → blue → cyan → purple)
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("█     █    █    ███████ ██████  ███ █     █ ", Style::default().fg(Color::Rgb(0, 51, 204))),   // deep blue
            ]));
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("██   ██   █ █      █    █     █  █   █   █  ", Style::default().fg(Color::Rgb(0, 102, 255))),  // blue
            ]));
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("█ █ █ █  █   █     █    █     █  █    █ █   ", Style::default().fg(Color::Rgb(0, 153, 255))),  // bright blue
            ]));
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("█  █  █ █     █    █    ██████   █     █    ", Style::default().fg(Color::Rgb(0, 204, 255))),  // cyan-blue
            ]));
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("█     █ ███████    █    █   █    █    █ █   ", Style::default().fg(Color::Cyan)),              // cyan
            ]));
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("█     █ █     █    █    █    █   █   █   █  ", Style::default().fg(Color::Rgb(153, 102, 255))), // bright purple
            ]));
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("█     █ █     █    █    █     █ ███ █     █ ", Style::default().fg(Color::Rgb(102, 51, 255))),  // purple
            ]));
            // Subtitle below
            lines.push(Line::styled("    AI coding assistant | /help for commands", Style::default().fg(Color::Gray)));
            lines.push(Line::raw(""));
        }

        // Render all messages
        for msg in &self.messages {
            match &msg.role {
                Role::User => {
                    // User: green left border + bold white text
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
                    // Assistant: separator with optional debug info
                    if self.debug_mode {
                        let token_info = format!("({}tok)", fmt_tokens(self.tokens_out));
                        let elapsed = self.request_start
                            .map(|s| format!(" {:.1}s", s.elapsed().as_secs_f64()))
                            .unwrap_or_default();
                        lines.push(Line::from(vec![
                            Span::styled("  \u{2500}\u{2500} \u{1f916} ", Style::default().fg(Color::DarkGray)),
                            Span::styled(format!("{}{}", token_info, elapsed), Style::default().fg(Color::DarkGray)),
                        ]));
                    } else {
                        lines.push(Line::styled("  \u{2500}\u{2500}", Style::default().fg(Color::DarkGray)));
                    }
                    let md_lines = render_markdown(&msg.content, max_w);
                    lines.extend(md_lines);
                    lines.push(Line::raw(""));
                }
                Role::Thinking => {
                    let line_count = msg.content.lines().count();
                    if self.thinking_collapsed && !self.debug_mode {
                        // Normal mode: collapsed
                        let preview = msg.content.lines().next().unwrap_or("");
                        lines.push(Line::from(vec![
                            Span::styled("  \u{1f4ad} \u{25b6} ", Style::default().fg(Color::DarkGray)),
                            Span::styled(
                                format!("({} lines) {}", line_count, truncate(preview, max_w.saturating_sub(20))),
                                Style::default().fg(Color::DarkGray)
                            ),
                        ]));
                    } else {
                        // Expanded (debug mode or user toggled) - show full content
                        lines.push(Line::from(vec![
                            Span::styled("  \u{1f4ad} \u{25bc} ", Style::default().fg(Color::DarkGray)),
                            Span::styled(
                                format!("Thinking ({} lines)", line_count),
                                Style::default().fg(Color::DarkGray)
                            ),
                        ]));
                        let md_lines = render_markdown(&msg.content, max_w.saturating_sub(4));
                        // Show all lines without limit
                        for line in md_lines.iter() {
                            let text = line.spans.iter().map(|s| s.content.as_ref()).collect::<String>();
                            // Dim gray for thinking content (less prominent than assistant)
                            lines.push(Line::styled(format!("    {}", text), Style::default().fg(Color::DarkGray)));
                        }
                    }
                }
                Role::Tool { name, is_error } => {
                    let status_icon = if *is_error { "\u{2717}" } else { "\u{2713}" };
                    let status_color = if *is_error { Color::Red } else { Color::Green };
                    let line_count = msg.content.lines().count();
                    let preview = msg.content.lines().next().unwrap_or("");

                    // Tool-specific icon for better visual identification
                    let tool_icon = match name.as_str() {
                        "read" => "\u{1f4d6}",      // 📖
                        "write" => "\u{1f4dd}",     // 📝
                        "edit" | "multi_edit" => "\u{270f}",  // ✏️
                        "bash" => "\u{26a1}",       // ⚡
                        "search" | "glob" | "ls" => "\u{1f50d}",  // 🔍
                        "websearch" => "\u{1f310}", // 🌐
                        "webfetch" => "\u{1f517}",  // 🔗
                        "ask" => "\u{2753}",        // ❓
                        _ => "\u{1f527}",           // 🔧
                    };

                    // Summary line (always shown)
                    let summary = if *is_error {
                        truncate(preview, max_w.saturating_sub(name.len() + 10))
                    } else {
                        match name.as_str() {
                            "read" => format!("{} lines", line_count),
                            "write" => "written".into(),
                            "edit" | "multi_edit" => "applied".into(),
                            "bash" => {
                                if line_count <= 1 {
                                    truncate(preview, max_w.saturating_sub(name.len() + 10))
                                } else {
                                    format!("{} lines output", line_count)
                                }
                            }
                            "search" | "glob" | "ls" => format!("{} results", line_count),
                            _ => truncate(preview, max_w.saturating_sub(name.len() + 10)),
                        }
                    };

                    // Tool header line - prominent with bold name
                    lines.push(Line::from(vec![
                        Span::styled(format!("  {} ", tool_icon), Style::default().fg(status_color)),
                        Span::styled(name.clone(), Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
                        Span::styled(" ", Style::default()),
                        Span::styled(status_icon, Style::default().fg(status_color)),
                        Span::styled(format!(" \u{2192} {}", summary), Style::default().fg(Color::Gray)),
                    ]));

                    // Content preview: detect diff format (lines starting with "- " or "+ " after edit header)
                    // More strict detection: first line must be "Successfully edited" and following lines must start with "- " or "+ "
                    let content_lines: Vec<&str> = msg.content.lines().collect();
                    let has_diff = content_lines.len() > 1 
                        && content_lines.first().map(|l| l.starts_with("Successfully edited")).unwrap_or(false)
                        && content_lines.iter().skip(1).any(|l| l.starts_with("- ") || l.starts_with("+ "));
                    let preview_count = if *is_error {
                        if self.debug_mode { 8 } else { 3 }
                    } else if self.debug_mode {
                        5
                    } else if has_diff {
                        4  // Show diff lines for edit/multi_edit
                    } else {
                        match name.as_str() {
                            "bash" => 2,
                            "search" | "glob" | "ls" => 2,
                            "read" => 3,
                            "todo_write" => 0,  // Special handling below
                            "write" => 0,
                            _ => 1,
                        }
                    };

                    // Special rendering for todo_write: show full list with colored status
                    if name == "todo_write" && !*is_error {
                        for line in msg.content.lines().skip(1) {
                            let trimmed = line.trim();
                            let line_color = if trimmed.starts_with("[~]") {
                                Color::Yellow
                            } else if trimmed.starts_with("[x]") {
                                Color::Green
                            } else if trimmed.starts_with("[ ]") {
                                Color::Gray
                            } else {
                                Color::DarkGray
                            };
                            lines.push(Line::styled(
                                format!("    {}", truncate(trimmed, max_w.saturating_sub(4))),
                                Style::default().fg(line_color)
                            ));
                        }
                    } else if preview_count > 0 {
                        // Different handling for read vs other tools
                        if name == "read" {
                            // Read: show first lines directly (no header to skip)
                            for line in msg.content.lines().take(preview_count) {
                                lines.push(Line::styled(
                                    format!("    {}", truncate(line, max_w.saturating_sub(4))),
                                    Style::default().fg(Color::Gray)
                                ));
                            }
                            let total_lines = msg.content.lines().count();
                            if total_lines > preview_count {
                                lines.push(Line::styled(
                                    format!("    \u{2026} ({} more)", total_lines - preview_count),
                                    Style::default().fg(Color::DarkGray)
                                ));
                            }
                        } else {
                            // Other tools: skip first line (summary header)
                            // Diff display: use bright colors and clear markers for edit changes
                            for line in msg.content.lines().skip(1).take(preview_count) {
                                let (marker, line_style) = if line.starts_with("+ ") {
                                    // Added line: bright green with ✓ marker
                                    ("✓", Style::default().fg(Color::LightGreen))
                                } else if line.starts_with("- ") {
                                    // Removed line: bright red with ✗ marker
                                    ("✗", Style::default().fg(Color::LightRed))
                                } else {
                                    // Other content: dim
                                    (" ", Style::default().fg(Color::DarkGray))
                                };
                                let truncated = truncate(line, max_w.saturating_sub(6));
                                lines.push(Line::styled(
                                    format!("  {} {}", marker, truncated),
                                    line_style
                                ));
                            }
                            let total_lines = msg.content.lines().skip(1).count();
                            if total_lines > preview_count {
                                lines.push(Line::styled(
                                    format!("    \u{2026} ({} more lines)", total_lines - preview_count),
                                    Style::default().fg(Color::DarkGray)
                                ));
                            }
                        }
                    }
                }
                Role::System => {
                    let content = &msg.content;
                    if content.contains("APPROVAL REQUIRED") || content.contains("requires approval") || content.contains("Allow?") {
                        // Approval: prominent red bold
                        let wrapped = word_wrap(content, max_w);
                        for line in wrapped {
                            lines.push(Line::styled(format!("  ⚡ {}", line), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)));
                        }
                        lines.push(Line::raw(""));
                    } else if self.debug_mode || content.contains('\n') {
                        // Debug mode or multi-line: show full content
                        for line in content.lines() {
                            lines.push(Line::styled(
                                format!("  {}", truncate(line, max_w)),
                                Style::default().fg(Color::DarkGray)
                            ));
                        }
                        lines.push(Line::raw(""));
                    } else {
                        // Normal: single line compact
                        lines.push(Line::styled(
                            format!("  {}", truncate(content, max_w)),
                            Style::default().fg(Color::DarkGray)
                        ));
                    }
                }
                Role::Ask => {
                    // Ask/Approval requests - VERY PROMINENT display
                    // Yellow background with bright content
                    lines.push(Line::styled("", Style::default()));
                    for line in msg.content.lines() {
                        // Use bright yellow/white text with bold for prominence
                        let styled_line = if line.contains("AWAITING INPUT") || line.contains("⚡") {
                            Line::styled(
                                line.to_string(),
                                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                            )
                        } else if line.starts_with("╔") || line.starts_with("║") || line.starts_with("╚") || line.starts_with("─") {
                            Line::styled(
                                line.to_string(),
                                Style::default().fg(Color::Cyan)
                            )
                        } else if line.starts_with("📌") || line.starts_with("▸") {
                            Line::styled(
                                line.to_string(),
                                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                            )
                        } else {
                            Line::styled(
                                line.to_string(),
                                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                            )
                        };
                        lines.push(styled_line);
                    }
                    lines.push(Line::styled("", Style::default()));
                }
            }
        }

        // Current thinking (streaming)
        if !self.thinking.is_empty() {
            if self.thinking_collapsed && !self.debug_mode {
                let preview = self.thinking.lines().next().unwrap_or("");
                lines.push(Line::from(vec![
                    Span::styled("  \u{1f4ad} \u{25b6} ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("Thinking... {}", truncate(preview, max_w.saturating_sub(20))),
                        Style::default().fg(Color::DarkGray)
                    ),
                ]));
            } else {
                // Expanded - show full content during streaming
                lines.push(Line::from(vec![
                    Span::styled("  \u{1f4ad} \u{25bc} ", Style::default().fg(Color::DarkGray)),
                    Span::styled("Thinking...", Style::default().fg(Color::DarkGray)),
                ]));
                let md_lines = render_markdown(&self.thinking, max_w.saturating_sub(4));
                // Show all lines without limit
                for line in md_lines.iter() {
                    let text = line.spans.iter().map(|s| s.content.as_ref()).collect::<String>();
                    lines.push(Line::styled(format!("    {}", text), Style::default().fg(Color::DarkGray)));
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
            let elapsed = self.request_start
                .map(|s| format!(" ({:.1}s)", s.elapsed().as_secs_f64()))
                .unwrap_or_default();
            let spinner_frame = self.frame % SPINNER.len();
            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", SPINNER[spinner_frame]), Style::default().fg(Color::LightGreen)),
                Span::styled(format!("Thinking...{}", elapsed), Style::default().fg(Color::DarkGray)),
            ]));
        }

        if is_tool_activity && self.streaming.is_empty() && self.thinking.is_empty() {
            let tool_label = if !self.activity_detail.is_empty() {
                format!("{}({})", self.activity.label(), self.activity_detail)
            } else {
                self.activity.label()
            };
            let elapsed = self.request_start
                .map(|s| format!(" {:.1}s)", s.elapsed().as_secs_f64()))
                .unwrap_or_default();
            let spinner_frame = self.frame % SPINNER.len();
            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", SPINNER[spinner_frame]), Style::default().fg(Color::LightGreen)),
                Span::styled(tool_label, Style::default().fg(self.activity.color())),
                Span::styled(elapsed, Style::default().fg(Color::DarkGray)),
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

        // Selection highlight (preserve span styles)
        let highlighted_lines = if let Some(sel) = selection {
            lines.into_iter().enumerate().map(|(i, line)| {
                if i >= sel.start_line && i <= sel.end_line {
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

        // Render with scroll indicator
        if !self.auto_scroll && max_scroll > 0 {
            let pct = (scroll_offset as f64 / max_scroll as f64 * 100.0) as u16;
            let indicator = Line::styled(
                format!("  ↑ {}/{} ({:.0}%) — End to bottom", scroll_offset, max_scroll, pct),
                Style::default().fg(Color::DarkGray)
            );
            let indicator_area = Rect::new(area.x, area.y, area.width, 1);
            f.render_widget(Paragraph::new(indicator), indicator_area);
            // Note: scroll_offset is relative to total lines, not the visible area
            // The scroll indicator just shows position, doesn't affect scrolling
            let msg_area = Rect::new(area.x, area.y + 1, area.width, area.height.saturating_sub(1));
            f.render_widget(Paragraph::new(highlighted_lines).scroll((scroll_offset, 0)), msg_area);
        } else {
            f.render_widget(Paragraph::new(highlighted_lines).scroll((scroll_offset, 0)), area);
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
            Activity::Asking => ("⚡ ", Color::Red),
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
                spans.push(Span::styled("[AWAITING REPLY: y/n or option] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)));
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
