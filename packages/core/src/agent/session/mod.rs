//! Agent session management.
//!
//! This module handles session-related concerns:
//! - Event emission
//! - Cancellation tokens
//! - Ask response channel

pub mod manager;

// TODO: Add these in Phase 6
// pub mod memory;
// pub mod reminder;

pub use manager::SessionManager;