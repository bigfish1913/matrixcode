//! /history command

use crate::commands::{Command, CommandContext};
use crate::types::Role;
use crate::utils::fmt_tokens;

pub struct HistoryCommand;

impl Command for HistoryCommand {
    fn name(&self) -> &'static str {
        "history"
    }

    fn help(&self) -> Option<&'static str> {
        Some("Show session history")
    }

    fn execute(&self, ctx: &mut CommandContext, _args: &[&str]) {
        let (user_count, assistant_count, tool_count) = ctx.app.messages.iter().fold(
            (0, 0, 0),
            |(u, a, t), m| match m.role {
                Role::User => (u + 1, a, t),
                Role::Assistant => (u, a + 1, t),
                Role::Tool { .. } => (u, a, t + 1),
                _ => (u, a, t),
            },
        );
        let queue_count = ctx.app.pending_messages.len();

        if ctx.app.debug_mode {
            ctx.push_system(format!(
                "📊 Session: {} user, {} assistant, {} tools, {} queued, {}tok output",
                user_count,
                assistant_count,
                tool_count,
                queue_count,
                fmt_tokens(ctx.app.session_total_out)
            ));
        }
        ctx.auto_scroll();
    }
}