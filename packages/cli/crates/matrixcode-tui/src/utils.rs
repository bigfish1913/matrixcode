use serde_json::Value;

/// Truncate string at char boundary
pub fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.into() }
    else { s.chars().take(n.saturating_sub(3)).collect::<String>() + "..." }
}

/// Word wrap text to specified width
/// Returns vector of lines, each at most `width` characters
pub fn word_wrap(text: &str, width: usize) -> Vec<String> {
    if width <= 2 { return text.lines().map(|s| s.to_string()).collect(); }
    
    let mut result: Vec<String> = Vec::new();
    
    for line in text.lines() {
        if line.is_empty() {
            result.push(String::new());
            continue;
        }
        
        if visual_width(line) <= width {
            result.push(line.to_string());
            continue;
        }
        
        // Wrap long line
        let mut current = String::new();
        let mut visual_width = 0;
        
        for ch in line.chars() {
            // Estimate visual width: Chinese = 2, ASCII = 1
            let ch_width = if ch > '\u{7F}' { 2 } else { 1 };
            
            if visual_width + ch_width > width && !current.is_empty() {
                result.push(current.clone());
                current.clear();
                visual_width = 0;
            }
            
            current.push(ch);
            visual_width += ch_width;
        }
        
        if !current.is_empty() {
            result.push(current);
        }
    }
    
    result
}

/// Format token count for display
pub fn fmt_tokens(n: u64) -> String {
    if n < 1_000 { n.to_string() }
    else if n < 1_000_000 { format!("{:.1}K", n as f64 / 1_000.0) }
    else { format!("{:.1}M", n as f64 / 1_000_000.0) }
}

/// Render a progress bar
#[allow(dead_code)]
pub fn progress_bar(pct: f64, width: usize) -> String {
    let filled = ((pct / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    let mut s = String::with_capacity(width);
    for i in 0..width {
        s.push(if i < filled { '█' } else { '░' });
    }
    s
}

/// Calculate visual width of a string (Chinese chars = 2, ASCII = 1)
pub fn visual_width(s: &str) -> usize {
    s.chars().map(|ch| if ch > '\u{7F}' { 2 } else { 1 }).sum()
}

/// Truncate string from the start to fit within visual width
pub fn truncate_visual(s: &str, max_width: usize) -> String {
    let mut result = String::new();
    let mut width = 0;
    for ch in s.chars() {
        let ch_w = if ch > '\u{7F}' { 2 } else { 1 };
        if width + ch_w > max_width {
            break;
        }
        result.push(ch);
        width += ch_w;
    }
    result
}

/// Truncate string from the end (keep last N visual columns)
pub fn truncate_visual_end(s: &str, max_width: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut result = String::new();
    let mut width = 0;
    for &ch in chars.iter().rev() {
        let ch_w = if ch > '\u{7F}' { 2 } else { 1 };
        if width + ch_w > max_width {
            break;
        }
        result.insert(0, ch);
        width += ch_w;
    }
    result
}

/// Extract substring from a line using visual column positions
/// Handles multi-byte characters (e.g., Chinese chars take 2 columns)
pub fn extract_by_visual_col(line: &str, start_col: usize, end_col: usize) -> String {
    let mut result = String::new();
    let mut visual_pos = 0;
    
    for ch in line.chars() {
        let ch_width = if ch > '\u{7F}' { 2 } else { 1 };
        
        if visual_pos >= end_col {
            break;
        }
        if visual_pos + ch_width > start_col {
            result.push(ch);
        }
        visual_pos += ch_width;
    }
    
    result
}

/// Extract tool detail info from input parameters
pub fn extract_tool_detail(tool_name: &str, input: Option<&Value>) -> String {
    let Some(input) = input else { return String::new() };
    match tool_name.to_lowercase().as_str() {
        "read" => input.get("path").and_then(|v| v.as_str())
            .map(|s| truncate(s, 40)).unwrap_or_default(),
        "write" => input.get("path").and_then(|v| v.as_str())
            .map(|s| truncate(s, 40)).unwrap_or_default(),
        "edit" | "multi_edit" => input.get("path").and_then(|v| v.as_str())
            .map(|s| truncate(s, 40)).unwrap_or_default(),
        "search" => input.get("pattern").and_then(|v| v.as_str())
            .map(|s| truncate(s, 30)).unwrap_or_default(),
        "glob" => input.get("pattern").and_then(|v| v.as_str())
            .map(|s| truncate(s, 30)).unwrap_or_default(),
        "ls" => input.get("path").and_then(|v| v.as_str())
            .map(|s| truncate(s, 40)).unwrap_or_default(),
        "bash" => input.get("command").and_then(|v| v.as_str())
            .map(|s| truncate(s, 40)).unwrap_or_default(),
        "websearch" => input.get("query").and_then(|v| v.as_str())
            .map(|s| truncate(s, 30)).unwrap_or_default(),
        "webfetch" => input.get("url").and_then(|v| v.as_str())
            .map(|s| truncate(s, 40)).unwrap_or_default(),
        _ => String::new(),
    }
}
