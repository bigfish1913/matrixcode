//! Lifecycle Management Module
//!
//! Manages the lifecycle of extension services including connection,
//! reconnection, heartbeat monitoring, and graceful shutdown.

mod manager;

pub use manager::*;