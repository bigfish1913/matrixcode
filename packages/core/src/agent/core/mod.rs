//! Agent core components.
//!
//! This module contains the core building blocks of the Agent:
//! - [`config`]: Configuration constants (max iterations, retries, etc.)
//! - [`state`]: State management (messages, token counts, pending inputs)
//! - [`executor`]: Execution engine (main loop logic) - TODO

pub mod config;
pub mod state;

// TODO: Add this in Phase 7
// pub mod executor;

pub use config::AgentConfig;
pub use config::MAX_ITERATIONS;
pub use state::AgentState;