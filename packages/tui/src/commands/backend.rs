//! Backend-forwarding commands
//!
//! These commands simply forward to the backend agent for processing.

use crate::commands::{Command, CommandContext};

pub struct BackendCommand {
    name: &'static str,
    aliases: &'static [&'static str],
}

impl BackendCommand {
    pub fn new(name: &'static str) -> Self {
        Self { name, aliases: &[] }
    }
}

impl Command for BackendCommand {
    fn name(&self) -> &'static str {
        self.name
    }

    fn aliases(&self) -> &[&'static str] {
        self.aliases
    }

    fn execute(&self, ctx: &mut CommandContext, _args: &[&str]) {
        let cmd = format!("/{}", self.name);
        ctx.send_to_backend(cmd);
        ctx.auto_scroll();
    }
}
