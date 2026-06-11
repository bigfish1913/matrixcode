//! Debug logging for MatrixCode operations
//!
//! Tracks: API calls, compression, memory saves, tool executions
//!
//! Features:
//! - Debug mode toggle: only logs when debug mode is enabled
//! - Session-based files: each session gets its own log file
//! - Auto rotation: rotates when file exceeds size limit

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::constants::MATRIX_DIR;
use crate::event::AgentEvent;
use crate::truncate::truncate_with_suffix;
use tokio::sync::mpsc;

static API_CALL_COUNT: AtomicU64 = AtomicU64::new(0);
static COMPRESSION_COUNT: AtomicU64 = AtomicU64::new(0);
static MEMORY_SAVE_COUNT: AtomicU64 = AtomicU64::new(0);
static TOOL_CALL_COUNT: AtomicU64 = AtomicU64::new(0);

/// Maximum log file size before rotation (10 MB)
const MAX_LOG_SIZE: u64 = 10 * 1024 * 1024;

/// Debug logger that writes to file
pub struct DebugLog {
    file: Mutex<Option<File>>,
    current_file: Mutex<Option<PathBuf>>,
    enabled: AtomicBool,
    session_id: Mutex<Option<String>>,
}

impl Default for DebugLog {
    fn default() -> Self {
        Self::new()
    }
}

impl DebugLog {
    /// Create a new debug logger
    /// Initially disabled, call enable() to start logging
    pub fn new() -> Self {
        Self {
            file: Mutex::new(None),
            current_file: Mutex::new(None),
            enabled: AtomicBool::new(false),
            session_id: Mutex::new(None),
        }
    }

    /// Enable debug logging with a session ID
    /// Creates a session-specific log file
    pub fn enable(&self, session_id: Option<&str>) {
        self.enabled.store(true, Ordering::Relaxed);

        // Generate session ID if not provided
        let sid = session_id
            .map(|s| s.to_string())
            .unwrap_or_else(Self::generate_session_id);

        if let Ok(mut guard) = self.session_id.lock() {
            *guard = Some(sid.clone());
        }

        // Create new log file for this session
        if let Ok(mut file_guard) = self.file.lock()
            && let Ok(mut path_guard) = self.current_file.lock() {
                match Self::open_session_log_file(&sid, 0) {
                    Ok((file, path)) => {
                        *file_guard = Some(file);
                        *path_guard = Some(path);
                    }
                    Err(e) => {
                        eprintln!("Failed to create debug log file: {}", e);
                    }
                }
            }
    }

    /// Disable debug logging
    pub fn disable(&self) {
        self.enabled.store(false, Ordering::Relaxed);
        if let Ok(mut guard) = self.file.lock() {
            *guard = None;
        }
    }

    /// Check if debug logging is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Generate a timestamp-based session ID
    fn generate_session_id() -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        format!("session_{}", now)
    }

    /// Get log directory path
    fn get_log_dir() -> Result<PathBuf, std::io::Error> {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "HOME not set"))?;
        let mut path = PathBuf::from(home);
        path.push(MATRIX_DIR);
        path.push("logs");
        fs::create_dir_all(&path)?;
        Ok(path)
    }

    /// Open a session-specific log file
    fn open_session_log_file(
        session_id: &str,
        rotation: u32,
    ) -> Result<(File, PathBuf), std::io::Error> {
        let mut path = Self::get_log_dir()?;
        let filename = if rotation == 0 {
            format!("{}.log", session_id)
        } else {
            format!("{}.{}.log", session_id, rotation)
        };
        path.push(&filename);
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok((file, path))
    }

    /// Rotate log file if it exceeds size limit
    fn rotate_if_needed(&self) {
        if let Ok(mut file_guard) = self.file.lock()
            && let Ok(mut path_guard) = self.current_file.lock()
            && let Ok(session_guard) = self.session_id.lock()
            && let (Some(file), Some(_path), Some(session_id)) = (
                file_guard.as_mut(),
                path_guard.as_ref(),
                session_guard.as_ref(),
            ) {
                // Check file size
                if let Ok(metadata) = file.metadata()
                    && metadata.len() > MAX_LOG_SIZE {
                        // Find next available rotation number
                        let mut rotation = 1u32;
                        loop {
                            let mut new_path = Self::get_log_dir().unwrap_or_default();
                            let filename = format!("{}.{}.log", session_id, rotation);
                            new_path.push(&filename);
                            if !new_path.exists() {
                                break;
                            }
                            rotation += 1;
                            // Prevent infinite loop
                            if rotation > 10000 {
                                log::warn!("Log rotation limit reached (10000), using current rotation");
                                break;
                            }
                        }

                        // Open new rotated file
                        if let Ok((new_file, new_path)) =
                            Self::open_session_log_file(session_id, rotation)
                        {
                            *file_guard = Some(new_file);
                            *path_guard = Some(new_path);
                        }
                    }
            }
    }

    /// Clean up old log files (keep last N sessions)
    pub fn cleanup_old_logs(keep_count: usize) -> Result<(), std::io::Error> {
        let log_dir = Self::get_log_dir()?;
        let mut log_files: Vec<_> = fs::read_dir(&log_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "log")
                    .unwrap_or(false)
            })
            .collect();

        // Sort by modification time (oldest first)
        log_files.sort_by_key(|e| {
            e.metadata()
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH)
        });

        // Remove oldest files beyond keep_count
        let total_sessions = log_files.len();
        if total_sessions > keep_count {
            for entry in log_files.iter().take(total_sessions - keep_count) {
                let _ = fs::remove_file(entry.path());
            }
        }

        Ok(())
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
        if !self.is_enabled() {
            return;
        }
        let count = API_CALL_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
        let msg = format!(
            "API#{}: model={}, input_tokens={}, cached={}",
            count, model, input_tokens, cached
        );
        self.write_log("API", &msg);
    }

    /// Log compression trigger
    pub fn compression(&self, original_tokens: u32, compressed_tokens: u32, ratio: f32) {
        if !self.is_enabled() {
            return;
        }
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
        if !self.is_enabled() {
            return;
        }
        let count = MEMORY_SAVE_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
        let msg = format!(
            "MEMORY#{}: entries={}, summary_len={}chars",
            count, entries, summary_len
        );
        self.write_log("MEMORY", &msg);
    }

    /// Log keyword extraction
    pub fn keywords_extracted(&self, keywords: &[String], source: &str) {
        if !self.is_enabled() {
            return;
        }
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
        if !self.is_enabled() {
            return;
        }
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
        if !self.is_enabled() {
            return;
        }
        let msg = format!(
            "SESSION: messages={}, total_tokens={}",
            message_count, total_tokens
        );
        self.write_log("SESSION", &msg);
    }

    /// Log generic debug message
    pub fn log(&self, category: &str, message: &str) {
        if !self.is_enabled() {
            return;
        }
        self.write_log(category, message);
    }

    /// Log API request body (for debug)
    pub fn api_request(&self, url: &str, body: &str) {
        if !self.is_enabled() {
            return;
        }
        // Write full content to file
        let body_preview = if body.len() > 5000 {
            truncate_with_suffix(body, 5000)
        } else {
            body.to_string()
        };
        let msg = format!(
            "API_REQUEST: url={}\n---REQUEST_BODY---\n{}\n---END---",
            url, body_preview
        );
        self.write(&format!("[{}] {}", Self::timestamp(), msg));

        // Send brief summary to debug panel
        let panel_msg = format!("url={} | body_len={}chars", url, body.len());
        self.send_debug_event("API_REQUEST", &panel_msg);
    }

    /// Log API response (for debug)
    pub fn api_response(&self, status: u16, body: &str) {
        if !self.is_enabled() {
            return;
        }
        // Write full content to file
        let body_preview = if body.len() > 10000 {
            truncate_with_suffix(body, 10000)
        } else {
            body.to_string()
        };
        let msg = format!(
            "API_RESPONSE: status={}\n---RESPONSE_BODY---\n{}\n---END---",
            status, body_preview
        );
        self.write(&format!("[{}] {}", Self::timestamp(), msg));

        // Send brief summary to debug panel
        let panel_msg = format!("status={} | body_len={}chars", status, body.len());
        self.send_debug_event("API_RESPONSE", &panel_msg);
    }

    /// Log streaming chunk (for debug, limited)
    pub fn stream_chunk(&self, chunk_type: &str, content: &str) {
        if !self.is_enabled() {
            return;
        }
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
    pub fn memory_ai_keywords(
        &self,
        model: &str,
        keywords_count: usize,
        source_len: usize,
        used_ai: bool,
    ) {
        if !self.is_enabled() {
            return;
        }
        let method = if used_ai { "AI" } else { "rule" };
        let msg = format!(
            "MEMORY_AI_KEYWORDS: model={}, method={}, keywords={}, source_len={}chars",
            model, method, keywords_count, source_len
        );
        self.write_log("MEMORY", &msg);
    }

    /// Log AI memory detection (memory extraction from response)
    pub fn memory_ai_detection(
        &self,
        model: &str,
        entries_count: usize,
        text_len: usize,
        used_ai: bool,
    ) {
        if !self.is_enabled() {
            return;
        }
        let method = if used_ai { "AI" } else { "rule" };
        let msg = format!(
            "MEMORY_AI_DETECT: model={}, method={}, entries={}, text_len={}chars",
            model, method, entries_count, text_len
        );
        self.write_log("MEMORY", &msg);
    }

    fn write(&self, msg: &str) {
        // Check if debug mode is enabled
        if !self.is_enabled() {
            return;
        }

        // Write to file only, don't print to console (would mess up TUI)
        if let Ok(mut guard) = self.file.lock()
            && let Some(ref mut file) = *guard {
                let _ = file.write_all(msg.as_bytes());
                let _ = file.write_all(b"\n");
                let _ = file.flush();
            }

        // Check for rotation after write
        self.rotate_if_needed();
    }

    /// Write log with category and send event to TUI debug panel
    fn write_log(&self, category: &str, message: &str) {
        if !self.is_enabled() {
            return;
        }

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
        let matrix_env = cwd.join(MATRIX_DIR).join(".env");
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

/// Enable debug logging with optional session ID
pub fn enable_debug_logging(session_id: Option<&str>) {
    DEBUG_LOG.enable(session_id);
}

/// Disable debug logging
pub fn disable_debug_logging() {
    DEBUG_LOG.disable();
}

/// Check if debug logging is enabled
pub fn is_debug_enabled() -> bool {
    DEBUG_LOG.is_enabled()
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
