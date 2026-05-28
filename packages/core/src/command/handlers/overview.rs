//! /overview command handler

use std::future::Future;
use std::pin::Pin;

use crate::command::{Command, BackendContext};

pub struct OverviewCommand;

impl Command for OverviewCommand {
    fn name(&self) -> &'static str {
        "overview"
    }

    fn help(&self) -> Option<&'static str> {
        Some("Generate project overview")
    }

    fn execute<'a>(&'a self, ctx: &'a mut BackendContext<'_>) 
        -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> 
    {
        Box::pin(async move {
            let _ = ctx.event_tx.send(crate::AgentEvent::progress(
                "📊 Use /init to generate project overview and CodeGraph index.".to_string(),
                None,
            )).await;
            false
        })
    }
}