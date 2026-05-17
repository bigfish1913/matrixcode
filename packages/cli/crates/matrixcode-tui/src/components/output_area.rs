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