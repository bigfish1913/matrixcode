use ratatui::text::Line;

/// Render markdown using tui-markdown library
/// Note: Style conversion is simplified due to version differences
pub fn render_markdown<'a>(text: &'a str, _max_w: usize) -> Vec<Line<'a>> {
    let parsed = tui_markdown::from_str(text);
    parsed.lines.into_iter().map(|line| {
        // Flatten spans into a single styled line (simplified)
        let content = line.spans.iter().map(|s| s.content.as_ref()).collect::<String>();
        Line::raw(content)
    }).collect()
}