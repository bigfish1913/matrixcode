//! /system 命令处理器

use std::future::Future;
use std::pin::Pin;

use crate::command::{BackendContext, Command};
use crate::workflow::WorkflowRegistry;

pub struct System;

impl Command for System {
    fn name(&self) -> &'static str {
        "system"
    }

    fn help(&self) -> Option<&'static str> {
        Some("显示系统信息和完整的系统提示词")
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a mut BackendContext<'_>,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(async move {
            let msg = ctx.message.trim();

            // 解析子命令
            if msg == "/system" || msg == "/system status" {
                self.show_full(ctx).await
            } else if msg == "/system prompt" {
                self.show_prompt(ctx).await
            } else if msg == "/system tools" {
                self.show_tools(ctx).await
            } else if msg == "/system stats" {
                self.show_stats(ctx).await
            } else if msg == "/system skills" {
                self.show_skills(ctx).await
            } else if msg == "/system workflows" {
                self.show_workflows(ctx).await
            } else if msg == "/system config" {
                self.show_config(ctx).await
            } else if msg == "/system env" {
                self.show_env(ctx).await
            } else if msg == "/system help" {
                self.show_help(ctx).await
            } else {
                let _ = ctx.event_tx.send(crate::AgentEvent::progress(
                    "用法：/system [status|prompt|tools|stats|skills|workflows|config|env|help]".to_string(),
                    None,
                )).await;
                false
            }
        })
    }
}

impl System {
    /// 显示完整信息
    async fn show_full(&self, ctx: &mut BackendContext<'_>) -> bool {
        let mut output = String::new();

        // 1. 系统基本信息
        output.push_str("🖥️  **系统环境**\n");
        output.push_str(&format!(
            "  • 操作系统: {} ({})\n",
            std::env::consts::OS,
            std::env::consts::ARCH
        ));
        output.push_str(&format!(
            "  • 当前目录: {:?}\n",
            ctx.project_path.unwrap_or(&std::path::PathBuf::new())
        ));
        output.push_str(&format!("  • 当前模型: {}\n\n", ctx.model));

        // 2. Token 使用统计
        let (input_tokens, output_tokens) = ctx.agent.get_token_counts();
        output.push_str("📊 **Token 统计**\n");
        output.push_str(&format!("  • 输入: {} tokens\n", input_tokens));
        output.push_str(&format!("  • 输出: {} tokens\n", output_tokens));
        output.push_str(&format!(
            "  • 总计: {} tokens\n\n",
            input_tokens + output_tokens
        ));

        // 3. 工具列表
        let tools = ctx.agent.get_tools();
        output.push_str(&format!("🔧 **可用工具** (共 {} 个)\n", tools.len()));
        for (idx, tool) in tools.iter().enumerate() {
            if idx < 10 || tools.len() <= 15 {
                output.push_str(&format!("  • {}\n", tool.definition().name));
            } else if idx == 10 {
                output.push_str(&format!("  ... 还有 {} 个工具\n", tools.len() - 10));
                break;
            }
        }
        output.push('\n');

        // 4. 技能列表
        if !ctx.skills.is_empty() {
            output.push_str(&format!("📚 **可用技能** (共 {} 个)\n", ctx.skills.len()));
            for skill in ctx.skills.iter() {
                output.push_str(&format!("  • {}: {}\n", skill.name, skill.description));
            }
            output.push('\n');
        }

        // 5. 工作流列表
        if let Some(project_path) = ctx.project_path {
            let registry = WorkflowRegistry::new(Some(project_path));
            if !registry.is_empty() {
                output.push_str(&format!(
                    "⚡ **可用工作流** (共 {} 个)\n",
                    registry.list().len()
                ));
                for info in registry.list().iter().take(5) {
                    output.push_str(&format!("  • {}: ", info.id));
                    if let Some(ref desc) = info.description {
                        output.push_str(desc);
                    } else {
                        output.push_str(&info.name);
                    }
                    output.push('\n');
                }
                if registry.list().len() > 5 {
                    output.push_str(&format!(
                        "  ... 还有 {} 个工作流\n",
                        registry.list().len() - 5
                    ));
                }
                output.push('\n');
            }
        }

        // 6. 完整 System Prompt
        output.push_str("📜 **完整 System Prompt**\n");
        output.push_str(&format!(
            "长度: {} 字符\n\n",
            ctx.agent.get_system_prompt().len()
        ));
        output.push_str("```\n");
        output.push_str(ctx.agent.get_system_prompt());
        output.push_str("\n```\n");

        let _ = ctx
            .event_tx
            .send(crate::AgentEvent::progress(output, None))
            .await;
        false
    }

    /// 仅显示 System Prompt
    async fn show_prompt(&self, ctx: &mut BackendContext<'_>) -> bool {
        let prompt = ctx.agent.get_system_prompt();
        let mut output = String::new();

        output.push_str("📜 **System Prompt**\n");
        output.push_str(&format!("长度: {} 字符\n\n", prompt.len()));
        output.push_str("```\n");
        output.push_str(prompt);
        output.push_str("\n```\n");

        let _ = ctx
            .event_tx
            .send(crate::AgentEvent::progress(output, None))
            .await;
        false
    }

    /// 仅显示工具列表
    async fn show_tools(&self, ctx: &mut BackendContext<'_>) -> bool {
        let tools = ctx.agent.get_tools();
        let mut output = String::new();

        output.push_str(&format!("🔧 **可用工具** (共 {} 个)\n\n", tools.len()));

        for (idx, tool) in tools.iter().enumerate() {
            let def = tool.definition();
            output.push_str(&format!("{}. **{}**\n", idx + 1, def.name));
            output.push_str(&format!("   {}\n\n", def.description));
        }

        let _ = ctx
            .event_tx
            .send(crate::AgentEvent::progress(output, None))
            .await;
        false
    }

    /// 仅显示统计信息
    async fn show_stats(&self, ctx: &mut BackendContext<'_>) -> bool {
        let mut output = String::new();

        // 系统信息
        output.push_str("🖥️  **系统环境**\n");
        output.push_str(&format!(
            "  • 操作系统: {} ({})\n",
            std::env::consts::OS,
            std::env::consts::ARCH
        ));
        output.push_str(&format!(
            "  • 当前目录: {:?}\n",
            ctx.project_path.unwrap_or(&std::path::PathBuf::new())
        ));
        output.push_str(&format!("  • 当前模型: {}\n\n", ctx.model));

        // Token 统计
        let (input_tokens, output_tokens) = ctx.agent.get_token_counts();
        output.push_str("📊 **Token 统计**\n");
        output.push_str(&format!("  • 输入: {} tokens\n", input_tokens));
        output.push_str(&format!("  • 输出: {} tokens\n", output_tokens));
        output.push_str(&format!(
            "  • 总计: {} tokens\n\n",
            input_tokens + output_tokens
        ));

        // 工具统计
        let tools = ctx.agent.get_tools();
        output.push_str("🔧 **资源统计**\n");
        output.push_str(&format!("  • 可用工具: {} 个\n", tools.len()));
        output.push_str(&format!("  • 可用技能: {} 个\n", ctx.skills.len()));

        // 工作流统计
        if let Some(project_path) = ctx.project_path {
            let registry = WorkflowRegistry::new(Some(project_path));
            output.push_str(&format!("  • 可用工作流: {} 个\n", registry.list().len()));
        } else {
            output.push_str("  • 可用工作流: 0 个\n");
        }

        output.push_str(&format!(
            "  • System Prompt 长度: {} 字符\n",
            ctx.agent.get_system_prompt().len()
        ));

        let _ = ctx
            .event_tx
            .send(crate::AgentEvent::progress(output, None))
            .await;
        false
    }

    /// 仅显示技能列表
    async fn show_skills(&self, ctx: &mut BackendContext<'_>) -> bool {
        let mut output = String::new();

        if ctx.skills.is_empty() {
            output.push_str("📚 **可用技能**: 无\n");
        } else {
            output.push_str(&format!("📚 **可用技能** (共 {} 个)\n\n", ctx.skills.len()));
            for (idx, skill) in ctx.skills.iter().enumerate() {
                output.push_str(&format!("{}. **{}**\n", idx + 1, skill.name));
                output.push_str(&format!("   {}\n\n", skill.description));
            }
        }

        let _ = ctx
            .event_tx
            .send(crate::AgentEvent::progress(output, None))
            .await;
        false
    }

    /// 显示工作流列表
    async fn show_workflows(&self, ctx: &mut BackendContext<'_>) -> bool {
        let mut output = String::new();

        if let Some(project_path) = ctx.project_path {
            let registry = WorkflowRegistry::new(Some(project_path));
            let workflows = registry.list();

            if workflows.is_empty() {
                output.push_str("⚡ **可用工作流**: 无\n");
                output.push_str("\n提示：在项目 .matrix/workflows/ 目录下创建 workflow.yaml 文件来添加自定义工作流。\n");
            } else {
                output.push_str(&format!(
                    "⚡ **可用工作流** (共 {} 个)\n\n",
                    workflows.len()
                ));
                for (idx, info) in workflows.iter().enumerate() {
                    output.push_str(&format!("{}. **{}** (`{}`)\n", idx + 1, info.name, info.id));
                    if let Some(ref desc) = info.description {
                        output.push_str(&format!("   {}\n", desc));
                    }
                    if !info.required_inputs.is_empty() {
                        output.push_str(&format!(
                            "   需要输入: {}\n",
                            info.required_inputs.join(", ")
                        ));
                    }
                    output.push('\n');
                }
                output.push_str(
                    "调用方式: 使用 workflow_run 工具，传入 workflow_id 和可选的 inputs 参数。\n",
                );
            }
        } else {
            output.push_str("⚡ **可用工作流**: 无项目路径\n");
            output.push_str("\n提示：需要在项目目录下才能查看工作流。\n");
        }

        let _ = ctx
            .event_tx
            .send(crate::AgentEvent::progress(output, None))
            .await;
        false
    }

    /// 显示配置信息（隐藏敏感信息）
    async fn show_config(&self, ctx: &mut BackendContext<'_>) -> bool {
        let mut output = String::new();

        output.push_str("⚙️  **配置信息**\n\n");

        // Provider 信息
        if let Some(ref provider) = ctx.config.provider {
            output.push_str(&format!("  • Provider: {}\n", provider));
        } else {
            output.push_str("  • Provider: anthropic (默认)\n");
        }

        // Model 信息
        if let Some(ref model) = ctx.config.model {
            output.push_str(&format!("  • Model: {}\n", model));
        } else {
            output.push_str(&format!("  • Model: {} (默认)\n", ctx.model));
        }

        // Base URL
        if let Some(ref base_url) = ctx.config.base_url {
            output.push_str(&format!("  • Base URL: {}\n", base_url));
        }

        // API Key（隐藏）
        if ctx.config.api_key.is_some() {
            output.push_str("  • API Key: 已设置（已隐藏）\n");
        } else {
            output.push_str("  • API Key: 未设置\n");
        }

        // 其他配置
        output.push_str(&format!("  • Extended Thinking: {}\n", ctx.config.think));
        output.push_str(&format!(
            "  • Markdown Rendering: {}\n",
            ctx.config.markdown
        ));
        output.push_str(&format!("  • Max Tokens: {}\n", ctx.config.max_tokens));

        if let Some(ref context_size) = ctx.config.context_size {
            output.push_str(&format!("  • Context Size: {}\n", context_size));
        }

        // Approve Mode
        if let Some(ref approve_mode) = ctx.config.approve_mode {
            output.push_str(&format!("  • Approve Mode: {}\n", approve_mode));
        }

        // Multi-model
        if let Some(ref multi_model) = ctx.config.multi_model {
            output.push_str(&format!("  • Multi-Model: {}\n", multi_model));
        }

        if let Some(ref plan_model) = ctx.config.plan_model {
            output.push_str(&format!("  • Plan Model: {}\n", plan_model));
        }

        if let Some(ref compress_model) = ctx.config.compress_model {
            output.push_str(&format!("  • Compress Model: {}\n", compress_model));
        }

        if let Some(ref fast_model) = ctx.config.fast_model {
            output.push_str(&format!("  • Fast Model: {}\n", fast_model));
        }

        // MCP Servers
        if let Some(ref mcp_servers) = ctx.config.mcp_servers
            && !mcp_servers.is_empty() {
                output.push_str(&format!("\n  • MCP Servers: {} 个\n", mcp_servers.len()));
                for (name, _config) in mcp_servers.iter().take(5) {
                    output.push_str(&format!("    - {}\n", name));
                }
                if mcp_servers.len() > 5 {
                    output.push_str(&format!("    ... 还有 {} 个\n", mcp_servers.len() - 5));
                }
            }

        // Config 路径
        output.push_str("\n📁 **配置文件路径**\n");
        if let Some(path) = crate::Config::matrix_config_path() {
            output.push_str(&format!("  • Matrix Config: {:?}\n", path));
        }
        if let Some(path) = crate::Config::claude_settings_path() {
            output.push_str(&format!("  • Claude Settings: {:?}\n", path));
        }

        let _ = ctx
            .event_tx
            .send(crate::AgentEvent::progress(output, None))
            .await;
        false
    }

    /// 显示环境变量（隐藏敏感信息）
    async fn show_env(&self, ctx: &mut BackendContext<'_>) -> bool {
        let mut output = String::new();

        output.push_str("🌍 **环境变量**\n\n");

        // API 相关环境变量
        let api_key = std::env::var("API_KEY").ok();
        let anthropic_token = std::env::var("ANTHROPIC_AUTH_TOKEN").ok();
        let base_url = std::env::var("BASE_URL").ok();
        let anthropic_base = std::env::var("ANTHROPIC_BASE_URL").ok();
        let model = std::env::var("MODEL").ok();
        let anthropic_model = std::env::var("ANTHROPIC_MODEL").ok();

        // API Key（隐藏）
        if api_key.is_some() || anthropic_token.is_some() {
            output.push_str("  • API_KEY / ANTHROPIC_AUTH_TOKEN: 已设置（已隐藏）\n");
        } else {
            output.push_str("  • API_KEY / ANTHROPIC_AUTH_TOKEN: 未设置\n");
        }

        // Base URL
        if let Some(url) = base_url {
            output.push_str(&format!("  • BASE_URL: {}\n", url));
        } else if let Some(url) = anthropic_base {
            output.push_str(&format!("  • ANTHROPIC_BASE_URL: {}\n", url));
        } else {
            output.push_str("  • BASE_URL: 未设置（使用默认）\n");
        }

        // Model
        if let Some(m) = model {
            output.push_str(&format!("  • MODEL: {}\n", m));
        } else if let Some(m) = anthropic_model {
            output.push_str(&format!("  • ANTHROPIC_MODEL: {}\n", m));
        } else {
            output.push_str("  • MODEL: 未设置（使用配置或默认）\n");
        }

        // 其他常用环境变量
        let provider = std::env::var("PROVIDER").ok();
        if let Some(p) = provider {
            output.push_str(&format!("  • PROVIDER: {}\n", p));
        }

        let max_tokens = std::env::var("MAX_TOKENS").ok();
        if let Some(t) = max_tokens {
            output.push_str(&format!("  • MAX_TOKENS: {}\n", t));
        }

        let think = std::env::var("THINK").ok();
        if let Some(t) = think {
            output.push_str(&format!("  • THINK: {}\n", t));
        }

        // 系统环境变量
        output.push_str("\n💻 **系统环境**\n");
        if let Ok(home) = std::env::var("HOME") {
            output.push_str(&format!("  • HOME: {}\n", home));
        } else if let Ok(userprofile) = std::env::var("USERPROFILE") {
            output.push_str(&format!("  • USERPROFILE: {}\n", userprofile));
        }

        if let Ok(pwd) = std::env::var("PWD") {
            output.push_str(&format!("  • PWD: {}\n", pwd));
        }

        if let Ok(path) = std::env::var("PATH") {
            // 截断过长的 PATH (safe UTF-8 boundary)
            if path.len() > 100 {
                let truncated: String = path.chars().take(100).collect();
                output.push_str(&format!("  • PATH: {}... (已截断)\n", truncated));
            } else {
                output.push_str(&format!("  • PATH: {}\n", path));
            }
        }

        // 优先级说明
        output.push_str("\n📋 **配置优先级** (从高到低)\n");
        output.push_str("  1. 环境变量 (API_KEY, BASE_URL, MODEL 等)\n");
        output.push_str("  2. ~/.matrix/config.json (MatrixCode 配置)\n");
        output.push_str("  3. ~/.claude/settings.json (Claude Code 配置)\n");
        output.push_str("  4. 默认值\n");

        let _ = ctx
            .event_tx
            .send(crate::AgentEvent::progress(output, None))
            .await;
        false
    }

    /// 显示帮助
    async fn show_help(&self, ctx: &mut BackendContext<'_>) -> bool {
        let help = r#"🖥️  **/system 命令帮助**

用法：
  /system           显示完整系统信息（默认）
  /system status    显示完整系统信息
  /system prompt    仅显示 System Prompt
  /system tools      仅显示工具列表（含描述）
  /system stats      仅显示统计信息
  /system skills     仅显示技能列表
  /system workflows  仅显示工作流列表
  /system config     仅显示配置信息（隐藏敏感信息）
  /system env        仅显示环境变量（隐藏敏感信息）
  /system help       显示此帮助信息

说明：
  - prompt: 查看发送给 LLM 的完整系统提示词
  - tools: 查看所有可用工具及其描述
  - stats: 查看运行时统计（Token、工具数、工作流数等）
  - skills: 查看当前加载的技能
  - workflows: 查看项目中的自动化工作流
  - config: 查看当前配置（API Key 等敏感信息已隐藏）
  - env: 查看环境变量（API Key 等敏感信息已隐藏）

示例：
  /system prompt    # 调试 Prompt 问题
  /system config    # 检查配置是否正确
  /system workflows # 查看可用的工作流
"#;

        let _ = ctx
            .event_tx
            .send(crate::AgentEvent::progress(help.to_string(), None))
            .await;
        false
    }
}
