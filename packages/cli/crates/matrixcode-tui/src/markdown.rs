use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use pulldown_cmark::{Parser, Event, Tag, TagEnd, HeadingLevel, CodeBlockKind, Options};
use unicode_width::UnicodeWidthStr;
use std::sync::OnceLock;

/// Lazy-loaded syntax set for code highlighting
static SYNTAX_SET: OnceLock<syntect::parsing::SyntaxSet> = OnceLock::new();
/// Lazy-loaded theme set for code highlighting
static THEME_SET: OnceLock<syntect::highlighting::ThemeSet> = OnceLock::new();

fn get_syntax_set() -> &'static syntect::parsing::SyntaxSet {
    SYNTAX_SET.get_or_init(|| syntect::parsing::SyntaxSet::load_defaults_newlines())
}

fn get_theme_set() -> &'static syntect::highlighting::ThemeSet {
    THEME_SET.get_or_init(|| syntect::highlighting::ThemeSet::load_defaults())
}

/// Inline code style for `code`
fn inline_code_style() -> Style {
    Style::default()
        .fg(Color::Yellow)
        .bg(Color::DarkGray)
}

/// Math formula style
fn math_style() -> Style {
    Style::default()
        .fg(Color::LightMagenta)
        .bg(Color::DarkGray)
}

/// Heading style for # ## ### etc.
fn heading_style(level: HeadingLevel) -> Style {
    let color = match level {
        HeadingLevel::H1 => Color::Cyan,
        HeadingLevel::H2 => Color::LightCyan,
        HeadingLevel::H3 => Color::White,
        _ => Color::Gray,
    };
    Style::default()
        .fg(color)
        .add_modifier(Modifier::BOLD)
}

/// Bold text style
fn bold_style() -> Style {
    Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD)
}

/// Italic text style
fn italic_style() -> Style {
    Style::default()
        .fg(Color::Gray)
        .add_modifier(Modifier::ITALIC)
}

/// Strikethrough style
fn strikethrough_style() -> Style {
    Style::default()
        .fg(Color::Gray)
        .add_modifier(Modifier::CROSSED_OUT)
}

/// Link style
fn link_style() -> Style {
    Style::default()
        .fg(Color::Blue)
        .add_modifier(Modifier::UNDERLINED)
}

/// Normal text style
fn text_style() -> Style {
    Style::default().fg(Color::Gray)
}

/// List bullet style
fn bullet_style() -> Style {
    Style::default().fg(Color::DarkGray)
}

/// Render markdown text with proper code block handling
/// Returns a vector of Lines for rendering in ratatui
pub fn render_markdown(text: &str, _max_width: usize) -> Vec<Line<'static>> {
    if text.is_empty() {
        return Vec::new();
    }

    // Enable all extensions for better markdown support
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_MATH);

    let parser = Parser::new_ext(text, options);
    let mut renderer = MarkdownRenderer::new();
    renderer.render(parser);
    renderer.lines
}

/// Markdown renderer state
struct MarkdownRenderer {
    lines: Vec<Line<'static>>,
    current_spans: Vec<Span<'static>>,
    style_stack: Vec<Style>,
    in_code_block: bool,
    code_block_lang: Option<String>,
    code_block_content: String,
    list_depth: usize,
    // Table support
    in_table_cell: bool,
    current_table_row: Vec<String>,
    current_cell_content: String,
    table_header: Vec<String>,
    table_rows: Vec<Vec<String>>,
    in_table_header: bool,
}

impl MarkdownRenderer {
    fn new() -> Self {
        Self {
            lines: Vec::new(),
            current_spans: Vec::new(),
            style_stack: vec![text_style()],
            in_code_block: false,
            code_block_lang: None,
            code_block_content: String::new(),
            list_depth: 0,
            in_table_cell: false,
            current_table_row: Vec::new(),
            current_cell_content: String::new(),
            table_header: Vec::new(),
            table_rows: Vec::new(),
            in_table_header: false,
        }
    }

    fn current_style(&self) -> Style {
        *self.style_stack.last().unwrap_or(&text_style())
    }

    fn push_style(&mut self, style: Style) {
        self.style_stack.push(style);
    }

    fn pop_style(&mut self) {
        if self.style_stack.len() > 1 {
            self.style_stack.pop();
        }
    }

    fn flush_line(&mut self) {
        if !self.current_spans.is_empty() {
            self.lines.push(Line::from(self.current_spans.clone()));
            self.current_spans.clear();
        }
    }

    fn add_text(&mut self, text: &str) {
        let style = self.current_style();
        self.current_spans.push(Span::styled(text.to_string(), style));
    }

    fn render(&mut self, parser: Parser) {
        for event in parser {
            match event {
                Event::Start(tag) => self.handle_start(tag),
                Event::End(tag_end) => self.handle_end(tag_end),
                Event::Text(text) => {
                    if self.in_code_block {
                        self.code_block_content.push_str(&text);
                    } else if self.in_table_cell {
                        self.current_cell_content.push_str(&text);
                    } else {
                        self.add_text(&text);
                    }
                }
                Event::Code(code) => {
                    self.current_spans.push(Span::styled(code.to_string(), inline_code_style()));
                }
                Event::InlineMath(math) => {
                    // Inline math: $formula$
                    self.current_spans.push(Span::styled(
                        format!("${}$", math),
                        math_style()
                    ));
                }
                Event::DisplayMath(math) => {
                    // Display math: $$formula$$ - render as separate block
                    self.flush_line();
                    self.lines.push(Line::styled(
                        format!("  $${}$$", math),
                        math_style()
                    ));
                }
                Event::Html(html) => self.add_text(&html),
                Event::InlineHtml(html) => self.add_text(&html),
                Event::SoftBreak => self.add_text(" "),
                Event::HardBreak => self.flush_line(),
                Event::Rule => {
                    self.flush_line();
                    self.lines.push(Line::styled("─".repeat(40), Style::default().fg(Color::DarkGray)));
                }
                Event::FootnoteReference(_) | Event::TaskListMarker(_) => {}
            }
        }
        self.flush_line();
    }

    fn handle_start(&mut self, tag: Tag) {
        match tag {
            Tag::Heading { level, .. } => {
                self.flush_line();
                self.push_style(heading_style(level));
            }
            Tag::Paragraph => {
                self.flush_line();
            }
            Tag::CodeBlock(kind) => {
                self.flush_line();
                self.in_code_block = true;
                self.code_block_lang = match kind {
                    CodeBlockKind::Fenced(lang) => Some(lang.to_string()),
                    CodeBlockKind::Indented => None,
                };
            }
            Tag::List(_) => {
                self.flush_line();
                self.list_depth += 1;
            }
            Tag::Item => {
                self.flush_line();
                let indent = "  ".repeat(self.list_depth.saturating_sub(1));
                self.current_spans.push(Span::styled(format!("{}• ", indent), bullet_style()));
            }
            Tag::BlockQuote(_) => {
                self.flush_line();
                self.push_style(Style::default().fg(Color::DarkGray));
                self.current_spans.push(Span::styled("│ ", Style::default().fg(Color::DarkGray)));
            }
            Tag::Strong => {
                self.push_style(bold_style());
            }
            Tag::Emphasis => {
                self.push_style(italic_style());
            }
            Tag::Strikethrough => {
                self.push_style(strikethrough_style());
            }
            Tag::Link { dest_url: _, .. } => {
                self.push_style(link_style());
                self.current_spans.push(Span::styled("[", Style::default().fg(Color::DarkGray)));
            }
            Tag::Image { title, .. } => {
                self.add_text("📷 ");
                if !title.is_empty() {
                    self.add_text(&title);
                }
            }
            Tag::Table(_) => {
                self.flush_line();
                self.table_header.clear();
                self.table_rows.clear();
            }
            Tag::TableHead => {
                self.in_table_header = true;
            }
            Tag::TableRow => {
                self.current_table_row.clear();
            }
            Tag::TableCell => {
                self.current_cell_content.clear();
                self.in_table_cell = true;
            }
            Tag::FootnoteDefinition(_) => {}
            Tag::HtmlBlock => {}
            Tag::DefinitionList => {}
            Tag::DefinitionListTitle => {}
            Tag::DefinitionListDefinition => {}
            Tag::Superscript => self.push_style(Style::default().fg(Color::Gray)),
            Tag::Subscript => self.push_style(Style::default().fg(Color::Gray)),
            Tag::MetadataBlock(_) => {}
        }
    }

    fn handle_end(&mut self, tag_end: TagEnd) {
        match tag_end {
            TagEnd::Heading(_) => {
                self.flush_line();
                self.pop_style();
            }
            TagEnd::Paragraph => {
                self.flush_line();
                self.lines.push(Line::raw(""));
            }
            TagEnd::CodeBlock => {
                self.flush_code_block();
                self.in_code_block = false;
                self.code_block_lang = None;
                self.code_block_content.clear();
            }
            TagEnd::List(_) => {
                self.flush_line();
                self.list_depth = self.list_depth.saturating_sub(1);
            }
            TagEnd::Item => {
                self.flush_line();
            }
            TagEnd::BlockQuote(_) => {
                self.flush_line();
                self.pop_style();
            }
            TagEnd::Strong => self.pop_style(),
            TagEnd::Emphasis => self.pop_style(),
            TagEnd::Strikethrough => self.pop_style(),
            TagEnd::Link => {
                self.pop_style();
                self.current_spans.push(Span::styled("]", Style::default().fg(Color::DarkGray)));
            }
            TagEnd::Image => {}
            TagEnd::Table => {
                self.flush_line();
                self.render_table();
            }
            TagEnd::TableHead => {
                // TableHead end: move collected cells to table_header
                self.table_header = self.current_table_row.clone();
                self.current_table_row.clear();
                self.in_table_header = false;
            }
            TagEnd::TableRow => {
                if self.in_table_header {
                    self.table_header = self.current_table_row.clone();
                } else {
                    self.table_rows.push(self.current_table_row.clone());
                }
                self.current_table_row.clear();
            }
            TagEnd::TableCell => {
                self.current_table_row.push(self.current_cell_content.clone());
                self.current_cell_content.clear();
                self.in_table_cell = false;
            }
            TagEnd::FootnoteDefinition => {}
            TagEnd::HtmlBlock => {}
            TagEnd::DefinitionList => {}
            TagEnd::DefinitionListTitle => {}
            TagEnd::DefinitionListDefinition => {}
            TagEnd::Superscript => self.pop_style(),
            TagEnd::Subscript => self.pop_style(),
            TagEnd::MetadataBlock(_) => {}
        }
    }

    fn flush_code_block(&mut self) {
        let lang = self.code_block_lang.as_deref().unwrap_or("");
        let code = &self.code_block_content;

        // Optional: show language label on first line
        if !lang.is_empty() {
            self.lines.push(Line::styled(
                format!("// {}", lang),
                Style::default().fg(Color::DarkGray)
            ));
        }

        for line_spans in self.highlight_code_with_colors(lang, code) {
            self.lines.push(Line::from(line_spans));
        }
    }

    fn highlight_code_with_colors(&self, lang: &str, code: &str) -> Vec<Vec<Span<'static>>> {
        let ss = get_syntax_set();
        let ts = get_theme_set();

        let syntax = ss.find_syntax_by_token(lang)
            .or_else(|| ss.find_syntax_by_extension(lang))
            .unwrap_or_else(|| ss.find_syntax_plain_text());

        let theme = ts.themes.get("base16-eighties.dark")
            .or_else(|| ts.themes.get("Solarized (dark)"))
            .or_else(|| ts.themes.get("base16-mono.dark"))
            .or_else(|| ts.themes.values().next())
            .unwrap();

        use syntect::easy::HighlightLines;
        let mut highlighter = HighlightLines::new(syntax, theme);

        code.lines()
            .map(|line| {
                let highlighted = highlighter.highlight_line(line, ss).unwrap_or_default();
                highlighted.iter().map(|(style, text)| {
                    let fg = syntect_color_to_ratatui(style.foreground);
                    Span::styled(text.to_string(), Style::default().fg(fg))
                }).collect()
            })
            .collect()
    }

    fn render_table(&mut self) {
        if self.table_header.is_empty() {
            return;
        }

        // Calculate column widths using Unicode display width (+2 for padding)
        let mut widths: Vec<usize> = self.table_header.iter()
            .map(|c| c.width().max(3) + 2)
            .collect();

        for row in &self.table_rows {
            for (i, cell) in row.iter().enumerate() {
                if i < widths.len() {
                    widths[i] = widths[i].max(cell.width() + 2);
                }
            }
        }

        // Build border parts
        let top_border = self.make_border(&widths, "┌", "┬", "┐");
        let mid_border = self.make_border(&widths, "├", "┼", "┤");
        let bottom_border = self.make_border(&widths, "└", "┴", "┘");

        // Top border
        self.lines.push(Line::styled(top_border, Style::default().fg(Color::DarkGray)));

        // Header row
        let header_line = self.format_table_row(&self.table_header, &widths);
        self.lines.push(Line::styled(header_line, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));

        // Separator after header
        self.lines.push(Line::styled(mid_border, Style::default().fg(Color::DarkGray)));

        // Data rows
        for row in &self.table_rows {
            let row_line = self.format_table_row(row, &widths);
            self.lines.push(Line::styled(row_line, Style::default().fg(Color::Gray)));
        }

        // Bottom border
        self.lines.push(Line::styled(bottom_border, Style::default().fg(Color::DarkGray)));
    }

    fn make_border(&self, widths: &[usize], left: &str, mid: &str, right: &str) -> String {
        let parts: Vec<String> = widths.iter().map(|w| "─".repeat(*w)).collect();
        format!("{}{}{}", left, parts.join(mid), right)
    }

    fn format_table_row(&self, cells: &[String], widths: &[usize]) -> String {
        let formatted: Vec<String> = cells.iter()
            .enumerate()
            .map(|(i, cell)| {
                let cell_width = widths.get(i).copied().unwrap_or(cell.width() + 2);
                self.center_cell(cell, cell_width)
            })
            .collect();
        format!("│{}│", formatted.join("│"))
    }

    fn center_cell(&self, content: &str, cell_width: usize) -> String {
        let content_width = content.width();
        let total_padding = cell_width.saturating_sub(content_width);

        if total_padding == 0 {
            return content.to_string();
        }

        let left_pad = total_padding / 2;
        let right_pad = total_padding - left_pad;

        format!("{}{}{}", " ".repeat(left_pad), content, " ".repeat(right_pad))
    }
}

fn syntect_color_to_ratatui(c: syntect::highlighting::Color) -> Color {
    let r = c.r;
    let g = c.g;
    let b = c.b;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    if max - min < 20 {
        if r < 80 { return Color::DarkGray; }
        if r < 160 { return Color::Gray; }
        return Color::White;
    }

    if r >= g && r >= b && r > 150 {
        if g > 100 && b < 100 { return Color::Yellow; }
        if b > 100 && g < 100 { return Color::Magenta; }
        return Color::Red;
    }
    if g >= r && g >= b && g > 150 {
        if b > 100 && r < 100 { return Color::Cyan; }
        return Color::Green;
    }
    if b >= r && b >= g && b > 150 {
        return Color::Blue;
    }

    Color::Rgb(r, g, b)
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
        let text = result[0].spans.iter().map(|s| s.content.as_ref()).collect::<String>();
        assert!(text.contains("Title"));
    }

    #[test]
    fn test_table() {
        let md = "| A | B |\n|---|---|\n| 1 | 2 |";
        let result = render_markdown(md, 80);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_math() {
        let md = "Inline: $E=mc^2$";
        let result = render_markdown(md, 80);
        assert!(!result.is_empty());
        let text = result[0].spans.iter().map(|s| s.content.as_ref()).collect::<String>();
        assert!(text.contains("$E=mc^2$"));
    }
}

#[cfg(test)]
mod debug_tests {
    use super::*;

    #[test]
    fn debug_table() {
        let md = "| A | B |\n|---|---|\n| 1 | 2 |";
        println!("\n=== Simple Table ===");
        let lines = render_markdown(md, 60);
        for (i, line) in lines.iter().enumerate() {
            let text = line.spans.iter().map(|s| s.content.as_ref()).collect::<String>();
            println!("[{}] '{}'", i, text);
        }
    }

    #[test]
    fn debug_table_chinese() {
        let md = "| 名称 | 数值 |\n|------|------|\n| 测试 | 123 |\n| 数据 | 456 |";
        println!("\n=== Chinese Table ===");
        let lines = render_markdown(md, 80);
        for (i, line) in lines.iter().enumerate() {
            let text = line.spans.iter().map(|s| s.content.as_ref()).collect::<String>();
            println!("[{}] '{}'", i, text);
        }
    }

    #[test]
    fn debug_table_multi_row() {
        let md = "| Col1 | Col2 | Col3 |\n|------|------|------|\n| A1 | B1 | C1 |\n| A2 | B2 | C2 |\n| A3 | B3 | C3 |";
        println!("\n=== Multi-row Table ===");
        let lines = render_markdown(md, 80);
        for (i, line) in lines.iter().enumerate() {
            let text = line.spans.iter().map(|s| s.content.as_ref()).collect::<String>();
            println!("[{}] '{}'", i, text);
        }
    }

    #[test]
    fn debug_math() {
        let md = "Inline: $E=mc^2$\n\nBlock:\n$$\\sum_{i=1}^n i$$";
        println!("\n=== Math ===");
        let lines = render_markdown(md, 80);
        for (i, line) in lines.iter().enumerate() {
            let text = line.spans.iter().map(|s| s.content.as_ref()).collect::<String>();
            println!("[{}] '{}'", i, text);
        }
    }
}