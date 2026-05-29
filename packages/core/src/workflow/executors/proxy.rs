//! Proxy Executor
//!
//! 代理工具执行器，包装 ProxyToolExecutor 为 NodeExecutor。

use anyhow::{Context, Result};
use async_trait::async_trait;
use std::sync::Arc;

use super::node_executor::NodeExecutor;
use crate::tools::toolproxy::{ProxyToolDef, ProxyToolExecutor};
use crate::workflow::context::WorkflowContext;
use crate::workflow::def::NodeDef;
use crate::workflow::template::TemplateRenderer;

/// 代理工具执行器
///
/// 包装 ProxyToolExecutor 为 NodeExecutor，让 workflow 可以调用代理工具。
pub struct ProxyExecutor {
    /// 代理工具执行器
    executor: Arc<dyn ProxyToolExecutor>,
    /// 工具定义列表
    tool_defs: Vec<ProxyToolDef>,
    /// 模板渲染器
    template_renderer: TemplateRenderer,
}

impl ProxyExecutor {
    /// 创建新的代理执行器
    pub fn new(executor: Arc<dyn ProxyToolExecutor>, tool_defs: Vec<ProxyToolDef>) -> Self {
        Self {
            executor,
            tool_defs,
            template_renderer: TemplateRenderer::new(),
        }
    }

    /// 检查是否支持该工具
    pub fn has_tool(&self, name: &str) -> bool {
        self.tool_defs.iter().any(|t| t.definition.name == name)
    }

    /// 获取工具超时时间
    pub fn get_timeout(&self, name: &str) -> u64 {
        self.tool_defs
            .iter()
            .find(|t| t.definition.name == name)
            .map(|t| t.timeout_ms)
            .unwrap_or(30000)
    }
}

#[async_trait]
impl NodeExecutor for ProxyExecutor {
    async fn execute(
        &self,
        node: &NodeDef,
        context: &mut WorkflowContext,
    ) -> Result<serde_json::Value> {
        // 获取工具名称
        let tool_name = node
            .task
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Proxy executor requires a task name"))?;

        // 检查工具是否存在
        if !self.has_tool(tool_name) {
            return Err(anyhow::anyhow!("Proxy tool '{}' not found", tool_name));
        }

        // 渲染参数
        let params = self
            .template_renderer
            .render_params(&node.params, &context.variables)?;

        // 执行代理工具
        let result = self
            .executor
            .exec(tool_name, params.clone())
            .await
            .with_context(|| format!("Proxy tool '{}' execution failed", tool_name))?;

        // 解析结果
        let output = if let Ok(json) = serde_json::from_str::<serde_json::Value>(&result) {
            json
        } else {
            serde_json::json!({
                "result": result,
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

    fn name(&self) -> &str {
        "proxy_executor"
    }
}
