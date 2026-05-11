use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};

use super::{Tool, ToolDefinition};

pub struct MultiEditTool;

#[async_trait]
impl Tool for MultiEditTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "multi_edit".to_string(),
            description:
                "Apply multiple exact-string replacements to one file in a single \
                 atomic write. Each edit must match exactly once against the file \
                 state after prior edits in the same call. If any edit fails the \
                 file is not modified."
                    .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The file path to edit"
                    },
                    "edits": {
                        "type": "array",
                        "description": "Ordered list of {old_string, new_string} replacements",
                        "items": {
                            "type": "object",
                            "properties": {
                                "old_string": {"type": "string"},
                                "new_string": {"type": "string"}
                            },
                            "required": ["old_string", "new_string"]
                        }
                    }
                },
                "required": ["path", "edits"]
            }),
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let path = params["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'path'"))?;
        let edits = params["edits"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("missing 'edits' array"))?;
        if edits.is_empty() {
            anyhow::bail!("'edits' must contain at least one entry");
        }

        let mut content = tokio::fs::read_to_string(path).await?;

        for (idx, edit) in edits.iter().enumerate() {
            let old_string = edit["old_string"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("edit {}: missing 'old_string'", idx))?;
            let new_string = edit["new_string"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("edit {}: missing 'new_string'", idx))?;

            if old_string.is_empty() {
                anyhow::bail!("edit {}: 'old_string' must not be empty", idx);
            }

            let count = content.matches(old_string).count();
            if count == 0 {
                anyhow::bail!("edit {}: old_string not found", idx);
            }
            if count > 1 {
                anyhow::bail!(
                    "edit {}: old_string found {} times — must be unique",
                    idx,
                    count
                );
            }

            content = content.replacen(old_string, new_string, 1);
        }

        tokio::fs::write(path, &content).await?;
        Ok(format!("Applied {} edit(s) to {}", edits.len(), path))
    }
}
