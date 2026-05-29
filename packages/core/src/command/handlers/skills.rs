//! /skills 命令处理器

use std::future::Future;
use std::pin::Pin;

use crate::command::{BackendContext, Command};

pub struct Skills;

impl Command for Skills {
    fn name(&self) -> &'static str {
        "skills"
    }

    fn help(&self) -> Option<&'static str> {
        Some("列出可用技能")
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a mut BackendContext<'_>,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(async move {
            if ctx.skills.is_empty() {
                let _ = ctx
                    .event_tx
                    .send(crate::AgentEvent::progress("无可用技能".to_string(), None))
                    .await;
            } else {
                let mut info = "🎯 可用技能：\n\n".to_string();
                for skill in ctx.skills {
                    info.push_str(&format!("  • {} - {}\n", skill.name, skill.description));
                }
                let _ = ctx
                    .event_tx
                    .send(crate::AgentEvent::progress(info, None))
                    .await;
            }
            false
        })
    }
}
