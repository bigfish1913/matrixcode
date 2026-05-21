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

mod compressor;
mod config;
mod types;

// Re-export all public items
pub use compressor::*;
pub use config::*;
pub use types::*;
