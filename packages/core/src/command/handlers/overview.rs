//! /overview 命令处理器

use std::future::Future;
use std::pin::Pin;

use crate::command::{BackendContext, Command};

pub struct Overview;

impl Command for Overview {
    fn name(&self) -> &'static str {
        "overview"
    }

    fn help(&self) -> Option<&'static str> {
        Some("显示项目概览")
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a mut BackendContext<'_>,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(async move {
            // 检查项目路径
            let Some(project_path) = ctx.project_path else {
                let _ = ctx
                    .event_tx
                    .send(crate::AgentEvent::progress(
                        "未指定项目路径".to_string(),
                        None,
                    ))
                    .await;
                return false;
            };

            // 尝试读取项目结构
            let readme_path = project_path.join("README.md");
            let mut info = "📂 项目概览：\n\n".to_string();

            // 读取 README
            if readme_path.exists()
                && let Ok(content) = tokio::fs::read_to_string(&readme_path).await {
                    let lines: Vec<&str> = content.lines().take(20).collect();
                    info.push_str("README:\n");
                    info.push_str(&lines.join("\n"));
                    info.push_str("\n\n");
                }

            // 列出目录结构
            info.push_str("目录结构：\n");
            if let Ok(mut entries) = tokio::fs::read_dir(project_path).await {
                let mut dirs = Vec::new();
                let mut files = Vec::new();

                while let Ok(Some(entry)) = entries.next_entry().await {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with('.') || name == "target" || name == "node_modules" {
                        continue;
                    }
                    if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                        dirs.push(name);
                    } else {
                        files.push(name);
                    }
                }

                dirs.sort();
                files.sort();

                for dir in dirs.iter().take(10) {
                    info.push_str(&format!("  📁 {}/\n", dir));
                }
                for file in files.iter().take(10) {
                    info.push_str(&format!("  📄 {}\n", file));
                }

                if dirs.len() > 10 || files.len() > 10 {
                    info.push_str(&format!(
                        "  ... 还有 {} 个目录, {} 个文件\n",
                        dirs.len().saturating_sub(10),
                        files.len().saturating_sub(10)
                    ));
                }
            }

            let _ = ctx
                .event_tx
                .send(crate::AgentEvent::progress(info, None))
                .await;
            false
        })
    }
}
