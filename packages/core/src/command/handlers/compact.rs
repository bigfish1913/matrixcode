//! /compact 和 /compress 命令处理器

use std::future::Future;
use std::pin::Pin;

use crate::command::{BackendContext, Command};

pub struct Compact;

impl Command for Compact {
    fn name(&self) -> &'static str {
        "compact"
    }

    fn aliases(&self) -> &[&'static str] {
        &["compress"]
    }

    fn help(&self) -> Option<&'static str> {
        Some("压缩对话历史以减少 token 使用")
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a mut BackendContext<'_>,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(async move {
            // 获取当前消息数量
            let count = ctx.agent.get_messages().len();

            if count == 0 {
                let _ = ctx
                    .event_tx
                    .send(crate::AgentEvent::progress(
                        "对话历史为空，无需压缩".to_string(),
                        None,
                    ))
                    .await;
                return false;
            }

            let _ = ctx
                .event_tx
                .send(crate::AgentEvent::progress(
                    format!("当前对话历史：{} 条消息", count),
                    None,
                ))
                .await;

            // 注意：完整的压缩功能需要 AI 参与
            // 这里只提供状态信息
            let _ = ctx
                .event_tx
                .send(crate::AgentEvent::progress(
                    "💡 提示：压缩功能在对话过程中自动执行\n使用 /clear 清空对话历史".to_string(),
                    None,
                ))
                .await;

            false
        })
    }
}
