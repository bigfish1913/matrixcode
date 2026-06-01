//! Command pattern for TUI commands
//!
//! Each command implements the `Command` trait and is registered
//! in the `CommandRegistry` for dispatch.

use std::collections::HashMap;

use crate::app::TuiApp;
use crate::types::{Activity, Message, Role};

/// Command execution context
///
/// Provides access to TuiApp state and helper methods.
pub struct CommandContext<'a> {
    pub app: &'a mut TuiApp,
}

impl CommandContext<'_> {
    /// Push a system message to the message list
    pub fn push_system(&mut self, content: String) {
        self.app.push_message(Message {
            role: Role::System,
            content,
            is_pending: false,
        });
    }

    /// Push a user message to the message list
    pub fn push_user(&mut self, content: String) {
        self.app.push_message(Message {
            role: Role::User,
            content,
            is_pending: false,
        });
    }

    /// Send a message to the backend agent
    pub fn send_to_backend(&self, msg: String) {
        self.app.tx.try_send(msg).ok();
    }

    /// Check if the app is idle (not processing)
    pub fn is_idle(&self) -> bool {
        self.app.activity == Activity::Idle
    }

    /// Sync approve mode to shared atomic
    pub fn sync_approve_mode(&mut self) {
        if let Some(ref shared) = self.app.shared_approve_mode {
            shared.store(
                self.app.approve_mode.to_u8(),
                std::sync::atomic::Ordering::Relaxed,
            );
        }
        // If switching to auto and agent is waiting for approval, auto-approve
        if self.app.approve_mode == crate::types::ApproveMode::Auto
            && self.app.waiting_for_ask
            && let Some(ref ask_tx) = self.app.ask_tx
        {
            ask_tx.try_send("y".to_string()).ok();
            self.app.waiting_for_ask = false;
        }
        self.app
            .tx
            .try_send(format!("/mode:{}", self.app.approve_mode))
            .ok();
    }

    /// Enable auto scroll
    pub fn auto_scroll(&mut self) {
        self.app.auto_scroll = true;
    }
}

/// Command trait for implementing TUI commands
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

    /// Help text for the command (optional, for documentation).
    /// Implemented by many commands but not yet displayed in UI.
    #[allow(dead_code)]
    fn help(&self) -> Option<&'static str> {
        None
    }

    /// Execute the command
    fn execute(&self, ctx: &mut CommandContext, args: &[&str]);
}

/// Registry for all commands
pub struct CommandRegistry {
    commands: HashMap<&'static str, Box<dyn Command>>,
}

impl CommandRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }

    /// Register a command
    pub fn register(&mut self, command: Box<dyn Command>) {
        let name = command.name();
        self.commands.insert(name, command);
    }

    /// Dispatch a command string to the appropriate handler
    pub fn dispatch(&self, cmd: &str, ctx: &mut CommandContext) -> bool {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        let command_name = parts.first().copied().unwrap_or("");

        // Strip leading / if present
        let command_name = command_name.strip_prefix('/').unwrap_or(command_name);
        let args = &parts[1..];

        if let Some(command) = self.commands.get(command_name) {
            command.execute(ctx, args);
            true
        } else {
            // Try aliases
            for cmd in self.commands.values() {
                if cmd.aliases().contains(&command_name) {
                    cmd.execute(ctx, args);
                    return true;
                }
            }
            false
        }
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// Built-in commands
mod backend;
mod clear;
mod compact;
mod cron;
mod debug;
mod exit;
mod help;
mod history;
mod init;
mod r#loop;
mod mcp;
mod mode;
mod model;
mod new_cmd;
mod retry;
mod session;
mod workflow;

pub use clear::ClearCommand;
pub use compact::CompactCommand;
pub use cron::CronCommand;
pub use debug::DebugCommand;
pub use exit::ExitCommand;
pub use help::HelpCommand;
pub use history::HistoryCommand;
pub use init::InitCommand;
pub use r#loop::LoopCommand;
pub use mcp::McpCommand;
pub use mode::ModeCommand;
pub use model::ModelCommand;
pub use new_cmd::NewCommand;
pub use retry::RetryCommand;
pub use session::SessionCommand;
pub use workflow::WorkflowCommand;

use backend::BackendCommand;

/// Create the default command registry with all built-in commands
pub fn create_registry() -> CommandRegistry {
    let mut registry = CommandRegistry::new();

    // Simple commands
    registry.register(Box::new(ExitCommand));
    registry.register(Box::new(ClearCommand));
    registry.register(Box::new(HelpCommand));
    registry.register(Box::new(DebugCommand));

    // Medium commands
    registry.register(Box::new(ModeCommand));
    registry.register(Box::new(ModelCommand));
    registry.register(Box::new(HistoryCommand));
    registry.register(Box::new(RetryCommand));
    registry.register(Box::new(NewCommand));
    registry.register(Box::new(CompactCommand));
    registry.register(Box::new(InitCommand));
    registry.register(Box::new(SessionCommand));

    // Complex commands
    registry.register(Box::new(LoopCommand));
    registry.register(Box::new(CronCommand));
    registry.register(Box::new(WorkflowCommand));
    registry.register(Box::new(McpCommand));

    // Backend-forwarding commands (single handler for multiple commands)
    registry.register(Box::new(BackendCommand::new("tools")));
    registry.register(Box::new(BackendCommand::new("system")));
    registry.register(Box::new(BackendCommand::new("skills")));
    registry.register(Box::new(BackendCommand::new("memory")));
    registry.register(Box::new(BackendCommand::new("overview")));
    registry.register(Box::new(BackendCommand::new("config")));
    registry.register(Box::new(BackendCommand::new("save")));
    registry.register(Box::new(BackendCommand::new("sessions")));

    registry
}

use std::sync::OnceLock;

/// Global command registry (initialized once)
static REGISTRY: OnceLock<CommandRegistry> = OnceLock::new();

/// Get the global command registry
fn get_registry() -> &'static CommandRegistry {
    REGISTRY.get_or_init(create_registry)
}

/// Handle a command string (entry point for TuiApp)
pub fn handle_command(app: &mut TuiApp, cmd: &str) {
    let registry = get_registry();
    let mut ctx = CommandContext { app };

    if !registry.dispatch(cmd, &mut ctx) {
        // Unknown command: forward to backend for skill handling
        // Backend will check if it matches a skill name (e.g., /om:debug)
        if ctx.is_idle() {
            ctx.push_user(cmd.to_string());
            ctx.send_to_backend(cmd.to_string());
            ctx.app.activity = Activity::Thinking;
            ctx.app.request_start = Some(std::time::Instant::now());
        } else {
            // Queue message when AI is processing
            ctx.app.pending_messages.push(cmd.to_string());
        }
        ctx.auto_scroll();
    }
}
