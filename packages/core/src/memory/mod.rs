//! Memory system for MatrixCode.
//!
//! This module implements automatic memory accumulation for AI agents.
//! It captures user preferences, project decisions, key findings, and solutions
//! across sessions, providing persistent context.
//!
//! # Module Structure
//!
//! - **config**: Constants and configuration
//! - **types**: Core types (MemoryCategory, MemoryEntry, AutoMemory)
//! - **storage**: File storage with locking
//! - **retrieval**: TF-IDF search, keyword extraction
//! - **extractor**: AI and rule-based memory detection
//! - **learning**: Feedback learning, behavior inference
//! - **project**: Project structure analysis
//! - **keywords_config**: Customizable keywords configuration

mod config;
mod extractor;
mod keywords_config;
mod learning;
mod project;
mod retrieval;
mod storage;
mod types;

// Re-export all public items
pub use config::*;
pub use extractor::*;
pub use keywords_config::*;
pub use learning::*;
pub use project::*;
pub use retrieval::*;
pub use storage::*;
pub use types::*;
