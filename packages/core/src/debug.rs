//! Debug logging for MatrixCode operations
//!
//! Tracks: API calls, compression, memory saves, tool executions

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::truncate::truncate_with_suffix;
use crate::event::AgentEvent;
use tokio::sync::mpsc;

static API_CALL_COUNT: AtomicU64 = AtomicU64::new(0);
static COMPRESSION_COUNT: AtomicU64 = AtomicU64::new(0);
static MEMORY_SAVE_COUNT: AtomicU64 = AtomicU64::new(0);
static TOOL_CALL_COUNT: AtomicU64 = AtomicU64::new(0);

/// Debug logger that writes to file
pub struct DebugLog {
    file: Option<Mutex<File>>,
}

impl Default for DebugLog {
    fn default() -> Self {
        Self::new()
    }
}

impl DebugLog {
    /// Create a new debug logger
    /// Writes to ~/.matrix/debug.log if possible
    pub fn new() -> Self {
        let file = Self::open_log_file().ok().map(Mutex::new);
        Self { file }
    }

    fn open_log_file() -> Result<File, std::io::Error> {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "HOME not set"))?;
        let mut path = PathBuf::from(home);
        path.push(".matrix");
        std::fs::create_dir_all(&path)?;
        path.push("debug.log");
        OpenOptions::new().create(true).append(true).open(path)
    }

    fn timestamp() -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let secs = now % 60;
        let mins = (now / 60) % 60;
        let hours = (now / 3600) % 24;
        format!("{:02}:{:02}:{:02}", hours, mins, secs)
    }

    /// Log an API call
    pub fn api_call(&self, model: &str, input_tokens: u32, cached: bool) {
        let count = API_CALL_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
        let msg = format!(
            "API#{}: model={}, input_tokens={}, cached={}",
            count,
            model,
            input_tokens,
            cached
        );
        self.write_log("API", &msg);
    }

    /// Log compression trigger
    pub fn compression(&self, original_tokens: u32, compressed_tokens: u32, ratio: f32) {
        let count = COMPRESSION_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
        let saved = original_tokens - compressed_tokens;
        let msg = format!(
            "COMPRESSION#{}: original={}, compressed={}, saved={}, ratio={:.1}%",
            count,
            original_tokens,
            compressed_tokens,
            saved,
            ratio * 100.0
        );
        self.write_log("COMPRESS", &msg);
    }

    /// Log memory save
    pub fn memory_save(&self, entries: usize, summary_len: usize) {
        let count = MEMORY_SAVE_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
        let msg = format!(
            "MEMORY#{}: entries={}, summary_len={}chars",
            count,
            entries,
            summary_len
        );
        self.write_log("MEMORY", &msg);
    }

    /// Log keyword extraction
    pub fn keywords_extracted(&self, keywords: &[String], source: &str) {
        let msg = format!(
            "{} extracted from {}chars | keywords: {}",
            keywords.len(),
            source.len(),
            keywords.join(", ")
        );
        self.write_log("KEYWORDS", &msg);
    }

    /// Log tool execution
    pub fn tool_call(&self, tool: &str, input_preview: &str, result_preview: &str) {
        let count = TOOL_CALL_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
        let msg = format!(
            "TOOL#{}: {} | input: {} | result: {}",
            count,
            tool,
            truncate(input_preview, 50),
            truncate(result_preview, 50)
        );
        self.write_log("TOOL", &msg);
    }

    /// Log session save
    pub fn session_save(&self, message_count: usize, total_tokens: u64) {
        let msg = format!(
            "SESSION: messages={}, total_tokens={}",
            message_count,
            total_tokens
        );
        self.write_log("SESSION", &msg);
    }

    /// Log generic debug message
    pub fn log(&self, category: &str, message: &str) {
        self.write_log(category, message);
    }

    /// Log API request body (for debug)
    pub fn api_request(&self, url: &str, body: &str) {
        // Write full content to file
        let body_preview = if body.len() > 5000 {
            truncate_with_suffix(body, 5000)
        } else {
            body.to_string()
        };
        let msg = format!(
            "API_REQUEST: url={}\n---REQUEST_BODY---\n{}\n---END---",
            url,
            body_preview
        );
        self.write(&format!("[{}] {}", Self::timestamp(), msg));

        // Send brief summary to debug panel
        let panel_msg = format!("url={} | body_len={}chars", url, body.len());
        self.send_debug_event("API_REQUEST", &panel_msg);
    }

    /// Log API response (for debug)
    pub fn api_response(&self, status: u16, body: &str) {
        // Write full content to file
        let body_preview = if body.len() > 10000 {
            truncate_with_suffix(body, 10000)
        } else {
            body.to_string()
        };
        let msg = format!(
            "API_RESPONSE: status={}\n---RESPONSE_BODY---\n{}\n---END---",
            status,
            body_preview
        );
        self.write(&format!("[{}] {}", Self::timestamp(), msg));

        // Send brief summary to debug panel
        let panel_msg = format!("status={} | body_len={}chars", status, body.len());
        self.send_debug_event("API_RESPONSE", &panel_msg);
    }

    /// Log streaming chunk (for debug, limited)
    pub fn stream_chunk(&self, chunk_type: &str, content: &str) {
        // Only log small chunks to avoid flooding
        let preview = if content.len() > 200 {
            truncate_with_suffix(content, 200)
        } else {
            content.to_string()
        };
        let msg = format!(
            "[{}] STREAM_CHUNK: type={} | {}",
            Self::timestamp(),
            chunk_type,
            preview
        );
        self.write(&msg);
    }

    /// Log AI memory extraction (keyword extraction with fast model)
    pub fn memory_ai_keywords(&self, model: &str, keywords_count: usize, source_len: usize, used_ai: bool) {
        let method = if used_ai { "AI" } else { "rule" };
        let msg = format!(
            "MEMORY_AI_KEYWORDS: model={}, method={}, keywords={}, source_len={}chars",
            model,
            method,
            keywords_count,
            source_len
        );
        self.write_log("MEMORY", &msg);
    }

    /// Log AI memory detection (memory extraction from response)
    pub fn memory_ai_detection(&self, model: &str, entries_count: usize, text_len: usize, used_ai: bool) {
        let method = if used_ai { "AI" } else { "rule" };
        let msg = format!(
            "MEMORY_AI_DETECT: model={}, method={}, entries={}, text_len={}chars",
            model,
            method,
            entries_count,
            text_len
        );
        self.write_log("MEMORY", &msg);
    }

    fn write(&self, msg: &str) {
        // Write to file only, don't print to console (would mess up TUI)
        if let Some(ref file) = self.file
            && let Ok(mut f) = file.lock()
        {
            let _ = f.write_all(msg.as_bytes());
            let _ = f.write_all(b"\n");
        }
    }

    /// Write log with category and send event to TUI debug panel
    fn write_log(&self, category: &str, message: &str) {
        let msg = format!("[{}] {}: {}", Self::timestamp(), category, message);
        self.write(&msg);

        // Send event to TUI debug panel
        self.send_debug_event(category, message);
    }

    /// Send debug event to TUI panel only (no file write)
    fn send_debug_event(&self, category: &str, message: &str) {
        if let Ok(guard) = DEBUG_EVENT_SENDER.lock()
            && let Some(ref sender) = *guard
        {
            let _ = sender.try_send(AgentEvent::debug_log(category, message));
        }
    }

    /// Get statistics
    pub fn stats(&self) -> DebugStats {
        DebugStats {
            api_calls: API_CALL_COUNT.load(Ordering::Relaxed),
            compressions: COMPRESSION_COUNT.load(Ordering::Relaxed),
            memory_saves: MEMORY_SAVE_COUNT.load(Ordering::Relaxed),
            tool_calls: TOOL_CALL_COUNT.load(Ordering::Relaxed),
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    truncate_with_suffix(s, max)
}

/// Debug statistics
#[derive(Debug, Clone)]
pub struct DebugStats {
    pub api_calls: u64,
    pub compressions: u64,
    pub memory_saves: u64,
    pub tool_calls: u64,
}

impl DebugStats {
    pub fn format(&self) -> String {
        format!(
            "API: {} │ Compress: {} │ Memory: {} │ Tools: {}",
            self.api_calls, self.compressions, self.memory_saves, self.tool_calls
        )
    }
}

/// Global debug logger (lazy initialized)
static DEBUG_LOG: once_cell::sync::Lazy<DebugLog> = once_cell::sync::Lazy::new(|| {
    // Try to load .env file first (from current directory)
    let _ = dotenvy::dotenv();

    // Also try project-level .matrix/.env
    if let Ok(cwd) = std::env::current_dir() {
        let matrix_env = cwd.join(".matrix").join(".env");
        if matrix_env.exists() {
            let _ = dotenvy::from_path(&matrix_env);
        }
    }

    DebugLog::new()
});

/// Global event sender for TUI debug panel
static DEBUG_EVENT_SENDER: once_cell::sync::Lazy<Mutex<Option<mpsc::Sender<AgentEvent>>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(None));

/// Get the global debug logger
pub fn debug_log() -> &'static DebugLog {
    &DEBUG_LOG
}

/// Set event sender for TUI debug panel
/// This allows debug logs to be displayed in the TUI debug panel
pub fn set_debug_event_sender(sender: mpsc::Sender<AgentEvent>) {
    if let Ok(mut guard) = DEBUG_EVENT_SENDER.lock() {
        *guard = Some(sender);
    }
}

/// Convenience macros
#[macro_export]
macro_rules! debug_api {
    ($model:expr, $tokens:expr, $cached:expr) => {
        $crate::debug::debug_log().api_call($model, $tokens, $cached)
    };
}

#[macro_export]
macro_rules! debug_compress {
    ($orig:expr, $comp:expr, $ratio:expr) => {
        $crate::debug::debug_log().compression($orig, $comp, $ratio)
    };
}

#[macro_export]
macro_rules! debug_memory {
    ($entries:expr, $len:expr) => {
        $crate::debug::debug_log().memory_save($entries, $len)
    };
}

#[macro_export]
macro_rules! debug_keywords {
    ($keywords:expr, $source:expr) => {
        $crate::debug::debug_log().keywords_extracted($keywords, $source)
    };
}

#[macro_export]
macro_rules! debug_tool {
    ($tool:expr, $input:expr, $result:expr) => {
        $crate::debug::debug_log().tool_call($tool, $input, $result)
    };
}

#[macro_export]
macro_rules! debug_session {
    ($msgs:expr, $tokens:expr) => {
        $crate::debug::debug_log().session_save($msgs, $tokens)
    };
}

#[macro_export]
macro_rules! debug_log_msg {
    ($cat:expr, $msg:expr) => {
        $crate::debug::debug_log().log($cat, $msg)
    };
}

#[macro_export]
macro_rules! debug_api_request {
    ($url:expr, $body:expr) => {
        $crate::debug::debug_log().api_request($url, $body)
    };
}

#[macro_export]
macro_rules! debug_api_response {
    ($status:expr, $body:expr) => {
        $crate::debug::debug_log().api_response($status, $body)
    };
}

#[macro_export]
macro_rules! debug_stream_chunk {
    ($type:expr, $content:expr) => {
        $crate::debug::debug_log().stream_chunk($type, $content)
    };
}

#[macro_export]
macro_rules! debug_memory_ai_keywords {
    ($model:expr, $count:expr, $len:expr, $ai:expr) => {
        $crate::debug::debug_log().memory_ai_keywords($model, $count, $len, $ai)
    };
}

#[macro_export]
macro_rules! debug_memory_ai_detect {
    ($model:expr, $count:expr, $len:expr, $ai:expr) => {
        $crate::debug::debug_log().memory_ai_detection($model, $count, $len, $ai)
    };
}
