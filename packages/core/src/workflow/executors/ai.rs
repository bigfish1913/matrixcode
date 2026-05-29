//! AI Executor
//!
//! AI 模型调用执行器，调用 Provider 执行任务节点。

use anyhow::{Context, Result};
use async_trait::async_trait;
use std::sync::Arc;

use super::node_executor::NodeExecutor;
use crate::providers::{
    ChatRequest, ChatResponse, ContentBlock, Message, MessageContent, Provider,
};
use crate::workflow::context::WorkflowContext;
use crate::workflow::def::NodeDef;
use crate::workflow::template::TemplateRenderer;

/// AI 执行器配置
#[derive(Debug, Clone)]
pub struct AiExecutorConfig {
    /// 系统提示模板
    pub system_prompt: Option<String>,
    /// 最大输出 token 数
    pub max_tokens: u32,
    /// 是否启用思考模式
    pub enable_thinking: bool,
    /// 是否启用流式输出
    pub enable_streaming: bool,
}

impl Default for AiExecutorConfig {
    fn default() -> Self {
        Self {
            system_prompt: None,
            max_tokens: 4096,
            enable_thinking: false,
            enable_streaming: false,
        }
    }
}

/// AI 执行器
///
/// 调用 AI Provider 执行任务节点。
pub struct AiExecutor {
    /// Provider 实例
    provider: Arc<dyn Provider>,
    /// 配置
    config: AiExecutorConfig,
    /// 模板渲染器
    template_renderer: TemplateRenderer,
}

impl AiExecutor {
    /// 创建新的 AI 执行器
    pub fn new(provider: Arc<dyn Provider>) -> Self {
        Self {
            provider,
            config: AiExecutorConfig::default(),
            template_renderer: TemplateRenderer::new(),
        }
    }

    /// 使用配置创建 AI 执行器
    pub fn with_config(provider: Arc<dyn Provider>, config: AiExecutorConfig) -> Self {
        Self {
            provider,
            config,
            template_renderer: TemplateRenderer::new(),
        }
    }

    /// 从响应中提取文本内容
    pub fn extract_text_content(response: &ChatResponse) -> Result<String> {
        let mut text_parts = Vec::new();
        for block in &response.content {
            match block {
                ContentBlock::Text { text } => {
                    text_parts.push(text.clone());
                }
                ContentBlock::Thinking { thinking, .. } => {
                    text_parts.push(format!("[Thinking]\n{}", thinking));
                }
                _ => {}
            }
        }
        Ok(text_parts.join("\n"))
    }

    /// 从响应中提取结构化输出
    pub fn extract_structured_output(response: &ChatResponse) -> Result<serde_json::Value> {
        // 尝试从文本中解析 JSON
        for block in &response.content {
            if let ContentBlock::Text { text } = block {
                // 尝试解析为 JSON
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(text) {
                    return Ok(json);
                }
            }
        }

        // 如果没有找到 JSON，返回文本内容
        let text = Self::extract_text_content(response)?;
        let stop_reason_str = match response.stop_reason {
            crate::providers::StopReason::EndTurn => "end_turn",
            crate::providers::StopReason::ToolUse => "tool_use",
            crate::providers::StopReason::MaxTokens => "max_tokens",
        };
        Ok(serde_json::json!({
            "text": text,
            "stop_reason": stop_reason_str,
            "usage": {
                "input_tokens": response.usage.input_tokens,
                "output_tokens": response.usage.output_tokens,
            }
        }))
    }
}

#[async_trait]
impl NodeExecutor for AiExecutor {
    async fn execute(
        &self,
        node: &NodeDef,
        context: &mut WorkflowContext,
    ) -> Result<serde_json::Value> {
        // 获取任务名称
        let task_name = node
            .task
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("AI executor requires a task name"))?;

        // 构建用户消息
        let mut prompt_parts = Vec::new();

        // 添加任务名称
        prompt_parts.push(format!("Task: {}", task_name));

        // 添加任务描述
        if let Some(desc) = &node.description {
            prompt_parts.push(format!("Description: {}", desc));
        }

        // 渲染并添加参数
        for (key, value) in &node.params {
            let rendered_value = if let serde_json::Value::String(s) = value {
                self.template_renderer.render(s, &context.variables)?
            } else {
                value.to_string()
            };
            prompt_parts.push(format!("{}: {}", key, rendered_value));
        }

        // 添加上下文信息
        if !context.variables.is_empty() {
            prompt_parts.push("\nContext:".to_string());
            for (key, value) in &context.variables {
                prompt_parts.push(format!("  {}: {}", key, value));
            }
        }

        let user_message = prompt_parts.join("\n");

        // 构建聊天请求
        let messages = vec![Message {
            role: crate::providers::Role::User,
            content: MessageContent::Text(user_message),
        }];

        let request = ChatRequest {
            messages,
            tools: Vec::new(),
            system: self.config.system_prompt.clone(),
            think: self.config.enable_thinking,
            max_tokens: self.config.max_tokens,
            server_tools: Vec::new(),
            enable_caching: false,
        };

        // 调用 Provider
        let response = self
            .provider
            .chat(request)
            .await
            .with_context(|| format!("AI executor failed for task '{}'", task_name))?;

        // 提取输出
        let output = Self::extract_structured_output(&response)?;

        // 更新上下文
        let output_ref = &output;
        if let serde_json::Value::Object(map) = output_ref {
            for (key, value) in map {
                context.set_variable(key.clone(), value.clone());
            }
        }

        Ok(output)
    }

    fn name(&self) -> &str {
        "ai_executor"
    }
}
