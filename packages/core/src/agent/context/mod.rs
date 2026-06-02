//! Agent context management.
//!
//! This module handles context-related concerns:
//! - Project overview, memory summary, skills
//! - System prompt building
//! - CodeGraph tool management

pub mod manager;

// TODO: Add these in Phase 4
// pub mod compression;
// pub mod pending;

pub use manager::AgentContext;