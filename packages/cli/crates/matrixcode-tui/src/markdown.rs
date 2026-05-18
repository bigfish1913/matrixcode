use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use crate::utils::word_wrap;

/// Render markdown with table support and word wrap
pub fn render_markdown(text: &str, max_w: usize) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    
    // Enable table parsing and other extensions
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    
    let parser = Parser::new_ext(text, opts);
    
    let mut in_table = false;
    let mut table_rows: Vec<Vec<String>> = Vec::new();
    let mut current_row: Vec<String> = Vec::new();
    let mut current_cell = String::new();
    let mut is_header_row = false;
    
    // Collect non-table content chunks
    let mut chunks: Vec<String> = Vec::new();
    let mut current_chunk = String::new();
    
    for event in parser {
        match event {
            Event::Start(Tag::Table(_)) => {
                // Flush current chunk
                if !current_chunk.trim().is_empty() {
                    chunks.push(current_chunk.clone());
                    current_chunk.clear();
                }
                in_table = true;
                table_rows.clear();
            }
            Event::Start(Tag::TableHead) => {
                is_header_row = true;
                current_row.clear();
            }
            Event::Start(Tag::TableRow) => {
                if !is_header_row {
                    current_row.clear();
                }
            }
            Event::Start(Tag::TableCell) => {
                current_cell.clear();
            }
            Event::End(TagEnd::TableCell) => {
                current_row.push(current_cell.trim().to_string());
            }
            Event::End(TagEnd::TableHead) => {
                if !current_row.is_empty() {
                    table_rows.push(current_row.clone());
                }
                is_header_row = false;
            }
            Event::End(TagEnd::TableRow) => {
                if !is_header_row && !current_row.is_empty() {
                    table_rows.push(current_row.clone());
                }
            }
            Event::End(TagEnd::Table) => {
                // Render table
                if !table_rows.is_empty() {
                    let table_lines = render_table(&table_rows, max_w);
                    lines.extend(table_lines);
                }
                in_table = false;
                table_rows.clear();
            }
            Event::Text(t) | Event::Code(t) if in_table => {
                current_cell.push_str(&t);
            }
            _ if in_table => {}
            _ => {
                // Reconstruct markdown for non-table content
                reconstruct_event(&mut current_chunk, event);
            }
        }
    }
    
    // Flush final chunk
    if !current_chunk.trim().is_empty() {
        chunks.push(current_chunk);
    }
    
    // Render non-table chunks with tui-markdown
    for chunk in chunks {
        if chunk.trim().is_empty() {
            continue;
        }
        
        // Pre-wrap long lines before rendering
        let wrapped_chunk = word_wrap_markdown(&chunk, max_w);
        
        // Process and render markdown with heading styling
        let md_lines = tui_markdown::from_str(&wrapped_chunk);
        for line in md_lines.lines {
            let content = line.spans.iter().map(|s| s.content.as_ref()).collect::<String>();
            
            // Skip code fence markers
            if content.trim().starts_with("```") {
                continue;
            }
            
            // Check for headings and apply style
            let trimmed = content.trim();
            let heading_level = if trimmed.starts_with("###### ") { 6 }
                               else if trimmed.starts_with("##### ") { 5 }
                               else if trimmed.starts_with("#### ") { 4 }
                               else if trimmed.starts_with("### ") { 3 }
                               else if trimmed.starts_with("## ") { 2 }
                               else if trimmed.starts_with("# ") { 1 }
                               else { 0 };
            
            if heading_level > 0 {
                // Apply heading style based on level
                let heading_style = match heading_level {
                    1 => Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                    2 => Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
                    3 => Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                    4 => Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    5 => Style::default().fg(Color::Magenta),
                    6 => Style::default().fg(Color::DarkGray),
                    _ => Style::default(),
                };
                
                // Remove heading marker (#) and render with style
                let heading_text = trimmed[heading_level + 1..].trim();  // Skip "# " (level + 1 space)
                lines.push(Line::styled(
                    format!("  {}", heading_text),
                    heading_style
                ));
                lines.push(Line::raw(""));  // Add spacing after heading
            } else {
                // Convert to owned Line
                let owned_spans: Vec<Span<'static>> = line.spans.iter().map(|s| {
                    Span::styled(s.content.to_string(), s.style)
                }).collect();
                lines.push(Line::from(owned_spans));
            }
        }
    }
    
    lines
}

/// Word wrap markdown content while preserving structure
fn word_wrap_markdown(text: &str, width: usize) -> String {
    if width == 0 { return text.to_string(); }
    
    let mut result = String::new();
    let mut in_code_block = false;
    
    for line in text.lines() {
        // Check for code block markers
        if line.trim().starts_with("```") {
            in_code_block = !in_code_block;
            result.push_str(line);
            result.push('\n');
            continue;
        }
        
        // Don't wrap code block content
        if in_code_block {
            result.push_str(line);
            result.push('\n');
            continue;
        }
        
        // Don't wrap headings, list markers, blockquotes
        let is_special = line.trim().starts_with('#') 
            || line.trim().starts_with('-') 
            || line.trim().starts_with('*')
            || line.trim().starts_with('>')
            || line.trim().is_empty();
        
        if is_special || line.chars().count() <= width {
            result.push_str(line);
            result.push('\n');
        } else {
            // Wrap long paragraph lines
            let wrapped = word_wrap(line, width);
            for w in wrapped {
                result.push_str(&w);
                result.push('\n');
            }
        }
    }
    
    result
}

fn reconstruct_event(s: &mut String, event: Event<'_>) {
    match event {
        Event::Text(t) => s.push_str(&t),
        Event::Code(c) => {
            s.push('`');
            s.push_str(&c);
            s.push('`');
        }
        Event::SoftBreak => s.push(' '),
        Event::HardBreak => s.push('\n'),
        Event::Start(tag) => push_tag_start(s, tag),
        Event::End(tag) => push_tag_end(s, tag),
        Event::Rule => s.push_str("---\n"),
        Event::TaskListMarker(checked) => {
            s.push_str(if checked { "[x] " } else { "[ ] " });
        }
        _ => {}
    }
}

fn push_tag_start(s: &mut String, tag: Tag<'_>) {
    match tag {
        Tag::Paragraph => {}
        Tag::Heading { level, .. } => {
            for _ in 0..level as usize {
                s.push('#');
            }
            s.push(' ');
        }
        Tag::BlockQuote(_) => s.push_str("> "),
        Tag::CodeBlock(kind) => {
            s.push_str("```");
            if let pulldown_cmark::CodeBlockKind::Fenced(lang) = kind {
                s.push_str(&lang);
            }
            s.push('\n');
        }
        Tag::List(_) => {}
        Tag::Item => s.push_str("- "),
        Tag::Emphasis => s.push('*'),
        Tag::Strong => s.push_str("**"),
        Tag::Strikethrough => s.push_str("~~"),
        Tag::Link { .. } => s.push('['),
        _ => {}
    }
}

fn push_tag_end(s: &mut String, tag: TagEnd) {
    match tag {
        TagEnd::Paragraph => s.push('\n'),
        TagEnd::Heading(_) => s.push('\n'),
        TagEnd::BlockQuote(_) => s.push('\n'),
        TagEnd::CodeBlock => s.push_str("```\n"),
        TagEnd::List(_) => {}
        TagEnd::Item => s.push('\n'),
        TagEnd::Emphasis => s.push('*'),
        TagEnd::Strong => s.push_str("**"),
        TagEnd::Strikethrough => s.push_str("~~"),
        TagEnd::Link => s.push(')'),
        _ => {}
    }
}

fn render_table(rows: &[Vec<String>], max_w: usize) -> Vec<Line<'static>> {
    if rows.is_empty() {
        return Vec::new();
    }
    
    let mut lines: Vec<Line<'static>> = Vec::new();
    
    // Calculate column widths
    let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if num_cols == 0 {
        return Vec::new();
    }
    
    let mut col_widths: Vec<usize> = vec![3; num_cols];
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < num_cols {
                col_widths[i] = col_widths[i].max(cell.chars().count().min(20));
            }
        }
    }
    
    // Limit total width
    let total_width = col_widths.iter().sum::<usize>() + (num_cols - 1) * 3 + 4;
    if total_width > max_w && max_w > 10 {
        let available = max_w - (num_cols - 1) * 3 - 4;
        let per_col = available / num_cols;
        for w in &mut col_widths {
            *w = (*w).min(per_col.max(3));
        }
    }
    
    // Use simple ASCII borders for better compatibility
    // Render top border: +---+---+
    let mut top = String::from("+");
    for (i, w) in col_widths.iter().enumerate() {
        top.push_str(&"-".repeat(*w + 2)); // +2 for padding
        if i < num_cols - 1 {
            top.push('+');
        }
    }
    top.push('+');
    lines.push(Line::styled(top, Style::default().fg(Color::DarkGray)));
    
    // Render rows
    for (row_idx, row) in rows.iter().enumerate() {
        let is_header = row_idx == 0;
        let style = if is_header {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        
        // Row: | cell | cell |
        let mut spans: Vec<Span<'static>> = vec![Span::styled("| ", Style::default().fg(Color::DarkGray))];
        
        for (i, cell) in row.iter().enumerate() {
            let w = col_widths.get(i).copied().unwrap_or(3);
            let padded = pad_cell(cell, w);
            spans.push(Span::styled(padded, style));
            if i < num_cols - 1 {
                spans.push(Span::styled(" | ", Style::default().fg(Color::DarkGray)));
            }
        }
        
        // Fill missing columns
        for i in row.len()..num_cols {
            let w = col_widths.get(i).copied().unwrap_or(3);
            spans.push(Span::styled(" ".repeat(w), Style::default()));
            if i < num_cols - 1 {
                spans.push(Span::styled(" | ", Style::default().fg(Color::DarkGray)));
            }
        }
        
        spans.push(Span::styled(" |", Style::default().fg(Color::DarkGray)));
        lines.push(Line::from(spans));
        
        // Add separator after header
        if is_header {
            let mut sep = String::from("+");
            for (i, w) in col_widths.iter().enumerate() {
                sep.push_str(&"-".repeat(*w + 2));
                if i < num_cols - 1 {
                    sep.push('+');
                }
            }
            sep.push('+');
            lines.push(Line::styled(sep, Style::default().fg(Color::DarkGray)));
        }
    }
    
    // Render bottom border
    let mut bottom = String::from("+");
    for (i, w) in col_widths.iter().enumerate() {
        bottom.push_str(&"-".repeat(*w + 2));
        if i < num_cols - 1 {
            bottom.push('+');
        }
    }
    bottom.push('+');
    lines.push(Line::styled(bottom, Style::default().fg(Color::DarkGray)));
    
    lines.push(Line::raw(""));
    lines
}

fn pad_cell(s: &str, width: usize) -> String {
    let chars: Vec<char> = s.chars().take(width).collect();
    let len = chars.len();
    if len >= width {
        chars.iter().collect()
    } else {
        let mut result: String = chars.iter().collect();
        result.extend(std::iter::repeat_n(' ', width - len));
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_table() {
        let md = "| Name | Value |\n|------|-------|\n| A | 1 |\n| B | 2 |";
        let lines = render_markdown(md, 80);
        // Should have borders (ASCII |)
        let has_border = lines.iter().any(|l| {
            l.spans.iter().map(|s| s.content.as_ref()).collect::<String>().contains('|')
        });
        assert!(has_border, "Should render table borders");
        // Should have header styled differently
        assert!(lines.len() >= 5, "Should have at least 5 lines for table");
    }
    
    #[test]
    fn test_code_block() {
        let md = "```rust\nfn main() {}\n```";
        let lines = render_markdown(md, 80);
        // Should not contain ``` markers
        for line in &lines {
            let content = line.spans.iter().map(|s| s.content.as_ref()).collect::<String>();
            assert!(!content.contains("```"), "Found fence marker: {}", content);
        }
        // Should contain code content
        let has_code = lines.iter().any(|l| {
            l.spans.iter().map(|s| s.content.as_ref()).collect::<String>().contains("fn main")
        });
        assert!(has_code, "Should contain code content");
    }
    
    #[test]
    fn test_heading() {
        let md = "# Heading\n\nContent";
        let lines = render_markdown(md, 80);
        let has_heading = lines.iter().any(|l| {
            let content = l.spans.iter().map(|s| s.content.as_ref()).collect::<String>();
            content.contains("Heading")
        });
        assert!(has_heading);
        
        // Check heading is styled (cyan bold for level 1)
        let heading_line = lines.iter().find(|l| {
            l.spans.iter().map(|s| s.content.as_ref()).collect::<String>().contains("Heading")
        });
        if let Some(line) = heading_line {
            // Should not contain # marker
            let content = line.spans.iter().map(|s| s.content.as_ref()).collect::<String>();
            assert!(!content.trim().starts_with("#"), "Heading should not have # marker");
        }
    }
    
    #[test]
    fn test_heading_levels() {
        let md = "# Title 1\n## Title 2\n### Title 3\n#### Title 4\n##### Title 5\n###### Title 6";
        let lines = render_markdown(md, 80);
        
        // All headings should be present without # markers
        for i in 1..=6 {
            let has_title = lines.iter().any(|l| {
                let content = l.spans.iter().map(|s| s.content.as_ref()).collect::<String>();
                content.contains(&format!("Title {}", i))
            });
            assert!(has_title, "Should have Title {}", i);
        }
        
        // No # markers should appear
        for line in &lines {
            let content = line.spans.iter().map(|s| s.content.as_ref()).collect::<String>();
            let trimmed = content.trim();
            assert!(!trimmed.starts_with("# "), "Should not have # markers");
        }
    }
    
    #[test]
    fn test_mixed() {
        let md = "# Title\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\nText here";
        let lines = render_markdown(md, 80);
        let has_title = lines.iter().any(|l| {
            l.spans.iter().map(|s| s.content.as_ref()).collect::<String>().contains("Title")
        });
        let has_table = lines.iter().any(|l| {
            l.spans.iter().map(|s| s.content.as_ref()).collect::<String>().contains('|')
        });
        let has_text = lines.iter().any(|l| {
            l.spans.iter().map(|s| s.content.as_ref()).collect::<String>().contains("Text")
        });
        assert!(has_title);
        assert!(has_table);
        assert!(has_text);
    }
}