//! MatrixCode Core - Agent Logic, No UI
//!
//! This crate contains only Agent core logic, no UI handling.
//! All outputs are structured AgentEvent, UI layer renders them.

pub mod event;
pub mod agent;
pub mod config;
pub mod session;
pub mod memory;
pub mod compress;
pub mod models;
pub mod approval;
pub mod cancel;
pub mod workspace;
pub mod overview;
pub mod prompt;
pub mod skills;
pub mod protocol;
pub mod providers;
pub mod tools;

// Public exports
pub use event::{AgentEvent, EventCollector, EventData, EventType};
pub use agent::{Agent, AgentBuilder};
pub use config::Config;
pub use session::{Session, SessionManager};
pub use providers::{Provider, Message, ContentBlock, ChatRequest, ChatResponse};
pub use providers::anthropic::AnthropicProvider;
pub use cancel::CancellationToken;

/// Core version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");