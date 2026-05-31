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
//! - **smart_retrieval**: Advanced retrieval with focus and time decay
//! - **adaptive**: User feedback learning and system adaptation
//! - **conversation_pattern**: Pattern types (ConversationPattern, PatternType, PatternSource)
//! - **pattern_registry**: Pattern registry for managing conversation patterns
//! - **unified_extraction**: Unified extraction result structure
//! - **unified_registry**: Unified registry for learning from extraction

mod adaptive;
mod config;
mod conversation_pattern;
mod entry;
mod extractor;
mod learning;
mod manager;
mod pattern_registry;
mod project;
mod retrieval;
mod smart_retrieval;
mod storage;
mod unified_extraction;
mod unified_registry;

// Re-export all public items
pub use adaptive::*;
pub use config::*;
pub use conversation_pattern::*;
pub use entry::*;
pub use extractor::*;
pub use learning::*;
pub use manager::*;
pub use pattern_registry::*;
pub use project::*;
pub use retrieval::*;
pub use smart_retrieval::*;
pub use storage::*;
pub use unified_extraction::*;
pub use unified_registry::*;
