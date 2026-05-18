//! Debug logging for MatrixCode operations
//! 
//! Tracks: API calls, compression, memory saves, tool executions

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

static API_CALL_COUNT: AtomicU64 = AtomicU64::new(0);
static COMPRESSION_COUNT: AtomicU64 = AtomicU64::new(0);
static MEMORY_SAVE_COUNT: AtomicU64 = AtomicU64::new(0);
static TOOL_CALL_COUNT: AtomicU64 = AtomicU64::new(0);

/// Debug logger that writes to file and optionally prints to console
pub struct DebugLog {
    file: Option<Mutex<File>>,
    verbose: bool,
}

impl DebugLog {
    /// Create a new debug logger
    /// Writes to ~/.matrix/debug.log if possible
    pub fn new(verbose: bool) -> Self {
        let file = Self::open_log_file().ok().map(Mutex::new);
        Self { file, verbose }
    }

    fn open_log_file() -> Result<File, std::io::Error> {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "HOME not set"))?;
        let mut path = PathBuf::from(home);
        path.push(".matrix");
        std::fs::create_dir_all(&path)?;
        path.push("debug.log");
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
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
            "[{}] API#{}: model={}, input_tokens={}, cached={}",
            Self::timestamp(), count, model, input_tokens, cached
        );
        self.write(&msg);
    }

    /// Log compression trigger
    pub fn compression(&self, original_tokens: u32, compressed_tokens: u32, ratio: f32) {
        let count = COMPRESSION_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
        let saved = original_tokens - compressed_tokens;
        let msg = format!(
            "[{}] COMPRESSION#{}: original={}, compressed={}, saved={}, ratio={:.1}%",
            Self::timestamp(), count, original_tokens, compressed_tokens, saved, ratio * 100.0
        );
        self.write(&msg);
    }

    /// Log memory save
    pub fn memory_save(&self, entries: usize, summary_len: usize) {
        let count = MEMORY_SAVE_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
        let msg = format!(
            "[{}] MEMORY#{}: entries={}, summary_len={}chars",
            Self::timestamp(), count, entries, summary_len
        );
        self.write(&msg);
    }

    /// Log tool execution
    pub fn tool_call(&self, tool: &str, input_preview: &str, result_preview: &str) {
        let count = TOOL_CALL_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
        let msg = format!(
            "[{}] TOOL#{}: {} | input: {} | result: {}",
            Self::timestamp(), count, tool, 
            truncate(input_preview, 50),
            truncate(result_preview, 50)
        );
        self.write(&msg);
    }

    /// Log session save
    pub fn session_save(&self, message_count: usize, total_tokens: u64) {
        let msg = format!(
            "[{}] SESSION: messages={}, total_tokens={}",
            Self::timestamp(), message_count, total_tokens
        );
        self.write(&msg);
    }

    /// Log generic debug message
    pub fn log(&self, category: &str, message: &str) {
        let msg = format!("[{}] {}: {}", Self::timestamp(), category, message);
        self.write(&msg);
    }

    fn write(&self, msg: &str) {
        // Write to file
        if let Some(ref file) = self.file
            && let Ok(mut f) = file.lock() {
                let _ = f.write_all(msg.as_bytes());
                let _ = f.write_all(b"\n");
            }
        // Print to console if verbose
        if self.verbose {
            println!("{}", msg);
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
    if s.len() > max {
        format!("{}...", &s[..max.saturating_sub(3)])
    } else {
        s.to_string()
    }
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
    let verbose = std::env::var("MATRIXCODE_DEBUG")
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false);
    DebugLog::new(verbose)
});

/// Get the global debug logger
pub fn debug_log() -> &'static DebugLog {
    &DEBUG_LOG
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