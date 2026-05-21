use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};

use super::{Tool, ToolDefinition};

pub struct ReadTool;

#[async_trait]
impl Tool for ReadTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "read".to_string(),
            description: "读取指定路径的文件内容".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "要读取的文件路径"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "起始行号（从 0 开始）"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "最大读取行数"
                    }
                },
                "required": ["path"]
            }),
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        // Create spinner immediately at the start to fill the gap before actual operation
        // let mut spinner = ToolSpinner::new("preparing read");

        let path = params["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'path'"))?;

        // Update spinner message for the actual read operation
        // spinner.set_message(&format!("reading {}", path));

        let content = tokio::fs::read_to_string(path).await?;

        let offset = params["offset"].as_u64().unwrap_or(0) as usize;
        let limit = params["limit"].as_u64().map(|l| l as usize);

        let lines: Vec<&str> = content.lines().collect();
        let end = limit
            .map(|l| (offset + l).min(lines.len()))
            .unwrap_or(lines.len());
        let selected = &lines[offset.min(lines.len())..end.min(lines.len())];

        let result: String = selected
            .iter()
            .enumerate()
            .map(|(i, line)| format!("{:4} | {}", offset + i + 1, line))
            .collect::<Vec<_>>()
            .join("\n");

        // spinner.finish_success(&format!("{} lines", selected.len()));
        Ok(result)
    }
}
