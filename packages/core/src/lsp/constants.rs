//! LSP timeout configuration constants
//!
//! Separated timeout values for different LSP lifecycle phases:
//! - Process startup (fast, system-level operation)
//! - Server initialization (slow, needs to build indexes)
//! - Individual requests (medium, per-call timeout)

use std::time::Duration;

/// Timeout for starting the LSP server process
///
/// This is a system-level operation that should complete quickly.
/// If the process fails to start within this time, it indicates
/// a fundamental problem (missing binary, permission denied, etc.)
pub const PROCESS_STARTUP_TIMEOUT: Duration = Duration::from_secs(5);

/// Timeout for initializing the LSP server
///
/// This includes building workspace indexes, loading dependencies,
/// and preparing the server for use. Large projects (especially Rust)
/// may need 60-120 seconds to fully initialize.
///
/// We set this to 120s to accommodate very large Rust projects.
/// If timeout occurs, the process continues running in background
/// and status will be updated when ready.
pub const SERVER_INIT_TIMEOUT: Duration = Duration::from_secs(120);

/// Timeout for individual LSP requests
///
/// Each request (hover, definition, references, etc.) has its own
/// timeout. This prevents slow requests from blocking the entire system.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Legacy timeout constant (deprecated)
///
/// This was the original single timeout for entire LSP startup.
/// Now replaced by PROCESS_STARTUP_TIMEOUT + SERVER_INIT_TIMEOUT.
/// Kept for backward compatibility during migration.
#[deprecated(since = "0.1.0", note = "Use PROCESS_STARTUP_TIMEOUT and SERVER_INIT_TIMEOUT instead")]
pub const LSP_STARTUP_TIMEOUT_SECS: u64 = 5;

/// Legacy wait timeout (backward compatibility)
///
/// Maps to SERVER_INIT_TIMEOUT for backward compatibility.
/// Use SERVER_INIT_TIMEOUT directly in new code.
pub const LSP_WAIT_TIMEOUT_SECS: u64 = 120;