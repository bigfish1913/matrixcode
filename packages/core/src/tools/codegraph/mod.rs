//! CodeGraph integration for code symbol indexing and search.
//!
//! Integrates CodeGraph (https://github.com/colbymchenry/codegraph) for:
//! - Symbol search across the codebase
//! - Call graph analysis (callers/callees)
//! - Impact analysis for code changes
//! - Task context building for AI agents
//!
//! Uses SQLite direct access for fast queries, and CLI for index building.
//!
//! # Auto-sync
//!
//! CodeGraphWatcher provides automatic file watching and index synchronization.
//! When source files change, it automatically runs `codegraph sync` to keep
//! the index up-to-date.

// Public API
pub use self::ignore::WATCH_EXTENSIONS;
pub use self::install::{ensure_codegraph, get_codegraph_path, is_codegraph_installed};
pub use self::manager::CodeGraphManager;
pub use self::project::find_project_root;
pub use self::tools::{
    CodeGraphCalleesTool, CodeGraphCallersTool, CodeGraphFilesTool, CodeGraphSearchTool,
    CodeGraphStatusTool, CodeGraphSyncTool, codegraph_tools, codegraph_tools_if_installed,
    codegraph_tools_with_auto_detect, should_inject_codegraph_tools,
};
pub use self::types::{CodeGraphEnv, Edge, FileInfo, IndexStatus, Node, PendingChanges};
pub use self::watcher::{CodeGraphWatcher, WatcherHandle};

// Internal modules
mod git;
mod ignore;
mod install;
mod manager;
mod project;
mod tools;
mod types;
mod watcher;
