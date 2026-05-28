//! 后端命令处理器
//!
//! 每个处理器实现 Command trait。

mod config;
mod overview;
mod skills;
mod tools;
mod system;
mod compact;
mod workflow;
mod memory;
mod save;
mod sessions;
mod load;
mod new;
mod mode;

use std::sync::Arc;
use super::registry::CommandRegistry;

pub use config::Config;
pub use overview::Overview;
pub use skills::Skills;
pub use tools::Tools;
pub use system::System;
pub use compact::Compact;
pub use workflow::Workflow;
pub use memory::Memory;
pub use save::Save;
pub use sessions::Sessions;
pub use load::Load;
pub use new::New;
pub use mode::Mode;

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
}