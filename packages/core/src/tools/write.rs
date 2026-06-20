use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};

use super::{Tool, ToolDefinition};
use crate::approval::RiskLevel;
use crate::path_validator::{validate_path, validate_content_size};

pub struct WriteTool;

#[async_trait]
impl Tool for WriteTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "write".to_string(),
            description: "向文件写入内容，若文件不存在则创建。自动验证路径安全性，限制单次写入最大10MB".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "要写入的文件路径（会自动验证安全性，阻止路径穿越和系统文件写入）"
                    },
                    "content": {
                        "type": "string",
                        "description": "要写入的内容（单次写入最大10MB，超大内容请分批写入）"
                    }
                },
                "required": ["path", "content"]
            }),
            ..Default::default()
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let path_str = params["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'path'"))?;
        let content = params["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'content'"))?;

        // 1. Validate content size (prevent accidental huge writes)
        validate_content_size(content)?;

        // 2. Validate path security (prevent path traversal and system file writes)
        // For writes, we use strict validation (is_write=true)
        let validated_path = validate_path(path_str, None, true)?;

        // 3. Create parent directories if needed
        if let Some(parent) = validated_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // 4. Write the file with validated path
        let total_bytes = content.len();
        let size_mb = total_bytes as f64 / 1_000_000.0;
        
        // Write the file
        tokio::fs::write(&validated_path, content).await?;
        
        // 5. Provide helpful feedback based on file size
        let size_feedback = if size_mb > 1.0 {
            format!(
                " ({:.2} MB - large file written successfully. \
                Consider splitting if this causes performance issues)",
                size_mb
            )
        } else if size_mb > 0.1 {
            format!(" ({:.2} MB)", size_mb)
        } else {
            format!(" ({:.2} KB)", total_bytes as f64 / 1_000.0)
        };

        Ok(format!(
            "Successfully wrote {} bytes{} to {}\nPath validated: {}",
            total_bytes, size_feedback, path_str, validated_path.display()
        ))
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Mutating
    }
}
