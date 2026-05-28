//! /save 命令实现
//!
//! 保存当前会话。

use super::super::command_trait::Command;
use super::super::backend_context::BackendContext;

/// Save 命令
///
/// 用法：
/// - /save - 保存当前会话
/// - /save <name> - 保存并重命名会话
pub struct Save;

impl Command for Save {
    fn name(&self) -> &'static str {
        "save"
    }

    fn help(&self) -> Option<&'static str> {
        Some("保存当前会话。用法: /save [name]")
    }

    fn execute<'a>(&'a self, ctx: &'a mut BackendContext<'_>) 
        -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + 'a>> 
    {
        Box::pin(async move {
            let parts: Vec<&str> = ctx.message.split_whitespace().collect();
            let name = parts.get(1).copied();

            if let Some(mgr) = ctx.session_mgr {
                let messages = ctx.agent.get_messages();
                mgr.set_messages(messages.to_vec());
                
                if let Some(n) = name {
                    if let Err(e) = mgr.rename_current(n) {
                        let _ = ctx.event_tx.send(crate::AgentEvent::error(
                            format!("Failed to rename: {}", e), None, None
                        )).await;
                    }
                }
                
                if let Err(e) = mgr.save_current() {
                    let _ = ctx.event_tx.send(crate::AgentEvent::error(
                        format!("Failed to save: {}", e), None, None
                    )).await;
                } else {
                    let _ = ctx.event_tx.send(crate::AgentEvent::progress(
                        "✓ Session saved", None
                    )).await;
                }
            } else {
                let _ = ctx.event_tx.send(crate::AgentEvent::progress(
                    "❌ Session manager not available", None
                )).await;
            }

            false // 不转发给 agent
        })
    }
}