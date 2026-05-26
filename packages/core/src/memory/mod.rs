//! Memory system for MatrixCode.
//!
//! This module implements automatic memory accumulation for AI agents.
//! It captures user preferences, project decisions, key findings, and solutions
//! across sessions, providing persistent context.
//!
//! # Module Structure
//!
//! - **config**: Constants and configuration
//! - **entry**: Core types (MemoryCategory, MemoryEntry)
//! - **manager**: Memory manager (AutoMemory, SearchIndex, MemoryStatistics)
//! - **storage**: File storage with locking
//! - **retrieval**: TF-IDF search, keyword extraction
//! - **extractor**: AI and rule-based memory detection
//! - **learning**: Feedback learning, behavior inference
//! - **project**: Project structure analysis
//! - **keywords_config**: Customizable keywords configuration

mod config;
mod entry;
mod extractor;
mod keywords_config;
mod learning;
mod manager;
mod project;
mod retrieval;
mod storage;

// Re-export all public items
pub use config::*;
pub use entry::*;
pub use extractor::*;
pub use keywords_config::*;
pub use learning::*;
pub use manager::*;
pub use project::*;
pub use retrieval::*;
pub use storage::*;
