//! /system command handler

use std::future::Future;
use std::pin::Pin;

use crate::command::{Command, BackendContext};

pub struct SystemCommand;

impl Command for SystemCommand {
    fn name(&self) -> &'static str {
        "system"
    }

    fn help(&self) -> Option<&'static str> {
        Some("Show system information and stats")
    }

    fn execute<'a>(&'a self, ctx: &'a mut BackendContext<'_>) 
        -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> 
    {
        Box::pin(async move {
            let mut info = "📋 System Information:\n\n".to_string();
            info.push_str("⚙️ Configuration:\n");
            info.push_str(&format!("  Provider: {}\n", ctx.config.provider.as_deref().unwrap_or("auto")));
            info.push_str(&format!("  Model: {}\n", ctx.model));
            info.push_str(&format!("  Think: {}\n", ctx.config.think));
            info.push_str(&format!("  Max Tokens: {}\n", ctx.config.max_tokens));
            info.push_str(&format!("  Context Size: {}\n", ctx.config.context_size.unwrap_or(0)));
            info.push_str(&format!("  Approve Mode: {}\n", ctx.config.approve_mode.as_deref().unwrap_or("ask")));

            let system_prompt = ctx.agent.get_system_prompt();
            let clean_prompt = clean_markdown_tables(system_prompt);
            let prompt_preview = if clean_prompt.len() > 500 {
                format!("{}... ({} chars total)", &clean_prompt[..500], clean_prompt.len())
            } else {
                clean_prompt
            };
            info.push_str(&format!("\n📝 System Prompt Preview:\n{}\n", prompt_preview));

            let tools = ctx.agent.get_tools();
            info.push_str(&format!("\n🔧 Tools: {} available\n", tools.len()));

            let messages = ctx.agent.get_messages();
            info.push_str(&format!("💬 Messages: {} in history\n", messages.len()));

            let (input_tokens, output_tokens) = ctx.agent.get_token_counts();
            info.push_str(&format!("📊 Tokens: {} in, {} out\n", input_tokens, output_tokens));

            let _ = ctx.event_tx.send(crate::AgentEvent::progress(info, None)).await;
            false
        })
    }
}

fn clean_markdown_tables(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim().starts_with("|---") && line.trim().chars().filter(|c| *c == '|').count() <= 3)
        .map(|line| line.replace("|", " ").trim().to_string())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}