//! Hint bar rendering - shows command hints, tool status, etc.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::TuiApp;
use crate::types::Activity;
use crate::utils::truncate;

impl TuiApp {
    /// Check if hint bar should be shown
    pub(crate) fn should_show_hint(&self) -> bool {
        // Show hint when:
        // 1. Input starts with '/' (command mode)
        // 2. Tool is executing (show tool details)
        // 3. Activity is not Idle (show status hint)
        self.input.starts_with('/')
            || matches!(
                self.activity,
                Activity::Reading | Activity::Writing | Activity::Editing
                    | Activity::Searching | Activity::Running
                    | Activity::WebSearch | Activity::WebFetch
                    | Activity::Tool(_)
            )
            || self.activity == Activity::Asking
    }

    pub(crate) fn draw_hint(&self, f: &mut ratatui::Frame, area: Rect) {
        let mut spans: Vec<Span> = Vec::new();

        // Command hints when input starts with '/'
        if self.input.starts_with('/') {
            spans.push(Span::styled("💡 ", Style::default().fg(Color::Cyan)));
            spans.push(Span::styled(
                "Commands: ",
                Style::default().fg(Color::DarkGray),
            ));

            // Show available commands
            let commands = [
                ("/init", "生成项目概览"),
                ("/skills", "技能列表"),
                ("/memory", "记忆管理"),
                ("/compress", "压缩上下文"),
                ("/new", "新建会话"),
                ("/save", "保存会话"),
                ("/help", "帮助信息"),
            ];

            // Filter based on current input
            let current_cmd = self.input.split_whitespace().next().unwrap_or("");
            let matching: Vec<_> = commands
                .iter()
                .filter(|(cmd, _)| cmd.starts_with(current_cmd) || current_cmd.starts_with('/'))
                .take(4)
                .collect();

            for (i, (cmd, desc)) in matching.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
                }
                spans.push(Span::styled(*cmd, Style::default().fg(Color::Yellow)));
                spans.push(Span::styled(
                    format!(" {}", desc),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            if matching.is_empty() {
                spans.push(Span::styled(
                    "No matching commands",
                    Style::default().fg(Color::Red),
                ));
            }
        } else if matches!(
            self.activity,
            Activity::Reading | Activity::Writing | Activity::Editing
                | Activity::Searching | Activity::Running
                | Activity::WebSearch | Activity::WebFetch
                | Activity::Tool(_)
        ) {
            // Show tool execution details
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
                    _ => "🔧",
                },
                _ => "⚙️",
            };

            spans.push(Span::styled(
                format!("{} ", tool_icon),
                Style::default().fg(self.activity.color()),
            ));
            spans.push(Span::styled(
                self.activity.label(),
                Style::default().fg(self.activity.color()).add_modifier(Modifier::BOLD),
            ));

            // Show full tool input details
            if let Some(ref input) = self.activity_input {
                let tool_name = match self.activity {
                    Activity::Running => "bash",
                    Activity::Reading => "read",
                    Activity::Writing => "write",
                    Activity::Editing => "edit",
                    Activity::Searching => "search",
                    Activity::WebSearch => "websearch",
                    Activity::WebFetch => "webfetch",
                    Activity::Tool(ref name) => name.as_str(),
                    _ => "",
                };

                let detail = extract_hint_detail(tool_name, input, area.width as usize);
                if !detail.is_empty() {
                    spans.push(Span::styled(" ", Style::default()));
                    spans.push(Span::styled(detail, Style::default().fg(Color::Gray)));
                }
            }
        } else if self.activity == Activity::Asking {
            spans.push(Span::styled("⚡ ", Style::default().fg(Color::Red)));
            spans.push(Span::styled(
                "Awaiting response...",
                Style::default().fg(Color::DarkGray),
            ));
        }

        if spans.is_empty() {
            // Default hint
            spans.push(Span::styled(
                "Shift+Enter: multiline │ ↑↓: history │ Tab: complete",
                Style::default().fg(Color::DarkGray),
            ));
        }

        f.render_widget(Paragraph::new(Line::from(spans)), area);
    }
}

/// Extract detail for hint bar (truncated for single line)
fn extract_hint_detail(tool_name: &str, input: &serde_json::Value, max_width: usize) -> String {
    let detail = match tool_name {
        "bash" => input.get("command").and_then(|v| v.as_str()).map(|s| s.to_string()),
        "read" | "write" | "edit" => input.get("file_path").and_then(|v| v.as_str()).map(|s| s.to_string()),
        "search" => input.get("pattern").and_then(|v| v.as_str()).map(|s| format!("pattern: {}", s)),
        "websearch" => input.get("query").and_then(|v| v.as_str()).map(|s| format!("query: {}", s)),
        "webfetch" => input.get("url").and_then(|v| v.as_str()).map(|s| s.to_string()),
        _ => None,
    };

    detail.map(|d| truncate(&d, max_width.saturating_sub(15))).unwrap_or_default()
}