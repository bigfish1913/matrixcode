use serde_json::Value;

/// Truncate string at char boundary
pub fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.into() }
    else { s.chars().take(n.saturating_sub(3)).collect::<String>() + "..." }
}

/// Format token count for display
pub fn fmt_tokens(n: u64) -> String {
    if n < 1_000 { n.to_string() }
    else if n < 1_000_000 { format!("{:.1}K", n as f64 / 1_000.0) }
    else { format!("{:.1}M", n as f64 / 1_000_000.0) }
}

/// Render a progress bar
pub fn progress_bar(pct: f64, width: usize) -> String {
    let filled = ((pct / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    let mut s = String::with_capacity(width);
    for i in 0..width {
        s.push(if i < filled { '█' } else { '░' });
    }
    s
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
