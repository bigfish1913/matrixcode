//! /skills command handler

use std::future::Future;
use std::pin::Pin;

use crate::command::{Command, BackendContext};

pub struct SkillsCommand;

impl Command for SkillsCommand {
    fn name(&self) -> &'static str {
        "skills"
    }

    fn help(&self) -> Option<&'static str> {
        Some("List available skills")
    }

    fn execute<'a>(&'a self, ctx: &'a mut BackendContext<'_>) 
        -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> 
    {
        Box::pin(async move {
            if ctx.skills.is_empty() {
                let _ = ctx.event_tx.send(crate::AgentEvent::progress(
                    "No skills available".to_string(),
                    None,
                )).await;
            } else {
                let mut info = "🎯 Available Skills:\n\n".to_string();
                for skill in ctx.skills {
                    info.push_str(&format!("  • {} - {}\n", skill.name, skill.description));
                }
                let _ = ctx.event_tx.send(crate::AgentEvent::progress(info, None)).await;
            }
            false
        })
    }
}