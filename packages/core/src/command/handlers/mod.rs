//! Command handlers for backend commands
//!
//! Each handler implements the Command trait.

mod config;
mod overview;
mod skills;
mod tools;
mod system;
mod compact;
mod workflow;
mod memory;

use std::sync::Arc;
use super::registry::CommandRegistry;

pub use config::ConfigCommand;
pub use overview::OverviewCommand;
pub use skills::SkillsCommand;
pub use tools::ToolsCommand;
pub use system::SystemCommand;
pub use compact::CompactCommand;
pub use workflow::WorkflowCommand;
pub use memory::MemoryCommand;

/// Register all commands to the registry
pub fn register_commands(registry: &mut CommandRegistry) {
    registry.register(Arc::new(ConfigCommand));
    registry.register(Arc::new(OverviewCommand));
    registry.register(Arc::new(SkillsCommand));
    registry.register(Arc::new(ToolsCommand));
    registry.register(Arc::new(SystemCommand));
    registry.register(Arc::new(CompactCommand));
    registry.register(Arc::new(WorkflowCommand));
    registry.register(Arc::new(MemoryCommand));
}