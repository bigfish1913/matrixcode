use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Code block style for fenced code blocks (```)
fn code_block_style() -> Style {
    Style::default()
        .fg(Color::LightGreen)
        .bg(Color::DarkGray)
}

/// Inline code style for `code`
fn inline_code_style() -> Style {
    Style::default()
        .fg(Color::Yellow)
        .bg(Color::DarkGray)
}

/// Heading style for # ## ### etc.
fn heading_style(level: usize) -> Style {
    let color = match level {
        1 => Color::Cyan,
        2 => Color::LightCyan,
        3 => Color::White,
        _ => Color::Gray,
    };
    Style::default()
        .fg(color)
        .add_modifier(Modifier::BOLD)
}

/// Render markdown text with proper code block handling
/// Returns a vector of Lines for rendering in ratatui
pub fn render_markdown(text: &str, _max_width: usize) -> Vec<Line<'static>> {
    if text.is_empty() {
        return Vec::new();
    }
    
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut in_code_block = false;
    let mut code_block_lines: Vec<String> = Vec::new();
    
    for line in text.lines() {
        // Check for fenced code block start/end
        if line.trim().starts_with("```") {
            if !in_code_block {
                // Start of code block
                in_code_block = true;
                // Show language hint if present
                let lang = line.trim().strip_prefix("```").map(|s| s.trim()).unwrap_or("");
                let header = if lang.is_empty() { 
                    "┌─ code ─".to_string() 
                } else { 
                    format!("┌─ {} ─", lang) 
                };
                lines.push(Line::styled(
                    header,
                    Style::default().fg(Color::DarkGray)
                ));
            } else {
                // End of code block
                in_code_block = false;
                // Render collected code lines with proper styling
                for code_line in &code_block_lines {
                    lines.push(Line::styled(
                        format!("│ {}", code_line),
                        code_block_style()
                    ));
                }
                lines.push(Line::styled(
                    "└───────",
                    Style::default().fg(Color::DarkGray)
                ));
                code_block_lines.clear();
            }
            continue;
        }
        
        if in_code_block {
            // Collect code block content
            code_block_lines.push(line.to_string());
        } else {
            // Process inline markdown and headings
            let processed_line = process_inline_markdown(line);
            lines.extend(processed_line);
        }
    }
    
    // Handle unclosed code block
    if in_code_block && !code_block_lines.is_empty() {
        for code_line in &code_block_lines {
            lines.push(Line::styled(
                format!("│ {}", code_line),
                code_block_style()
            ));
        }
    }
    
    lines
}

/// Process inline markdown elements (headings, lists, inline code, bold, etc.)
fn process_inline_markdown(line: &str) -> Vec<Line<'static>> {
    let mut result_lines: Vec<Line<'static>> = Vec::new();
    
    // Check for heading (starts with #)
    let trimmed = line.trim_start();
    let heading_level = trimmed.chars().take_while(|c| *c == '#').count();
    if heading_level > 0 && trimmed.chars().nth(heading_level) == Some(' ') {
        // It's a heading: remove # symbols and render with heading style
        let heading_text = trimmed[heading_level + 1..].trim();
        if !heading_text.is_empty() {
            result_lines.push(Line::styled(
                heading_text.to_string(),
                heading_style(heading_level)
            ));
            return result_lines;
        }
    }
    
    // Check for list item (starts with - or * followed by space)
    if trimmed.starts_with("- ") || (trimmed.starts_with("* ") && !trimmed.starts_with("** ")) {
        let list_content = &trimmed[2..];
        let mut spans: Vec<Span<'static>> = vec![
            Span::styled("  • ", Style::default().fg(Color::DarkGray)),
        ];
        let processed = process_inline_elements(list_content);
        spans.extend(processed);
        result_lines.push(Line::from(spans));
        return result_lines;
    }
    
    // Check for numbered list (starts with number followed by .)
    if let Some(rest) = trimmed.strip_prefix(|c: char| c.is_ascii_digit()) {
        if rest.starts_with(". ") {
            let list_content = &rest[2..];
            let processed = process_inline_elements(list_content);
            result_lines.push(Line::from(processed));
            return result_lines;
        }
    }
    
    // Regular line - process inline elements
    let processed = process_inline_elements(line);
    result_lines.push(Line::from(processed));
    
    result_lines
}

/// Process inline elements (inline code, bold) and return spans
fn process_inline_elements(line: &str) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut chars = line.chars().peekable();
    let mut current_text = String::new();
    
    while let Some(ch) = chars.next() {
        // Check for inline code `...`
        if ch == '`' {
            // Flush current text
            if !current_text.is_empty() {
                spans.push(Span::styled(current_text.clone(), Style::default().fg(Color::Gray)));
                current_text.clear();
            }
            
            // Collect inline code content (handle unclosed gracefully)
            let mut code_content = String::new();
            while let Some(c) = chars.next() {
                if c == '`' {
                    break;
                }
                code_content.push(c);
            }
            // If no closing `, treat the whole rest as code (graceful handling)
            spans.push(Span::styled(code_content, inline_code_style()));
        } else if ch == '*' {
            // Check for bold **...**
            if chars.peek() == Some(&'*') {
                chars.next(); // consume second *
                
                // Flush current text
                if !current_text.is_empty() {
                    spans.push(Span::styled(current_text.clone(), Style::default().fg(Color::Gray)));
                    current_text.clear();
                }
                
                // Collect bold content (handle unclosed gracefully)
                let mut bold_content = String::new();
                while let Some(c) = chars.next() {
                    if c == '*' && chars.peek() == Some(&'*') {
                        chars.next(); // consume second *
                        break;
                    }
                    bold_content.push(c);
                }
                spans.push(Span::styled(
                    bold_content,
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                ));
            } else {
                current_text.push(ch);
            }
        } else {
            current_text.push(ch);
        }
    }
    
    // Flush remaining text
    if !current_text.is_empty() {
        spans.push(Span::styled(current_text, Style::default().fg(Color::Gray)));
    }
    
    if spans.is_empty() {
        spans.push(Span::raw(""));
    }
    
    spans
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
    fn test_heading() {
        let result = render_markdown("# Title", 80);
        assert!(!result.is_empty(), "Heading should produce lines");
        let text = result[0].spans.iter().map(|s| s.content.as_ref()).collect::<String>();
        assert!(text.contains("Title"), "Should contain 'Title' without #");
    }
    
    #[test]
    fn test_heading_level_2() {
        let result = render_markdown("## Subtitle", 80);
        assert!(!result.is_empty());
        let text = result[0].spans.iter().map(|s| s.content.as_ref()).collect::<String>();
        assert!(text.contains("Subtitle"));
    }
    
    #[test]
    fn test_list() {
        let result = render_markdown("- Item one", 80);
        assert!(!result.is_empty());
        let text = result[0].spans.iter().map(|s| s.content.as_ref()).collect::<String>();
        assert!(text.contains("Item one"));
    }
    
    #[test]
    fn test_inline_code() {
        let result = render_markdown("Use `code` here", 80);
        assert!(!result.is_empty());
        // Should have 3 spans: "Use ", "code", " here"
        assert!(result[0].spans.len() >= 2);
    }
    
    #[test]
    fn test_bold() {
        let result = render_markdown("This is **bold** text", 80);
        assert!(!result.is_empty());
        // Should have multiple spans
        assert!(result[0].spans.len() >= 2);
    }
    
    #[test]
    fn test_code_block() {
        let result = render_markdown("```rust\nfn main() {}\n```", 80);
        assert!(result.len() >= 3, "Code block should have header, content, footer");
        // Check header contains language
        let header = result[0].spans.iter().map(|s| s.content.as_ref()).collect::<String>();
        assert!(header.contains("rust"));
    }
    
    #[test]
    fn test_empty() {
        let result = render_markdown("", 80);
        assert!(result.is_empty(), "Empty input should produce empty output");
    }
    
    #[test]
    fn test_unclosed_inline_code() {
        // Should gracefully handle unclosed inline code
        let result = render_markdown("Use `code here", 80);
        assert!(!result.is_empty());
        let text = result[0].spans.iter().map(|s| s.content.as_ref()).collect::<String>();
        assert!(text.contains("code"));
    }
    
    #[test]
    fn test_unclosed_bold() {
        // Should gracefully handle unclosed bold
        let result = render_markdown("This is **bold text", 80);
        assert!(!result.is_empty());
        let text = result[0].spans.iter().map(|s| s.content.as_ref()).collect::<String>();
        assert!(text.contains("bold"));
    }
}