use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};

use super::{Tool, ToolDefinition};
use crate::approval::RiskLevel;

pub struct MultiEditTool;

#[async_trait]
impl Tool for MultiEditTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "multi_edit".to_string(),
            description:
                "对单个文件应用多处精确字符串替换，一次性原子写入。\
                 每个编辑必须在前序编辑后的文件状态中精确匹配一次。\
                 若任一编辑失败，文件不会被修改。"
                    .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "要编辑的文件路径"
                    },
                    "edits": {
                        "type": "array",
                        "description": "有序的 {old_string, new_string} 替换列表",
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

        // Show spinner while editing - RAII guard ensures cleanup on error
        // let mut spinner = ToolSpinner::new(&format!("multi-editing {} ({} edits)", path, edits.len()));

        let mut content = tokio::fs::read_to_string(path).await?;

        for (idx, edit) in edits.iter().enumerate() {
            let old_string = edit["old_string"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("edit {}: missing 'old_string'", idx))?;
            let new_string = edit["new_string"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("edit {}: missing 'new_string'", idx))?;

            if old_string.is_empty() {
                // spinner.finish_error("empty old_string");
                anyhow::bail!("edit {}: 'old_string' must not be empty", idx);
            }

            let count = content.matches(old_string).count();
            if count == 0 {
                // spinner.finish_error(&format!("edit {} not found", idx));
                anyhow::bail!("edit {}: old_string not found", idx);
            }
            if count > 1 {
                // spinner.finish_error(&format!("edit {} multiple matches", idx));
                anyhow::bail!(
                    "edit {}: old_string found {} times — must be unique",
                    idx,
                    count
                );
            }

            content = content.replacen(old_string, new_string, 1);
        }

        tokio::fs::write(path, &content).await?;

        // Return diff-style output
        let mut diff = format!("Applied {} edit(s) to {}\n", edits.len(), path);
        for (idx, edit) in edits.iter().enumerate() {
            let old_string = edit["old_string"].as_str().unwrap_or("");
            let new_string = edit["new_string"].as_str().unwrap_or("");
            if edits.len() > 1 {
                diff.push_str(&format!("edit {}:\n", idx + 1));
            }
            for line in old_string.lines().take(3) {
                diff.push_str(&format!("- {}\n", line));
            }
            if old_string.lines().count() > 3 {
                diff.push_str(&format!("  ... ({} more lines removed)\n", old_string.lines().count() - 3));
            }
            for line in new_string.lines().take(3) {
                diff.push_str(&format!("+ {}\n", line));
            }
            if new_string.lines().count() > 3 {
                diff.push_str(&format!("  ... ({} more lines added)\n", new_string.lines().count() - 3));
            }
        }
        Ok(diff)
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Mutating
    }
}