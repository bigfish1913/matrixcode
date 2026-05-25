//! Workflow Module
//!
//! 工作流引擎核心框架，提供基于 YAML 定义的 DAG 工作流管理。
//!
//! # 模块结构
//!
//! - `def`: 工作流定义结构（WorkflowDef, NodeDef, EdgeDef 等）
//! - `parser`: YAML 解析器
//! - `context`: 运行时状态管理
//! - `template`: 模板渲染引擎（{{var}} 替换）
//! - `rule_engine`: 验证规则引擎
//! - `engine`: 状态机引擎
//! - `executors`: 节点执行器（AI、工具、条件、验证）
//!
//! # 快速开始
//!
//! ```rust,ignore
//! use matrixcode::workflow::{parse_workflow, WorkflowEngine, WorkflowContext};
//!
//! // 解析工作流定义
//! let yaml = r#"
//! id: hello-workflow
//! name: Hello Workflow
//! nodes:
//!   - id: start
//!     type: start
//!     name: Start
//!   - id: greet
//!     type: task
//!     name: Greet
//!     task: greet
//!   - id: end
//!     type: end
//!     name: End
//! edges:
//!   - from: start
//!     to: greet
//!   - from: greet
//!     to: end
//! "#;
//!
//! let workflow = parse_workflow(yaml)?;
//!
//! // 创建引擎并运行
//! let engine = WorkflowEngine::new(workflow)?;
//! let context = engine.run(HashMap::new()).await?;
//!
//! println!("Workflow status: {:?}", context.status);
//! ```

pub mod def;
pub mod parser;
pub mod context;
pub mod template;
pub mod rule_engine;
pub mod engine;
pub mod executors;
pub mod persistence;
pub mod registry;

// 重导出公共接口
pub use def::{
    WorkflowDef,
    NodeDef,
    EdgeDef,
    NodeType,
    FailureStrategy,
    InputDef,
    OutputDef,
    BranchDef,
    ParallelBranchDef,
};

pub use parser::{
    parse_workflow,
    parse_workflow_from_file,
    to_yaml,
};

pub use context::{
    WorkflowContext,
    WorkflowStatus,
    NodeStatus,
    NodeExecution,
};

pub use template::{
    TemplateRenderer,
    render as render_template,
};

pub use rule_engine::{
    Rule,
    RuleEngine,
    ValidationResult,
    evaluate_expression,
};

pub use engine::{
    WorkflowEngine,
    WorkflowEvent,
    EventListener,
    TaskExecutor,
    DefaultTaskExecutor,
};

pub use persistence::{
    WorkflowPersistence,
};

pub use registry::{
    WorkflowRegistry,
    WorkflowInfo,
    WorkflowSource,
};

pub use executors::{
    NodeExecutor,
    AiExecutor,
    AiExecutorConfig,
    ToolExecutor,
    ToolExecutorConfig,
    ConditionExecutor,
    ValidateExecutor,
    ValidateExecutorConfig,
    CompositeExecutor,
    CompositeMode,
    ExecutorFactory,
};

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_parse_and_validate_workflow() {
        let yaml = r#"
id: integration-test
name: Integration Test Workflow
version: "1.0.0"
description: A test workflow for integration testing
inputs:
  - name: user_name
    type: string
    required: true
  - name: count
    type: number
    required: false
    default: 1
outputs:
  - name: result
    value: "{{output}}"
variables:
  greeting: "Hello"
nodes:
  - id: start
    type: start
    name: Start
  - id: process
    type: task
    name: Process
    task: process_task
    params:
      name: "{{user_name}}"
      count: "{{count}}"
    on_failure:
      type: retry
      max_attempts: 3
      interval_ms: 1000
    timeout_ms: 30000
  - id: end
    type: end
    name: End
edges:
  - from: start
    to: process
  - from: process
    to: end
"#;
        let workflow = parse_workflow(yaml).unwrap();
        assert_eq!(workflow.id, "integration-test");
        assert_eq!(workflow.nodes.len(), 3);
        assert_eq!(workflow.edges.len(), 2);
        assert!(workflow.validate().is_ok());
    }

    #[tokio::test]
    async fn test_workflow_execution() {
        let workflow_def = WorkflowDef {
            id: "test-exec".to_string(),
            name: "Test Execution".to_string(),
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
                    on_failure: FailureStrategy::Abort,
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
                    on_failure: FailureStrategy::Abort,
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
            default_failure_strategy: FailureStrategy::Abort,
            timeout_ms: None,
        };

        let engine = WorkflowEngine::new(workflow_def).unwrap();
        let context = engine.run(HashMap::new()).await.unwrap();

        assert_eq!(context.status, WorkflowStatus::Completed);
    }

    #[test]
    fn test_template_rendering() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), serde_json::json!("World"));
        vars.insert("count".to_string(), serde_json::json!(42));

        let result = render_template("Hello, {{name}}! Count: {{count}}", &vars).unwrap();
        assert_eq!(result, "Hello, World! Count: 42");
    }

    #[test]
    fn test_rule_validation() {
        let mut engine = RuleEngine::new();
        let mut context = HashMap::new();
        context.insert("status".to_string(), serde_json::json!("success"));
        context.insert("count".to_string(), serde_json::json!(10));

        let rule = Rule::All {
            rules: vec![
                Rule::Equals {
                    field: "status".to_string(),
                    value: serde_json::json!("success"),
                },
                Rule::GreaterThan {
                    field: "count".to_string(),
                    value: 5.0,
                },
            ],
        };

        let result = engine.validate(&rule, &context).unwrap();
        assert!(result.passed);
    }
}