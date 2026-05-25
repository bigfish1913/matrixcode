//! Workflow Discovery Tool
//!
//! 让 AI 发现已有的自动化工作流

use crate::tools::{Tool, ToolDefinition};
use crate::workflow::WorkflowRegistry;
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

/// Tool to discover available workflows
pub struct WorkflowDiscoverTool;

#[async_trait]
impl Tool for WorkflowDiscoverTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "workflow_discover".to_string(),
            description: "发现可执行的自动化流程。返回 workflow ID、描述和所需输入参数列表。".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    async fn execute(&self, _params: Value) -> Result<String> {
        let project_path = std::env::current_dir().ok();
        let registry = WorkflowRegistry::new(project_path.as_ref());

        if registry.is_empty() {
            return Ok("未发现 workflow。在 .matrix/workflows/ 或 ~/.matrix/workflows/ 目录创建 YAML 文件。".to_string());
        }

        let mut result = format!("发现 {} 个 workflow:\n\n", registry.count());
        for info in registry.list() {
            result.push_str(&format!("• {} - ", info.id));
            if let Some(ref desc) = info.description {
                result.push_str(desc);
            } else {
                result.push_str(&info.name);
            }
            if !info.required_inputs.is_empty() {
                result.push_str(&format!(" [需要: {}]", info.required_inputs.join(", ")));
            }
            result.push_str("\n");
        }

        Ok(result)
    }
}