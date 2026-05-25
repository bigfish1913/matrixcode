//! Workflow Definition Structures
//!
//! 定义工作流的核心数据结构，包括节点、边、类型和失败策略。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 节点类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    /// 开始节点
    Start,
    /// 结束节点
    End,
    /// 任务节点
    Task,
    /// 条件分支节点
    Condition,
    /// 并行节点
    Parallel,
    /// 子工作流节点
    SubWorkflow,
    /// 等待节点
    Wait,
    /// 人工审批节点
    Approval,
}

/// 失败策略类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureStrategyType {
    Retry,
    Ignore,
    Abort,
    Goto,
}

/// 失败策略配置（用于 YAML 解析）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureStrategyConfig {
    /// 策略类型
    #[serde(rename = "type", default = "default_failure_strategy_type")]
    pub strategy_type: FailureStrategyType,
    /// 最大重试次数（仅 retry）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_attempts: Option<u32>,
    /// 重试间隔（毫秒，仅 retry）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval_ms: Option<u64>,
    /// 目标节点ID（仅 goto）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

fn default_failure_strategy_type() -> FailureStrategyType {
    FailureStrategyType::Abort
}

impl From<FailureStrategyConfig> for FailureStrategy {
    fn from(config: FailureStrategyConfig) -> Self {
        match config.strategy_type {
            FailureStrategyType::Retry => FailureStrategy::Retry {
                max_attempts: config.max_attempts.unwrap_or(1),
                interval_ms: config.interval_ms,
            },
            FailureStrategyType::Ignore => FailureStrategy::Ignore,
            FailureStrategyType::Abort => FailureStrategy::Abort,
            FailureStrategyType::Goto => FailureStrategy::Goto {
                target: config.target.unwrap_or_default(),
            },
        }
    }
}

impl From<FailureStrategy> for FailureStrategyConfig {
    fn from(strategy: FailureStrategy) -> Self {
        match strategy {
            FailureStrategy::Retry { max_attempts, interval_ms } => FailureStrategyConfig {
                strategy_type: FailureStrategyType::Retry,
                max_attempts: Some(max_attempts),
                interval_ms,
                target: None,
            },
            FailureStrategy::Ignore => FailureStrategyConfig {
                strategy_type: FailureStrategyType::Ignore,
                max_attempts: None,
                interval_ms: None,
                target: None,
            },
            FailureStrategy::Abort => FailureStrategyConfig {
                strategy_type: FailureStrategyType::Abort,
                max_attempts: None,
                interval_ms: None,
                target: None,
            },
            FailureStrategy::Goto { target } => FailureStrategyConfig {
                strategy_type: FailureStrategyType::Goto,
                max_attempts: None,
                interval_ms: None,
                target: Some(target),
            },
        }
    }
}

/// 失败策略
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailureStrategy {
    /// 重试
    Retry {
        /// 最大重试次数
        max_attempts: u32,
        /// 重试间隔（毫秒）
        interval_ms: Option<u64>,
    },
    /// 忽略继续
    Ignore,
    /// 终止工作流
    Abort,
    /// 跳转到指定节点
    Goto {
        /// 目标节点ID
        target: String,
    },
}

impl Default for FailureStrategy {
    fn default() -> Self {
        FailureStrategy::Abort
    }
}

impl Serialize for FailureStrategy {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let config: FailureStrategyConfig = self.clone().into();
        config.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for FailureStrategy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let config: FailureStrategyConfig = FailureStrategyConfig::deserialize(deserializer)?;
        Ok(config.into())
    }
}

/// 边定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeDef {
    /// 边ID
    #[serde(default = "generate_edge_id")]
    pub id: String,
    /// 源节点ID
    pub from: String,
    /// 目标节点ID
    pub to: String,
    /// 条件表达式（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    /// 边标签
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

fn generate_edge_id() -> String {
    format!("edge_{}", uuid::Uuid::new_v4())
}

/// 节点定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeDef {
    /// 节点ID
    pub id: String,
    /// 节点类型
    #[serde(rename = "type")]
    pub node_type: NodeType,
    /// 节点名称
    pub name: String,
    /// 节点描述
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// 任务名称（仅任务节点）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// 任务参数
    #[serde(default)]
    pub params: HashMap<String, serde_json::Value>,
    /// 失败策略
    #[serde(default)]
    pub on_failure: FailureStrategy,
    /// 超时时间（毫秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    /// 条件分支（仅条件节点）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branches: Option<Vec<BranchDef>>,
    /// 并行分支（仅并行节点）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_branches: Option<Vec<ParallelBranchDef>>,
    /// 子工作流名称（仅子工作流节点）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow: Option<String>,
    /// 等待时间（毫秒，仅等待节点）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_ms: Option<u64>,
    /// 审批人列表（仅审批节点）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approvers: Option<Vec<String>>,
}

/// 条件分支定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchDef {
    /// 分支名称
    pub name: String,
    /// 条件表达式
    pub condition: String,
    /// 目标节点ID
    pub target: String,
}

/// 并行分支定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelBranchDef {
    /// 分支名称
    pub name: String,
    /// 分支节点列表
    pub nodes: Vec<NodeDef>,
}

/// 工作流定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDef {
    /// 工作流ID
    pub id: String,
    /// 工作流名称
    pub name: String,
    /// 版本
    #[serde(default = "default_version")]
    pub version: String,
    /// 描述
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// 输入参数定义
    #[serde(default)]
    pub inputs: Vec<InputDef>,
    /// 输出参数定义
    #[serde(default)]
    pub outputs: Vec<OutputDef>,
    /// 节点列表
    pub nodes: Vec<NodeDef>,
    /// 边列表
    #[serde(default)]
    pub edges: Vec<EdgeDef>,
    /// 全局变量
    #[serde(default)]
    pub variables: HashMap<String, serde_json::Value>,
    /// 默认失败策略
    #[serde(default)]
    pub default_failure_strategy: FailureStrategy,
    /// 超时时间（毫秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

/// 输入参数定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputDef {
    /// 参数名
    pub name: String,
    /// 参数类型
    #[serde(rename = "type", default = "default_input_type")]
    pub input_type: String,
    /// 是否必填
    #[serde(default)]
    pub required: bool,
    /// 默认值
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    /// 描述
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

fn default_input_type() -> String {
    "string".to_string()
}

/// 输出参数定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputDef {
    /// 参数名
    pub name: String,
    /// 值表达式
    pub value: String,
    /// 描述
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl WorkflowDef {
    /// 根据ID查找节点
    pub fn get_node(&self, id: &str) -> Option<&NodeDef> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// 获取开始节点
    pub fn get_start_node(&self) -> Option<&NodeDef> {
        self.nodes.iter().find(|n| n.node_type == NodeType::Start)
    }

    /// 获取结束节点
    pub fn get_end_node(&self) -> Option<&NodeDef> {
        self.nodes.iter().find(|n| n.node_type == NodeType::End)
    }

    /// 获取从指定节点出发的边
    pub fn get_outgoing_edges(&self, node_id: &str) -> Vec<&EdgeDef> {
        self.edges.iter().filter(|e| e.from == node_id).collect()
    }

    /// 验证工作流定义
    pub fn validate(&self) -> anyhow::Result<()> {
        // 检查必须有开始节点
        if self.get_start_node().is_none() {
            anyhow::bail!("Workflow must have a start node");
        }

        // 检查必须有结束节点
        if self.get_end_node().is_none() {
            anyhow::bail!("Workflow must have an end node");
        }

        // 检查节点ID唯一性
        let mut node_ids = std::collections::HashSet::new();
        for node in &self.nodes {
            if !node_ids.insert(&node.id) {
                anyhow::bail!("Duplicate node id: {}", node.id);
            }
        }

        // 检查边引用的节点是否存在
        for edge in &self.edges {
            if !node_ids.contains(&edge.from) {
                anyhow::bail!("Edge references unknown source node: {}", edge.from);
            }
            if !node_ids.contains(&edge.to) {
                anyhow::bail!("Edge references unknown target node: {}", edge.to);
            }
        }

        // 检查必填输入参数
        for input in &self.inputs {
            if input.required && input.default.is_none() {
                // 必填参数没有默认值，需要在运行时提供
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_def_validation() {
        let workflow = WorkflowDef {
            id: "test-workflow".to_string(),
            name: "Test Workflow".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            inputs: vec![],
            outputs: vec![],
            nodes: vec![
                NodeDef {
                    id: "start".to_string(),
                    node_type: NodeType::Start,
                    name: "Start".to_string(),
                    description: None,
                    task: None,
                    params: HashMap::new(),
                    on_failure: FailureStrategy::default(),
                    timeout_ms: None,
                    branches: None,
                    parallel_branches: None,
                    workflow: None,
                    wait_ms: None,
                    approvers: None,
                },
                NodeDef {
                    id: "end".to_string(),
                    node_type: NodeType::End,
                    name: "End".to_string(),
                    description: None,
                    task: None,
                    params: HashMap::new(),
                    on_failure: FailureStrategy::default(),
                    timeout_ms: None,
                    branches: None,
                    parallel_branches: None,
                    workflow: None,
                    wait_ms: None,
                    approvers: None,
                },
            ],
            edges: vec![EdgeDef {
                id: "e1".to_string(),
                from: "start".to_string(),
                to: "end".to_string(),
                condition: None,
                label: None,
            }],
            variables: HashMap::new(),
            default_failure_strategy: FailureStrategy::default(),
            timeout_ms: None,
        };

        assert!(workflow.validate().is_ok());
    }
}