//! /debug command

use crate::commands::{Command, CommandContext};

pub struct DebugCommand;

impl Command for DebugCommand {
    fn name(&self) -> &'static str {
        "debug"
    }

    fn help(&self) -> Option<&'static str> {
        Some("Toggle debug mode")
    }

    fn execute(&self, ctx: &mut CommandContext, _args: &[&str]) {
        ctx.app.debug_mode = !ctx.app.debug_mode;
        ctx.auto_scroll();
    }
}
