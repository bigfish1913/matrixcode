//! /clear command

use crate::commands::{Command, CommandContext};

pub struct ClearCommand;

impl Command for ClearCommand {
    fn name(&self) -> &'static str {
        "clear"
    }

    fn help(&self) -> Option<&'static str> {
        Some("Clear messages")
    }

    fn execute(&self, ctx: &mut CommandContext, _args: &[&str]) {
        if ctx.is_idle() {
            ctx.app.messages.clear();
            ctx.app.pending_messages.clear();
        } else {
            ctx.push_system("⚠️ Cannot clear while AI is processing".into());
        }
        ctx.auto_scroll();
    }
}
