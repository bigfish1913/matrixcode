//! Workflow Definition Structures
//!
//! 定义工作流的核心数据结构，包括节点、边、类型和失败策略。
//!
//! ## Execution Modes (Pipeline vs Parallel)
//!
//! **Pipeline** (流式处理，无屏障):
//! - 任务完成后立即流转到下一阶段
//! - 不等待其他任务完成
//! - Wall-clock = 最慢的单任务链
//! - 适用场景：批量文件处理、流式数据转换
//!
//! **Parallel** (并行执行，有屏障):
//! - 所有任务并行启动
//! - 必须等待全部完成才能继续
//! - 适用场景：多维度审查、跨源数据收集、结果汇总

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
    /// 流式节点 (无屏障等待)
    Pipeline,
    /// 子工作流节点
    SubWorkflow,
    /// 等待节点
    Wait,
    /// 人工审批节点
    Approval,
}

/// 执行模式 - 控制并行分支的执行策略
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    /// Pipeline: 流式处理，无屏障等待
    /// 任务 A 完成后立即流转到下一阶段，不等待其他任务
    /// Wall-clock = 最慢的单任务链
    /// 适用场景：批量文件处理、流式数据转换
    Pipeline,

    /// Parallel: 并行执行，有屏障等待 (默认)
    /// 所有任务必须全部完成才能继续下一阶段
    /// 适用场景：多维度审查、跨源数据收集、结果汇总
    #[default]
    Parallel,
}

impl ExecutionMode {
    /// 是否需要屏障等待
    pub fn has_barrier(&self) -> bool {
        match self {
            Self::Pipeline => false,
            Self::Parallel => true,
        }
    }

    /// 获取显示名称
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Pipeline => "流式",
            Self::Parallel => "并行",
        }
    }

    /// 获取描述
    pub fn description(&self) -> &'static str {
        match self {
            Self::Pipeline => "任务流转执行，不等待其他任务",
            Self::Parallel => "所有任务并行执行，等待全部完成",
        }
    }
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
            FailureStrategy::Retry {
                max_attempts,
                interval_ms,
            } => FailureStrategyConfig {
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
#[derive(Debug, Clone, PartialEq, Eq, Default)]
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
    #[default]
    Abort,
    /// 跳转到指定节点
    Goto {
        /// 目标节点ID
        target: String,
    },
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
    /// 并行分支（仅并行节点和流式节点）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_branches: Option<Vec<ParallelBranchDef>>,
    /// 执行模式（仅并行/流式节点，默认从节点类型推断）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_mode: Option<ExecutionMode>,
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

impl NodeDef {
    /// 获取节点的执行模式
    /// - 对于 Pipeline 类型节点，返回 Pipeline 模式
    /// - 对于 Parallel 类型节点，返回 Parallel 模式（或自定义模式）
    /// - 其他类型节点返回 None
    pub fn get_execution_mode(&self) -> Option<ExecutionMode> {
        match self.node_type {
            NodeType::Pipeline => Some(ExecutionMode::Pipeline),
            NodeType::Parallel => Some(
                self.execution_mode.unwrap_or(ExecutionMode::Parallel)
            ),
            _ => None,
        }
    }

    /// 是否需要屏障等待
    pub fn has_barrier(&self) -> bool {
        self.get_execution_mode()
            .map(|m| m.has_barrier())
            .unwrap_or(false)
    }
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
    /// 执行模式（可选，默认为 Parallel）
    /// - Pipeline: 流式处理，无屏障等待
    /// - Parallel: 并行执行，等待全部完成
    #[serde(default)]
    pub mode: ExecutionMode,
}

impl ParallelBranchDef {
    /// 创建默认并行分支（Parallel 模式）
    pub fn new(name: String, nodes: Vec<NodeDef>) -> Self {
        Self {
            name,
            nodes,
            mode: ExecutionMode::default(),
        }
    }

    /// 创建 Pipeline 模式分支（流式处理）
    pub fn pipeline(name: String, nodes: Vec<NodeDef>) -> Self {
        Self {
            name,
            nodes,
            mode: ExecutionMode::Pipeline,
        }
    }

    /// 创建 Parallel 模式分支（有屏障等待）
    pub fn parallel(name: String, nodes: Vec<NodeDef>) -> Self {
        Self {
            name,
            nodes,
            mode: ExecutionMode::Parallel,
        }
    }

    /// 是否需要屏障等待
    pub fn has_barrier(&self) -> bool {
        self.mode.has_barrier()
    }

    /// 获取执行模式描述
    pub fn mode_description(&self) -> &'static str {
        self.mode.description()
    }
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
                    execution_mode: None,
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
                    execution_mode: None,
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

    #[test]
    fn test_execution_mode_default() {
        let mode = ExecutionMode::default();
        assert_eq!(mode, ExecutionMode::Parallel);
        assert!(mode.has_barrier());
    }

    #[test]
    fn test_execution_mode_pipeline_no_barrier() {
        let mode = ExecutionMode::Pipeline;
        assert!(!mode.has_barrier());
        assert_eq!(mode.display_name(), "流式");
    }

    #[test]
    fn test_execution_mode_parallel_has_barrier() {
        let mode = ExecutionMode::Parallel;
        assert!(mode.has_barrier());
        assert_eq!(mode.display_name(), "并行");
    }

    #[test]
    fn test_parallel_branch_def_default_mode() {
        let branch = ParallelBranchDef::new("test".to_string(), vec![]);
        assert_eq!(branch.mode, ExecutionMode::Parallel);
        assert!(branch.has_barrier());
    }

    #[test]
    fn test_parallel_branch_def_pipeline_mode() {
        let branch = ParallelBranchDef::pipeline("test".to_string(), vec![]);
        assert_eq!(branch.mode, ExecutionMode::Pipeline);
        assert!(!branch.has_barrier());
    }

    #[test]
    fn test_parallel_branch_def_parallel_mode() {
        let branch = ParallelBranchDef::parallel("test".to_string(), vec![]);
        assert_eq!(branch.mode, ExecutionMode::Parallel);
        assert!(branch.has_barrier());
    }

    #[test]
    fn test_node_def_get_execution_mode_pipeline() {
        let node = NodeDef {
            id: "pipeline-node".to_string(),
            node_type: NodeType::Pipeline,
            name: "Pipeline Node".to_string(),
            description: None,
            task: None,
            params: HashMap::new(),
            on_failure: FailureStrategy::default(),
            timeout_ms: None,
            branches: None,
            parallel_branches: None,
            execution_mode: None,
            workflow: None,
            wait_ms: None,
            approvers: None,
        };
        assert_eq!(node.get_execution_mode(), Some(ExecutionMode::Pipeline));
        assert!(!node.has_barrier());
    }

    #[test]
    fn test_node_def_get_execution_mode_parallel() {
        let node = NodeDef {
            id: "parallel-node".to_string(),
            node_type: NodeType::Parallel,
            name: "Parallel Node".to_string(),
            description: None,
            task: None,
            params: HashMap::new(),
            on_failure: FailureStrategy::default(),
            timeout_ms: None,
            branches: None,
            parallel_branches: None,
            execution_mode: None,
            workflow: None,
            wait_ms: None,
            approvers: None,
        };
        assert_eq!(node.get_execution_mode(), Some(ExecutionMode::Parallel));
        assert!(node.has_barrier());
    }

    #[test]
    fn test_node_def_get_execution_mode_custom() {
        // Parallel node with custom Pipeline mode
        let node = NodeDef {
            id: "custom-node".to_string(),
            node_type: NodeType::Parallel,
            name: "Custom Node".to_string(),
            description: None,
            task: None,
            params: HashMap::new(),
            on_failure: FailureStrategy::default(),
            timeout_ms: None,
            branches: None,
            parallel_branches: None,
            execution_mode: Some(ExecutionMode::Pipeline), // Override
            workflow: None,
            wait_ms: None,
            approvers: None,
        };
        assert_eq!(node.get_execution_mode(), Some(ExecutionMode::Pipeline));
        assert!(!node.has_barrier());
    }

    #[test]
    fn test_node_def_get_execution_mode_other_types() {
        let node = NodeDef {
            id: "task-node".to_string(),
            node_type: NodeType::Task,
            name: "Task Node".to_string(),
            description: None,
            task: Some("do_something".to_string()),
            params: HashMap::new(),
            on_failure: FailureStrategy::default(),
            timeout_ms: None,
            branches: None,
            parallel_branches: None,
            execution_mode: None,
            workflow: None,
            wait_ms: None,
            approvers: None,
        };
        assert_eq!(node.get_execution_mode(), None);
    }
}
