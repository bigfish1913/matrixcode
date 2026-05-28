//! Command registry for backend commands
//!
//! Manages command registration and dispatch.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use super::command_trait::Command;
use super::backend_context::BackendContext;

/// Registry for all backend commands
pub struct CommandRegistry {
    commands: HashMap<&'static str, Arc<dyn Command>>,
}

impl CommandRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }

    /// Register a command
    pub fn register(&mut self, command: Arc<dyn Command>) {
        let name = command.name();
        self.commands.insert(name, command);
    }

    /// Dispatch a message to the appropriate command handler (async)
    ///
    /// Returns:
    /// - `Some(true)` if command found and should forward to agent
    /// - `Some(false)` if command found and handled
    /// - `None` if no command matched
    pub async fn dispatch(&self, msg: &str, ctx: &mut BackendContext<'_>) -> Option<bool> {
        // Try to match by command name
        for command in self.commands.values() {
            if command.matches(msg) {
                return Some(command.execute(ctx).await);
            }
        }

        None
    }

    /// Get a command by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn Command>> {
        self.commands.get(name).cloned()
    }

    /// List all registered commands
    pub fn list_commands(&self) -> Vec<&'static str> {
        let mut names: Vec<_> = self.commands.keys().copied().collect();
        names.sort();
        names
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Global command registry (initialized once)
static REGISTRY: OnceLock<CommandRegistry> = OnceLock::new();

/// Create the default command registry with all built-in commands
fn create_registry() -> CommandRegistry {
    let mut registry = CommandRegistry::new();
    
    // Register commands from handlers module
    super::handlers::register_commands(&mut registry);
    
    registry
}

/// Get the global command registry
pub fn get_registry() -> &'static CommandRegistry {
    REGISTRY.get_or_init(create_registry)
}