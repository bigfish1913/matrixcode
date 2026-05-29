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

mod ai;
mod composite;
mod condition;
mod factory;
mod node_executor;
mod proxy;
mod tool;
mod validate;

#[cfg(test)]
mod tests;

// 重新导出所有公共类型
pub use ai::{AiExecutor, AiExecutorConfig};
pub use composite::{CompositeExecutor, CompositeMode};
pub use condition::ConditionExecutor;
pub use factory::ExecutorFactory;
pub use node_executor::NodeExecutor;
pub use proxy::ProxyExecutor;
pub use tool::{ToolExecutor, ToolExecutorConfig};
pub use validate::{ValidateExecutor, ValidateExecutorConfig};
