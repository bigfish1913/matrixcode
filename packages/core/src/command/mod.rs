//! 后端命令系统
//!
//! 使用策略模式实现后端命令，命令定义在 core 中供 TUI 和 CLI 共享。

mod backend_context;
mod command_trait;
pub mod handlers;
mod registry;

pub use backend_context::BackendContext;
pub use command_trait::Command;
pub use registry::{CommandRegistry, get_registry};
