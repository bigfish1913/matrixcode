use ratatui::style::{Color, Modifier, Style};
use tui_markdown::{from_str_with_options, Options, StyleSheet};

/// Custom style sheet for MatrixCode TUI dark theme
#[derive(Debug, Clone)]
pub struct MatrixCodeStyleSheet;

impl StyleSheet for MatrixCodeStyleSheet {
    fn heading(&self, level: u8) -> Style {
        match level {
            1 => Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED),
            2 => Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            3 => Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::ITALIC),
            _ => Style::default()
                .fg(Color::LightCyan)
                .add_modifier(Modifier::ITALIC),
        }
    }

    fn code(&self) -> Style {
        Style::default().fg(Color::Gray).bg(Color::Black)
    }

    fn link(&self) -> Style {
        Style::default()
            .fg(Color::Blue)
            .add_modifier(Modifier::UNDERLINED)
    }

    fn blockquote(&self) -> Style {
        Style::default().fg(Color::Yellow)
    }

    fn heading_meta(&self) -> Style {
        Style::default().fg(Color::DarkGray)
    }

    fn metadata_block(&self) -> Style {
        Style::default().fg(Color::LightYellow)
    }
}

/// Render markdown text using tui-markdown with custom styling
/// Returns a vector of Lines for rendering in ratatui
pub fn render_markdown(text: &str, _max_width: usize) -> Vec<ratatui::text::Line<'static>> {
    if text.is_empty() {
        return Vec::new();
    }
    
    let options = Options::new(MatrixCodeStyleSheet);
    let rendered = from_str_with_options(text, &options);
    
    // Default text color for dark theme (white/gray for visibility)
    let default_fg = Color::Gray;
    
    // Convert Text<'_> to Vec<Line<'static>> by making all content owned
    rendered.lines.into_iter().map(|line| {
        let spans: Vec<ratatui::text::Span<'static>> = line.spans
            .into_iter()
            .map(|span| {
                let content: String = span.content.to_string();
                // Add default foreground color if style has no fg color
                let style = if span.style.fg.is_none() {
                    span.style.fg(default_fg)
                } else {
                    span.style
                };
                ratatui::text::Span::styled(content, style)
            })
            .collect();
        ratatui::text::Line::from(spans)
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_plain_text() {
        let result = render_markdown("Hello world", 80);
        assert!(!result.is_empty(), "Plain text should produce lines");
        let text = result[0].spans.iter().map(|s| s.content.as_ref()).collect::<String>();
        assert!(text.contains("Hello"), "Should contain 'Hello'");
    }
    
    #[test]
    fn test_markdown_heading() {
        let result = render_markdown("# Title\nSome content", 80);
        assert!(result.len() >= 2, "Heading + content should produce multiple lines");
    }
    
    #[test]
    fn test_empty() {
        let result = render_markdown("", 80);
        assert!(result.is_empty(), "Empty input should produce empty output");
    }
}