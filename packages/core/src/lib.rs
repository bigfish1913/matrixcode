//! MatrixCode Core - Agent Logic, No UI
//!
//! This crate contains only Agent core logic, no UI handling.
//! All outputs are structured AgentEvent, UI layer renders them.

pub mod agent;
pub mod approval;
pub mod cancel;
pub mod compress;
pub mod config;
pub mod debug;
pub mod event;
pub mod memory;
pub mod models;
pub mod overview;
pub mod path_validator;
pub mod prompt;
pub mod providers;
pub mod session;
pub mod skills;
pub mod tools;
pub mod truncate;
pub mod workspace;

// Public exports
pub use agent::{Agent, AgentBuilder};
pub use approval::ApproveMode;
pub use config::Config;
pub use debug::{DebugLog, DebugStats, debug_log};
pub use event::{AgentEvent, EventCollector, EventData, EventType};
pub use providers::{
    ChatRequest, ChatResponse, ContentBlock, Message, MessageContent, Provider, ProviderType, Role,
    create_provider, create_provider_with_headers, create_minimal_provider, infer_provider_type,
};
pub use session::{Session, SessionManager};
pub use truncate::{find_boundary, truncate_bytes, truncate_chars, truncate_with_suffix};

/// Core version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
