//! Hint bar rendering - shows command hints, tool status, todo progress, etc.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::TuiApp;
use crate::types::Activity;

impl TuiApp {
    /// Check if hint bar should be shown
    pub(crate) fn should_show_hint(&self) -> bool {
        // Show hint when:
        // 1. Input starts with '/' (command mode)
        // 2. Activity is Asking (show prompt)
        // 3. Has todo items (show progress)
        // 4. Multiline input waiting for confirmation
        // Note: tool execution is shown by the activity indicator to avoid duplicate lines.
        self.input.starts_with('/')
            || self.activity == Activity::Thinking
            || self.activity == Activity::Asking
            || !self.todo_items.is_empty()
            || self.multiline_confirm_send
    }

    pub(crate) fn draw_hint(&self, f: &mut ratatui::Frame, area: Rect) {
        let mut spans: Vec<Span> = Vec::new();

        // Show todo progress first if we have todos and are working
        let (completed, total) = self.todo_progress();
        if total > 0
            && matches!(
                self.activity,
                Activity::Thinking
                    | Activity::Reading
                    | Activity::Writing
                    | Activity::Editing
                    | Activity::Searching
                    | Activity::Running
                    | Activity::Tool(_)
            )
        {
            let progress_color = if completed == total {
                Color::Green
            } else if completed > 0 {
                Color::Yellow
            } else {
                Color::Gray
            };
            spans.push(Span::styled(
                format!("📋 {}/{} ", completed, total),
                Style::default().fg(progress_color),
            ));
        }

        // Command hints when input starts with '/'
        if self.input.starts_with('/') {
            if !spans.is_empty() {
                spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
            }
            spans.push(Span::styled("💡 ", Style::default().fg(Color::Cyan)));
            spans.push(Span::styled(
                "命令: ",
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
                    "无匹配命令",
                    Style::default().fg(Color::Red),
                ));
            }
        } else if self.activity == Activity::Asking {
            if !spans.is_empty() {
                spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
            }
            spans.push(Span::styled("⚡ ", Style::default().fg(Color::Red)));
            spans.push(Span::styled(
                "等待响应...",
                Style::default().fg(Color::DarkGray),
            ));
        }

        if spans.is_empty() {
            // Default hint
            if self.multiline_confirm_send {
                // Multiline input waiting for confirmation
                spans.push(Span::styled(
                    "⚠️ 再次按 [Enter] 发送，或 [Esc] 取消",
                    Style::default().fg(Color::Yellow),
                ));
            } else {
                spans.push(Span::styled(
                    "[Shift+Enter] 换行 │ [↑↓] 历史 │ [Tab] 补全",
                    Style::default().fg(Color::DarkGray),
                ));
            }
        }

        f.render_widget(Paragraph::new(Line::from(spans)), area);
    }
}
