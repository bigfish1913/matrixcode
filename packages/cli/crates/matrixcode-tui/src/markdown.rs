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
            // Process inline code and other markdown
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

/// Process inline markdown elements (inline code, bold, etc.)
fn process_inline_markdown(line: &str) -> Vec<Line<'static>> {
    let mut result_lines: Vec<Line<'static>> = Vec::new();
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
            
            // Collect inline code content
            let mut code_content = String::new();
            while let Some(c) = chars.next() {
                if c == '`' {
                    break;
                }
                code_content.push(c);
            }
            if !code_content.is_empty() {
                spans.push(Span::styled(code_content, inline_code_style()));
            }
        } else if ch == '*' {
            // Check for bold **...**
            if chars.peek() == Some(&'*') {
                chars.next(); // consume second *
                
                // Flush current text
                if !current_text.is_empty() {
                    spans.push(Span::styled(current_text.clone(), Style::default().fg(Color::Gray)));
                    current_text.clear();
                }
                
                // Collect bold content
                let mut bold_content = String::new();
                while let Some(c) = chars.next() {
                    if c == '*' && chars.peek() == Some(&'*') {
                        chars.next(); // consume second *
                        break;
                    }
                    bold_content.push(c);
                }
                if !bold_content.is_empty() {
                    spans.push(Span::styled(
                        bold_content,
                        Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                    ));
                }
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
    
    if !spans.is_empty() {
        result_lines.push(Line::from(spans));
    } else {
        result_lines.push(Line::raw(""));
    }
    
    result_lines
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