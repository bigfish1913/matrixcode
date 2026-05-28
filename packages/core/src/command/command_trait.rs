//! Command trait definition for backend commands
//!
//! Each command implements this trait and registers in CommandRegistry.

use std::future::Future;
use std::pin::Pin;

use super::backend_context::BackendContext;

/// Command trait for implementing backend commands
///
/// Each command should implement this trait and register itself
/// in the CommandRegistry.
pub trait Command: Send + Sync {
    /// Primary command name (without leading /)
    fn name(&self) -> &'static str;

    /// Aliases for the command (without leading /)
    fn aliases(&self) -> &[&'static str] {
        &[]
    }

    /// Help text for the command
    fn help(&self) -> Option<&'static str> {
        None
    }

    /// Check if this command matches the given message
    ///
    /// Default implementation matches:
    /// - Exact: `/name`
    /// - With args: `/name ...`
    /// - Aliases: `/alias` or `/alias ...`
    fn matches(&self, msg: &str) -> bool {
        let name = self.name();
        if msg == format!("/{}", name) || msg.starts_with(&format!("/{} ", name)) {
            return true;
        }
        
        for alias in self.aliases() {
            if msg == format!("/{}", alias) || msg.starts_with(&format!("/{} ", alias)) {
                return true;
            }
        }
        
        false
    }

    /// Execute the command asynchronously
    ///
    /// Returns:
    /// - `true` if the message should be forwarded to agent
    /// - `false` if the command has been fully handled
    fn execute<'a>(&'a self, ctx: &'a mut BackendContext<'_>) 
        -> Pin<Box<dyn Future<Output = bool> + Send + 'a>>;
}