//! /context 命令处理器
//!
//! 显示当前上下文信息，包括即将发送给大模型的完整内容预览。

use std::future::Future;
use std::pin::Pin;

use crate::command::{BackendContext, Command};
use crate::AgentEvent;

pub struct Context;

impl Command for Context {
    fn name(&self) -> &'static str {
        "context"
    }

    fn aliases(&self) -> &[&'static str] {
        &["ctx"]
    }

    fn help(&self) -> Option<&'static str> {
        Some("显示当前上下文信息（消息、记忆、焦点等）")
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a mut BackendContext<'_>,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(async move {
            let msg = ctx.message.trim();

            if msg == "/context" || msg == "/context status" || msg == "/ctx" {
                self.show_status(ctx).await
            } else if msg == "/context full" || msg == "/ctx full" {
                self.show_full_preview(ctx).await
            } else if msg == "/context messages" || msg == "/ctx messages" {
                self.show_messages(ctx).await
            } else if msg == "/context memory" || msg == "/ctx memory" {
                self.show_memory(ctx).await
            } else if msg == "/context focus" || msg == "/ctx focus" {
                self.show_focus(ctx).await
            } else {
                let _ = ctx
                    .event_tx
                    .send(AgentEvent::progress(
                        "用法：/context [status|full|messages|memory|focus]\n\
                         别名：/ctx\n\
                         - status: 显示上下文状态概览\n\
                         - full: 显示完整上下文预览\n\
                         - messages: 显示消息列表\n\
                         - memory: 显示记忆信息\n\
                         - focus: 显示焦点信息".to_string(),
                        None,
                    ))
                    .await;
                false
            }
        })
    }
}

impl Context {
    /// Show context status overview
    async fn show_status(&self, ctx: &mut BackendContext<'_>) -> bool {
        let info = ctx.agent.get_context_info();

        let mut output = String::new();
        output.push_str("📊 **上下文状态**\n\n");

        // Basic stats
        output.push_str(&format!("**模型**: {}\n", info.model_name));
        output.push_str(&format!("**消息数量**: {}\n", info.message_count));
        output.push_str(&format!("**估算输入 tokens**: ~{}\n", info.estimated_input_tokens));
        output.push_str(&format!("**累计输入 tokens**: {}\n", info.total_input_tokens));
        output.push_str(&format!("**累计输出 tokens**: {}\n", info.total_output_tokens));
        output.push_str(&format!("**最大 tokens 设置**: {}\n\n", info.max_tokens));

        // System prompt preview
        output.push_str("**系统提示预览**:\n");
        output.push_str(&format!("> {}...\n\n",
            truncate_string(&info.system_prompt_preview, 200)));

        // Memory
        if let Some(memory) = &info.memory_summary {
            output.push_str("**记忆摘要**: ✅ 已加载\n");
            output.push_str(&format!("> {}...\n\n", truncate_string(memory, 100)));
        } else {
            output.push_str("**记忆摘要**: ❌ 未加载\n\n");
        }

        // Project overview
        if let Some(overview) = &info.project_overview_preview {
            output.push_str("**项目概述**: ✅ 已加载\n");
            output.push_str(&format!("> {}...\n\n", truncate_string(overview, 100)));
        } else {
            output.push_str("**项目概述**: ❌ 未加载\n\n");
        }

        // Recent messages
        output.push_str("**最近 5 条消息**:\n");
        for msg_preview in &info.recent_messages_preview {
            output.push_str(&format!("  {}\n", msg_preview));
        }

        // Memory storage status
        if let Some(storage) = &ctx.memory_storage {
            output.push_str("\n**记忆存储**: ✅ 已初始化\n");
        } else {
            output.push_str("\n**记忆存储**: ❌ 未初始化\n");
        }

        // Session info
        if let Some(session_mgr) = &ctx.session_mgr {
            if session_mgr.has_current() {
                if let Some(current) = session_mgr.current_id() {
                    output.push_str(&format!("\n**当前会话**: {}\n",
                        truncate_string(current, 20)));
                }
            } else {
                output.push_str("\n**当前会话**: 新会话\n");
            }
        }

        let _ = ctx
            .event_tx
            .send(AgentEvent::progress(output, None))
            .await;

        false
    }

    /// Show full context preview (everything sent to LLM)
    async fn show_full_preview(&self, ctx: &mut BackendContext<'_>) -> bool {
        let preview = ctx.agent.get_full_context_preview();

        // Calculate total length
        let total_chars = preview.chars().count();
        let estimated_tokens = total_chars / 3; // rough estimate

        let output = format!(
            "📄 **完整上下文预览** ({} 字符, ~{} tokens)\n\n```text\n{}\n```",
            total_chars,
            estimated_tokens,
            preview
        );

        let _ = ctx
            .event_tx
            .send(AgentEvent::progress(output, None))
            .await;

        false
    }

    /// Show message list
    async fn show_messages(&self, ctx: &mut BackendContext<'_>) -> bool {
        let info = ctx.agent.get_context_info();

        let mut output = String::new();
        output.push_str(&format!("📨 **消息列表** (共 {} 条)\n\n", info.message_count));

        for msg_preview in &info.recent_messages_preview {
            output.push_str(&format!("{}\n", msg_preview));
        }

        if info.message_count > 5 {
            output.push_str(&format!("\n... 还有 {} 条较早消息", info.message_count - 5));
        }

        let _ = ctx
            .event_tx
            .send(AgentEvent::progress(output, None))
            .await;

        false
    }

    /// Show memory information
    async fn show_memory(&self, ctx: &mut BackendContext<'_>) -> bool {
        let info = ctx.agent.get_context_info();

        let mut output = String::new();
        output.push_str("🧠 **记忆信息**\n\n");

        if let Some(memory) = &info.memory_summary {
            output.push_str("**已加载记忆摘要**:\n");
            output.push_str(&format!("{}\n", memory));
        } else {
            output.push_str("**未加载记忆摘要**\n");
        }

        // Memory storage details
        if let Some(storage) = &ctx.memory_storage {
            output.push_str("\n**记忆存储状态**:\n");
            output.push_str("  ✅ 已初始化\n");
            // Could add more details here if needed
        } else {
            output.push_str("\n**记忆存储状态**: ❌ 未初始化\n");
        }

        let _ = ctx
            .event_tx
            .send(AgentEvent::progress(output, None))
            .await;

        false
    }

    /// Show focus information
    async fn show_focus(&self, ctx: &mut BackendContext<'_>) -> bool {
        let mut output = String::new();
        output.push_str("🎯 **焦点信息**\n\n");

        // Check if agent has focus manager (via memory storage)
        // Focus information is typically in the memory summary
        let info = ctx.agent.get_context_info();

        if let Some(memory) = &info.memory_summary {
            // Extract focus-related info from memory
            if memory.contains("焦点") || memory.contains("focus") {
                output.push_str("**焦点状态**: ✅ 已跟踪\n");
                output.push_str(&format!("{}\n", memory));
            } else {
                output.push_str("**焦点状态**: 信息在记忆摘要中\n");
                output.push_str(&format!("{}\n", memory));
            }
        } else {
            output.push_str("**焦点状态**: ❌ 无焦点信息\n");
        }

        // Note: FocusManager is not directly accessible from BackendContext
        // This shows what's available through the memory system

        output.push_str("\n**提示**: 焦点信息会随记忆一起发送给大模型\n");
        output.push_str("使用 `/context full` 可查看完整上下文\n");

        let _ = ctx
            .event_tx
            .send(AgentEvent::progress(output, None))
            .await;

        false
    }
}

/// Helper function to truncate string
fn truncate_string(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max).collect();
        format!("{}...", truncated)
    }
}