use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};

use super::{Tool, ToolDefinition};
use crate::approval::RiskLevel;

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
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        // Create spinner immediately at the start to fill the gap before actual operation
        // let mut spinner = ToolSpinner::new("preparing edit");

        let path = params["path"].as_str().ok_or_else(|| anyhow::anyhow!("missing 'path'"))?;
        let old_string = params["old_string"].as_str().ok_or_else(|| anyhow::anyhow!("missing 'old_string'"))?;
        let new_string = params["new_string"].as_str().ok_or_else(|| anyhow::anyhow!("missing 'new_string'"))?;

        // Update spinner message for the actual edit operation
        // spinner.set_message(&format!("editing {}", path));

        let content = tokio::fs::read_to_string(path).await?;

        let count = content.matches(old_string).count();
        if count == 0 {
            // spinner.finish_error("not found");
            anyhow::bail!("old_string not found in {}", path);
        }
        if count > 1 {
            // spinner.finish_error("multiple matches");
            anyhow::bail!("old_string found {} times in {} — must be unique", count, path);
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
        // spinner.finish_success("edited");
        Ok(diff)
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Mutating
    }
}