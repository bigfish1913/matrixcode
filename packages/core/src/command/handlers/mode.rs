//! /mode 命令实现
//!
//! 设置审批模式。

use crate::ApproveMode;

use super::super::backend_context::BackendContext;
use super::super::command_trait::Command;

/// Mode 命令
///
/// 用法：
/// - /mode:ask 或 /mode:询问 - Ask 模式（默认，变更前询问）
/// - /mode:auto 或 /mode:自动 - 自动模式（不询问）
/// - /mode:strict 或 /mode:严格 - 严格模式（所有操作都询问）
pub struct Mode;

impl Command for Mode {
    fn name(&self) -> &'static str {
        "mode"
    }

    fn help(&self) -> Option<&'static str> {
        Some("设置审批模式。用法: /mode:<ask|auto|strict> 或 /mode:<询问|自动|严格>")
    }

    /// 特殊匹配：/mode:xxx 前缀
    fn matches(&self, msg: &str) -> bool {
        msg.starts_with("/mode:")
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a mut BackendContext<'_>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + 'a>> {
        Box::pin(async move {
            let mode_str = ctx.message.strip_prefix("/mode:").unwrap_or("");

            let mode = match mode_str.to_lowercase().as_str() {
                // English
                "ask" | "a" => ApproveMode::Ask,
                "auto" => ApproveMode::Auto,
                "strict" => ApproveMode::Strict,
                // Chinese
                "询问" => ApproveMode::Ask,
                "自动" => ApproveMode::Auto,
                "严格" => ApproveMode::Strict,
                _ => {
                    let _ = ctx
                        .event_tx
                        .send(crate::AgentEvent::error(
                            format!("未知模式: {}. 支持: ask/auto/strict 或 询问/自动/严格", mode_str),
                            None,
                            None,
                        ))
                        .await;
                    return false;
                }
            };

            ctx.agent.set_approve_mode(mode);

            let mode_name = match mode {
                ApproveMode::Ask => "ask (询问 - 变更前确认)",
                ApproveMode::Auto => "auto (自动 - 无需确认)",
                ApproveMode::Strict => "strict (严格 - 所有操作都确认)",
            };

            let _ = ctx
                .event_tx
                .send(crate::AgentEvent::progress(
                    format!("✓ 模式设置为: {}", mode_name),
                    None,
                ))
                .await;

            false // 不转发给 agent
        })
    }
}
