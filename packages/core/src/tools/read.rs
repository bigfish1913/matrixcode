use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};

use super::{Tool, ToolDefinition};

/// Maximum file size to read without warning (5MB)
const MAX_FILE_SIZE: u64 = 5_000_000;
/// Maximum lines to return if no limit specified
const DEFAULT_MAX_LINES: usize = 500;

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
            ..Default::default()
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let path = params["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'path'"))?;

        // Check file size first
        let metadata = tokio::fs::metadata(path).await?;
        let file_size = metadata.len();

        let offset = params["offset"].as_u64().unwrap_or(0) as usize;
        let limit = params["limit"].as_u64().map(|l| l as usize);

        // Warn for large files
        if file_size > MAX_FILE_SIZE && limit.is_none() {
            return Ok(format!(
                "⚠️ File is large ({:.1}MB). Use offset/limit parameters to read specific sections.\n\
                 Example: read(path=\"{}\", offset=0, limit=100) to read first 100 lines.",
                file_size as f64 / 1_000_000.0,
                path
            ));
        }

        let content = tokio::fs::read_to_string(path).await?;

        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        // Apply default limit if not specified
        let effective_limit = limit.unwrap_or(DEFAULT_MAX_LINES);
        let end = (offset + effective_limit).min(total_lines);
        let selected = &lines[offset.min(total_lines)..end.min(total_lines)];

        let mut result = String::new();

        // Add header for truncated reads
        if offset > 0 || end < total_lines {
            result.push_str(&format!(
                "📄 {} (lines {}-{} of {})\n\n",
                path,
                offset + 1,
                end,
                total_lines
            ));
        }

        for (i, line) in selected.iter().enumerate() {
            result.push_str(&format!("{:4} | {}\n", offset + i + 1, line));
        }

        // Add truncation notice
        if end < total_lines {
            result.push_str(&format!(
                "\n... ({} more lines, use offset={} limit=N to continue)",
                total_lines - end,
                end
            ));
        }

        Ok(result)
    }
}
