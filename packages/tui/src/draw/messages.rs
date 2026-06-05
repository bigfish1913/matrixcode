//! Messages area rendering.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use serde_json::Value;

use crate::BORDER_PADDING;
use crate::SPINNER;
use crate::app::TuiApp;
use crate::draw::helpers::estimate_message_tokens;
use crate::markdown::render_markdown;
use crate::types::{Activity, Role, SubmitMode};
use crate::utils::{fmt_tokens, truncate, word_wrap};

fn push_user_message_lines(lines: &mut Vec<Line<'_>>, content: &str, max_w: usize, is_pending: bool) {
    let wrapped = word_wrap(content, max_w.saturating_sub(2));
    for line in wrapped {
        let status = if is_pending { " ⏳" } else { "" };
        lines.push(Line::from(vec![
            Span::styled("│ ", Style::default().fg(Color::Green)),
            Span::styled(
                format!("{}{}", line, status),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
    }
    lines.push(Line::raw(""));
}

/// Extract full detail lines from tool input for display
fn extract_full_detail(tool_name: &str, input: &Value) -> Vec<String> {
    let mut lines = Vec::new();

    match tool_name {
        "bash" => {
            if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
                // Show full command, wrap if too long
                for line in cmd.lines() {
                    lines.push(format!("$ {}", line));
                }
            }
        }
        "read" => {
            if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
                lines.push(format!("file: {}", path));
            }
            if let Some(offset) = input.get("offset").and_then(|v| v.as_u64()) {
                lines.push(format!("offset: {}", offset));
            }
            if let Some(limit) = input.get("limit").and_then(|v| v.as_u64()) {
                lines.push(format!("limit: {}", limit));
            }
        }
        "write" | "edit" | "multi_edit" => {
            if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
                lines.push(format!("file: {}", path));
            }
        }
        "search" | "grep" | "glob" => {
            if let Some(pattern) = input.get("pattern").and_then(|v| v.as_str()) {
                lines.push(format!("pattern: {}", pattern));
            }
            if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
                lines.push(format!("path: {}", path));
            }
        }
        "websearch" => {
            if let Some(query) = input.get("query").and_then(|v| v.as_str()) {
                lines.push(format!("query: {}", query));
            }
        }
        "webfetch" => {
            if let Some(url) = input.get("url").and_then(|v| v.as_str()) {
                lines.push(format!("url: {}", url));
            }
        }
        _ => {
            // Generic: show key fields
            if let Some(obj) = input.as_object() {
                for (key, value) in obj.iter().take(3) {
                    if let Some(val_str) = value.as_str() {
                        lines.push(format!("{}: {}", key, truncate(val_str, 80)));
                    }
                }
            }
        }
    }

    lines
}

impl TuiApp {
    pub(crate) fn draw_messages(&self, f: &mut ratatui::Frame, area: Rect) {
        let mut lines: Vec<Line> = Vec::new();
        let max_w = area.width.saturating_sub(BORDER_PADDING as u16) as usize;

        // Welcome (responsive) - adapt to screen height
        if self.show_welcome && self.messages.is_empty() {
            // Full ASCII art needs ~20 lines to leave room for input/status
            if area.height >= 20 {
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
                let version = env!("CARGO_PKG_VERSION");
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("    AI 编码助手 | v{} | /help 帮助", version),
                        Style::default().fg(Color::Gray),
                    ),
                ]));
            } else {
                // Compact mode for small screens: just version and hint
                let version = env!("CARGO_PKG_VERSION");
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("  MatrixCode v{}", version),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "  AI 编码助手",
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
                lines.push(Line::styled(
                    "  /help 帮助 | /shortcuts 快捷键 | [Ctrl+V] 粘贴",
                    Style::default().fg(Color::DarkGray),
                ));
            }
            lines.push(Line::raw(""));
        }

        // Render all messages
        for msg in &self.messages {
            match &msg.role {
                Role::User => {
                    // User: green left border + bold white text
                    // Pending appended messages are rendered after the current live output below.
                    if !msg.is_pending {
                        push_user_message_lines(&mut lines, &msg.content, max_w, false);
                    }
                }
                Role::Assistant => {
                    // Assistant: separator with optional debug info
                    if self.debug_mode {
                        let token_info = format!("({}tok)", fmt_tokens(self.tokens_out));
                        lines.push(Line::from(vec![
                            Span::styled("  ─── 🤖 ", Style::default().fg(Color::DarkGray)),
                            Span::styled(token_info, Style::default().fg(Color::DarkGray)),
                        ]));
                    } else {
                        lines.push(Line::styled("  ───", Style::default().fg(Color::DarkGray)));
                    }
                    let md_lines = render_markdown(&msg.content, max_w);
                    lines.extend(md_lines);
                    lines.push(Line::raw(""));
                }
                Role::Thinking => {
                    let line_count = msg.content.lines().count();
                    if self.thinking_collapsed && !self.debug_mode {
                        // Collapsed mode: show preview with expand hint
                        let preview = msg.content.lines().next().unwrap_or("");
                        lines.push(Line::from(vec![
                            Span::styled(
                                "  \u{1f4ad} \u{25b6} ",
                                Style::default().fg(Color::DarkGray),
                            ),
                            Span::styled(
                                format!(
                                    "({} 行) {}",
                                    line_count,
                                    truncate(preview, max_w.saturating_sub(30))
                                ),
                                Style::default().fg(Color::DarkGray),
                            ),
                            Span::styled(" [Alt+T] 展开", Style::default().fg(Color::DarkGray)),
                        ]));
                    } else {
                        // Expanded mode: show full content
                        // In debug mode, show extra info like token count
                        let debug_info = if self.debug_mode {
                            let tok = estimate_message_tokens(&msg.content) as u64;
                            format!(" ({} 行, ~{}tok)", line_count, fmt_tokens(tok))
                        } else {
                            format!(" ({} 行)", line_count)
                        };
                        lines.push(Line::from(vec![
                            Span::styled(
                                "  \u{1f4ad} \u{25bc} ",
                                Style::default().fg(Color::DarkGray),
                            ),
                            Span::styled(
                                format!("思考内容{}", debug_info),
                                Style::default().fg(Color::DarkGray),
                            ),
                            if !self.debug_mode {
                                Span::styled(" [Alt+T] 折叠", Style::default().fg(Color::DarkGray))
                            } else {
                                Span::raw("")
                            },
                        ]));
                        let md_lines =
                            render_markdown(&msg.content, max_w.saturating_sub(BORDER_PADDING));
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
                    is_pending,
                } => {
                    // Skip pending state rendering - activity bar handles it
                    if *is_pending {
                        continue;
                    }

                    // Completed tool: show result
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
                            "read" => format!("{} 行", line_count),
                            "write" => "已写入".into(),
                            "edit" | "multi_edit" => "已应用".into(),
                            "bash" => {
                                if line_count <= 1 {
                                    truncate(preview, max_w.saturating_sub(name.len() + 10))
                                } else {
                                    format!("{} 行输出", line_count)
                                }
                            }
                            "search" | "glob" | "ls" => format!("{} 结果", line_count),
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
                                format!(
                                    "    {}",
                                    truncate(trimmed, max_w.saturating_sub(BORDER_PADDING))
                                ),
                                Style::default().fg(line_color),
                            ));
                        }
                    } else if *is_error {
                        // Error: show with red border for prominence (single-line style)
                        lines.push(Line::styled(
                            "  ┌─ 错误 ───────────────────────────┐",
                            Style::default().fg(Color::Red),
                        ));
                        for line in msg.content.lines().take(preview_count) {
                            let truncated = truncate(line, max_w.saturating_sub(8));
                            lines.push(Line::styled(
                                format!("  │ {}", truncated),
                                Style::default().fg(Color::Red),
                            ));
                        }
                        let total_lines = msg.content.lines().count();
                        if total_lines > preview_count {
                            lines.push(Line::styled(
                                format!("  │ ... ({} 更多行)", total_lines - preview_count),
                                Style::default().fg(Color::DarkGray),
                            ));
                        }
                        lines.push(Line::styled(
                            "  └───────────────────────────────────┘",
                            Style::default().fg(Color::Red),
                        ));
                    } else if preview_count > 0 {
                        // Different handling for read vs other tools
                        if name == "read" {
                            // Read: show first lines directly (no header to skip)
                            for line in msg.content.lines().take(preview_count) {
                                lines.push(Line::styled(
                                    format!(
                                        "    {}",
                                        truncate(line, max_w.saturating_sub(BORDER_PADDING))
                                    ),
                                    Style::default().fg(Color::Gray),
                                ));
                            }
                            let total_lines = msg.content.lines().count();
                            if total_lines > preview_count {
                                lines.push(Line::styled(
                                    format!("    … ({} 更多)", total_lines - preview_count),
                                    Style::default().fg(Color::DarkGray),
                                ));
                            }
                        } else if (name == "edit" || name == "multi_edit") && has_diff {
                            // Diff display: use bright colors only for edit tools with actual diff
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
                                    format!("    … ({} 更多行)", total_lines - preview_count),
                                    Style::default().fg(Color::DarkGray),
                                ));
                            }
                        } else {
                            // Other tools: skip first line (summary header), normal styling
                            for line in msg.content.lines().skip(1).take(preview_count) {
                                let truncated =
                                    truncate(line, max_w.saturating_sub(BORDER_PADDING));
                                lines.push(Line::styled(
                                    format!("    {}", truncated),
                                    Style::default().fg(Color::DarkGray),
                                ));
                            }
                            let total_lines = msg.content.lines().skip(1).count();
                            if total_lines > preview_count {
                                lines.push(Line::styled(
                                    format!("    … ({} 更多行)", total_lines - preview_count),
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
                            "  [Tab] 切换",
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
                        } else if ['┌', '│', '└', '─'].contains(&line.chars().next().unwrap_or(' '))
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

                            // Check if this is an "Other" option (custom input)
                            let is_other_option = option_idx < self.ask_options.len()
                                && self.ask_options[option_idx].is_other;

                            // Rebuild line with cleaner checkbox symbols
                            let display_line = if is_checkbox && option_idx < self.ask_options.len()
                            {
                                let opt = &self.ask_options[option_idx];
                                // Use ◆/◇ for multi-select
                                let marker = if actually_checked { "◆" } else { "◇" };
                                // Add hint for Other option when selected but not checked
                                let hint = if is_other_option
                                    && option_idx == self.ask_selected_index
                                    && !actually_checked
                                {
                                    " ✏️ [Enter] 自定义"
                                } else {
                                    ""
                                };
                                let desc_text = opt
                                    .description
                                    .as_ref()
                                    .map(|d| format!(" {}", truncate(d, 25)))
                                    .unwrap_or_default();
                                let raw =
                                    format!("  {} {}{}{}", marker, opt.label, desc_text, hint);
                                truncate(&raw, max_w.saturating_sub(2))
                            } else {
                                truncate(line, max_w.saturating_sub(2))
                            };

                            // Check if this line matches current selection index
                            if option_idx == self.ask_selected_index {
                                // Current navigation position: bright highlight
                                if is_submit_option {
                                    // Submit option - yellow highlight
                                    Line::styled(
                                        format!("▶ {}", display_line.trim()),
                                        Style::default()
                                            .fg(Color::Black)
                                            .bg(if actually_checked {
                                                Color::Yellow
                                            } else {
                                                Color::Cyan
                                            })
                                            .add_modifier(Modifier::BOLD),
                                    )
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
                                // Checked but not current navigation: green text with ◆
                                Line::styled(
                                    format!("  {}", display_line.trim()),
                                    Style::default()
                                        .fg(Color::Green)
                                        .add_modifier(Modifier::BOLD),
                                )
                            } else {
                                // Regular option - gray with ◇
                                Line::styled(
                                    format!("  {}", display_line.trim()),
                                    Style::default().fg(Color::Gray),
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
                            "─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─",
                            Style::default().fg(Color::DarkGray),
                        ));
                        let selected_count =
                            self.ask_options.iter().filter(|opt| opt.selected).count();
                        if selected_count > 0 {
                            lines.push(Line::styled(
                                format!("  ◆ 已选 {} 项", selected_count),
                                Style::default()
                                    .fg(Color::Green)
                                    .add_modifier(Modifier::BOLD),
                            ));
                            lines.push(Line::styled(
                                "  [Enter] 提交",
                                Style::default().fg(Color::Yellow),
                            ));
                        } else {
                            lines.push(Line::styled(
                                "  ◇ 未选择",
                                Style::default().fg(Color::DarkGray),
                            ));
                        }
                    }

                    // "Other" input mode: show input prompt
                    if self.waiting_for_ask && self.ask_other_input_active {
                        lines.push(Line::styled(
                            "─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─",
                            Style::default().fg(Color::DarkGray),
                        ));
                        lines.push(Line::styled(
                            "  ✏️ 输入自定义内容:",
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ));
                        // Show current input with cursor indicator
                        let input_line = if self.input.is_empty() {
                            Line::from(vec![
                                Span::styled("    ", Style::default()),
                                Span::styled("▌", Style::default().fg(Color::Cyan)),
                                Span::styled(" ...", Style::default().fg(Color::DarkGray)),
                            ])
                        } else {
                            Line::from(vec![
                                Span::styled("    ", Style::default()),
                                Span::styled(&self.input, Style::default().fg(Color::White)),
                                Span::styled("▌", Style::default().fg(Color::Cyan)),
                            ])
                        };
                        lines.push(input_line);
                        lines.push(Line::styled(
                            "  [Enter] 确认  [Esc] 取消",
                            Style::default().fg(Color::DarkGray),
                        ));
                    }

                    lines.push(Line::styled("", Style::default()));
                }
            }
        }

        // Session selector - show in message area
        if self.waiting_for_session {
            lines.push(Line::styled("", Style::default()));
            lines.push(Line::styled(
                "会话选择器",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
            lines.push(Line::styled(
                "[↑↓] 导航  [Enter] 加载  [Esc] 取消",
                Style::default().fg(Color::DarkGray),
            ));
            lines.push(Line::raw(""));

            if self.session_list.is_empty() {
                lines.push(Line::styled(
                    "加载会话列表...",
                    Style::default().fg(Color::Yellow),
                ));
            } else {
                for (i, session) in self.session_list.iter().enumerate() {
                    let is_selected = self.session_selected_index == i;

                    let mut spans: Vec<Span> = Vec::new();

                    // Selection indicator
                    spans.push(Span::styled(
                        if is_selected { "▶ " } else { "  " },
                        Style::default().fg(if is_selected {
                            Color::Cyan
                        } else {
                            Color::DarkGray
                        }),
                    ));

                    // Session title (primary display)
                    spans.push(Span::styled(
                        session.title.clone(),
                        Style::default()
                            .fg(if is_selected {
                                Color::Yellow
                            } else {
                                Color::White
                            })
                            .add_modifier(Modifier::BOLD),
                    ));

                    // Separator
                    spans.push(Span::styled(
                        "  │  ",
                        Style::default().fg(Color::DarkGray),
                    ));

                    // Session ID (secondary info)
                    spans.push(Span::styled(
                        format!("[{}]", session.short_id.clone()),
                        Style::default().fg(Color::Gray),
                    ));

                    // Separator
                    spans.push(Span::styled(
                        "  │  ",
                        Style::default().fg(Color::DarkGray),
                    ));

                    // Message count
                    spans.push(Span::styled(
                        format!("{} 条消息", session.message_count),
                        Style::default().fg(Color::Gray),
                    ));

                    // Created at
                    spans.push(Span::styled(
                        "  │  ",
                        Style::default().fg(Color::DarkGray),
                    ));
                    spans.push(Span::styled(
                        session.created_at.clone(),
                        Style::default().fg(Color::Gray),
                    ));

                    lines.push(Line::from(spans));
                }

                // Footer hint
                lines.push(Line::raw(""));
                lines.push(Line::styled(
                    format!("找到 {} 个会话", self.session_list.len()),
                    Style::default().fg(Color::DarkGray),
                ));
            }
        }

        // Thinking content - show as message content (animation is in fixed bottom bar)
        if self.activity == Activity::Thinking && !self.thinking.is_empty() {
            if self.thinking_collapsed && !self.debug_mode {
                let preview = self.thinking.lines().next().unwrap_or("");
                lines.push(Line::from(vec![
                    Span::styled("  💭 ▶ ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        truncate(preview, max_w.saturating_sub(20)),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            } else {
                let md_lines =
                    render_markdown(&self.thinking, max_w.saturating_sub(BORDER_PADDING));
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

        // Streaming text - show content with separator if thinking was shown
        if !self.streaming.is_empty() {
            if self.activity == Activity::Thinking && !self.thinking.is_empty() {
                lines.push(Line::raw(""));
            }
            // Show realtime token count during streaming (debug mode)
            if self.debug_mode {
                let token_display = if self.current_request_tokens > 0 {
                    format!("({}tok)", fmt_tokens(self.current_request_tokens))
                } else {
                    "(0tok)".to_string()
                };
                lines.push(Line::from(vec![
                    Span::styled("  ─── 🤖 ", Style::default().fg(Color::DarkGray)),
                    Span::styled(token_display, Style::default().fg(Color::DarkGray)),
                ]));
            }
            let md_lines = render_markdown(&self.streaming, max_w);
            lines.extend(md_lines);
        }

        for msg in self
            .messages
            .iter()
            .filter(|msg| matches!(msg.role, Role::User) && msg.is_pending)
        {
            push_user_message_lines(&mut lines, &msg.content, max_w, true);
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

            let elapsed = self
                .tool_start
                .map(|s| format!(" ({:.1}s)", s.elapsed().as_secs_f64()))
                .unwrap_or_default();
            let spinner_frame = self.frame % SPINNER.len();

            // Main status line
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
                    self.activity.label(),
                    Style::default()
                        .fg(self.activity.color())
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(elapsed, Style::default().fg(Color::DarkGray)),
                Span::styled(" [Esc] 取消", Style::default().fg(Color::DarkGray)),
            ]));

            // Show full command/input details on separate lines
            if let Some(ref input) = self.activity_input {
                let tool_name = match self.activity {
                    Activity::Running => "bash",
                    Activity::Reading | Activity::Writing | Activity::Editing => "file",
                    Activity::Searching => "pattern",
                    Activity::WebSearch => "query",
                    Activity::WebFetch => "url",
                    Activity::Tool(ref name) => name.as_str(),
                    _ => "",
                };

                // Extract and display relevant fields with prominent styling
                let detail_lines = extract_full_detail(tool_name, input);
                for detail in detail_lines {
                    // Use brighter colors for better visibility
                    let detail_color = if detail.starts_with("$") {
                        Color::Yellow // Commands stand out
                    } else if detail.starts_with("file:") || detail.starts_with("pattern:") {
                        Color::Cyan // Key parameters
                    } else {
                        Color::Gray // Other details
                    };
                    lines.push(Line::from(vec![
                        Span::styled("    ", Style::default()),
                        Span::styled(detail, Style::default().fg(detail_color)),
                    ]));
                }
            }
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
            // Show notification if new message arrived while scrolled
            let notification = if self.new_message_while_scrolled.get() {
                " 📥 新消息! [End] 查看"
            } else {
                " ↑ 滚动中 [End] 底部"
            };
            let notification_color = if self.new_message_while_scrolled.get() {
                Color::Yellow
            } else {
                Color::DarkGray
            };
            let indicator = Line::styled(notification, Style::default().fg(notification_color));
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
            // Clear notification when auto_scroll is restored
            self.new_message_while_scrolled.set(false);
            f.render_widget(Paragraph::new(lines).scroll((scroll_offset, 0)), area);
        }
    }
}
