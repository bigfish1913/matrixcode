use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};

use super::{Tool, ToolDefinition};
use crate::approval::RiskLevel;

/// Maximum file size for safe editing (1MB)
const MAX_EDIT_FILE_SIZE: u64 = 1_000_000;

pub struct EditTool;

#[async_trait]
impl Tool for EditTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "edit".to_string(),
            description: "在文件中查找精确匹配的字符串并替换为新内容".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "要编辑的文件路径"
                    },
                    "old_string": {
                        "type": "string",
                        "description": "要查找并替换的原始字符串（必须精确匹配）"
                    },
                    "new_string": {
                        "type": "string",
                        "description": "替换后的新字符串"
                    }
                },
                "required": ["path", "old_string", "new_string"]
            }),
            ..Default::default()
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let path = params["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'path'"))?;
        let old_string = params["old_string"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'old_string'"))?;
        let new_string = params["new_string"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'new_string'"))?;

        // Check file size first
        let metadata = tokio::fs::metadata(path).await?;
        let file_size = metadata.len();

        if file_size > MAX_EDIT_FILE_SIZE {
            return Ok(format!(
                "⚠️ File is too large ({:.1}MB) for safe editing.\n\
                 Large file edits may cause memory issues.\n\
                 Consider using other methods:\n\
                 - Use `bash` with sed/awk for large files\n\
                 - Split the file into smaller sections first",
                file_size as f64 / 1_000_000.0
            ));
        }

        let content = tokio::fs::read_to_string(path).await?;

        let count = content.matches(old_string).count();
        if count == 0 {
            anyhow::bail!("old_string not found in {}", path);
        }
        if count > 1 {
            anyhow::bail!(
                "old_string found {} times in {} — must be unique",
                count,
                path
            );
        }

        let new_content = content.replacen(old_string, new_string, 1);
        tokio::fs::write(path, &new_content).await?;

        // Return diff-style output
        let old_lines: Vec<&str> = old_string.lines().collect();
        let new_lines: Vec<&str> = new_string.lines().collect();
        let mut diff = format!("Successfully edited {}\n", path);
        for line in &old_lines {
            diff.push_str(&format!("- {}\n", line));
        }
        for line in &new_lines {
            diff.push_str(&format!("+ {}\n", line));
        }
        Ok(diff)
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Mutating
    }
}
