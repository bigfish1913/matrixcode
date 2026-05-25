//! Workflow Match Tool
//!
//! 根据意图匹配工作流

use crate::tools::{Tool, ToolDefinition};
use crate::workflow::WorkflowRegistry;
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

/// Tool to match workflows by intent
pub struct WorkflowMatchTool;

#[async_trait]
impl Tool for WorkflowMatchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "workflow_match".to_string(),
            description: "根据意图查找匹配的 workflow。传入自然语言描述，返回最相关的 workflow 列表。".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "意图描述，如 '处理文本'、'生成代码'、'验证输出'"
                    }
                },
                "required": ["query"]
            }),
            ..Default::default()
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let query = params.get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("缺少 query 参数"))?;

        let project_path = std::env::current_dir().ok();
        let registry = WorkflowRegistry::new(project_path.as_ref());

        let matches = registry.match_workflows(query);

        if matches.is_empty() {
            return Ok(format!("未找到匹配 '{}' 的 workflow。用 workflow_discover 查看全部。", query));
        }

        let mut result = format!("匹配 '{}' 的 workflow:\n\n", query);
        for info in matches.iter().take(5) {
            result.push_str(&format!("• {} - ", info.id));
            if let Some(ref desc) = info.description {
                result.push_str(desc);
            } else {
                result.push_str(&info.name);
            }
            result.push_str("\n");
        }

        result.push_str("\n调用: workflow_run {\"workflow_id\": \"选定的ID\"}");
        Ok(result)
    }
}