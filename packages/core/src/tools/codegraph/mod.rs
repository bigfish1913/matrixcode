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
pub use self::types::{Node, Edge, IndexStatus, PendingChanges, FileInfo, CodeGraphEnv};
pub use self::manager::CodeGraphManager;
pub use self::watcher::CodeGraphWatcher;
pub use self::ignore::WATCH_EXTENSIONS;
pub use self::tools::{
    codegraph_tools, codegraph_tools_with_auto_detect, codegraph_tools_if_installed,
    should_inject_codegraph_tools,
    CodeGraphSearchTool, CodeGraphCallersTool, CodeGraphCalleesTool, 
    CodeGraphStatusTool, CodeGraphSyncTool,
};
pub use self::install::{get_codegraph_path, ensure_codegraph, is_codegraph_installed};
pub use self::project::find_project_root;

// Internal modules
mod types;
mod manager;
mod watcher;
mod tools;
mod install;
mod project;
mod git;
mod ignore;