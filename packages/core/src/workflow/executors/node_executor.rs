//! Node Executor trait definition

use anyhow::Result;
use async_trait::async_trait;

use crate::workflow::context::WorkflowContext;
use crate::workflow::def::NodeDef;

/// NodeExecutor trait - 节点执行器接口
///
/// 所有节点执行器必须实现此接口，支持异步执行和错误处理。
#[async_trait]
pub trait NodeExecutor: Send + Sync {
    /// 执行节点
    async fn execute(
        &self,
        node: &NodeDef,
        context: &mut WorkflowContext,
    ) -> Result<serde_json::Value>;

    /// 执行器名称（用于日志和调试）
    fn name(&self) -> &str;
}
