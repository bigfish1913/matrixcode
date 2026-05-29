//! /memory 命令处理器

use std::future::Future;
use std::pin::Pin;

use crate::command::{BackendContext, Command};

pub struct Memory;

impl Command for Memory {
    fn name(&self) -> &'static str {
        "memory"
    }

    fn help(&self) -> Option<&'static str> {
        Some("管理记忆存储")
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a mut BackendContext<'_>,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(async move {
            let msg = ctx.message.trim();

            // 解析子命令
            if msg == "/memory" || msg == "/memory status" {
                self.show_status(ctx).await
            } else if msg.starts_with("/memory search ") {
                let query = msg.strip_prefix("/memory search ").unwrap_or("");
                self.search(ctx, query).await
            } else if msg == "/memory clear" {
                self.clear(ctx).await
            } else {
                let _ = ctx
                    .event_tx
                    .send(crate::AgentEvent::progress(
                        "用法：/memory [status|search <query>|clear]".to_string(),
                        None,
                    ))
                    .await;
                false
            }
        })
    }
}

impl Memory {
    async fn show_status(&self, ctx: &mut BackendContext<'_>) -> bool {
        let mut info = "🧠 记忆状态：\n\n".to_string();

        if let Some(_storage) = &ctx.memory_storage {
            // 显示记忆统计
            info.push_str(&format!("记忆存储已初始化\n"));
            // TODO: 添加更多统计信息
        } else {
            info.push_str("记忆存储未初始化\n");
        }

        let _ = ctx
            .event_tx
            .send(crate::AgentEvent::progress(info, None))
            .await;
        false
    }

    async fn search(&self, ctx: &mut BackendContext<'_>, query: &str) -> bool {
        if query.is_empty() {
            let _ = ctx
                .event_tx
                .send(crate::AgentEvent::progress(
                    "请提供搜索关键词".to_string(),
                    None,
                ))
                .await;
            return false;
        }

        let mut info = format!("🔍 搜索记忆：\"{}\"\n\n", query);

        if let Some(_storage) = &ctx.memory_storage {
            // TODO: 实现记忆搜索
            info.push_str("搜索功能待实现\n");
        } else {
            info.push_str("记忆存储未初始化\n");
        }

        let _ = ctx
            .event_tx
            .send(crate::AgentEvent::progress(info, None))
            .await;
        false
    }

    async fn clear(&self, ctx: &mut BackendContext<'_>) -> bool {
        if let Some(_storage) = ctx.memory_storage {
            // TODO: 实现清空记忆
            let _ = ctx
                .event_tx
                .send(crate::AgentEvent::progress("记忆已清空".to_string(), None))
                .await;
        } else {
            let _ = ctx
                .event_tx
                .send(crate::AgentEvent::progress(
                    "记忆存储未初始化".to_string(),
                    None,
                ))
                .await;
        }
        false
    }
}
