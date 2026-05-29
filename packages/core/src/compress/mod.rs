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
//! - **phase_detector**: Conversation phase detection
//! - **dependency**: Message dependency tracking

mod compressor;
mod config;
mod dependency;
mod phase_detector;
mod pipeline;
mod scorer;
mod summarizer;
mod tool_compressor;
mod types;

// Re-export all public items
pub use compressor::*;
pub use config::*;
pub use dependency::*;
pub use phase_detector::*;
pub use pipeline::*;
pub use scorer::*;
pub use summarizer::*;
pub use tool_compressor::*;
pub use types::*;
