//! Output Area Component
//!
//! Displays conversation history with scrolling support and simplified Markdown rendering.

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

use crate::app::{AppState, OutputBlock, Role};

/// Maximum lines to show for tool results
const MAX_TOOL_RESULT_LINES: usize = 10;

/// Maximum characters per line before truncation
const MAX_LINE_WIDTH: usize = 500;

/// Output area component
pub struct OutputArea {
    /// Current scroll position (internal state)
    scroll_offset: usize,
}

impl OutputArea {
    /// Create new output area
    pub fn new() -> Self {
        Self { scroll_offset: 0 }
    }

    /// Apply simplified markdown formatting to text
    fn format_markdown_line(line: &str) -> Vec<Span> {
        // Simple markdown detection patterns
        let mut spans: Vec<Span> = Vec::new();
        let mut current_text = String::new();
        let chars = line.chars().collect::<Vec<_>>();
        let mut i = 0;

        while i < chars.len() {
            // Check for bold (**text**)
            if chars[i] == '*' && i + 1 < chars.len() && chars[i + 1] == '*' {
                if current_text.is_empty() {
                    // Look for closing **
                    let mut j = i + 2;
                    let mut bold_text = String::new();
                    while j < chars.len() - 1 {
                        if chars[j] == '*' && chars[j + 1] == '*' {
                            spans.push(Span::styled(
                                bold_text.clone(),
                                Style::default().add_modifier(Modifier::BOLD),
                            ));
                            i = j + 2;
                            current_text.clear();
                            break;
                        }
                        bold_text.push(chars[j]);
                        j += 1;
                    }
                    if j >= chars.len() - 1 {
                        // No closing **, treat as normal
                        current_text.push('*');
                        current_text.push('*');
                        i += 2;
                    }
                } else {
                    spans.push(Span::raw(current_text.clone()));
                    current_text.clear();
                }
                continue;
            }

            // Check for inline code (`code`)
            if chars[i] == '`' {
                if current_text.is_empty() {
                    let mut j = i + 1;
                    let mut code_text = String::new();
                    while j < chars.len() {
                        if chars[j] == '`' {
                            spans.push(Span::styled(
                                code_text.clone(),
                                Style::default().fg(Color::Green),
                            ));
                            i = j + 1;
                            current_text.clear();
                            break;
                        }
                        code_text.push(chars[j]);
                        j += 1;
                    }
                    if j >= chars.len() {
                        // No closing `, treat as normal
                        current_text.push('`');
                        i += 1;
                    }
                } else {
                    spans.push(Span::raw(current_text.clone()));
                    current_text.clear();
                }
                continue;
            }

            // Check for links [text](url) - simplified: just show text in cyan
            if chars[i] == '[' {
                if current_text.is_empty() {
                    let mut j = i + 1;
                    let mut link_text = String::new();
                    while j < chars.len() {
                        if chars[j] == ']' {
                            if j + 1 < chars.len() && chars[j + 1] == '(' {
                                // Skip to closing )
                                let mut k = j + 2;
                                while k < chars.len() && chars[k] != ')' {
                                    k += 1;
                                }
                                spans.push(Span::styled(
                                    link_text.clone(),
                                    Style::default().fg(Color::Cyan).add_modifier(Modifier::UNDERLINED),
                                ));
                                i = k + 1;
                                current_text.clear();
                            } else {
                                spans.push(Span::raw(format!("[{}]", link_text)));
                                i = j + 1;
                            }
                            break;
                        }
                        link_text.push(chars[j]);
                        j += 1;
                    }
                    if j >= chars.len() {
                        current_text.push('[');
                        i += 1;
                    }
                } else {
                    spans.push(Span::raw(current_text.clone()));
                    current_text.clear();
                }
                continue;
            }

            current_text.push(chars[i]);
            i += 1;
        }

        if !current_text.is_empty() {
            spans.push(Span::raw(current_text));
        }

        spans
    }

    /// Format a code block line
    fn format_code_block_line(line: &str, is_start: bool, lang: &str) -> Line<'static> {
        if is_start {
            Line::styled(
                format!("┌─ {} ", lang),
                Style::default().fg(Color::DarkGray),
            )
        } else {
            Line::styled(
                format!("│ {}", line),
                Style::default().fg(Color::Green),
            )
        }
    }

    /// Truncate line if too long
    fn truncate_line(line: &str) -> String {
        if line.len() > MAX_LINE_WIDTH {
            format!("{}...", &line[..MAX_LINE_WIDTH - 3])
        } else {
            line.to_string()
        }
    }

    /// Format tool result for display
    fn format_tool_result(result: &str, max_lines: usize) -> Vec<String> {
        let lines: Vec<&str> = result.lines().collect();
        if lines.len() <= max_lines {
            lines.iter().map(|l| Self::truncate_line(l)).collect()
        } else {
            let mut formatted = Vec::new();
            // Show first half
            for line in lines.iter().take(max_lines / 2) {
                formatted.push(Self::truncate_line(line));
            }
            formatted.push(format!("  ... ({} lines omitted)", lines.len() - max_lines));
            // Show last few lines
            for line in lines.iter().skip(lines.len() - max_lines / 2) {
                formatted.push(Self::truncate_line(line));
            }
            formatted
        }
    }

    /// Get role icon
    fn role_icon(role: &Role) -> &'static str {
        match role {
            Role::User => "👤",
            Role::Assistant => "🤖",
            Role::System => "⚙",
        }
    }

    /// Render the output area
    pub fn render(&mut self, f: &mut Frame, state: &AppState, area: ratatui::layout::Rect) {
        // Build lines from messages
        let mut lines: Vec<Line> = Vec::new();

        // Process messages
        for msg in &state.messages {
            let role_style = match msg.role {
                Role::User => Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                Role::Assistant => Style::default().fg(Color::Blue),
                Role::System => Style::default().fg(Color::Yellow),
            };

            let role_icon = Self::role_icon(&msg.role);
            let role_label = match msg.role {
                Role::User => "You",
                Role::Assistant => "Assistant",
                Role::System => "System",
            };

            // Add role header line
            lines.push(Line::styled(
                format!("{} {}:", role_icon, role_label),
                role_style,
            ));

            // Process content blocks
            for block in &msg.content {
                match block {
                    OutputBlock::Text(text) => {
                        // Process text with simplified markdown
                        let mut in_code_block = false;
                        let mut code_lang = String::new();

                        for line in text.lines() {
                            // Check for code block markers
                            if line.starts_with("```") {
                                if in_code_block {
                                    // End code block
                                    lines.push(Line::styled(
                                        "└─",
                                        Style::default().fg(Color::DarkGray),
                                    ));
                                    in_code_block = false;
                                } else {
                                    // Start code block
                                    code_lang = line.trim_start_matches("```").trim().to_string();
                                    if code_lang.is_empty() {
                                        code_lang = "code".to_string();
                                    }
                                    lines.push(Self::format_code_block_line("", true, &code_lang));
                                    in_code_block = true;
                                }
                            } else if in_code_block {
                                lines.push(Self::format_code_block_line(line, false, &code_lang));
                            } else {
                                // Apply markdown formatting
                                let spans = Self::format_markdown_line(line);
                                if spans.is_empty() {
                                    lines.push(Line::raw(""));
                                } else {
                                    lines.push(Line::from(spans));
                                }
                            }
                        }
                    }
                    OutputBlock::Thinking(thought) => {
                        // Thinking block with special styling
                        lines.push(Line::styled(
                            "  💭 Thinking...",
                            Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
                        ));
                        for line in thought.lines().take(3) {
                            lines.push(Line::styled(
                                format!("    {}", Self::truncate_line(line)),
                                Style::default().fg(Color::DarkGray),
                            ));
                        }
                        if thought.lines().count() > 3 {
                            lines.push(Line::styled(
                                "    ...",
                                Style::default().fg(Color::DarkGray),
                            ));
                        }
                    }
                    OutputBlock::ToolUse { name, result, is_error, .. } => {
                        let style = if *is_error {
                            Style::default().fg(Color::Red)
                        } else {
                            Style::default().fg(Color::Cyan)
                        };

                        let icon = Self::tool_icon(name);
                        lines.push(Line::styled(
                            format!("  {} Tool: {}", icon, name),
                            style.add_modifier(Modifier::BOLD),
                        ));

                        // Show truncated result
                        let formatted_result = Self::format_tool_result(result, MAX_TOOL_RESULT_LINES);
                        for line in formatted_result {
                            lines.push(Line::styled(format!("    {}", line), style));
                        }
                    }
                }
            }

            // Add separator between messages
            lines.push(Line::styled(
                "─".repeat(area.width as usize / 2),
                Style::default().fg(Color::DarkGray),
            ));
        }

        // If no messages, show welcome text
        if lines.is_empty() {
            lines.push(Line::styled(
                "  Welcome to MatrixCode Terminal Mode",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ));
            lines.push(Line::raw(""));
            lines.push(Line::styled(
                "  Type a message to start conversation, or use commands:",
                Style::default().fg(Color::DarkGray),
            ));
            lines.push(Line::styled(
                "    /help  - Show available commands",
                Style::default().fg(Color::Green),
            ));
            lines.push(Line::styled(
                "    /exit  - Exit the terminal",
                Style::default().fg(Color::Yellow),
            ));
        }

        // Calculate total lines and scroll
        let total_lines = lines.len();
        let visible_height = area.height.saturating_sub(2) as usize; // Account for borders

        // Use state's scroll_offset for user-controlled scrolling
        if state.scroll_offset == 0 {
            // Auto-scroll to bottom (show latest messages)
            self.scroll_offset = total_lines.saturating_sub(visible_height);
        } else {
            // User-controlled scroll
            self.scroll_offset = state.scroll_offset.min(total_lines.saturating_sub(1));
        }

        // Ensure scroll doesn't go negative or exceed bounds
        self.scroll_offset = self.scroll_offset.max(0);
        if self.scroll_offset > total_lines.saturating_sub(visible_height) {
            self.scroll_offset = total_lines.saturating_sub(visible_height);
        }

        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Conversation ")
                    .title_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            )
            .wrap(Wrap { trim: false })
            .scroll((self.scroll_offset as u16, 0));

        f.render_widget(paragraph, area);

        // Render scrollbar if content exceeds visible area
        if total_lines > visible_height {
            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"))
                .track_symbol(Some("│"))
                .thumb_symbol("█");

            let mut scrollbar_state = ScrollbarState::new(total_lines)
                .position(self.scroll_offset)
                .viewport_content_length(visible_height);

            f.render_stateful_widget(
                scrollbar,
                area.inner(ratatui::layout::Margin { vertical: 1, horizontal: 0 }),
                &mut scrollbar_state,
            );
        }
    }

    /// Get icon for a tool name
    fn tool_icon(name: &str) -> &'static str {
        match name.to_lowercase().as_str() {
            "read" | "readfile" => "📖",
            "write" | "writefile" => "📝",
            "edit" | "editfile" => "✏️",
            "bash" | "shell" => "⚡",
            "search" | "grep" => "🔍",
            "glob" | "ls" => "📁",
            "websearch" => "🌐",
            "webfetch" => "🔗",
            _ => "🔧",
        }
    }
}

impl Default for OutputArea {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== truncate_line Tests =====

    #[test]
    fn test_truncate_line_short() {
        let line = "This is a short line";
        let result = OutputArea::truncate_line(line);
        assert_eq!(result, "This is a short line");
    }

    #[test]
    fn test_truncate_line_long() {
        let line = "x".repeat(600);
        let result = OutputArea::truncate_line(&line);
        assert!(result.ends_with("..."));
        assert_eq!(result.len(), MAX_LINE_WIDTH);
        assert!(result.starts_with(&"x".repeat(MAX_LINE_WIDTH - 3)));
    }

    #[test]
    fn test_truncate_line_boundary() {
        // Exactly MAX_LINE_WIDTH chars - should not be truncated
        let line = "x".repeat(MAX_LINE_WIDTH);
        let result = OutputArea::truncate_line(&line);
        assert_eq!(result.len(), MAX_LINE_WIDTH);
        assert!(!result.ends_with("..."));
    }

    #[test]
    fn test_truncate_line_one_over_boundary() {
        let line = "x".repeat(MAX_LINE_WIDTH + 1);
        let result = OutputArea::truncate_line(&line);
        assert!(result.ends_with("..."));
        assert_eq!(result.len(), MAX_LINE_WIDTH);
    }

    #[test]
    fn test_truncate_line_empty() {
        let result = OutputArea::truncate_line("");
        assert_eq!(result, "");
    }

    // ===== role_icon Tests =====

    #[test]
    fn test_role_icon_user() {
        assert_eq!(OutputArea::role_icon(&Role::User), "👤");
    }

    #[test]
    fn test_role_icon_assistant() {
        assert_eq!(OutputArea::role_icon(&Role::Assistant), "🤖");
    }

    #[test]
    fn test_role_icon_system() {
        assert_eq!(OutputArea::role_icon(&Role::System), "⚙");
    }

    // ===== tool_icon Tests =====

    #[test]
    fn test_tool_icon_read() {
        assert_eq!(OutputArea::tool_icon("read"), "📖");
    }

    #[test]
    fn test_tool_icon_readfile() {
        assert_eq!(OutputArea::tool_icon("readfile"), "📖");
    }

    #[test]
    fn test_tool_icon_write() {
        assert_eq!(OutputArea::tool_icon("write"), "📝");
    }

    #[test]
    fn test_tool_icon_writefile() {
        assert_eq!(OutputArea::tool_icon("writefile"), "📝");
    }

    #[test]
    fn test_tool_icon_edit() {
        assert_eq!(OutputArea::tool_icon("edit"), "✏️");
    }

    #[test]
    fn test_tool_icon_editfile() {
        assert_eq!(OutputArea::tool_icon("editfile"), "✏️");
    }

    #[test]
    fn test_tool_icon_bash() {
        assert_eq!(OutputArea::tool_icon("bash"), "⚡");
    }

    #[test]
    fn test_tool_icon_shell() {
        assert_eq!(OutputArea::tool_icon("shell"), "⚡");
    }

    #[test]
    fn test_tool_icon_search() {
        assert_eq!(OutputArea::tool_icon("search"), "🔍");
    }

    #[test]
    fn test_tool_icon_grep() {
        assert_eq!(OutputArea::tool_icon("grep"), "🔍");
    }

    #[test]
    fn test_tool_icon_glob() {
        assert_eq!(OutputArea::tool_icon("glob"), "📁");
    }

    #[test]
    fn test_tool_icon_ls() {
        assert_eq!(OutputArea::tool_icon("ls"), "📁");
    }

    #[test]
    fn test_tool_icon_websearch() {
        assert_eq!(OutputArea::tool_icon("websearch"), "🌐");
    }

    #[test]
    fn test_tool_icon_webfetch() {
        assert_eq!(OutputArea::tool_icon("webfetch"), "🔗");
    }

    #[test]
    fn test_tool_icon_unknown() {
        assert_eq!(OutputArea::tool_icon("unknown_tool"), "🔧");
    }

    #[test]
    fn test_tool_icon_case_insensitive() {
        assert_eq!(OutputArea::tool_icon("READ"), "📖");
        assert_eq!(OutputArea::tool_icon("Write"), "📝");
        assert_eq!(OutputArea::tool_icon("BASH"), "⚡");
    }

    // ===== format_tool_result Tests =====

    #[test]
    fn test_format_tool_result_short() {
        let result = "Line 1\nLine 2\nLine 3";
        let formatted = OutputArea::format_tool_result(result, 10);
        assert_eq!(formatted.len(), 3);
        assert_eq!(formatted[0], "Line 1");
        assert_eq!(formatted[1], "Line 2");
        assert_eq!(formatted[2], "Line 3");
    }

    #[test]
    fn test_format_tool_result_exact_limit() {
        let result = (1..=10).map(|i| format!("Line {}", i)).collect::<Vec<_>>().join("\n");
        let formatted = OutputArea::format_tool_result(&result, 10);
        assert_eq!(formatted.len(), 10);
        assert_eq!(formatted[0], "Line 1");
        assert_eq!(formatted[9], "Line 10");
        // No omitted message
        assert!(!formatted.iter().any(|l| l.contains("omitted")));
    }

    #[test]
    fn test_format_tool_result_long() {
        let result = (1..=20).map(|i| format!("Line {}", i)).collect::<Vec<_>>().join("\n");
        let formatted = OutputArea::format_tool_result(&result, 10);
        // Should have omitted message
        assert!(formatted.iter().any(|l| l.contains("omitted")));
        // Should show first 5 lines and last 5 lines
        assert!(formatted.contains(&"Line 1".to_string()));
        assert!(formatted.contains(&"Line 5".to_string()));
        assert!(formatted.contains(&"Line 16".to_string()));
        assert!(formatted.contains(&"Line 20".to_string()));
    }

    #[test]
    fn test_format_tool_result_single_line() {
        let result = "Single line result";
        let formatted = OutputArea::format_tool_result(result, 10);
        assert_eq!(formatted.len(), 1);
        assert_eq!(formatted[0], "Single line result");
    }

    #[test]
    fn test_format_tool_result_empty() {
        let formatted = OutputArea::format_tool_result("", 10);
        // Empty string has no lines, so result is empty
        assert!(formatted.is_empty());
    }

    #[test]
    fn test_format_tool_result_truncates_long_lines() {
        let long_line = "x".repeat(600);
        let formatted = OutputArea::format_tool_result(&long_line, 10);
        assert_eq!(formatted.len(), 1);
        assert!(formatted[0].ends_with("..."));
    }

    // ===== format_markdown_line Tests =====

    #[test]
    fn test_format_markdown_line_plain() {
        let spans = OutputArea::format_markdown_line("Hello world");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "Hello world");
    }

    #[test]
    fn test_format_markdown_line_empty() {
        let spans = OutputArea::format_markdown_line("");
        assert!(spans.is_empty());
    }

    #[test]
    fn test_format_markdown_line_bold() {
        let spans = OutputArea::format_markdown_line("This is **bold** text");
        assert!(!spans.is_empty());
        // Check that bold text was parsed - verify content contains "bold"
        let content: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(content.contains("bold"));
    }

    #[test]
    fn test_format_markdown_line_code() {
        let spans = OutputArea::format_markdown_line("Use `code` here");
        assert!(!spans.is_empty());
        // Check that inline code was parsed - verify content contains "code"
        let content: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(content.contains("code"));
    }

    #[test]
    fn test_format_markdown_line_link() {
        let spans = OutputArea::format_markdown_line("Click [here](https://example.com) for more");
        assert!(!spans.is_empty());
        // Check that link text was extracted
        let content: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(content.contains("here"));
        // URL should not appear in content
        assert!(!content.contains("https://example.com"));
    }

    #[test]
    fn test_format_markdown_line_multiple_formatting() {
        let spans = OutputArea::format_markdown_line("Use **bold** and `code` together");
        assert!(!spans.is_empty());
        let content: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(content.contains("bold"));
        assert!(content.contains("code"));
    }

    #[test]
    fn test_format_markdown_line_unclosed_bold() {
        // Unclosed bold should be treated as normal text
        let spans = OutputArea::format_markdown_line("This **is unclosed");
        let content: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(content.contains("**"));
    }

    #[test]
    fn test_format_markdown_line_unclosed_code() {
        // Unclosed code should be treated as normal text
        let spans = OutputArea::format_markdown_line("This `is unclosed");
        let content: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(content.contains("`"));
    }

    #[test]
    fn test_format_markdown_line_unclosed_link() {
        // Unclosed link should be treated as normal text
        let spans = OutputArea::format_markdown_line("Click [here for more");
        let content: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(content.contains("["));
    }

    // ===== OutputArea Construction Tests =====

    #[test]
    fn test_output_area_new() {
        let output_area = OutputArea::new();
        // Just verify it can be created
        let _ = output_area;
    }

    #[test]
    fn test_output_area_default() {
        let output_area = OutputArea::default();
        let _ = output_area;
    }
}