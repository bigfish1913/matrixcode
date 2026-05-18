use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

/// Render markdown using tui-markdown library
pub fn render_markdown<'a>(text: &'a str, _max_w: usize) -> Vec<Line<'a>> {
    let parsed = tui_markdown::from_str(text);
    parsed.lines.into_iter().map(|line| {
        let spans: Vec<Span<'a>> = line.spans.into_iter().map(|s| {
            Span::styled(s.content, convert_style(s.style))
        }).collect();
        Line::from(spans)
    }).collect()
}

fn convert_style(core: ratatui_core::style::Style) -> Style {
    Style::default()
        .fg(core.fg.map(convert_color).unwrap_or(Color::Reset))
        .bg(core.bg.map(convert_color).unwrap_or(Color::Reset))
        .add_modifier(convert_modifier(core.add_modifier))
}

fn convert_color(c: ratatui_core::style::Color) -> Color {
    match c {
        ratatui_core::style::Color::Reset => Color::Reset,
        ratatui_core::style::Color::Black => Color::Black,
        ratatui_core::style::Color::Red => Color::Red,
        ratatui_core::style::Color::Green => Color::Green,
        ratatui_core::style::Color::Yellow => Color::Yellow,
        ratatui_core::style::Color::Blue => Color::Blue,
        ratatui_core::style::Color::Magenta => Color::Magenta,
        ratatui_core::style::Color::Cyan => Color::Cyan,
        ratatui_core::style::Color::Gray => Color::Gray,
        ratatui_core::style::Color::DarkGray => Color::DarkGray,
        ratatui_core::style::Color::LightRed => Color::LightRed,
        ratatui_core::style::Color::LightGreen => Color::LightGreen,
        ratatui_core::style::Color::LightYellow => Color::LightYellow,
        ratatui_core::style::Color::LightBlue => Color::LightBlue,
        ratatui_core::style::Color::LightMagenta => Color::LightMagenta,
        ratatui_core::style::Color::LightCyan => Color::LightCyan,
        ratatui_core::style::Color::White => Color::White,
        ratatui_core::style::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
        ratatui_core::style::Color::Indexed(i) => Color::Indexed(i),
    }
}

fn convert_modifier(m: ratatui_core::style::Modifier) -> Modifier {
    let mut result = Modifier::empty();
    if m.contains(ratatui_core::style::Modifier::BOLD) { result.insert(Modifier::BOLD); }
    if m.contains(ratatui_core::style::Modifier::ITALIC) { result.insert(Modifier::ITALIC); }
    if m.contains(ratatui_core::style::Modifier::UNDERLINED) { result.insert(Modifier::UNDERLINED); }
    if m.contains(ratatui_core::style::Modifier::DIM) { result.insert(Modifier::DIM); }
    if m.contains(ratatui_core::style::Modifier::HIDDEN) { result.insert(Modifier::HIDDEN); }
    if m.contains(ratatui_core::style::Modifier::SLOW_BLINK) { result.insert(Modifier::SLOW_BLINK); }
    if m.contains(ratatui_core::style::Modifier::RAPID_BLINK) { result.insert(Modifier::RAPID_BLINK); }
    if m.contains(ratatui_core::style::Modifier::REVERSED) { result.insert(Modifier::REVERSED); }
    result
}