//! Agent module - event-driven AI agent implementation.
//!
//! Provides Agent struct with streaming responses, tool execution, and event output.

mod builder;
mod helpers;
mod run;
mod streaming;
mod tools;
mod types;

// Re-export public items from types directly
pub use types::{Agent, AgentBuilder};
pub use helpers::ContextInfo;
