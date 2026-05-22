//! Messages area rendering.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::SPINNER;
use crate::app::TuiApp;
use crate::markdown::render_markdown;
use crate::types::{Activity, Role, SubmitMode};
use crate::utils::{fmt_tokens, truncate, word_wrap};

impl TuiApp {
    pub(crate) fn draw_messages(&self, f: &mut ratatui::Frame, area: Rect) {
        let mut lines: Vec<Line> = Vec::new();
        let max_w = area.width.saturating_sub(4) as usize;

        // Welcome (responsive) - MATRIX in solid █ (banner font) - Blue-purple gradient (tech style)
        if self.show_welcome && self.messages.is_empty() {
            // MATRIX (blue-purple gradient: deep blue → blue → cyan → purple)
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    "█     █    █    ███████ ██████  ███ █     █ ",
                    Style::default().fg(Color::Rgb(0, 51, 204)),
                ), // deep blue
            ]));
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    "██   ██   █ █      █    █     █  █   █   █  ",
                    Style::default().fg(Color::Rgb(0, 102, 255)),
                ), // blue
            ]));
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    "█ █ █ █  █   █     █    █     █  █    █ █   ",
                    Style::default().fg(Color::Rgb(0, 153, 255)),
                ), // bright blue
            ]));
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    "█  █  █ █     █    █    ██████   █     █    ",
                    Style::default().fg(Color::Rgb(0, 204, 255)),
                ), // cyan-blue
            ]));
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    "█     █ ███████    █    █   █    █    █ █   ",
                    Style::default().fg(Color::Cyan),
                ), // cyan
            ]));
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    "█     █ █     █    █    █    █   █   █   █  ",
                    Style::default().fg(Color::Rgb(153, 102, 255)),
                ), // bright purple
            ]));
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    "█     █ █     █    █    █     █ ███ █     █ ",
                    Style::default().fg(Color::Rgb(102, 51, 255)),
                ), // purple
            ]));
            // Subtitle below
            lines.push(Line::styled(
                "    AI coding assistant | /help for commands",
                Style::default().fg(Color::Gray),
            ));
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
                            Span::styled(
                                line,
                                Style::default()
                                    .fg(Color::White)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]));
                    }
                    lines.push(Line::raw(""));
                }
                Role::Assistant => {
                    // Assistant: separator with optional debug info
                    if self.debug_mode {
                        let token_info = format!("({}tok)", fmt_tokens(self.tokens_out));
                        let elapsed = self
                            .request_start
                            .map(|s| format!(" {:.1}s", s.elapsed().as_secs_f64()))
                            .unwrap_or_default();
                        lines.push(Line::from(vec![
                            Span::styled(
                                "  \u{2500}\u{2500} \u{1f916} ",
                                Style::default().fg(Color::DarkGray),
                            ),
                            Span::styled(
                                format!("{}{}", token_info, elapsed),
                                Style::default().fg(Color::DarkGray),
                            ),
                        ]));
                    } else {
                        lines.push(Line::styled(
                            "  \u{2500}\u{2500}",
                            Style::default().fg(Color::DarkGray),
                        ));
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
                            Span::styled(
                                "  \u{1f4ad} \u{25b6} ",
                                Style::default().fg(Color::DarkGray),
                            ),
                            Span::styled(
                                format!(
                                    "({} lines) {}",
                                    line_count,
                                    truncate(preview, max_w.saturating_sub(20))
                                ),
                                Style::default().fg(Color::DarkGray),
                            ),
                        ]));
                    } else {
                        // Expanded (debug mode or user toggled) - show full content
                        lines.push(Line::from(vec![
                            Span::styled(
                                "  \u{1f4ad} \u{25bc} ",
                                Style::default().fg(Color::DarkGray),
                            ),
                            Span::styled(
                                format!("Thinking ({} lines)", line_count),
                                Style::default().fg(Color::DarkGray),
                            ),
                        ]));
                        let md_lines = render_markdown(&msg.content, max_w.saturating_sub(4));
                        // Show all lines without limit
                        for line in md_lines.iter() {
                            let text = line
                                .spans
                                .iter()
                                .map(|s| s.content.as_ref())
                                .collect::<String>();
                            // Dim gray for thinking content (less prominent than assistant)
                            lines.push(Line::styled(
                                format!("    {}", text),
                                Style::default().fg(Color::DarkGray),
                            ));
                        }
                    }
                }
                Role::Tool {
                    name,
                    detail,
                    is_error,
                } => {
                    let status_icon = if *is_error { "\u{2717}" } else { "\u{2713}" };
                    let status_color = if *is_error { Color::Red } else { Color::Green };
                    let line_count = msg.content.lines().count();
                    let preview = msg.content.lines().next().unwrap_or("");

                    // Tool-specific icon for better visual identification
                    let tool_icon = match name.as_str() {
                        "read" => "\u{1f4d6}",                   // 📖
                        "write" => "\u{1f4dd}",                  // 📝
                        "edit" | "multi_edit" => "\u{270f}",     // ✏️
                        "bash" => "\u{26a1}",                    // ⚡
                        "search" | "glob" | "ls" => "\u{1f50d}", // 🔍
                        "websearch" => "\u{1f310}",              // 🌐
                        "webfetch" => "\u{1f517}",               // 🔗
                        "ask" => "\u{2753}",                     // ❓
                        _ => "\u{1f527}",                        // 🔧
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

                    // Tool header line - prominent with bold name and detail
                    let detail_text = detail.as_ref().map(|d| truncate(d, 40)).unwrap_or_default();
                    let detail_span = if detail_text.is_empty() {
                        Span::styled(
                            format!(" \u{2192} {}", summary),
                            Style::default().fg(Color::Gray),
                        )
                    } else {
                        Span::styled(
                            format!(" {} \u{2192} {}", detail_text, summary),
                            Style::default().fg(Color::Cyan),
                        )
                    };

                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("  {} ", tool_icon),
                            Style::default().fg(status_color),
                        ),
                        Span::styled(
                            name.clone(),
                            Style::default()
                                .fg(status_color)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(" ", Style::default()),
                        Span::styled(status_icon, Style::default().fg(status_color)),
                        detail_span,
                    ]));

                    // Content preview: detect diff format (lines starting with "- " or "+ " after edit header)
                    // More strict detection: first line must be "Successfully edited" and following lines must start with "- " or "+ "
                    let content_lines: Vec<&str> = msg.content.lines().collect();
                    let has_diff = content_lines.len() > 1
                        && content_lines
                            .first()
                            .map(|l| l.starts_with("Successfully edited"))
                            .unwrap_or(false)
                        && content_lines
                            .iter()
                            .skip(1)
                            .any(|l| l.starts_with("- ") || l.starts_with("+ "));
                    let preview_count = if *is_error {
                        if self.debug_mode { 8 } else { 3 }
                    } else if self.debug_mode {
                        5
                    } else if has_diff {
                        4 // Show diff lines for edit/multi_edit
                    } else {
                        match name.as_str() {
                            "bash" => 2,
                            "search" | "glob" | "ls" => 2,
                            "read" => 3,
                            "todo_write" => 0, // Special handling below
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
                                Style::default().fg(line_color),
                            ));
                        }
                    } else if preview_count > 0 {
                        // Different handling for read vs other tools
                        if name == "read" {
                            // Read: show first lines directly (no header to skip)
                            for line in msg.content.lines().take(preview_count) {
                                lines.push(Line::styled(
                                    format!("    {}", truncate(line, max_w.saturating_sub(4))),
                                    Style::default().fg(Color::Gray),
                                ));
                            }
                            let total_lines = msg.content.lines().count();
                            if total_lines > preview_count {
                                lines.push(Line::styled(
                                    format!("    \u{2026} ({} more)", total_lines - preview_count),
                                    Style::default().fg(Color::DarkGray),
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
                                    line_style,
                                ));
                            }
                            let total_lines = msg.content.lines().skip(1).count();
                            if total_lines > preview_count {
                                lines.push(Line::styled(
                                    format!(
                                        "    \u{2026} ({} more lines)",
                                        total_lines - preview_count
                                    ),
                                    Style::default().fg(Color::DarkGray),
                                ));
                            }
                        }
                    }
                }
                Role::System => {
                    let content = &msg.content;
                    if content.contains("APPROVAL REQUIRED")
                        || content.contains("requires approval")
                        || content.contains("Allow?")
                    {
                        // Approval: prominent red bold
                        let wrapped = word_wrap(content, max_w);
                        for line in wrapped {
                            lines.push(Line::styled(
                                format!("  ⚡ {}", line),
                                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                            ));
                        }
                        lines.push(Line::raw(""));
                    } else if self.debug_mode || content.contains('\n') {
                        // Debug mode or multi-line: show full content
                        for line in content.lines() {
                            lines.push(Line::styled(
                                format!("  {}", truncate(line, max_w)),
                                Style::default().fg(Color::DarkGray),
                            ));
                        }
                        lines.push(Line::raw(""));
                    } else {
                        // Normal: single line compact
                        lines.push(Line::styled(
                            format!("  {}", truncate(content, max_w)),
                            Style::default().fg(Color::DarkGray),
                        ));
                    }
                }
                Role::Ask => {
                    // Ask/Approval requests - prominent display with selection highlight
                    lines.push(Line::styled("", Style::default()));

                    // Draw Tab headers for multi-question mode
                    if self.waiting_for_ask && self.ask_questions.len() > 1 {
                        let tabs: Vec<Span> = self
                            .ask_questions
                            .iter()
                            .enumerate()
                            .map(|(idx, _q)| {
                                let is_current = idx == self.current_question_idx;
                                let tab_text = format!(" 问题{} ", idx + 1);
                                if is_current {
                                    Span::styled(
                                        tab_text,
                                        Style::default()
                                            .fg(Color::Black)
                                            .bg(Color::Cyan)
                                            .add_modifier(Modifier::BOLD),
                                    )
                                } else {
                                    Span::styled(
                                        tab_text,
                                        Style::default().fg(Color::DarkGray).bg(Color::Reset),
                                    )
                                }
                            })
                            .collect();

                        // Add separator spans
                        let mut all_spans = Vec::new();
                        for (i, span) in tabs.into_iter().enumerate() {
                            all_spans.push(span);
                            if i < self.ask_questions.len() - 1 {
                                all_spans
                                    .push(Span::styled("│", Style::default().fg(Color::DarkGray)));
                            }
                        }
                        all_spans.push(Span::styled(
                            "  [Tab切换]",
                            Style::default().fg(Color::DarkGray),
                        ));

                        lines.push(Line::from(all_spans));
                        lines.push(Line::styled("", Style::default()));
                    }

                    // Check if we're actively in ask mode with options
                    let has_active_selection = self.waiting_for_ask && !self.ask_options.is_empty();

                    for line in msg.content.lines() {
                        let styled_line = if line.contains("AWAITING INPUT") || line.contains("⚡")
                        {
                            Line::styled(
                                line.to_string(),
                                Style::default()
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::BOLD),
                            )
                        } else if ['╔', '║', '╚', '─'].contains(&line.chars().next().unwrap_or(' '))
                        {
                            Line::styled(line.to_string(), Style::default().fg(Color::Cyan))
                        } else if line.starts_with("📌") || line.starts_with("▸") {
                            Line::styled(
                                line.to_string(),
                                Style::default()
                                    .fg(Color::Green)
                                    .add_modifier(Modifier::BOLD),
                            )
                        } else if has_active_selection && line.starts_with("  [") {
                            // Option line - parse and highlight
                            // Multi-select: [✓] or [ ], Single-select: [A], [B], [Y], [N]
                            let is_checkbox = line.contains("[✓]") || line.contains("[ ]");

                            // Determine option index
                            let option_idx = if is_checkbox {
                                // Find index by counting checkbox lines in current message
                                let mut idx = 0;
                                for l in msg.content.lines() {
                                    if l == line {
                                        break;
                                    }
                                    if l.starts_with("  [")
                                        && (l.contains("[✓]") || l.contains("[ ]"))
                                    {
                                        idx += 1;
                                    }
                                }
                                idx
                            } else {
                                // Single select: parse letter from "[X]"
                                // Format: "  [A] label" - letter is at position 3
                                let letter = line.chars().nth(3).unwrap_or(' ');
                                if letter == 'Y' {
                                    0
                                } else if letter == 'N' {
                                    1
                                } else if letter.is_ascii_uppercase() {
                                    (letter as u8 - b'A') as usize
                                } else {
                                    0
                                }
                            };

                            // Get actual checked state from ask_options (not from static text)
                            let actually_checked = if option_idx < self.ask_options.len() {
                                self.ask_options[option_idx].selected
                            } else {
                                line.contains("[✓]") // Fallback to text
                            };

                            // Check if this is a Submit option
                            let is_submit_option = option_idx < self.ask_options.len()
                                && self.ask_options[option_idx].is_submit;

                            // Rebuild line with actual checkbox state
                            let display_line = if is_checkbox && option_idx < self.ask_options.len()
                            {
                                let opt = &self.ask_options[option_idx];
                                let marker = if actually_checked { "[✓]" } else { "[ ]" };
                                format!("  {} {}{}", marker, opt.label, opt.format_description())
                            } else {
                                line.to_string()
                            };

                            // Check if this line matches current selection index
                            if option_idx == self.ask_selected_index {
                                // Current navigation position: bright highlight
                                if is_submit_option {
                                    // Submit option - yellow highlight
                                    if actually_checked {
                                        Line::styled(
                                            format!("▶ {}", display_line.trim()),
                                            Style::default()
                                                .fg(Color::Black)
                                                .bg(Color::Yellow)
                                                .add_modifier(Modifier::BOLD),
                                        )
                                    } else {
                                        Line::styled(
                                            format!("▶ {}", display_line.trim()),
                                            Style::default()
                                                .fg(Color::Black)
                                                .bg(Color::Cyan)
                                                .add_modifier(Modifier::BOLD),
                                        )
                                    }
                                } else if actually_checked {
                                    // Checked and selected: bright green
                                    Line::styled(
                                        format!("▶ {}", display_line.trim()),
                                        Style::default()
                                            .fg(Color::Black)
                                            .bg(Color::Green)
                                            .add_modifier(Modifier::BOLD),
                                    )
                                } else {
                                    // Unchecked but navigated: cyan highlight
                                    Line::styled(
                                        format!("▶ {}", display_line.trim()),
                                        Style::default()
                                            .fg(Color::Black)
                                            .bg(Color::Cyan)
                                            .add_modifier(Modifier::BOLD),
                                    )
                                }
                            } else if is_submit_option {
                                // Submit option not current
                                if actually_checked {
                                    Line::styled(
                                        format!("  {}", display_line.trim()),
                                        Style::default()
                                            .fg(Color::Yellow)
                                            .add_modifier(Modifier::BOLD),
                                    )
                                } else {
                                    Line::styled(
                                        format!("  {}", display_line.trim()),
                                        Style::default().fg(Color::White),
                                    )
                                }
                            } else if actually_checked {
                                // Checked but not current navigation: green text
                                Line::styled(
                                    format!("  {}", display_line.trim()),
                                    Style::default()
                                        .fg(Color::Green)
                                        .add_modifier(Modifier::BOLD),
                                )
                            } else {
                                // Regular option
                                Line::styled(
                                    format!("  {}", display_line.trim()),
                                    Style::default().fg(Color::White),
                                )
                            }
                        } else if has_active_selection && line.starts_with("  >>>") {
                            // Legacy Submit option format (for backward compatibility)
                            let submit_idx = self.ask_options.len() - 1;
                            if self.ask_selected_index == submit_idx {
                                Line::styled(
                                    format!(
                                        "▶ {}",
                                        line.trim().replace(">>>", "").replace("<<<", "").trim()
                                    ),
                                    Style::default()
                                        .fg(Color::Black)
                                        .bg(Color::Yellow)
                                        .add_modifier(Modifier::BOLD),
                                )
                            } else {
                                Line::styled(
                                    format!(
                                        "  {}",
                                        line.trim().replace(">>>", "").replace("<<<", "").trim()
                                    ),
                                    Style::default()
                                        .fg(Color::Yellow)
                                        .add_modifier(Modifier::BOLD),
                                )
                            }
                        } else {
                            Line::styled(
                                line.to_string(),
                                Style::default()
                                    .fg(Color::White)
                                    .add_modifier(Modifier::BOLD),
                            )
                        };
                        lines.push(styled_line);
                    }

                    // Button mode: show submit button area at bottom
                    if self.waiting_for_ask
                        && self.ask_submit_mode == SubmitMode::Button
                        && self.ask_multi_select
                    {
                        lines.push(Line::styled(
                            "─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─",
                            Style::default().fg(Color::DarkGray),
                        ));
                        let selected_count =
                            self.ask_options.iter().filter(|opt| opt.selected).count();
                        if selected_count > 0 {
                            lines.push(Line::styled(
                                format!("  [Enter] 提交 {} 个选中项", selected_count),
                                Style::default()
                                    .fg(Color::Green)
                                    .add_modifier(Modifier::BOLD),
                            ));
                        } else {
                            lines.push(Line::styled(
                                "  [Enter] 提交 (无选中项)",
                                Style::default().fg(Color::DarkGray),
                            ));
                        }
                    }

                    // "Other" input mode: show input prompt
                    if self.waiting_for_ask && self.ask_other_input_active {
                        lines.push(Line::styled(
                            "─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─",
                            Style::default().fg(Color::DarkGray),
                        ));
                        lines.push(Line::styled(
                            "  ✏️ 输入自定义内容:",
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ));
                        // Show current input with cursor indicator
                        let input_display = if self.input.is_empty() {
                            "_"
                        } else {
                            &self.input
                        };
                        lines.push(Line::styled(
                            format!("  {}", input_display),
                            Style::default().fg(Color::White),
                        ));
                        lines.push(Line::styled(
                            "  [Enter] 确认  [Esc] 返回选择",
                            Style::default().fg(Color::DarkGray),
                        ));
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
                    Span::styled(
                        "  \u{1f4ad} \u{25b6} ",
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        format!(
                            "Thinking... {}",
                            truncate(preview, max_w.saturating_sub(20))
                        ),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            } else {
                // Expanded - show full content during streaming
                lines.push(Line::from(vec![
                    Span::styled(
                        "  \u{1f4ad} \u{25bc} ",
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled("Thinking...", Style::default().fg(Color::DarkGray)),
                ]));
                let md_lines = render_markdown(&self.thinking, max_w.saturating_sub(4));
                // Show all lines without limit
                for line in md_lines.iter() {
                    let text = line
                        .spans
                        .iter()
                        .map(|s| s.content.as_ref())
                        .collect::<String>();
                    lines.push(Line::styled(
                        format!("    {}", text),
                        Style::default().fg(Color::DarkGray),
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
        let is_tool_activity = matches!(
            self.activity,
            Activity::Reading
                | Activity::Writing
                | Activity::Editing
                | Activity::Searching
                | Activity::Running
                | Activity::WebSearch
                | Activity::WebFetch
                | Activity::Tool(_)
        );

        if self.activity == Activity::Thinking
            && self.streaming.is_empty()
            && self.thinking.is_empty()
        {
            let elapsed = self
                .request_start
                .map(|s| format!(" ({:.1}s)", s.elapsed().as_secs_f64()))
                .unwrap_or_default();
            let spinner_frame = self.frame % SPINNER.len();
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {} ", SPINNER[spinner_frame]),
                    Style::default().fg(Color::LightGreen),
                ),
                Span::styled(
                    format!("Thinking...{}", elapsed),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }

        if is_tool_activity && self.streaming.is_empty() && self.thinking.is_empty() {
            // Tool icon for visual identification
            let tool_icon = match self.activity {
                Activity::Reading => "📖",
                Activity::Writing => "📝",
                Activity::Editing => "✏️",
                Activity::Searching => "🔍",
                Activity::Running => "⚡",
                Activity::WebSearch => "🌐",
                Activity::WebFetch => "🔗",
                Activity::Tool(ref name) => match name.as_str() {
                    "task" => "🚀",
                    "plan" => "📋",
                    "monitor" => "👀",
                    "skill" => "⚡",
                    _ => "🔧",
                },
                _ => "⚙️",
            };

            let tool_label = if !self.activity_detail.is_empty() {
                format!("{} {}", self.activity.label(), self.activity_detail)
            } else {
                self.activity.label()
            };
            let elapsed = self
                .request_start
                .map(|s| format!(" ({:.1}s)", s.elapsed().as_secs_f64()))
                .unwrap_or_default();
            let spinner_frame = self.frame % SPINNER.len();
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {} ", SPINNER[spinner_frame]),
                    Style::default().fg(Color::LightGreen),
                ),
                Span::styled(
                    format!("{} ", tool_icon),
                    Style::default().fg(self.activity.color()),
                ),
                Span::styled(
                    tool_label,
                    Style::default()
                        .fg(self.activity.color())
                        .add_modifier(Modifier::BOLD),
                ),
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

        // Respect user's auto_scroll setting - no force auto scroll
        // User can scroll to bottom to re-enable auto_scroll
        let scroll_offset = if self.auto_scroll {
            max_scroll
        } else {
            self.scroll_offset.min(max_scroll)
        };

        // Render with scroll indicator
        if !self.auto_scroll && max_scroll > 0 {
            let pct = (scroll_offset as f64 / max_scroll as f64 * 100.0) as u16;
            let indicator = Line::styled(
                format!(
                    "  ↑ {}/{} ({:.0}%) — End to bottom",
                    scroll_offset, max_scroll, pct
                ),
                Style::default().fg(Color::DarkGray),
            );
            let indicator_area = Rect::new(area.x, area.y, area.width, 1);
            f.render_widget(Paragraph::new(indicator), indicator_area);
            let msg_area = Rect::new(
                area.x,
                area.y + 1,
                area.width,
                area.height.saturating_sub(1),
            );
            f.render_widget(Paragraph::new(lines).scroll((scroll_offset, 0)), msg_area);
        } else {
            f.render_widget(Paragraph::new(lines).scroll((scroll_offset, 0)), area);
        }
    }
}
