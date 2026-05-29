//! /retry command

use crate::commands::{Command, CommandContext};
use crate::types::Activity;

pub struct RetryCommand;

impl Command for RetryCommand {
    fn name(&self) -> &'static str {
        "retry"
    }

    fn help(&self) -> Option<&'static str> {
        Some("Retry last queued message")
    }

    fn execute(&self, ctx: &mut CommandContext, _args: &[&str]) {
        if !ctx.app.pending_messages.is_empty() && ctx.is_idle() {
            let next_msg = ctx.app.pending_messages.remove(0);
            ctx.push_user(next_msg.clone());
            ctx.send_to_backend(next_msg);
            ctx.app.activity = Activity::Thinking;
            ctx.auto_scroll();
        } else if ctx.app.pending_messages.is_empty() {
            ctx.push_system("No pending messages to retry".into());
        } else {
            ctx.push_system("AI is busy, please wait".into());
        }
        ctx.auto_scroll();
    }
}
