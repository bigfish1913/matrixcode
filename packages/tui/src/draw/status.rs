//! Status bar rendering.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::helpers::estimate_message_tokens;
use crate::app::TuiApp;
use crate::types::{Activity, ApproveMode};
use crate::utils::{fmt_tokens, progress_bar, truncate};

impl TuiApp {
    pub(crate) fn draw_status(&self, f: &mut ratatui::Frame, area: Rect) {
        let actual_tokens = if self.tokens_in > 0 {
            self.tokens_in
        } else {
            self.messages
                .iter()
                .map(|m| estimate_message_tokens(&m.content) as u64)
                .sum::<u64>()
        };

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
            "Ready".to_string()
        } else if is_tool_activity {
            if !self.activity_detail.is_empty() {
                format!("{} {}", self.activity.label(), self.activity_detail)
            } else {
                self.activity.label().to_string()
            }
        } else if self.animated_token_count > 0 {
            format!("{} tok", fmt_tokens(self.animated_token_count))
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
                format!(" out {} ", fmt_tokens(self.session_total_out)),
                Style::default().fg(Color::DarkGray),
            ));
        }

        // Message count
        if width >= 65 {
            spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled(
                format!(" msg:{} ", self.messages.len()),
                Style::default().fg(Color::DarkGray),
            ));
        }

        // Cache info
        if width >= 80 && (self.cache_read > 0 || self.cache_created > 0) {
            spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled(
                format!(
                    " c {}k/{}k ",
                    self.cache_read / 1000,
                    self.cache_created / 1000
                ),
                Style::default().fg(Color::DarkGray),
            ));
        }

        // Version (at the end)
        if width >= 90 {
            spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled(
                format!(" v{} ", env!("CARGO_PKG_VERSION")),
                Style::default().fg(Color::DarkGray),
            ));
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

        // Server status indicators (MCP/LSP) - show after status
        if width >= 100 {
            spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));

            // MCP status
            let mcp_connected = !self.mcp_servers.is_empty()
                && self.mcp_servers.iter().any(|s| s.is_started);
            let mcp_tool_count = if mcp_connected {
                self.mcp_servers.iter().filter(|s| s.is_started).map(|s| s.tool_count).sum::<usize>()
            } else {
                0
            };
            let mcp_color = if mcp_connected {
                Color::Green
            } else if !self.mcp_servers.is_empty() {
                Color::Yellow  // Configured but not started
            } else {
                Color::DarkGray
            };
            let mcp_text = if mcp_tool_count > 0 {
                format!(" ●MCP({}) ", mcp_tool_count)
            } else if mcp_connected {
                " ●MCP ".to_string()
            } else if !self.mcp_servers.is_empty() {
                " ◐MCP ".to_string()  // Configured, not started
            } else {
                " ○MCP ".to_string()
            };
            spans.push(Span::styled(mcp_text, Style::default().fg(mcp_color)));

            // LSP status
            let lsp_connected_count = self.lsp_servers.iter()
                .filter(|s| matches!(s.status, crate::app::LspServerStatus::Connected))
                .count();
            let lsp_starting = self.lsp_servers.iter()
                .any(|s| matches!(s.status, crate::app::LspServerStatus::Starting));
            let lsp_error = self.lsp_servers.iter()
                .any(|s| matches!(s.status, crate::app::LspServerStatus::Error(_)));

            let (lsp_symbol, lsp_color) = if lsp_error {
                ("✗", Color::Red)
            } else if lsp_starting {
                ("◐", Color::Yellow)
            } else if lsp_connected_count > 0 {
                ("●", Color::Green)
            } else if self.lsp_servers.is_empty() {
                ("○", Color::DarkGray)
            } else {
                ("○", Color::DarkGray)
            };
            let lsp_text = if lsp_connected_count > 1 {
                format!(" {}LSP({}) ", lsp_symbol, lsp_connected_count)
            } else {
                format!(" {}LSP ", lsp_symbol)
            };
            spans.push(Span::styled(lsp_text, Style::default().fg(lsp_color)));

            // CodeGraph status
            let cg_status = self.codegraph_status.as_ref();
            let cg_initialized = cg_status.map(|s| s.initialized).unwrap_or(false);

            // Get pending count from pending_changes
            let pending_count = cg_status
                .and_then(|s| s.pending_changes.as_ref())
                .and_then(|p| p.as_object())
                .map(|obj| {
                    let added = obj.get("added").and_then(|v| v.as_u64()).unwrap_or(0);
                    let modified = obj.get("modified").and_then(|v| v.as_u64()).unwrap_or(0);
                    let removed = obj.get("removed").and_then(|v| v.as_u64()).unwrap_or(0);
                    added + modified + removed
                })
                .unwrap_or(0);

            let cg_color = if pending_count > 0 {
                Color::Yellow  // Pending files need sync
            } else if cg_initialized {
                Color::Green
            } else {
                Color::DarkGray
            };

            let cg_text = if pending_count > 0 {
                format!(" ●CG({}) ", pending_count)
            } else if cg_initialized {
                " ●CG ".to_string()
            } else {
                " ○CG ".to_string()
            };

            spans.push(Span::styled(cg_text, Style::default().fg(cg_color)));
        }

        f.render_widget(Paragraph::new(Line::from(spans)), area);
    }
}
