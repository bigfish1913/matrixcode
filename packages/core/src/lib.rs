//! MatrixCode Core - Agent Logic, No UI
//!
//! This crate contains only Agent core logic, no UI handling.
//! All outputs are structured AgentEvent, UI layer renders them.

pub mod agent;
pub mod approval;
pub mod cancel;
pub mod command;
pub mod compress;
pub mod config;
pub mod constants;
pub mod debug;
pub mod event;
pub mod lsp;
pub mod mcp;
pub mod memory;
pub mod models;
pub mod overview;
pub mod path_validator;
pub mod prompt;
pub mod providers;
pub mod session;
pub mod skills;
pub mod tokenizer;
pub mod tools;
pub mod truncate;
pub mod workflow;
pub mod workspace;

// Public exports
pub use agent::{Agent, AgentBuilder};
pub use approval::ApproveMode;
pub use config::Config;
pub use debug::{DebugLog, DebugStats, debug_log, set_debug_event_sender};
pub use event::{AgentEvent, EventCollector, EventData, EventType, HistoryMessage, SessionListItem};
pub use lsp::{LspManager, LspServerInfo, LspServerStatus};
pub use providers::{
    ChatRequest, ChatResponse, ContentBlock, Message, MessageContent, Provider, ProviderType, Role,
    create_minimal_provider, create_provider, create_provider_with_headers, infer_provider_type,
};
pub use session::{
    MessageSummary, Session, SessionFileLock, SessionIndex, SessionManager, SessionMetadata,
};
pub use truncate::{find_boundary, truncate_bytes, truncate_chars, truncate_with_suffix};

// Workflow exports
pub use workflow::{
    AiExecutor, ConditionExecutor, EdgeDef, ExecutorFactory, FailureStrategy, NodeDef,
    NodeExecutor, NodeType, Rule, RuleEngine, TemplateRenderer, ToolExecutor, ValidateExecutor,
    ValidationResult, WorkflowContext, WorkflowDef, WorkflowEngine, WorkflowPersistence,
    WorkflowStatus, evaluate_expression, parse_workflow, parse_workflow_from_file, render_template,
    to_yaml,
};

/// Core version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
