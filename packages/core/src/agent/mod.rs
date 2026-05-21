//! Agent module - event-driven AI agent implementation.
//!
//! Provides Agent struct with streaming responses, tool execution, and event output.

mod types;
mod builder;
mod run;
mod streaming;
mod tools;
mod helpers;

// Re-export public items from types directly
pub use types::{Agent, AgentBuilder};