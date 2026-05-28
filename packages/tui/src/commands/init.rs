//! /init command

use crate::commands::{Command, CommandContext};

pub struct InitCommand;

impl Command for InitCommand {
    fn name(&self) -> &'static str {
        "init"
    }

    fn help(&self) -> Option<&'static str> {
        Some("Initialize/reset project")
    }

    fn execute(&self, ctx: &mut CommandContext, args: &[&str]) {
        if args.is_empty() {
            // Generate project overview
            ctx.send_to_backend("/init".into());
        } else if args[0] == "status" {
            // Show project status
            ctx.send_to_backend("/init status".into());
        } else if matches!(args[0], "reset" | "clear") {
            // Reset configuration
            ctx.send_to_backend("/init reset".into());
        } else {
            ctx.push_system(
                "Unknown init command. Use: /init, /init status, /init reset".into(),
            );
        }
        ctx.auto_scroll();
    }
}