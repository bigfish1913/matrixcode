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
mod types;
mod phase_detector;
mod dependency;
mod scorer;
mod summarizer;
mod tool_compressor;
mod pipeline;

// Re-export all public items
pub use compressor::*;
pub use config::*;
pub use types::*;
pub use phase_detector::*;
pub use dependency::*;
pub use scorer::*;
pub use summarizer::*;
pub use tool_compressor::*;
pub use pipeline::*;

// Re-export tool result truncation constants and functions
pub use config::{MAX_TOOL_RESULT_TOKENS, TOOL_RESULT_TRUNCATED_SUFFIX, TOOL_RESULT_REPLACEMENT_MSG};
pub use compressor::{truncate_tool_results, replace_old_tool_results, compress_messages_with_truncation};
