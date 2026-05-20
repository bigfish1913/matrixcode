use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};

use super::{Tool, ToolDefinition};
use crate::approval::RiskLevel;

pub struct TodoWriteTool;

#[async_trait]
impl Tool for TodoWriteTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "todo_write".to_string(),
            description:
                "维护结构化待办列表，用于规划和跟踪多步骤工作。\
                 每次调用会替换整个列表。用于非 trivial 任务（3 步以上）以展示进度。\
                 同时只保持一个任务 'in_progress'，完成后立即标记为 'completed'。"
                    .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "todos": {
                        "type": "array",
                        "description": "待办项的完整替换列表",
                        "items": {
                            "type": "object",
                            "properties": {
                                "content": {
                                    "type": "string",
                                    "description": "命令式描述（如 '运行测试'）"
                                },
                                "activeForm": {
                                    "type": "string",
                                    "description": "进行式描述（如 '正在运行测试'）"
                                },
                                "status": {
                                    "type": "string",
                                    "enum": ["pending", "in_progress", "completed"]
                                }
                            },
                            "required": ["content", "activeForm", "status"]
                        }
                    }
                },
                "required": ["todos"]
            }),
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let todos = params["todos"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("missing 'todos' array"))?;

        // Show spinner while updating todo list - RAII guard ensures cleanup on error
        // let mut spinner = ToolSpinner::new(&format!("updating todos ({} items)", todos.len()));

        let mut in_progress_count = 0;
        let _completed_count = 0;
        let _pending_count = 0;
        let mut lines = Vec::with_capacity(todos.len() + 1);

        for (i, todo) in todos.iter().enumerate() {
            let content = todo["content"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("todo {}: missing 'content'", i))?;
            let active = todo["activeForm"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("todo {}: missing 'activeForm'", i))?;
            let status = todo["status"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("todo {}: missing 'status'", i))?;

            let (marker, display) = match status {
                "pending" => ("[ ]", content),
                "in_progress" => {
                    in_progress_count += 1;
                    ("[~]", active)
                }
                "completed" => ("[x]", content),
                other => anyhow::bail!("todo {}: invalid status '{}'", i, other),
            };

            lines.push(format!("  {} {}", marker, display));
        }

        if in_progress_count > 1 {
            // spinner.finish_error("multiple in_progress");
            anyhow::bail!(
                "at most one task may be 'in_progress' at a time (found {})",
                in_progress_count
            );
        }

        if lines.is_empty() {
            // spinner.finish_success("cleared");
            return Ok("Todo list cleared.".to_string());
        }

        let header = format!("Todos ({}):", lines.len());
        let mut out = String::with_capacity(header.len() + lines.iter().map(|l| l.len() + 1).sum::<usize>());
        out.push_str(&header);
        for l in &lines {
            out.push('\n');
            out.push_str(l);
        }

        // spinner.finish_success(&format!(
//     "{} pending, {} in_progress, {} done",
//     pending_count, in_progress_count, completed_count
// ));
        Ok(out)
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Mutating
    }
}