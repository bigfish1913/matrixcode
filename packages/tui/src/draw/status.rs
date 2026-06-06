//! Status bar rendering.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::TuiApp;
use crate::types::{Activity, ApproveMode};
use crate::utils::{fmt_tokens, progress_bar, truncate};

impl TuiApp {
    pub(crate) fn draw_status(&self, f: &mut ratatui::Frame, area: Rect) {
        // Use cached token count instead of recalculating every frame
        let actual_tokens = self.cached_actual_tokens;

        let context_pct = if self.context_size > 0 {
            (actual_tokens as f64 / self.context_size as f64 * 100.0).min(100.0)
        } else {
            0.0
        };
        let ctx_color = if context_pct < 50.0 {
            Color::DarkGray
        } else if context_pct < 75.0 {
            Color::Yellow
        } else {
            Color::Red
        };

        let mode_color = match self.approve_mode {
            ApproveMode::Ask => Color::DarkGray,
            ApproveMode::Auto => Color::Green,
            ApproveMode::Strict => Color::Red,
        };

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

        let status_text = if self.activity == Activity::Idle {
            "就绪".to_string()
        } else if is_tool_activity {
            // 只显示活动类型，不显示命令详情（避免太长）
            self.activity.label().to_string()
        } else if self.current_request_tokens > 0 {
            format!("{} token", fmt_tokens(self.current_request_tokens))
        } else {
            "...".to_string()
        };
        let status_color = if self.activity == Activity::Idle {
            Color::Green
        } else {
            Color::Yellow
        };

        let width = area.width as usize;
        let mut spans: Vec<Span> = Vec::new();

        // Model name
        let model_display = if width < 50 {
            truncate(&self.model, 12)
        } else {
            self.model.clone()
        };
        spans.push(Span::styled(
            format!(" {} ", model_display),
            Style::default().fg(Color::DarkGray),
        ));

        spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));

        // Mode indicator
        spans.push(Span::styled(
            format!(" {} ", self.approve_mode),
            Style::default().fg(mode_color),
        ));

        spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));

        // Context info
        if width >= 40 {
            let bar = if width >= 60 {
                progress_bar(context_pct, 6)
            } else {
                String::new()
            };
            let ctx_size_display = if width >= 70 {
                format!("/{:.0}k", self.context_size as f64 / 1_000.0)
            } else {
                String::new()
            };
            let ctx_full = if ctx_size_display.is_empty() {
                fmt_tokens(actual_tokens)
            } else {
                format!("{}{}", fmt_tokens(actual_tokens), ctx_size_display)
            };
            spans.push(Span::styled(
                format!(" {} {:.0}% {} ", bar, context_pct, ctx_full),
                Style::default().fg(ctx_color),
            ));
        }

        spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));

        // Output tokens
        if width >= 55 {
            spans.push(Span::styled(
                format!(" 输出 {} ", fmt_tokens(self.session_total_out)),
                Style::default().fg(Color::DarkGray),
            ));
        }

        // Message count
        if width >= 65 {
            spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled(
                format!(" 消息:{} ", self.messages.len()),
                Style::default().fg(Color::DarkGray),
            ));
        }

        // MCP servers info
        if width >= 75 && !self.mcp_servers.is_empty() {
            spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
            let running = self.mcp_servers.iter().filter(|s| s.is_started).count();
            let mcp_text = format!(" MCP:{} ", running);
            spans.push(Span::styled(mcp_text, Style::default().fg(Color::Cyan)));
        }

        // LSP servers with status message
        if width >= 70 && !self.lsp_servers.is_empty() {
            spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
            let running = self.lsp_servers.iter().filter(|s| s.status.is_ok()).count();
            let has_error = self.lsp_servers.iter().any(|s| s.status.is_error());
            let is_starting = self.lsp_servers.iter().any(|s| matches!(s.status, matrixcode_core::LspServerStatus::Starting));

            // Get status message: prioritize error > starting > connected count
            let status_msg = if has_error {
                // Simplify error display - just show "err" without long messages
                "err".into()
            } else if is_starting {
                "starting...".into()
            } else if running > 0 {
                format!("{} ok", running)
            } else {
                "off".into()
            };

            // Color based on status: Green if connected, Yellow if starting, Red if error, Gray if not started
            let lsp_color = if has_error {
                Color::LightRed
            } else if is_starting {
                Color::Yellow
            } else if running > 0 {
                Color::LightGreen
            } else {
                Color::DarkGray
            };

            let lsp_text = format!(" LSP:{} ", status_msg);
            spans.push(Span::styled(lsp_text, Style::default().fg(lsp_color)));
        }

        // CodeGraph status
        if width >= 75 {
            let cg = self.codegraph_status.as_ref();
            let (cg_text, cg_color) = match cg {
                None => (" CG:off ".into(), Color::DarkGray),
                Some(status) if !status.initialized => (" CG:off ".into(), Color::DarkGray),
                Some(status) if status.pending_changes.added > 0 || status.pending_changes.modified > 0 || status.pending_changes.removed > 0 => {
                    let total = status.pending_changes.added + status.pending_changes.modified + status.pending_changes.removed;
                    (format!(" CG:{}待同步 ", total), Color::Yellow)
                }
                Some(status) => {
                    // Show node count with clear format
                    (format!(" CG:ok:{} ", status.node_count), Color::LightGreen)
                }
            };
            spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled(cg_text, Style::default().fg(cg_color)));
        }

        // Cache info
        if width >= 80 && (self.cache_read > 0 || self.cache_created > 0) {
            spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled(
                format!(
                    " 缓存 {}k/{}k ",
                    self.cache_read / 1000,
                    self.cache_created / 1000
                ),
                Style::default().fg(Color::DarkGray),
            ));
        }

        // Task elapsed time (show during work and after completion, until new message)
        if width >= 85 && self.request_start.is_some() {
            if let Some(start) = self.request_start {
                let elapsed = start.elapsed();
                let total_secs = elapsed.as_secs();
                let mins = total_secs / 60;
                let secs = total_secs % 60;
                let time_str = if mins > 0 {
                    format!("{}:{:02}", mins, secs)
                } else {
                    format!("{}s", secs)
                };
                spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
                spans.push(Span::styled(
                    format!(" {} ", time_str),
                    Style::default().fg(Color::Yellow),
                ));
            }
        }

        // Debug stats
        if width >= 110 && self.debug_mode {
            spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled(
                format!(" api:{} tools:{} ", self.api_calls, self.tool_calls),
                Style::default().fg(Color::DarkGray),
            ));
        }

        // Status
        spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
        spans.push(Span::styled(
            format!(" {} ", status_text),
            Style::default().fg(status_color),
        ));

        f.render_widget(Paragraph::new(Line::from(spans)), area);
    }
}
