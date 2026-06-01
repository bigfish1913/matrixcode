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
//! - **semantic**: Semantic compression using AI summarization
//! - **priority**: Dynamic priority scoring for messages
//! - **cache**: Compression cache for performance optimization
//! - **focus_point**: AI-driven focus point management
//! - **focus_extractor**: AI-based focus extraction and classification
//! - **focus_manager**: Focus tracking and relevance calculation
//! - **coherence**: Semantic coherence detection
//! - **progressive**: Progressive compression strategy
//! - **complexity**: Complexity analysis for adaptive compression
//! - **hierarchical**: Hierarchical summarization strategies

mod ai_focus_tracker;
mod compressor;
mod config;
mod dependency;
mod phase_detector;
mod pipeline;
mod scorer;
mod summarizer;
mod tool_compressor;
mod types;
mod semantic;
mod priority;
mod cache;
mod integration;
mod focus;
mod focus_config;
mod focus_point;
mod focus_extractor;
mod focus_score_evaluator;
mod prompts_zh;
mod coherence;
mod progressive;
mod complexity;
mod hierarchical;
mod hardcode_config;
mod integrated_processor;

// Re-export all public items
pub use ai_focus_tracker::*;
pub use compressor::*;
pub use integrated_processor::*;
pub use config::*;
pub use dependency::*;
pub use phase_detector::*;
pub use pipeline::*;
pub use scorer::*;
pub use summarizer::*;
pub use tool_compressor::*;
pub use types::*;
pub use semantic::*;
pub use priority::*;
pub use cache::*;
pub use integration::*;
pub use focus::*;
pub use focus_config::*;
pub use focus_point::*;
pub use focus_extractor::*;
pub use focus_score_evaluator::*;
pub use coherence::*;
pub use progressive::*;
pub use complexity::*;
pub use hierarchical::*;
pub use hardcode_config::*;
