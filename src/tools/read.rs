use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

use super::{Tool, ToolDefinition};

pub struct ReadTool;

#[async_trait]
impl Tool for ReadTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "read".to_string(),
            description: "Read the contents of a file at the given path".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The file path to read"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Line number to start reading from (0-based)"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of lines to read"
                    }
                },
                "required": ["path"]
            }),
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let path = params["path"].as_str().ok_or_else(|| anyhow::anyhow!("missing 'path'"))?;
        
        // Show spinner while reading
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::with_template("{spinner:.cyan} {msg}")
                .unwrap_or_else(|_| ProgressStyle::default_spinner())
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
        );
        spinner.set_message(format!("reading {}", path));
        spinner.enable_steady_tick(Duration::from_millis(80));

        let content = tokio::fs::read_to_string(path).await?;

        let offset = params["offset"].as_u64().unwrap_or(0) as usize;
        let limit = params["limit"].as_u64().map(|l| l as usize);

        let lines: Vec<&str> = content.lines().collect();
        let end = limit.map(|l| (offset + l).min(lines.len())).unwrap_or(lines.len());
        let selected = &lines[offset.min(lines.len())..end.min(lines.len())];

        let result: String = selected
            .iter()
            .enumerate()
            .map(|(i, line)| format!("{:4} | {}", offset + i + 1, line))
            .collect::<Vec<_>>()
            .join("\n");

        spinner.finish_with_message(format!("✓ {} lines", selected.len()));
        Ok(result)
    }
}