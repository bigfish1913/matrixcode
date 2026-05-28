//! /compact command

use crate::commands::{Command, CommandContext};

pub struct CompactCommand;

impl Command for CompactCommand {
    fn name(&self) -> &'static str {
        "compact"
    }

    fn aliases(&self) -> &[&'static str] {
        &["compress"]
    }

    fn help(&self) -> Option<&'static str> {
        Some("Compress context")
    }

    fn execute(&self, ctx: &mut CommandContext, _args: &[&str]) {
        ctx.send_to_backend("/compact".into());
        ctx.auto_scroll();
    }
}