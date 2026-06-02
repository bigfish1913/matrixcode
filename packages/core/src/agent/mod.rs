//! Agent module - event-driven AI agent implementation.
//!
//! Provides Agent struct with streaming responses, tool execution, and event output.
//!
//! ## Module Structure (Refactoring in Progress)
//!
//! The agent module is being refactored to improve separation of concerns:
//!
//! **Current Structure** (will be phased out):
//! - `builder.rs`: Agent builder pattern
//! - `helpers.rs`: Helper functions and utilities
//! - `run.rs`: Main execution loop (716 lines, to be simplified)
//! - `streaming.rs`: Streaming response handling
//! - `tools.rs`: Tool execution logic
//! - `types.rs`: Agent struct definition (26 fields, to be split)
//!
//! **New Structure** (being introduced in phases):
//! - `core/`: Core configuration and state management
//!   - `config.rs`: Configuration constants (max iterations, retries, etc.)
//!   - `state.rs`: State management (messages, tokens) - Phase 3
//!   - `executor.rs`: Execution engine - Phase 7
//! - `context/`: Context management
//!   - `manager.rs`: Context manager - Phase 4
//! - `session/`: Session management
//!   - `manager.rs`: Session manager - Phase 6
//! - `tests/`: Comprehensive test coverage

// New modular structure (being introduced)
pub mod core;
pub mod context;
pub mod session;
// Note: agent/tools/ directory will be created in Phase 5
// Currently agent/tools.rs file exists and will be migrated
// pub mod tools; // TODO: Enable in Phase 5

// Existing structure (will be phased out in Phases 3-7)
mod builder;
mod helpers;
mod run;
mod streaming;
mod tools;
mod types;

// Re-export public items from types directly
pub use types::{Agent, AgentBuilder};
pub use helpers::ContextInfo;

// Re-export new core components for gradual adoption
pub use core::{AgentConfig, AgentState};
pub use context::AgentContext;
pub use session::SessionManager;
