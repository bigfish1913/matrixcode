//! /mode command

use crate::commands::{Command, CommandContext};
use crate::types::ApproveMode;

pub struct ModeCommand;

impl Command for ModeCommand {
    fn name(&self) -> &'static str {
        "mode"
    }

    fn help(&self) -> Option<&'static str> {
        Some("Change approve mode (ask/auto/strict)")
    }

    fn execute(&self, ctx: &mut CommandContext, args: &[&str]) {
        if args.is_empty() {
            // Status bar already shows mode, no message needed
        } else {
            match args[0] {
                "ask" => {
                    ctx.app.approve_mode = ApproveMode::Ask;
                    ctx.sync_approve_mode();
                }
                "auto" => {
                    ctx.app.approve_mode = ApproveMode::Auto;
                    ctx.sync_approve_mode();
                }
                "strict" => {
                    ctx.app.approve_mode = ApproveMode::Strict;
                    ctx.sync_approve_mode();
                }
                _ => {
                    ctx.push_system("Invalid mode. Use: ask, auto, strict".into());
                    return;
                }
            }
        }
        ctx.auto_scroll();
    }
}