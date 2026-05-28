//! /new command

use crate::commands::{Command, CommandContext};

pub struct NewCommand;

impl Command for NewCommand {
    fn name(&self) -> &'static str {
        "new"
    }

    fn help(&self) -> Option<&'static str> {
        Some("Start new session")
    }

    fn execute(&self, ctx: &mut CommandContext, _args: &[&str]) {
        if ctx.is_idle() {
            ctx.app.messages.clear();
            ctx.app.pending_messages.clear();
            ctx.app.tokens_in = 0;
            ctx.app.tokens_out = 0;
            ctx.app.session_total_out = 0;
            ctx.send_to_backend("/new".into());
        } else {
            ctx.push_system("⚠️ Cannot start new session while AI is processing".into());
        }
        ctx.auto_scroll();
    }
}