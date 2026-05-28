//! /tools 命令处理器

use std::future::Future;
use std::pin::Pin;

use crate::command::{Command, BackendContext};

pub struct Tools;

impl Command for Tools {
    fn name(&self) -> &'static str {
        "tools"
    }

    fn help(&self) -> Option<&'static str> {
        Some("列出可用工具")
    }

    fn execute<'a>(&'a self, ctx: &'a mut BackendContext<'_>) 
        -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> 
    {
        Box::pin(async move {
            let tools = ctx.agent.get_tools();
            let mut info = format!("🔧 可用工具：{}\n\n", tools.len());

            // 按类别分组
            let mut core_tools: Vec<_> = Vec::new();
            let mut file_tools: Vec<_> = Vec::new();
            let mut search_tools: Vec<_> = Vec::new();
            let mut web_tools: Vec<_> = Vec::new();
            let mut code_tools: Vec<_> = Vec::new();
            let mut mcp_tools: Vec<_> = Vec::new();
            let mut workflow_tools: Vec<_> = Vec::new();
            let mut other_tools: Vec<_> = Vec::new();

            for tool in tools.iter() {
                let def = tool.definition();
                let name = def.name.as_str();
                let desc = def.description.as_str();

                if name.starts_with("mcp_") || name.starts_with("mcp__") {
                    mcp_tools.push(tool);
                } else if name.starts_with("workflow_") || name.contains("workflow") {
                    workflow_tools.push(tool);
                } else if name.starts_with("code_") || desc.contains("CodeGraph") {
                    code_tools.push(tool);
                } else if name.starts_with("proxy_") || desc.contains("代理") {
                    other_tools.push(tool);
                } else {
                    match name {
                        "read" | "write" | "edit" | "multi_edit" | "ls" => file_tools.push(tool),
                        "grep" | "glob" | "search" => search_tools.push(tool),
                        "websearch" | "webfetch" => web_tools.push(tool),
                        "bash" | "task" | "todo_write" | "notebook_edit"
                        | "task_create" | "task_get" | "task_list" | "task_stop" => core_tools.push(tool),
                        "ask" | "enter_plan_mode" | "exit_plan_mode" | "monitor" => core_tools.push(tool),
                        _ => other_tools.push(tool),
                    }
                }
            }

            if !core_tools.is_empty() {
                info.push_str("📁 核心：\n");
                for tool in core_tools.iter().take(12) {
                    let def = tool.definition();
                    info.push_str(&format!("  {} - {}\n", def.name,
                        truncate_description(&def.description, 35)));
                }
            }

            if !file_tools.is_empty() {
                info.push_str("\n📄 文件：\n");
                for tool in file_tools.iter() {
                    let def = tool.definition();
                    info.push_str(&format!("  {} - {}\n", def.name,
                        truncate_description(&def.description, 35)));
                }
            }

            if !search_tools.is_empty() {
                info.push_str("\n🔍 搜索：\n");
                for tool in search_tools.iter() {
                    let def = tool.definition();
                    info.push_str(&format!("  {} - {}\n", def.name,
                        truncate_description(&def.description, 35)));
                }
            }

            if !code_tools.is_empty() {
                info.push_str("\n📊 CodeGraph：\n");
                for tool in code_tools.iter() {
                    let def = tool.definition();
                    info.push_str(&format!("  {} - {}\n", def.name,
                        truncate_description(&def.description, 35)));
                }
            }

            if !web_tools.is_empty() {
                info.push_str("\n🌐 网络：\n");
                for tool in web_tools.iter() {
                    let def = tool.definition();
                    info.push_str(&format!("  {} - {}\n", def.name,
                        truncate_description(&def.description, 35)));
                }
            }

            if !workflow_tools.is_empty() {
                info.push_str("\n🔄 工作流：\n");
                for tool in workflow_tools.iter().take(10) {
                    let def = tool.definition();
                    info.push_str(&format!("  {} - {}\n", def.name,
                        truncate_description(&def.description, 35)));
                }
            }

            if !mcp_tools.is_empty() {
                info.push_str("\n🔌 MCP：\n");
                for tool in mcp_tools.iter().take(15) {
                    let def = tool.definition();
                    info.push_str(&format!("  {} - {}\n", def.name,
                        truncate_description(&def.description, 35)));
                }
                if mcp_tools.len() > 15 {
                    info.push_str(&format!("  (+ {} 个更多)\n", mcp_tools.len() - 15));
                }
            }

            if !other_tools.is_empty() {
                info.push_str("\n🔧 其他：\n");
                for tool in other_tools.iter().take(10) {
                    let def = tool.definition();
                    info.push_str(&format!("  {} - {}\n", def.name,
                        truncate_description(&def.description, 35)));
                }
                if other_tools.len() > 10 {
                    info.push_str(&format!("  (+ {} 个更多)\n", other_tools.len() - 10));
                }
            }

            let _ = ctx.event_tx.send(crate::AgentEvent::progress(info, None)).await;
            false
        })
    }
}

fn truncate_description(desc: &str, max_len: usize) -> String {
    let first_line = desc.lines().next().unwrap_or(desc);
    let chars: Vec<char> = first_line.chars().collect();
    if chars.len() > max_len {
        chars[..max_len.saturating_sub(3)].iter().collect::<String>() + "..."
    } else {
        first_line.to_string()
    }
}