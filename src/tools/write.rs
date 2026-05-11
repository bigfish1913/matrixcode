use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};

use super::{Tool, ToolDefinition};

pub struct WriteTool;

#[async_trait]
impl Tool for WriteTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "write".to_string(),
            description: "Write content to a file, creating it if it doesn't exist".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The file path to write to"
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to write"
                    }
                },
                "required": ["path", "content"]
            }),
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let path = params["path"].as_str().ok_or_else(|| anyhow::anyhow!("missing 'path'"))?;
        let content = params["content"].as_str().ok_or_else(|| anyhow::anyhow!("missing 'content'"))?;

        if let Some(parent) = std::path::Path::new(path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(path, content).await?;
        Ok(format!("Successfully wrote {} bytes to {}", content.len(), path))
    }
}
