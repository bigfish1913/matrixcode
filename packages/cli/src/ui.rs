//! UI display utilities for MatrixCode CLI.
//!
//! This module contains all user-facing display functions, separating
//! presentation logic from core agent functionality.

use std::env;

// ============================================================================
// ANSI Color Codes
// ============================================================================

pub const CYAN: &str = "\x1b[36m";
pub const GREEN: &str = "\x1b[32m";
pub const YELLOW: &str = "\x1b[33m";
pub const BLUE: &str = "\x1b[34m";
pub const BOLD: &str = "\x1b[1m";
pub const DIM: &str = "\x1b[2;3m";
pub const RESET: &str = "\x1b[0m";

// ============================================================================
// Formatting Utilities
// ============================================================================

/// Format a number of tokens for display (e.g., "1.5K", "2.3M").
pub fn format_tokens(n: u64) -> String {
    if n < 1_000 {
        n.to_string()
    } else if n < 1_000_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{:.2}M", n as f64 / 1_000_000.0)
    }
}

/// Format a byte count for display (e.g., "1.5 KB", "2.3 MB").
pub fn format_bytes(n: usize) -> String {
    if n < 1024 {
        format!("{} B", n)
    } else if n < 1024 * 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else {
        format!("{:.2} MB", n as f64 / (1024.0 * 1024.0))
    }
}

/// Render a 0–100 percentage into a unicode progress bar.
pub fn bar(pct: f64, width: usize) -> String {
    let filled = ((pct / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    let mut s = String::with_capacity(width + 2);
    s.push('[');
    for i in 0..width {
        s.push(if i < filled { '█' } else { '░' });
    }
    s.push(']');
    s
}

/// Truncate a string to max characters, respecting char boundaries.
pub fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let mut end = max;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &s[..end])
    }
}

/// Truncate a string reference (returns slice).
pub fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

// ============================================================================
// Welcome and Help Display
// ============================================================================

/// Print welcome banner with version and model info.
pub fn print_welcome() {
    let version = env!("CARGO_PKG_VERSION");
    let model = env::var("MODEL_NAME")
        .or_else(|_| env::var("DEFAULT_MODEL"))
        .unwrap_or_else(|_| "default".to_string());
    
    println!();
    println!("{CYAN}{BOLD}  __  __       _             _____          _____ _____  {RESET}");
    println!("{CYAN} |  |/  | __ _| |_ ___ _ __  |  _  |_ _ ___|_   _|_   _| {RESET}");
    println!("{CYAN} | |/| |/ _` | __/ _ \\ '__| | |_| | '_/ _ \\ | |   | |   {RESET}");
    println!("{CYAN} | |  | | (_| | ||  __/ |    |  _  | ||  __/ | |   | |   {RESET}");
    println!("{CYAN} |_|  |_|__,_|\\__\\___|_|    |_| |_|_| \\___| |_|   |_|   {RESET}");
    println!();
    println!("{DIM}  ----------------------------------------------------------{RESET}");
    println!();
    println!("  {GREEN}Version{RESET}  {BOLD}v{}{RESET}", version);
    println!("  {GREEN}Model{RESET}    {BOLD}{}{RESET}", model);
    println!();
    println!("  {YELLOW}/help{RESET}  for commands    {YELLOW}/exit{RESET}  to quit");
    println!("  {BLUE}Ctrl+C{RESET} or {BLUE}ESC{RESET} to interrupt output during streaming");
    println!();
}

/// Print available commands and usage.
pub fn print_help() {
    println!("Available commands:");
    println!("  /help       - Show this help message");
    println!("  /status     - Show session status (messages, token usage)");
    println!("  /config     - Show configuration settings");
    println!("  /config init - Create default config file (~/.matrix/config.json)");
    println!("  /config set <k> <v> - Set config value (api_key, model, etc.)");
    println!("  /memory     - Show accumulated memories (new)");
    println!("  /memory add - Add manual memory entry (new)");
    println!("  /model      - Show current model information");
    println!("  /models     - Show full multi-model configuration");
    println!("  /skills     - Show loaded skills list");
    println!("  /history    - Show conversation history summary");
    println!("  /sessions   - List all saved sessions");
    println!("  /resume     - Show session picker to resume a session");
    println!("  /resume <id> - Resume a specific session by ID or name");
    println!("  /rename <name> - Give the current session a name");
    println!("  /init       - Generate/update project overview");
    println!("  /overview   - Show current project overview status");
    println!("  /plan       - Plan the current task (show last plan or new plan)");
    println!("  /plan <task> - Generate a plan for the specified task");
    println!("  /compress   - Manually compress context (balanced bias)");
    println!("  /compress <bias> - Compress with specific bias:");
    println!("      balanced     - Balanced preservation (default)");
    println!("      important    - Preserve tools, thinking, decisions");
    println!("      tools        - Focus on preserving tool operations");
    println!("      aggressive   - Remove as much as possible");
    println!("      preserve:tools,thinking keywords:决定,重要");
    println!("      preserve:tools,thinking,user keywords:决定,重要");
    println!("  /clear      - Clear context and start a new session");
    println!("  /mode       - Toggle approve mode (ask -> auto -> strict)");
    println!("  /exit       - Exit the REPL (also /quit or :q)");
    println!();
    println!("Keyboard shortcuts:");
    println!("  Shift+Tab   - Toggle approve mode (ask → auto → strict)");
    println!("  Alt+M       - Alternative: toggle approve mode");
    println!("  Ctrl+C      - Interrupt current output (at prompt: cancel input)");
    println!("  Ctrl+D      - Exit the REPL");
}

// ============================================================================
// Token Usage Display
// ============================================================================

/// Print a compact one-liner summarising token usage and context fullness.
pub fn print_usage_line(
    input_tokens: u32,
    output_tokens: u32,
    total_output: u64,
    cache_read: u32,
    cache_created: u32,
    context_size: Option<u32>,
) {
    if input_tokens == 0 && output_tokens == 0 {
        return;
    }

    let mut parts: Vec<String> = Vec::with_capacity(4);
    parts.push(format!(
        "in {} / out {} (session out: {})",
        format_tokens(input_tokens as u64),
        format_tokens(output_tokens as u64),
        format_tokens(total_output),
    ));
    
    if cache_read > 0 || cache_created > 0 {
        parts.push(format!(
            "cache r/w {}/{}",
            format_tokens(cache_read as u64),
            format_tokens(cache_created as u64),
        ));
    }
    
    if let Some(ctx) = context_size {
        let used = input_tokens;
        let pct = (used as f64 / ctx as f64 * 100.0).min(100.0);
        parts.push(format!(
            "ctx {} / {} ({:.1}%) {}",
            format_tokens(used as u64),
            format_tokens(ctx as u64),
            pct,
            bar(pct, 20),
        ));
    }

    println!("{DIM}{}{RESET}", parts.join(" | "));
}

// ============================================================================
// Tool Display
// ============================================================================

/// Print tool input in a readable format, expanding multi-line strings.
pub fn print_tool_input(name: &str, input: &serde_json::Value) {
    println!("[tool-input: {}]", name);
    
    if let serde_json::Value::Object(map) = input {
        for (key, value) in map {
            match value {
                serde_json::Value::String(s) => {
                    if s.contains('\n') {
                        println!("  {}:", key);
                        for line in s.lines() {
                            println!("    {}", line);
                        }
                    } else if s.len() > 100 {
                        let truncated = truncate(s, 97);
                        println!("  {}: \"{}...\" ({} chars)", key, truncated, s.len());
                    } else {
                        println!("  {}: \"{}\"", key, s);
                    }
                }
                serde_json::Value::Number(n) => {
                    println!("  {}: {}", key, n);
                }
                serde_json::Value::Bool(b) => {
                    println!("  {}: {}", key, b);
                }
                serde_json::Value::Null => {
                    println!("  {}: null", key);
                }
                serde_json::Value::Array(arr) => {
                    if arr.is_empty() {
                        println!("  {}: []", key);
                    } else if arr.len() <= 5 {
                        println!("  {}: {}", key, serde_json::to_string(arr).unwrap_or_default());
                    } else {
                        println!("  {}: [{} items]", key, arr.len());
                    }
                }
                serde_json::Value::Object(inner) => {
                    if inner.is_empty() {
                        println!("  {}: {{}}", key);
                    } else {
                        println!("  {}: {}", key, serde_json::to_string(inner).unwrap_or_default());
                    }
                }
            }
        }
    } else {
        println!("  {}", input);
    }
}

/// Print tool execution start indicator.
pub fn print_tool_start(name: &str) {
    println!("[tool: {}]", name);
}

/// Print tool result header.
pub fn print_tool_result(name: &str) {
    println!("[result: {}]", name);
}

/// Print tool error header.
pub fn print_tool_error(name: &str) {
    println!("[error: {}]", name);
}

// ============================================================================
// Message Display
// ============================================================================

/// Print a message role indicator for thinking blocks.
pub fn print_thinking_start() {
    print!("{DIM}[thinking] {RESET}");
}

/// Print thinking delta text.
pub fn print_thinking_delta(text: &str) {
    print!("{DIM}{text}{RESET}");
}

/// Print end of thinking block.
pub fn print_thinking_end() {
    print!("{RESET}\n\n");
}

// ============================================================================
// Status Messages
// ============================================================================

/// Print an info message.
pub fn info(msg: &str) {
    println!("[{}]", msg);
}

/// Print a warning message.
pub fn warn(msg: &str) {
    eprintln!("[warn] {}", msg);
}

/// Print an error message.
pub fn error(msg: &str) {
    eprintln!("[error] {}", msg);
}

/// Print a success message.
pub fn success(msg: &str) {
    println!("{GREEN}[{}] {RESET}", msg);
}