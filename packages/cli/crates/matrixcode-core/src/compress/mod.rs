//! Context compression for conversation history.
//!
//! This module implements intelligent compression of conversation history
//! to reduce token usage while preserving important information.
//!
//! # Module Structure
//!
//! - **config**: Compression configuration and bias settings
//! - **types**: Compression strategy, result, and segment types
//! - **compressor**: AI compressor and compression functions

mod config;
mod types;
mod compressor;

// Re-export all public items
pub use config::*;
pub use types::*;
pub use compressor::*;