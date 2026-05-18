use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::types::{Activity, ApproveMode, Role};
use crate::utils::{truncate, wrap_line, fmt_tokens, progress_bar};
use crate::markdown::render_markdown;
use crate::app::TuiApp;
use crate::SPINNER;

impl TuiApp {
    pub(crate) fn draw(&self, f: &mut ratatui::Frame) {
        let constraints = vec![
            Constraint::Length(1),           // Status (MatrixCode + Model + mode)
            Constraint::Min(3),              // Messages (弹性高度，最大化)
            Constraint::Length(1),           // Usage + Hints
            Constraint::Length(1),           // Input
        ];

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(f.area());

        self.draw_status(f, chunks[0]);
        self.draw_messages(f, chunks[1]);
        self.draw_usage(f, chunks[2]);
        self.draw_input(f, chunks[3]);
    }

    fn draw_status(&self, f: &mut ratatui::Frame, area: Rect) {
        let spans = vec![
            Span::styled(" MatrixCode ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled("│", Style::default().fg(Color::DarkGray)),
            Span::styled(format!(" {} ", self.model), Style::default().fg(Color::White)),
            Span::styled("│", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!(" mode:{} ", self.approve_mode.label()),
                Style::default().fg(match self.approve_mode {
                    ApproveMode::Ask => Color::Yellow,
                    ApproveMode::Auto => Color::Green,
                    ApproveMode::Strict => Color::Red,
                })
            ),
        ];
        f.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    fn draw_usage(&self, f: &mut ratatui::Frame, area: Rect) {
        if self.tokens_in == 0 && self.tokens_out == 0 {
            let hints = " /help │ PgUp/PgDn: scroll │ Home/End: top/bot │ Alt+T: thinking";
            f.render_widget(Paragraph::new(Line::styled(hints, Style::default().fg(Color::DarkGray))), area);
            return;
        }
        
        let context_pct = if self.context_size > 0 {
            (self.tokens_in as f64 / self.context_size as f64 * 100.0).min(100.0)
        } else { 0.0 };
        
        let ctx_color = if context_pct < 50.0 { Color::Green }
                       else if context_pct < 75.0 { Color::Yellow }
                       else { Color::Red };
        
        let bar = progress_bar(context_pct, 10);
        
        let mut parts: Vec<Span> = vec![
            Span::styled(
                format!("in {} / out {} (session: {})", 
                    fmt_tokens(self.tokens_in), 
                    fmt_tokens(self.tokens_out),
                    fmt_tokens(self.session_total_out)
                ),
                Style::default().fg(Color::Gray)
            ),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        ];
        
        parts.push(Span::styled(
            format!("cache r/w {}/{}", fmt_tokens(self.cache_read), fmt_tokens(self.cache_created)),
            Style::default().fg(Color::Cyan)
        ));
        parts.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
        
        parts.push(Span::styled(
            format!("ctx {} / {} ({:.1}%) {}", 
                fmt_tokens(self.tokens_in),
                fmt_tokens(self.context_size),
                context_pct,
                bar
            ),
            Style::default().fg(ctx_color)
        ));
        
        f.render_widget(Paragraph::new(Line::from(parts)), area);
    }

    fn draw_messages(&self, f: &mut ratatui::Frame, area: Rect) {
        let mut lines: Vec<Line> = Vec::new();
        let max_w = area.width.saturating_sub(5) as usize;

        // Welcome
        if self.show_welcome && self.messages.is_empty() {
            lines.push(Line::styled(
                "╭─────────────────────────────────────────────────────────────╮",
                Style::default().fg(Color::Cyan)
            ));
            lines.push(Line::styled(
                "│                     🤖 MatrixCode                           │",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            ));
            lines.push(Line::styled(
                "│   AI-powered coding assistant with extended thinking       │",
                Style::default().fg(Color::DarkGray)
            ));
            lines.push(Line::raw("│                                                             │"));
            lines.push(Line::styled(
                "│   Commands: /help /clear /history /mode /new /exit         │",
                Style::default().fg(Color::Gray)
            ));
            lines.push(Line::styled(
                "│   Shortcuts: Enter=send │ PgUp/PgDn=scroll │ Alt+T=thinking │",
                Style::default().fg(Color::Gray)
            ));
            lines.push(Line::styled(
                "╰─────────────────────────────────────────────────────────────╯",
                Style::default().fg(Color::Cyan)
            ));
            lines.push(Line::raw(""));
        }

        // Render all messages
        for msg in &self.messages {
            let icon = msg.role.icon();
            let label = msg.role.label();
            let color = msg.role.color();
            
            lines.push(Line::from(vec![
                Span::styled(icon, Style::default().fg(color)),
                Span::raw(" "),
                Span::styled(label, Style::default().fg(color).add_modifier(Modifier::BOLD)),
            ]));
            
            if matches!(msg.role, Role::Thinking) {
                if self.thinking_collapsed {
                    for line in msg.content.lines().take(2) {
                        for wrapped in wrap_line(line, max_w) {
                            lines.push(Line::styled(
                                format!("  {}", wrapped),
                                Style::default().fg(Color::DarkGray)
                            ));
                        }
                    }
                    if msg.content.lines().count() > 2 {
                        lines.push(Line::styled(
                            format!("  ... ({} lines)", msg.content.lines().count()),
                            Style::default().fg(Color::DarkGray)
                        ));
                    }
                } else {
                    for line in msg.content.lines() {
                        for wrapped in wrap_line(line, max_w) {
                            lines.push(Line::styled(
                                format!("  {}", wrapped),
                                Style::default().fg(Color::DarkGray)
                            ));
                        }
                    }
                }
            } else {
                if msg.role == Role::Assistant {
                    let md_lines = render_markdown(&msg.content, max_w);
                    lines.extend(md_lines);
                } else {
                    for line in msg.content.lines() {
                        lines.push(Line::styled(
                            format!("  {}", truncate(line, max_w)),
                            Style::default().fg(Color::White)
                        ));
                    }
                }
            }
            
            lines.push(Line::raw(""));
        }

        // Current thinking (streaming)
        if !self.thinking.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("💭 ", Style::default().fg(Color::Magenta)),
                Span::styled("Thinking", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            ]));
            
            if self.thinking_collapsed {
                for line in self.thinking.lines().take(1) {
                    for wrapped in wrap_line(line, max_w) {
                        lines.push(Line::styled(
                            format!("  {}", wrapped),
                            Style::default().fg(Color::DarkGray)
                        ));
                    }
                }
                if self.thinking.lines().count() > 1 {
                    lines.push(Line::styled(
                        format!("  ... ({} lines)", self.thinking.lines().count()),
                        Style::default().fg(Color::DarkGray)
                    ));
                }
            } else {
                for line in self.thinking.lines() {
                    for wrapped in wrap_line(line, max_w) {
                        lines.push(Line::styled(
                            format!("  {}", wrapped),
                            Style::default().fg(Color::DarkGray)
                        ));
                    }
                }
            }
            lines.push(Line::raw(""));
        }

        // Streaming text - markdown rendered
        if !self.streaming.is_empty() {
            let spinner = if self.activity != Activity::Idle {
                format!(" {} ", SPINNER[self.frame])
            } else {
                " ".to_string()
            };
            lines.push(Line::from(vec![
                Span::styled("🤖", Style::default().fg(Color::Blue)),
                Span::raw(" "),
                Span::styled("Assistant", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
                Span::styled(spinner, Style::default().fg(self.activity.color())),
            ]));
            let md_lines = render_markdown(&self.streaming, max_w);
            lines.extend(md_lines);
            lines.push(Line::styled("  ▌", Style::default().fg(Color::Cyan)));
        }
        
        // Activity indicator with detail
        if self.activity != Activity::Idle && self.streaming.is_empty() && self.thinking.is_empty() {
            let mut spans = vec![
                Span::styled(SPINNER[self.frame], Style::default().fg(self.activity.color())),
                Span::raw(" "),
                Span::styled(self.activity.label(), Style::default().fg(self.activity.color())),
            ];
            if !self.activity_detail.is_empty() {
                spans.push(Span::styled(
                    format!(" ({})", self.activity_detail),
                    Style::default().fg(Color::DarkGray)
                ));
            }
            lines.push(Line::from(spans));
        }

        // Scroll
        let total_lines = lines.len() as u16;
        let visible_height = area.height;
        let max_scroll = if total_lines > visible_height {
            total_lines.saturating_sub(visible_height)
        } else {
            0
        };
        
        let scroll_offset = if self.auto_scroll {
            max_scroll
        } else {
            self.scroll_offset.min(max_scroll)
        };

        f.render_widget(
            Paragraph::new(lines)
                .scroll((scroll_offset, 0)),
            area
        );
    }

    fn draw_input(&self, f: &mut ratatui::Frame, area: Rect) {
        if self.activity == Activity::Idle {
            let mut spans: Vec<Span> = vec![];
            spans.push(Span::styled("❯ ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
            
            if self.input.is_empty() {
                spans.push(Span::styled("_", Style::default().fg(Color::Cyan)));
            } else {
                spans.push(Span::styled(&self.input, Style::default().fg(Color::White)));
            }
            
            if !self.auto_scroll {
                spans.push(Span::styled(" [viewing history]", Style::default().fg(Color::DarkGray)));
            }
            
            f.render_widget(Paragraph::new(Line::from(spans)), area);
        } else {
            f.render_widget(Paragraph::new(Line::raw("")), area);
        }
    }
}
