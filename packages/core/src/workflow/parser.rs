//! YAML Parser for Workflow Definitions
//!
//! 解析 YAML 格式的工作流定义文件。

use anyhow::{Context, Result};
use std::path::Path;

use super::def::WorkflowDef;

/// 从字符串解析工作流定义
pub fn parse_workflow(yaml: &str) -> Result<WorkflowDef> {
    let workflow: WorkflowDef = serde_yaml::from_str(yaml)
        .with_context(|| "Failed to parse workflow YAML")?;

    workflow.validate()
        .with_context(|| "Workflow validation failed")?;

    Ok(workflow)
}

/// 从文件解析工作流定义
pub fn parse_workflow_from_file<P: AsRef<Path>>(path: P) -> Result<WorkflowDef> {
    let content = std::fs::read_to_string(path.as_ref())
        .with_context(|| "Failed to read workflow file")?;

    parse_workflow(&content)
}

/// 将工作流定义序列化为 YAML
pub fn to_yaml(workflow: &WorkflowDef) -> Result<String> {
    serde_yaml::to_string(workflow)
        .with_context(|| "Failed to serialize workflow to YAML")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_workflow() {
        let yaml = r#"
id: test-workflow
name: Test Workflow
version: "1.0.0"
nodes:
  - id: start
    type: start
    name: Start
  - id: end
    type: end
    name: End
edges:
  - from: start
    to: end
"#;
        let workflow = parse_workflow(yaml);
        assert!(workflow.is_ok());
        let workflow = workflow.unwrap();
        assert_eq!(workflow.id, "test-workflow");
        assert_eq!(workflow.nodes.len(), 2);
    }

    #[test]
    fn test_parse_workflow_with_task() {
        let yaml = r#"
id: task-workflow
name: Task Workflow
nodes:
  - id: start
    type: start
    name: Start
  - id: task1
    type: task
    name: Do Something
    task: do_something
    params:
      arg1: value1
    on_failure:
      type: retry
      max_attempts: 3
      interval_ms: 1000
  - id: end
    type: end
    name: End
edges:
  - from: start
    to: task1
  - from: task1
    to: end
"#;
        let workflow = parse_workflow(yaml);
        assert!(workflow.is_ok());
    }

    #[test]
    fn test_parse_workflow_missing_start() {
        let yaml = r#"
id: invalid-workflow
name: Invalid Workflow
nodes:
  - id: end
    type: end
    name: End
"#;
        let result = parse_workflow(yaml);
        assert!(result.is_err());
    }
}