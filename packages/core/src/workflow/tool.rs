//! Workflow Tools for Agent
//!
//! Provides tool interface for AI to discover and execute workflows

use crate::tools::{Tool, ToolDefinition};
use crate::workflow::{WorkflowRegistry, WorkflowEngine, WorkflowPersistence, WorkflowStatus};
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

/// Tool to run a workflow
pub struct WorkflowRunTool;

#[async_trait]
impl Tool for WorkflowRunTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "workflow_run".to_string(),
            description: "执行指定的 workflow。传入 workflow ID 和可选的输入参数，workflow 会按定义的节点顺序执行。".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "workflow_id": {
                        "type": "string",
                        "description": "要执行的 workflow ID。先用 workflow_discover 查看可用 ID。"
                    },
                    "inputs": {
                        "type": "object",
                        "description": "workflow 输入参数（JSON 对象）。键名必须匹配 workflow 的 required_inputs。"
                    }
                },
                "required": ["workflow_id"]
            }),
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let workflow_id = params.get("workflow_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("缺少 workflow_id 参数"))?;

        let inputs: std::collections::HashMap<String, Value> = params.get("inputs")
            .and_then(|v| v.as_object())
            .map(|m| m.clone().into_iter().collect())
            .unwrap_or_default();

        let project_path = std::env::current_dir().ok();
        let registry = WorkflowRegistry::new(project_path.as_ref());

        // Load workflow
        let workflow_def = registry.load_workflow(workflow_id)?
            .ok_or_else(|| anyhow::anyhow!("Workflow '{}' 不存在。用 workflow_discover 查看可用列表。", workflow_id))?;

        // Create engine and run
        let engine = WorkflowEngine::new(workflow_def)?;
        let context = engine.run(inputs).await?;

        // Save context
        let persistence = WorkflowPersistence::new(project_path.as_ref());
        if let Err(e) = persistence.save(&context) {
            log::warn!("Failed to save workflow context: {}", e);
        }

        // Build result
        let status = if context.status == WorkflowStatus::Completed {
            "✓ 完成".to_string()
        } else if context.status == WorkflowStatus::Failed {
            format!("❌ 失败: {}", context.error.unwrap_or_default())
        } else {
            format!("状态: {:?}", context.status)
        };

        Ok(format!(
            "Workflow '{}' 执行结果:\n\n实例ID: {}\n节点执行: {} 个\n{}\n\n变量输出: {}",
            workflow_id,
            context.instance_id,
            context.execution_path.len(),
            status,
            serde_json::to_string_pretty(&context.variables).unwrap_or_default()
        ))
    }
}

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

/// Get all workflow tools
pub fn workflow_tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(WorkflowDiscoverTool),
        Box::new(WorkflowRunTool),
        Box::new(WorkflowMatchTool),
    ]
}