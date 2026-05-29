//! /workflow 命令处理器

use std::future::Future;
use std::pin::Pin;

use crate::command::{BackendContext, Command};

pub struct Workflow;

impl Command for Workflow {
    fn name(&self) -> &'static str {
        "workflow"
    }

    fn help(&self) -> Option<&'static str> {
        Some("列出可用工作流")
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a mut BackendContext<'_>,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(async move {
            // TODO: 实现工作流发现
            let info = "🔄 可用工作流：\n\n暂无已注册的工作流。\n\n提示：使用 /workflow run <name> 运行工作流".to_string();
            let _ = ctx
                .event_tx
                .send(crate::AgentEvent::progress(info, None))
                .await;
            false
        })
    }
}
