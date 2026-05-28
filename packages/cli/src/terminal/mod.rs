//! Terminal mode module for MatrixCode CLI
//!
//! This module provides a clean, modular architecture for the terminal mode:
//! - `setup` - Initialization (config, API, runtime, channels)
//! - `session` - Session management (load, resume, save, list)
//! - `watcher` - CodeGraph watcher management
//! - `mcp_handler` - MCP server lifecycle
//! - `memory_handler` - Memory retrieval, feedback, extraction
//! - `commands` - Backend command handlers (strategy pattern)
//! - `agent` - Agent task execution

pub mod setup;
pub mod session;
pub mod watcher;
pub mod mcp_handler;
pub mod memory_handler;
pub mod commands;
pub mod agent;

pub use setup::run_terminal_mode;