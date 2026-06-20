//! Workflow Executors
//!
//! 工作流节点执行器，提供不同类型节点的执行实现。
//!
//! # 执行器类型
//!
//! - `AiExecutor`: AI 模型调用执行器（调用 Provider）
//! - `ToolExecutor`: 工具调用执行器（调用现有 Tools）
//! - `ProxyExecutor`: 代理工具执行器（调用 ProxyToolExecutor）
//! - `ConditionExecutor`: 条件判断执行器（调用 rule_engine）
//! - `ValidateExecutor`: 混合验证执行器（程序规则 + AI 验证）

mod node_executor;
mod ai;
mod tool;
mod proxy;
mod condition;
mod validate;
mod composite;
mod factory;

#[cfg(test)]
mod tests;

// 重新导出所有公共类型
pub use node_executor::NodeExecutor;
pub use ai::{AiExecutor, AiExecutorConfig};
pub use tool::{ToolExecutor, ToolExecutorConfig};
pub use proxy::ProxyExecutor;
pub use condition::ConditionExecutor;
pub use validate::{ValidateExecutor, ValidateExecutorConfig};
pub use composite::{CompositeExecutor, CompositeMode};
pub use factory::ExecutorFactory;