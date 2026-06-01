//! 后端命令处理器
//!
//! 每个处理器实现 Command trait。

mod compact;
mod config;
mod context;
mod load;
mod memory;
mod mode;
mod new;
mod overview;
mod save;
mod sessions;
mod skills;
mod system;
mod tools;
mod workflow;

use super::registry::CommandRegistry;
use std::sync::Arc;

pub use compact::Compact;
pub use config::Config;
pub use context::Context;
pub use load::Load;
pub use memory::Memory;
pub use mode::Mode;
pub use new::New;
pub use overview::Overview;
pub use save::Save;
pub use sessions::Sessions;
pub use skills::Skills;
pub use system::System;
pub use tools::Tools;
pub use workflow::Workflow;

/// 注册所有命令到注册表
pub fn register_commands(registry: &mut CommandRegistry) {
    registry.register(Arc::new(Config));
    registry.register(Arc::new(Overview));
    registry.register(Arc::new(Skills));
    registry.register(Arc::new(Tools));
    registry.register(Arc::new(System));
    registry.register(Arc::new(Compact));
    registry.register(Arc::new(Workflow));
    registry.register(Arc::new(Memory));
    registry.register(Arc::new(Save));
    registry.register(Arc::new(Sessions));
    registry.register(Arc::new(Load));
    registry.register(Arc::new(New));
    registry.register(Arc::new(Mode));
    registry.register(Arc::new(Context));
}
