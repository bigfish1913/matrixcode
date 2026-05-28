//! /model command

use crate::commands::{Command, CommandContext};

pub struct ModelCommand;

impl Command for ModelCommand {
    fn name(&self) -> &'static str {
        "model"
    }

    fn help(&self) -> Option<&'static str> {
        Some("Show/change model")
    }

    fn execute(&self, ctx: &mut CommandContext, args: &[&str]) {
        if args.is_empty() {
            // Status bar already shows model info
        } else if ctx.is_idle() {
            let new_model = args.join(" ");
            ctx.app.model = new_model;
        } else {
            ctx.push_system("⚠️ Cannot change model while AI is processing".into());
        }
        ctx.auto_scroll();
    }
}