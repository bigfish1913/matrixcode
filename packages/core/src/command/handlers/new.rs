//! /new 命令实现
//!
//! 开始新会话。

use super::super::command_trait::Command;
use super::super::backend_context::BackendContext;

/// New 命令
///
/// 用法：
/// - /new - 开始新会话
pub struct New;

impl Command for New {
    fn name(&self) -> &'static str {
        "new"
    }

    fn help(&self) -> Option<&'static str> {
        Some("开始新会话。用法: /new")
    }

    fn execute<'a>(&'a self, ctx: &'a mut BackendContext<'_>) 
        -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + 'a>> 
    {
        Box::pin(async move {
            if let Some(mgr) = ctx.session_mgr {
                // Start new session with None project path
                if let Err(e) = mgr.start_new(None) {
                    let _ = ctx.event_tx.send(crate::AgentEvent::error(
                        format!("Failed to start new session: {}", e), None, None
                    )).await;
                }
            }
            ctx.agent.clear_history();
            
            let _ = ctx.event_tx.send(crate::AgentEvent::progress(
                "✓ New session started", None
            )).await;

            false // 不转发给 agent
        })
    }
}