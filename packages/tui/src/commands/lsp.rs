//! /lsp command

use crate::commands::{Command, CommandContext};
use crate::app::LspServerStatus;

pub struct LspCommand;

impl Command for LspCommand {
    fn name(&self) -> &'static str {
        "lsp"
    }

    fn aliases(&self) -> &[&'static str] {
        &["lsp list", "lsp status"]
    }

    fn help(&self) -> Option<&'static str> {
        Some("Show LSP server status")
    }

    fn execute(&self, ctx: &mut CommandContext, _args: &[&str]) {
        if ctx.app.lsp_servers.is_empty() {
            ctx.push_system(
                "📋 未连接任何 LSP servers\n\nLSP 提供代码智能感知和诊断功能。\n配置文件: .matrix/lsp.json".into(),
            );
        } else {
            let mut content = "📋 LSP Servers:\n".to_string();
            for server in &ctx.app.lsp_servers {
                let status_text = match &server.status {
                    LspServerStatus::Connected => "● 运行中",
                    LspServerStatus::NotStarted => "○ 未启动",
                    LspServerStatus::Starting => "◐ 启动中",
                    LspServerStatus::Error(msg) => &format!("✗ 错误: {}", msg),
                };
                content.push_str(&format!(
                    "  • {} ({}) {}\n",
                    server.name, server.language, status_text
                ));
            }
            ctx.push_system(content);
        }
        ctx.auto_scroll();
    }
}