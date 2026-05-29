//! /load 命令实现
//!
//! 加载指定会话。

use super::super::backend_context::BackendContext;
use super::super::command_trait::Command;

/// Load 命令
///
/// 用法：
/// - /load <session_id> - 加载指定会话
pub struct Load;

impl Command for Load {
    fn name(&self) -> &'static str {
        "load"
    }

    fn help(&self) -> Option<&'static str> {
        Some("加载指定会话。用法: /load <session_id>")
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a mut BackendContext<'_>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + 'a>> {
        Box::pin(async move {
            let session_id = ctx.message.strip_prefix("/load ").unwrap_or("");

            if let Some(mgr) = ctx.session_mgr {
                if mgr.resume(session_id).is_ok() {
                    if let Some(msgs) = mgr.messages() {
                        let messages = msgs.to_vec();
                        ctx.agent.set_messages(messages.clone());
                        let _ = ctx
                            .event_tx
                            .send(crate::AgentEvent::progress(
                                format!(
                                    "✓ Session '{}' loaded ({} messages)",
                                    session_id,
                                    messages.len()
                                ),
                                None,
                            ))
                            .await;
                    }
                } else {
                    let _ = ctx
                        .event_tx
                        .send(crate::AgentEvent::progress(
                            format!("❌ Session '{}' not found", session_id),
                            None,
                        ))
                        .await;
                }
            } else {
                let _ = ctx
                    .event_tx
                    .send(crate::AgentEvent::progress(
                        "❌ Session manager not available",
                        None,
                    ))
                    .await;
            }

            false // 不转发给 agent
        })
    }
}
