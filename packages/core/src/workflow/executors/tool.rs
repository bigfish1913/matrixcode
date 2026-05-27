//! Tool Executor
//!
//! 工具调用执行器，调用现有 Tools 执行任务节点。

use anyhow::{Context, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use crate::tools::{Tool, ToolDefinition};
use crate::workflow::context::WorkflowContext;
use crate::workflow::def::NodeDef;
use crate::workflow::template::TemplateRenderer;
use super::node_executor::NodeExecutor;

/// 工具执行器配置
#[derive(Debug, Clone)]
pub struct ToolExecutorConfig {
    /// 是否记录工具调用结果
    pub log_results: bool,
    /// 是否允许工具调用失败
    pub allow_failure: bool,
}

impl Default for ToolExecutorConfig {
    fn default() -> Self {
        Self {
            log_results: true,
            allow_failure: false,
        }
    }
}

/// 工具执行器
///
/// 调用现有的 Tools 系统执行任务节点。
pub struct ToolExecutor {
    /// 工具集合
    tools: HashMap<String, Arc<dyn Tool>>,
    /// 配置
    config: ToolExecutorConfig,
    /// 模板渲染器
    template_renderer: TemplateRenderer,
}

impl ToolExecutor {
    /// 创建新的工具执行器
    pub fn new(tools: Vec<Box<dyn Tool>>) -> Self {
        let mut tool_map = HashMap::new();
        for tool in tools {
            let def = tool.definition();
            tool_map.insert(def.name, Arc::from(tool));
        }
        Self {
            tools: tool_map,
            config: ToolExecutorConfig::default(),
            template_renderer: TemplateRenderer::new(),
        }
    }

    /// 使用配置创建工具执行器
    pub fn with_config(tools: Vec<Box<dyn Tool>>, config: ToolExecutorConfig) -> Self {
        let mut tool_map = HashMap::new();
        for tool in tools {
            let def = tool.definition();
            tool_map.insert(def.name, Arc::from(tool));
        }
        Self {
            tools: tool_map,
            config,
            template_renderer: TemplateRenderer::new(),
        }
    }

    /// 注册单个工具
    pub fn register_tool(&mut self, tool: Box<dyn Tool>) {
        let def = tool.definition();
        self.tools.insert(def.name, Arc::from(tool));
    }

    /// 渲染参数
    pub fn render_params(
        &self,
        params: &HashMap<String, serde_json::Value>,
        context: &WorkflowContext,
    ) -> Result<serde_json::Value> {
        let mut rendered = HashMap::new();
        for (key, value) in params {
            let rendered_value = if let serde_json::Value::String(s) = value {
                let rendered_str = self.template_renderer.render(s, &context.variables)?;
                serde_json::Value::String(rendered_str)
            } else {
                value.clone()
            };
            rendered.insert(key.clone(), rendered_value);
        }
        Ok(serde_json::Value::Object(rendered.into_iter().collect()))
    }

    /// 检查工具是否存在
    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// 获取工具定义
    pub fn get_tool_definition(&self, name: &str) -> Option<ToolDefinition> {
        self.tools.get(name).map(|t| t.definition())
    }

    /// 获取所有工具定义
    pub fn get_all_tool_definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.definition()).collect()
    }
}

#[async_trait]
impl NodeExecutor for ToolExecutor {
    async fn execute(
        &self,
        node: &NodeDef,
        context: &mut WorkflowContext,
    ) -> Result<serde_json::Value> {
        // 获取工具名称
        let tool_name = node.task.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Tool executor requires a task name"))?;

        // 查找工具
        let tool = self.tools.get(tool_name)
            .ok_or_else(|| anyhow::anyhow!("Tool '{}' not found", tool_name))?;

        // 渲染参数
        let params = self.render_params(&node.params, context)?;

        // 执行工具
        let result = tool.execute(params.clone())
            .await
            .with_context(|| format!("Tool '{}' execution failed", tool_name));

        // 处理结果
        match result {
            Ok(output_str) => {
                // 尝试解析为 JSON
                let output = if let Ok(json) = serde_json::from_str::<serde_json::Value>(&output_str) {
                    json
                } else {
                    serde_json::json!({
                        "result": output_str,
                        "tool": tool_name,
                    })
                };

                // 更新上下文
                if let serde_json::Value::Object(map) = &output {
                    for (key, value) in map {
                        context.set_variable(key.clone(), value.clone());
                    }
                }

                Ok(output)
            }
            Err(e) => {
                if self.config.allow_failure {
                    Ok(serde_json::json!({
                        "error": e.to_string(),
                        "tool": tool_name,
                        "success": false,
                    }))
                } else {
                    Err(e)
                }
            }
        }
    }

    fn name(&self) -> &str {
        "tool_executor"
    }
}