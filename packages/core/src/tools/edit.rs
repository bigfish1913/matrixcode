use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};

use super::{Tool, ToolDefinition};
use crate::approval::RiskLevel;

/// Maximum file size for safe editing (1MB)
const MAX_EDIT_FILE_SIZE: u64 = 1_000_000;

/// Normalize line endings for cross-platform compatibility
/// Converts CRLF (\r\n) to LF (\n) to handle Windows/Linux differences
fn normalize_line_endings(s: &str) -> String {
    s.replace("\r\n", "\n").replace("\r", "\n")
}

/// Detect if original content uses CRLF line endings
fn uses_crlf(content: &str) -> bool {
    content.contains("\r\n")
}

pub struct EditTool;

#[async_trait]
impl Tool for EditTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "edit".to_string(),
            description: "在文件中查找精确匹配的字符串并替换为新内容。

【重要】编辑前必须先读取：
- 你必须在对话中至少用 read 工具读一次该文件
- 如果尝试编辑前没读文件，此工具会报错
- 确保了解文件当前状态和上下文后再修改

适用场景：
- 单处代码修改（改一个函数名）
- 精确替换（必须唯一匹配）
- 小范围改动（<10行）

不适用场景：
- ❌ 同一文件多处修改 → 用 multi_edit（批量替换）
- ❌ 大范围重构 → 先 enter_plan_mode 规划
- ❌ 创建新文件 → 用 write

优先级：[高] 小改动首选，精确且安全"
                .to_string(),
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

        // Normalize line endings for cross-platform compatibility
        // This handles the case where file uses CRLF but old_string uses LF
        let original_uses_crlf = uses_crlf(&content);
        let normalized_content = normalize_line_endings(&content);
        let normalized_old = normalize_line_endings(old_string);
        let normalized_new = normalize_line_endings(new_string);

        let count = normalized_content.matches(&normalized_old).count();
        if count == 0 {
            // Provide helpful error message with context
            let hint = if old_string.contains('\n') && !content.contains('\r') {
                "\n提示: 文件使用 CRLF 换行符，请确保 old_string 与文件内容完全匹配"
            } else {
                ""
            };
            anyhow::bail!("old_string not found in {}{}", path, hint);
        }
        if count > 1 {
            anyhow::bail!(
                "old_string found {} times in {} — must be unique",
                count,
                path
            );
        }

        // Perform replacement on normalized content
        let new_normalized_content = normalized_content.replacen(&normalized_old, &normalized_new, 1);

        // Restore original line ending style if needed
        let final_content = if original_uses_crlf {
            new_normalized_content.replace('\n', "\r\n")
        } else {
            new_normalized_content
        };

        tokio::fs::write(path, &final_content).await?;

        // Return diff-style output
        let old_lines: Vec<&str> = normalized_old.lines().collect();
        let new_lines: Vec<&str> = normalized_new.lines().collect();
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
