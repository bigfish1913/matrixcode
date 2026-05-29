//! Workflow Context and Runtime State
//!
//! 管理工作流的运行时状态和上下文数据。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 工作流状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum WorkflowStatus {
    /// 待执行
    #[default]
    Pending,
    /// 运行中
    Running,
    /// 已暂停
    Paused,
    /// 已完成
    Completed,
    /// 已失败
    Failed,
    /// 已取消
    Cancelled,
}

/// 节点状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum NodeStatus {
    /// 待执行
    #[default]
    Pending,
    /// 运行中
    Running,
    /// 已完成
    Completed,
    /// 已失败
    Failed,
    /// 已跳过
    Skipped,
}

/// 节点执行记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeExecution {
    /// 节点ID
    pub node_id: String,
    /// 执行状态
    pub status: NodeStatus,
    /// 开始时间
    pub started_at: Option<DateTime<Utc>>,
    /// 结束时间
    pub finished_at: Option<DateTime<Utc>>,
    /// 重试次数
    pub retry_count: u32,
    /// 错误信息
    pub error: Option<String>,
    /// 输出数据
    pub output: Option<serde_json::Value>,
}

impl NodeExecution {
    pub fn new(node_id: String) -> Self {
        Self {
            node_id,
            status: NodeStatus::Pending,
            started_at: None,
            finished_at: None,
            retry_count: 0,
            error: None,
            output: None,
        }
    }

    pub fn start(&mut self) {
        self.status = NodeStatus::Running;
        self.started_at = Some(Utc::now());
    }

    pub fn complete(&mut self, output: Option<serde_json::Value>) {
        self.status = NodeStatus::Completed;
        self.finished_at = Some(Utc::now());
        self.output = output;
    }

    pub fn fail(&mut self, error: String) {
        self.status = NodeStatus::Failed;
        self.finished_at = Some(Utc::now());
        self.error = Some(error);
    }

    pub fn skip(&mut self) {
        self.status = NodeStatus::Skipped;
        self.finished_at = Some(Utc::now());
    }

    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }

    /// 获取执行时长（毫秒）
    pub fn duration_ms(&self) -> Option<i64> {
        match (self.started_at, self.finished_at) {
            (Some(start), Some(end)) => Some((end - start).num_milliseconds()),
            _ => None,
        }
    }
}

/// 工作流上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowContext {
    /// 工作流实例ID
    pub instance_id: String,
    /// 工作流定义ID
    pub workflow_id: String,
    /// 当前状态
    pub status: WorkflowStatus,
    /// 当前节点ID
    pub current_node_id: Option<String>,
    /// 输入参数
    pub inputs: HashMap<String, serde_json::Value>,
    /// 变量存储
    pub variables: HashMap<String, serde_json::Value>,
    /// 节点执行记录
    pub node_executions: HashMap<String, NodeExecution>,
    /// 执行历史（节点ID顺序）
    pub execution_path: Vec<String>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
    /// 开始时间
    pub started_at: Option<DateTime<Utc>>,
    /// 结束时间
    pub finished_at: Option<DateTime<Utc>>,
    /// 错误信息
    pub error: Option<String>,
}

impl WorkflowContext {
    /// 创建新的工作流上下文
    pub fn new(workflow_id: String, inputs: HashMap<String, serde_json::Value>) -> Self {
        let now = Utc::now();
        Self {
            instance_id: format!("inst_{}", uuid::Uuid::new_v4()),
            workflow_id,
            status: WorkflowStatus::Pending,
            current_node_id: None,
            inputs,
            variables: HashMap::new(),
            node_executions: HashMap::new(),
            execution_path: Vec::new(),
            created_at: now,
            updated_at: now,
            started_at: None,
            finished_at: None,
            error: None,
        }
    }

    /// 设置变量
    pub fn set_variable(&mut self, key: String, value: serde_json::Value) {
        self.variables.insert(key, value);
        self.updated_at = Utc::now();
    }

    /// 获取变量
    pub fn get_variable(&self, key: &str) -> Option<&serde_json::Value> {
        self.variables.get(key)
    }

    /// 设置输入参数
    pub fn set_input(&mut self, key: String, value: serde_json::Value) {
        self.inputs.insert(key, value);
        self.updated_at = Utc::now();
    }

    /// 获取输入参数
    pub fn get_input(&self, key: &str) -> Option<&serde_json::Value> {
        self.inputs.get(key)
    }

    /// 开始工作流
    pub fn start(&mut self) {
        self.status = WorkflowStatus::Running;
        self.started_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    /// 完成工作流
    pub fn complete(&mut self) {
        self.status = WorkflowStatus::Completed;
        self.finished_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    /// 失败工作流
    pub fn fail(&mut self, error: String) {
        self.status = WorkflowStatus::Failed;
        self.error = Some(error);
        self.finished_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    /// 取消工作流
    pub fn cancel(&mut self) {
        self.status = WorkflowStatus::Cancelled;
        self.finished_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    /// 暂停工作流
    pub fn pause(&mut self) {
        self.status = WorkflowStatus::Paused;
        self.updated_at = Utc::now();
    }

    /// 恢复工作流
    pub fn resume(&mut self) {
        self.status = WorkflowStatus::Running;
        self.updated_at = Utc::now();
    }

    /// 设置当前节点
    pub fn set_current_node(&mut self, node_id: String) {
        self.current_node_id = Some(node_id.clone());
        self.execution_path.push(node_id);
        self.updated_at = Utc::now();
    }

    /// 获取或创建节点执行记录
    pub fn get_or_create_node_execution(&mut self, node_id: &str) -> &mut NodeExecution {
        self.node_executions
            .entry(node_id.to_string())
            .or_insert_with(|| NodeExecution::new(node_id.to_string()))
    }

    /// 获取节点执行记录
    pub fn get_node_execution(&self, node_id: &str) -> Option<&NodeExecution> {
        self.node_executions.get(node_id)
    }

    /// 获取总执行时长（毫秒）
    pub fn total_duration_ms(&self) -> Option<i64> {
        match (self.started_at, self.finished_at) {
            (Some(start), Some(end)) => Some((end - start).num_milliseconds()),
            _ => None,
        }
    }

    /// 检查是否可以继续执行
    pub fn can_continue(&self) -> bool {
        matches!(
            self.status,
            WorkflowStatus::Pending | WorkflowStatus::Running
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_context_new() {
        let ctx = WorkflowContext::new("test-workflow".to_string(), HashMap::new());
        assert_eq!(ctx.status, WorkflowStatus::Pending);
        assert!(ctx.current_node_id.is_none());
    }

    #[test]
    fn test_workflow_context_lifecycle() {
        let mut ctx = WorkflowContext::new("test-workflow".to_string(), HashMap::new());

        ctx.start();
        assert_eq!(ctx.status, WorkflowStatus::Running);

        ctx.set_current_node("node1".to_string());
        assert_eq!(ctx.current_node_id, Some("node1".to_string()));

        ctx.complete();
        assert_eq!(ctx.status, WorkflowStatus::Completed);
        assert!(ctx.finished_at.is_some());
    }

    #[test]
    fn test_node_execution() {
        let mut exec = NodeExecution::new("node1".to_string());
        assert_eq!(exec.status, NodeStatus::Pending);

        exec.start();
        assert_eq!(exec.status, NodeStatus::Running);

        exec.complete(Some(serde_json::json!({"result": "ok"})));
        assert_eq!(exec.status, NodeStatus::Completed);
        assert!(exec.output.is_some());
    }

    #[test]
    fn test_pause_resume() {
        let mut ctx = WorkflowContext::new("test-workflow".to_string(), HashMap::new());

        ctx.start();
        assert_eq!(ctx.status, WorkflowStatus::Running);
        assert!(ctx.can_continue());

        ctx.pause();
        assert_eq!(ctx.status, WorkflowStatus::Paused);
        assert!(!ctx.can_continue());

        ctx.resume();
        assert_eq!(ctx.status, WorkflowStatus::Running);
        assert!(ctx.can_continue());
    }
}
