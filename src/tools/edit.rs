use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};

use super::{Tool, ToolDefinition};
use super::spinner::ToolSpinner;

pub struct EditTool;

#[async_trait]
impl Tool for EditTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "edit".to_string(),
            description: "Replace an exact string match in a file with new content".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The file path to edit"
                    },
                    "old_string": {
                        "type": "string",
                        "description": "The exact string to find and replace"
                    },
                    "new_string": {
                        "type": "string",
                        "description": "The replacement string"
                    }
                },
                "required": ["path", "old_string", "new_string"]
            }),
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let path = params["path"].as_str().ok_or_else(|| anyhow::anyhow!("missing 'path'"))?;
        let old_string = params["old_string"].as_str().ok_or_else(|| anyhow::anyhow!("missing 'old_string'"))?;
        let new_string = params["new_string"].as_str().ok_or_else(|| anyhow::anyhow!("missing 'new_string'"))?;

        // Show spinner while editing - RAII guard ensures cleanup on error
        let spinner = ToolSpinner::new(&format!("editing {}", path));

        let content = tokio::fs::read_to_string(path).await?;

        let count = content.matches(old_string).count();
        if count == 0 {
            spinner.finish_error("not found");
            anyhow::bail!("old_string not found in {}", path);
        }
        if count > 1 {
            spinner.finish_error("multiple matches");
            anyhow::bail!("old_string found {} times in {} — must be unique", count, path);
        }

        let new_content = content.replacen(old_string, new_string, 1);
        tokio::fs::write(path, &new_content).await?;

        spinner.finish_success("edited");
        Ok(format!("Successfully edited {}", path))
    }
}