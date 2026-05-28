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
        // Always estimate actual context size from messages (like debug panel)
        // This gives accurate current context usage, not cumulative API tokens
        let actual_tokens = self.messages
            .iter()
            .map(|m| estimate_message_tokens(&m.content) as u64)
            .sum::<u64>();

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
        } else if self.current_request_tokens > 0 {
            format!("{} tok", fmt_tokens(self.current_request_tokens))
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

        f.render_widget(Paragraph::new(Line::from(spans)), area);
    }
}
