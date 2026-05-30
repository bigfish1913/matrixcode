//! /session command - Interactive session selector
//!
//! Displays a list of saved sessions with keyboard navigation.

use crate::commands::{Command, CommandContext};
use crate::types::Activity;

pub struct SessionCommand;

impl Command for SessionCommand {
    fn name(&self) -> &'static str {
        "session"
    }

    fn aliases(&self) -> &[&'static str] {
        &["sessions"]
    }

    fn help(&self) -> Option<&'static str> {
        Some("Interactive session selector. Use arrow keys to navigate, Enter to load.")
    }

    fn execute(&self, ctx: &mut CommandContext, _args: &[&str]) {
        // Only activate when idle
        if ctx.app.activity != Activity::Idle {
            ctx.push_system("Cannot switch session while AI is processing".into());
            return;
        }

        // Request session list from backend
        ctx.send_to_backend("/sessions".to_string());
        
        // Set waiting state - will be activated when session list arrives
        ctx.app.waiting_for_session = true;
        ctx.app.session_selected_index = 0;
        ctx.app.session_list.clear();
        
        // Set activity to Asking-like state to block normal input
        ctx.app.activity = Activity::Asking;
        
        ctx.auto_scroll();
    }
}