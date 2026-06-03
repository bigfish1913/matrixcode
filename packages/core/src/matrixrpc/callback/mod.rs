//! Callback Module
//!
//! Handles callback requests from external extension services.
//! Enables external nodes to access MatrixCode's AI, tool, and context capabilities.

mod handler;
mod ai;
mod tool;
mod context;
mod security;

pub use handler::{CallbackHandler, CallbackError, CallbackResult, CallbackConfig, CallbackType};
pub use ai::{AiCallbackHandler, AiCallbackRequest, AiCallbackResult, AiCallbackError, AiModelConfig};
pub use tool::{ToolCallbackHandler, ToolCallbackRequest, ToolCallbackResult, ToolCallbackError, AllowedToolsConfig};
pub use context::{ContextCallbackHandler, ContextCallbackRequest, ContextCallbackResult, ContextCallbackError, ContextOperation};
pub use security::{SecurityValidator, SecurityError, SecurityConfig, TokenInfo, ValidationResult};