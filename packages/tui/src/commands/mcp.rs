//! /mcp command

use crate::commands::{Command, CommandContext};

pub struct McpCommand;

impl Command for McpCommand {
    fn name(&self) -> &'static str {
        "mcp"
    }

    fn aliases(&self) -> &[&'static str] {
        &["mcp list"]
    }

    fn help(&self) -> Option<&'static str> {
        Some("List MCP servers")
    }

    fn execute(&self, ctx: &mut CommandContext, _args: &[&str]) {
        if ctx.app.mcp_servers.is_empty() {
            ctx.push_system(
                "📋 未连接任何 MCP servers\n\n使用 --mcp 参数启动:\n  matrixcode-tui --mcp \"playwright:npx -y @playwright/mcp@latest\"".into(),
            );
        } else {
            let mut content = "📋 MCP Servers:\n".to_string();
            for server in &ctx.app.mcp_servers {
                let status = if server.is_started { "✓ 运行中" } else { "✗ 未启动" };
                content.push_str(&format!(
                    "  • {} {} ({} 工具)\n",
                    server.name, status, server.tool_count
                ));
            }
            ctx.push_system(content);
        }
        ctx.auto_scroll();
    }
}