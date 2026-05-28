//! Backend command system
//!
//! This module implements a strategy pattern for backend commands.
//! Commands are defined in core and shared between TUI and CLI.

mod command_trait;
mod backend_context;
mod registry;
pub mod handlers;

pub use command_trait::Command;
pub use backend_context::BackendContext;
pub use registry::{CommandRegistry, get_registry};