//! Markdown rendering using ratatui-markdown library.

use ratatui::text::{Line, Span};
use ratatui::style::{Color, Modifier, Style};
use ratatui_markdown::markdown::{MarkdownRenderer, RenderHooks};
use ratatui_markdown::theme::{RichTextTheme, Generation, ThemeConfig};

/// Custom theme for markdown rendering that matches our TUI style.
pub struct MarkdownTheme;

impl RichTextTheme for MarkdownTheme {
    fn generation(&self) -> Generation {
        Generation(1)
    }

    fn get_text_color(&self) -> Color {
        Color::Gray
    }

    fn get_muted_text_color(&self) -> Color {
        Color::DarkGray
    }

    fn get_primary_color(&self) -> Color {
        Color::Cyan
    }

    fn get_secondary_color(&self) -> Color {
        Color::LightCyan
    }

    fn get_info_color(&self) -> Color {
        Color::LightBlue
    }

    fn get_border_color(&self) -> Color {
        Color::DarkGray
    }

    fn get_focused_border_color(&self) -> Color {
        Color::White
    }

    fn get_popup_selected_background(&self) -> Color {
        Color::DarkGray
    }

    fn get_popup_selected_text_color(&self) -> Color {
        Color::White
    }

    fn get_background_color(&self) -> Color {
        Color::Black
    }

    fn get_json_key_color(&self) -> Color {
        Color::LightCyan
    }

    fn get_json_string_color(&self) -> Color {
        Color::Green
    }

    fn get_json_number_color(&self) -> Color {
        Color::Yellow
    }

    fn get_json_bool_color(&self) -> Color {
        Color::Magenta
    }

    fn get_json_null_color(&self) -> Color {
        Color::DarkGray
    }

    fn get_accent_yellow(&self) -> Color {
        Color::Yellow
    }
}

/// Custom render hooks to fix styling issues.
struct CustomRenderHooks;

impl RenderHooks for CustomRenderHooks {
    fn heading1(&self, text: &str) -> Option<Line<'static>> {
        let text = text.replace('\t', "    ");
        Some(Line::from(Span::styled(
            text,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )))
    }

    fn heading2(&self, text: &str) -> Option<Line<'static>> {
        let text = text.replace('\t', "    ");
        Some(Line::from(Span::styled(
            text,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )))
    }

    fn heading3(&self, text: &str) -> Option<Line<'static>> {
        let text = text.replace('\t', "    ");
        Some(Line::from(Span::styled(
            text,
            Style::default()
                .fg(Color::LightCyan)
                .add_modifier(Modifier::BOLD),
        )))
    }

    fn inline_code(&self, code: &str) -> Option<Line<'static>> {
        let code = code.replace('\t', "    ");
        Some(Line::from(Span::styled(
            format!("`{}`", code),
            Style::default().fg(Color::Yellow).bg(Color::DarkGray),
        )))
    }
}

/// Render markdown text using ratatui-markdown.
/// Returns a vector of Lines for rendering in ratatui.
pub fn render_markdown(text: &str, max_width: usize) -> Vec<Line<'static>> {
    if text.is_empty() {
        return Vec::new();
    }

    let renderer = MarkdownRenderer::new(max_width)
        .with_render_hooks(Box::new(CustomRenderHooks));
    let blocks = renderer.parse(text);
    let theme = MarkdownTheme;
    renderer.render(&blocks, &theme)
}

/// Render markdown text with a custom theme.
#[allow(dead_code)]
pub fn render_markdown_with_theme(text: &str, max_width: usize, theme: &impl RichTextTheme) -> Vec<Line<'static>> {
    if text.is_empty() {
        return Vec::new();
    }

    let renderer = MarkdownRenderer::new(max_width)
        .with_render_hooks(Box::new(CustomRenderHooks));
    let blocks = renderer.parse(text);
    renderer.render(&blocks, theme)
}

/// Render markdown text using ThemeConfig for customizable styling.
#[allow(dead_code)]
pub fn render_markdown_with_config(text: &str, max_width: usize, config: &ThemeConfig) -> Vec<Line<'static>> {
    if text.is_empty() {
        return Vec::new();
    }

    let renderer = MarkdownRenderer::new(max_width)
        .with_render_hooks(Box::new(CustomRenderHooks));
    let blocks = renderer.parse(text);
    renderer.render(&blocks, config)
}

/// Create a default theme config with sensible colors for TUI.
#[allow(dead_code)]
pub fn default_theme_config() -> ThemeConfig {
    ThemeConfig::builder()
        .with_text_color(Color::Gray)
        .with_muted_text_color(Color::DarkGray)
        .with_primary_color(Color::Cyan)
        .with_secondary_color(Color::LightCyan)
        .with_info_color(Color::LightBlue)
        .with_border_color(Color::DarkGray)
        .with_focused_border_color(Color::White)
        .with_accent_yellow(Color::Yellow)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_text() {
        let result = render_markdown("Hello world", 80);
        assert!(!result.is_empty());
        let text = result[0].spans.iter().map(|s| s.content.as_ref()).collect::<String>();
        assert!(text.contains("Hello"));
    }

    #[test]
    fn test_heading() {
        let result = render_markdown("# Title", 80);
        assert!(!result.is_empty());
        // Heading should have cyan color
        assert_eq!(result[0].spans[0].style.fg, Some(Color::Cyan));
    }

    #[test]
    fn test_heading2() {
        let result = render_markdown("## Subtitle", 80);
        assert!(!result.is_empty());
        assert!(result[0].spans.len() > 0);
        assert_eq!(result[0].spans[0].style.fg, Some(Color::White));
    }

    #[test]
    fn test_heading3() {
        let result = render_markdown("### Section", 80);
        assert!(!result.is_empty());
        assert!(result[0].spans.len() > 0);
        assert_eq!(result[0].spans[0].style.fg, Some(Color::LightCyan));
    }

    #[test]
    fn test_inline_code() {
        let result = render_markdown("Use `code` here", 80);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_bold() {
        let result = render_markdown("**Bold** text", 80);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_italic() {
        let result = render_markdown("*italic* text", 80);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_list() {
        let result = render_markdown("- Item 1\n- Item 2", 80);
        assert!(!result.is_empty());
        assert!(result.len() >= 2);
    }

    #[test]
    fn test_numbered_list() {
        let result = render_markdown("1. First\n2. Second", 80);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_code_block() {
        let result = render_markdown("```rust\nfn main() {}\n```", 80);
        assert!(!result.is_empty());
        // Code block should have multiple lines (header, content, footer)
        assert!(result.len() >= 3);
    }

    #[test]
    fn test_blockquote() {
        let result = render_markdown("> Quote text", 80);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_table() {
        let result = render_markdown("| A | B |\n|---|---|\n| 1 | 2 |", 80);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_horizontal_rule() {
        let result = render_markdown("---\nText after", 80);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_paragraph() {
        let result = render_markdown("First paragraph.\n\nSecond paragraph.", 80);
        assert!(!result.is_empty());
        // Should have blank line between paragraphs
        assert!(result.len() >= 3);
    }

    #[test]
    fn test_with_custom_config() {
        let config = default_theme_config();
        let result = render_markdown_with_config("# Test", 80, &config);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_empty_input() {
        let result = render_markdown("", 80);
        assert!(result.is_empty());
    }

    #[test]
    fn test_width_constraint() {
        let long_text = "This is a very long line that should be wrapped according to the max width constraint provided";
        let result = render_markdown(long_text, 40);
        assert!(!result.is_empty());
        // Check that lines are wrapped
        for line in &result {
            let line_width: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
            // Allow some tolerance for word boundaries
            assert!(line_width <= 50, "Line too wide: {}", line_width);
        }
    }
}