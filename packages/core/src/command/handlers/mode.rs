//! /mode 命令实现
//!
//! 设置审批模式。

use crate::ApproveMode;

use super::super::command_trait::Command;
use super::super::backend_context::BackendContext;

/// Mode 命令
///
/// 用法：
/// - /mode:ask - Ask 模式（默认，变更前询问）
/// - /mode:auto - 自动模式（不询问）
/// - /mode:strict - 严格模式（所有操作都询问）
pub struct Mode;

impl Command for Mode {
    fn name(&self) -> &'static str {
        "mode"
    }

    fn help(&self) -> Option<&'static str> {
        Some("设置审批模式。用法: /mode:<ask|auto|strict>")
    }

    /// 特殊匹配：/mode:xxx 前缀
    fn matches(&self, msg: &str) -> bool {
        msg.starts_with("/mode:")
    }

    fn execute<'a>(&'a self, ctx: &'a mut BackendContext<'_>) 
        -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + 'a>> 
    {
        Box::pin(async move {
            let mode_str = ctx.message.strip_prefix("/mode:").unwrap_or("");
            
            let mode = match mode_str.to_lowercase().as_str() {
                "ask" | "a" => ApproveMode::Ask,
                "auto" => ApproveMode::Auto,
                "strict" => ApproveMode::Strict,
                _ => {
                    let _ = ctx.event_tx.send(crate::AgentEvent::error(
                        format!("Unknown mode: {}. Use ask/auto/strict", mode_str),
                        None,
                        None,
                    )).await;
                    return false;
                }
            };

            ctx.agent.set_approve_mode(mode);
            
            let mode_name = match mode {
                ApproveMode::Ask => "ask (default)",
                ApproveMode::Auto => "auto (no confirmation)",
                ApproveMode::Strict => "strict (confirm all)",
            };
            
            let _ = ctx.event_tx.send(crate::AgentEvent::progress(
                format!("✓ Mode set to: {}", mode_name),
                None,
            )).await;

            false // 不转发给 agent
        })
    }
}