//! Composite Executor
//!
//! 组合执行器，可以组合多个执行器按顺序或条件执行。

use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

use super::node_executor::NodeExecutor;
use crate::workflow::context::WorkflowContext;
use crate::workflow::def::NodeDef;

/// 组合执行模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositeMode {
    /// 按顺序执行所有
    Sequential,
    /// 执行第一个成功的
    FirstSuccess,
    /// 并行执行（需要 tokio）
    Parallel,
}

/// 组合执行器
///
/// 可以组合多个执行器，按顺序或条件执行。
pub struct CompositeExecutor {
    /// 子执行器列表
    pub executors: Vec<Arc<dyn NodeExecutor>>,
    /// 执行模式
    mode: CompositeMode,
}

impl CompositeExecutor {
    /// 创建新的组合执行器
    pub fn new(executors: Vec<Arc<dyn NodeExecutor>>, mode: CompositeMode) -> Self {
        Self { executors, mode }
    }

    /// 添加执行器
    pub fn add_executor(&mut self, executor: Arc<dyn NodeExecutor>) {
        self.executors.push(executor);
    }
}

#[async_trait]
impl NodeExecutor for CompositeExecutor {
    async fn execute(
        &self,
        node: &NodeDef,
        context: &mut WorkflowContext,
    ) -> Result<serde_json::Value> {
        match self.mode {
            CompositeMode::Sequential => {
                let mut outputs = Vec::new();
                for executor in &self.executors {
                    let output = executor.execute(node, context).await?;
                    outputs.push(output);
                }
                Ok(serde_json::Value::Array(outputs))
            }
            CompositeMode::FirstSuccess => {
                for executor in &self.executors {
                    if let Ok(output) = executor.execute(node, context).await {
                        return Ok(output);
                    }
                }
                Err(anyhow::anyhow!("All executors failed"))
            }
            CompositeMode::Parallel => {
                // 并行执行需要更复杂的实现
                // 这里简化为顺序执行
                let mut outputs = Vec::new();
                for executor in &self.executors {
                    let output = executor.execute(node, context).await?;
                    outputs.push(output);
                }
                Ok(serde_json::Value::Array(outputs))
            }
        }
    }

    fn name(&self) -> &str {
        "composite_executor"
    }
}
