use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};

use super::{Tool, ToolDefinition};
use crate::approval::RiskLevel;
use crate::path_validator::{validate_content_size, validate_path};

pub struct WriteTool;

impl Default for WriteTool {
    fn default() -> Self {
        Self::new()
    }
}

impl WriteTool {
    /// Create a new WriteTool
    pub fn new() -> Self {
        Self
    }

    /// Create WriteTool with specific verification strategy (deprecated - verification now at Agent level)
    pub fn with_verification_strategy(_strategy: crate::tools::code_quality_hook::VerificationStrategy) -> Self {
        Self // Verification is handled at Agent level now
    }
}

#[async_trait]
impl Tool for WriteTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "write".to_string(),
            description: "向文件写入内容，若文件不存在则创建。

【重要】写入现有文件前必须先读取：
- 如果文件已存在，必须先用 read 工具读取当前内容
- 如果没先读文件，此工具会失败
- 了解现有内容可防止意外覆盖重要信息

【代码质量验证】写入代码文件后 Agent 会自动验证：
- 根据 verify_strategy 配置决定验证时机
- 'pre' 策略：写入前验证，失败则阻止写入并返回错误给 AI 纠正
- 'post' 策略：写入后验证，结果附加在输出中
- 支持的验证：cargo check / tsc / python -m py_compile / go vet

优先用 edit 工具修改现有文件（只发送 diff）
只在以下情况使用此工具：
- 创建新文件
- 完整重写文件（用户明确要求）

路径安全：自动验证路径安全性，阻止路径穿越和系统文件写入"
                .to_string(),
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

        // 1. Validate content size (prevent accidental huge writes)
        let content = params["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'content'"))?;
        validate_content_size(content)?;

        // 2. Validate path security (prevent path traversal and system file writes)
        let validated_path = validate_path(path_str, None, true)?;

        // 3. Create parent directories if needed
        if let Some(parent) = validated_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // 4. Write the file
        let total_bytes = content.len();
        tokio::fs::write(&validated_path, content).await?;

        // 5. Provide helpful feedback based on file size
        let size_feedback = if total_bytes > 1_000_000 {
            format!(
                " ({:.2} MB - large file written successfully. \
                Consider splitting if this causes performance issues)",
                total_bytes as f64 / 1_000_000.0
            )
        } else if total_bytes > 100_000 {
            format!(" ({:.2} MB)", total_bytes as f64 / 1_000_000.0)
        } else {
            format!(" ({:.2} KB)", total_bytes as f64 / 1_000.0)
        };

        Ok(format!(
            "Successfully wrote {} bytes{} to {}\nPath validated: {}",
            total_bytes,
            size_feedback,
            path_str,
            validated_path.display()
        ))
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Mutating
    }
}