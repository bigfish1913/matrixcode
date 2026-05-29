//! /sessions 命令实现
//!
//! 列出和管理会话。

use super::super::backend_context::BackendContext;
use super::super::command_trait::Command;

/// Sessions 命令
///
/// 用法：
/// - /sessions - 列出所有会话
/// - /sessions cleanup - 清理旧会话
/// - /sessions stats - 显示会话统计
/// - /resume - 别名，列出会话
pub struct Sessions;

impl Command for Sessions {
    fn name(&self) -> &'static str {
        "sessions"
    }

    fn aliases(&self) -> &[&'static str] {
        &["resume"]
    }

    fn help(&self) -> Option<&'static str> {
        Some("列出和管理会话。用法: /sessions [cleanup|stats]")
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a mut BackendContext<'_>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + 'a>> {
        Box::pin(async move {
            const SESSION_CLEANUP_DAYS: u64 = 30;
            const DISPLAY_SESSIONS_LIMIT: usize = 20;

            let subcmd = if ctx.message.starts_with("/sessions ") {
                ctx.message.strip_prefix("/sessions ").unwrap_or("")
            } else {
                ""
            };

            if let Some(mgr) = ctx.session_mgr {
                if subcmd == "cleanup" {
                    let removed = mgr.cleanup_old_sessions(SESSION_CLEANUP_DAYS).unwrap_or(0);
                    let _ = ctx
                        .event_tx
                        .send(crate::AgentEvent::progress(
                            format!("✓ Removed {} old sessions", removed),
                            None,
                        ))
                        .await;
                } else if subcmd == "stats" {
                    let sessions = mgr.list_sessions();
                    let _ = ctx
                        .event_tx
                        .send(crate::AgentEvent::progress(
                            format!("📊 {} sessions total", sessions.len()),
                            None,
                        ))
                        .await;
                } else {
                    let sessions = mgr.list_sessions();
                    if sessions.is_empty() {
                        let _ = ctx
                            .event_tx
                            .send(crate::AgentEvent::progress("No saved sessions", None))
                            .await;
                    } else {
                        let mut info = format!("📚 Sessions ({}):\n", sessions.len());
                        for session in sessions.iter().take(DISPLAY_SESSIONS_LIMIT) {
                            info.push_str(&format!(
                                "• {} - {} msgs\n",
                                session.short_id(),
                                session.message_count
                            ));
                        }
                        let _ = ctx
                            .event_tx
                            .send(crate::AgentEvent::progress(info, None))
                            .await;
                    }
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
