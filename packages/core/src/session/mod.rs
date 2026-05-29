//! Session management module
//!
//! Provides session persistence, storage, and file locking.

mod manager;
mod metadata;
#[allow(clippy::module_inception)]
mod session;

pub use manager::SessionManager;
pub use metadata::{MessageSummary, SessionIndex, SessionMetadata};
pub use session::{Session, SessionFileLock};
