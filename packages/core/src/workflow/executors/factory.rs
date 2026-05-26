//! Executor Factory
//!
//! 用于创建和管理各种执行器实例。

use anyhow::Result;
use std::sync::Arc;

use crate::providers::Provider;
use super::ai::{AiExecutor, AiExecutorConfig};
use super::condition::ConditionExecutor;
use super::node_executor::NodeExecutor;
use super::tool::ToolExecutor;
use super::validate::{ValidateExecutor, ValidateExecutorConfig};

/// 执行器工厂
///
/// 用于创建和管理各种执行器实例。
pub struct ExecutorFactory {
    /// Provider 实例（用于 AI 执行器）
    pub provider: Option<Arc<dyn Provider>>,
    /// 工具名称列表（用于延迟创建工具执行器）
    pub tool_names: Vec<String>,
}

impl ExecutorFactory {
    /// 创建新的执行器工厂
    pub fn new() -> Self {
        Self {
            provider: None,
            tool_names: Vec::new(),
        }
    }

    /// 设置 Provider
    pub fn with_provider(mut self, provider: Arc<dyn Provider>) -> Self {
        self.provider = Some(provider);
        self
    }

    /// 设置工具名称列表
    pub fn with_tool_names(mut self, names: Vec<String>) -> Self {
        self.tool_names = names;
        self
    }

    /// 创建 AI 执行器
    pub fn create_ai_executor(&self) -> Result<Arc<dyn NodeExecutor>> {
        let provider = self.provider.clone()
            .ok_or_else(|| anyhow::anyhow!("Provider not configured for AI executor"))?;
        Ok(Arc::new(AiExecutor::new(provider)))
    }

    /// 创建带配置的 AI 执行器
    pub fn create_ai_executor_with_config(&self, config: AiExecutorConfig) -> Result<Arc<dyn NodeExecutor>> {
        let provider = self.provider.clone()
            .ok_or_else(|| anyhow::anyhow!("Provider not configured for AI executor"))?;
        Ok(Arc::new(AiExecutor::with_config(provider, config)))
    }

    /// 创建工具执行器（使用默认工具集）
    pub fn create_tool_executor(&self) -> Arc<dyn NodeExecutor> {
        let tools = if let Some(provider) = &self.provider {
            crate::tools::all_tools_with_provider(
                std::sync::Arc::new(Vec::new()),
                provider.clone()
            )
        } else {
            crate::tools::all_tools()
        };
        Arc::new(ToolExecutor::new(tools))
    }

    /// 创建条件执行器
    pub fn create_condition_executor(&self) -> Arc<dyn NodeExecutor> {
        Arc::new(ConditionExecutor::new())
    }

    /// 创建验证执行器
    pub fn create_validate_executor(&self) -> Arc<dyn NodeExecutor> {
        Arc::new(ValidateExecutor::new())
    }

    /// 创建带 AI 的验证执行器
    pub fn create_validate_executor_with_ai(&self, config: ValidateExecutorConfig) -> Result<Arc<dyn NodeExecutor>> {
        let provider = self.provider.clone()
            .ok_or_else(|| anyhow::anyhow!("Provider not configured for AI validation"))?;
        Ok(Arc::new(ValidateExecutor::with_ai(provider, config)))
    }

    /// 根据任务类型创建执行器
    pub fn create_executor_for_task(&self, task_type: &str) -> Result<Arc<dyn NodeExecutor>> {
        match task_type {
            "ai" | "claude" | "gpt" => self.create_ai_executor(),
            "tool" | "bash" | "read" | "write" | "edit" => Ok(self.create_tool_executor()),
            "condition" | "branch" => Ok(self.create_condition_executor()),
            "validate" | "check" => Ok(self.create_validate_executor()),
            _ => Err(anyhow::anyhow!("Unknown task type: {}", task_type)),
        }
    }
}

impl Default for ExecutorFactory {
    fn default() -> Self {
        Self::new()
    }
}