//! /exit, /quit, /q command

use crate::commands::{Command, CommandContext};

pub struct ExitCommand;

impl Command for ExitCommand {
    fn name(&self) -> &'static str {
        "exit"
    }

    fn aliases(&self) -> &[&'static str] {
        &["quit", "q"]
    }

    fn help(&self) -> Option<&'static str> {
        Some("Exit MatrixCode")
    }

    fn execute(&self, ctx: &mut CommandContext, _args: &[&str]) {
        ctx.app.exit = true;
    }
}
