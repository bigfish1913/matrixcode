//! /config command handler

use std::future::Future;
use std::pin::Pin;

use crate::command::{Command, BackendContext};

pub struct ConfigCommand;

impl Command for ConfigCommand {
    fn name(&self) -> &'static str {
        "config"
    }

    fn help(&self) -> Option<&'static str> {
        Some("Show current configuration")
    }

    fn execute<'a>(&'a self, ctx: &'a mut BackendContext<'_>) 
        -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> 
    {
        Box::pin(async move {
            let mut info = "⚙️ Current Configuration:\n\n".to_string();
            info.push_str(&format!("Provider: {}\n", ctx.config.provider.as_deref().unwrap_or("auto")));
            info.push_str(&format!("Model: {}\n", ctx.model));
            info.push_str(&format!("Think: {}\n", ctx.config.think));
            info.push_str(&format!("Max Tokens: {}\n", ctx.config.max_tokens));
            let _ = ctx.event_tx.send(crate::AgentEvent::progress(info, None)).await;
            false
        })
    }
}