//! 后端命令系统
//!
//! 使用策略模式实现后端命令，命令定义在 core 中供 TUI 和 CLI 共享。

mod command_trait;
mod backend_context;
mod registry;
pub mod handlers;

pub use command_trait::Command;
pub use backend_context::BackendContext;
pub use registry::{CommandRegistry, get_registry};