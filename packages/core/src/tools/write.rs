use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};
use std::path::Path;

use super::{Tool, ToolDefinition};
use crate::approval::RiskLevel;
use crate::path_validator::{validate_path, validate_content_size};

pub struct WriteTool;

/// Runtime code validation - basic syntax check before writing
async fn validate_code_syntax(path: &str, content: &str) -> Result<(), String> {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "rs" => validate_rust_syntax(content).await,
        "ts" | "tsx" | "js" | "jsx" => validate_js_syntax(content).await,
        "py" => validate_python_syntax(content).await,
        _ => Ok(()), // No validation for other file types
    }
}

/// Quick Rust syntax validation using rustfmt
async fn validate_rust_syntax(content: &str) -> Result<(), String> {
    use tokio::process::Command;
    use tokio::io::AsyncWriteExt;

    // Create temp file
    let temp_path = std::env::temp_dir().join(format!("matrixcode_validate_{}.rs", std::process::id()));

    // Write content to temp file
    let mut file = tokio::fs::File::create(&temp_path).await
        .map_err(|e| format!("Failed to create temp file: {}", e))?;
    file.write_all(content.as_bytes()).await
        .map_err(|e| format!("Failed to write temp file: {}", e))?;
    drop(file);

    // Try rustfmt --check (fast syntax check)
    let output = Command::new("rustfmt")
        .args(["--check", "--emit=stdout"])
        .arg(&temp_path)
        .output()
        .await;

    // Clean up temp file
    let _ = tokio::fs::remove_file(&temp_path).await;

    match output {
        Ok(o) => {
            if !o.status.success() {
                let stderr = String::from_utf8_lossy(&o.stderr);
                if !stderr.is_empty() {
                    // Extract first error line
                    let first_error = stderr.lines()
                        .find(|l| l.contains("error"))
                        .unwrap_or("Rust syntax error detected");
                    return Err(format!("Code gate blocked: Rust syntax error\n{}\n\nPlease fix before writing.", first_error));
                }
            }
            Ok(())
        }
        Err(_) => {
            // rustfmt not available, skip validation
            Ok(())
        }
    }
}

/// Basic JS/TS syntax validation (check for obvious issues)
async fn validate_js_syntax(content: &str) -> Result<(), String> {
    // Check for unbalanced braces (basic)
    let open_braces = content.matches('{').count();
    let close_braces = content.matches('}').count();
    if open_braces != close_braces {
        return Err(format!(
            "Code gate blocked: Unbalanced braces\nOpen: {}, Close: {}\n\nPlease fix before writing.",
            open_braces, close_braces
        ));
    }

    // Check for unbalanced parentheses
    let open_parens = content.matches('(').count();
    let close_parens = content.matches(')').count();
    if open_parens != close_parens {
        return Err(format!(
            "Code gate blocked: Unbalanced parentheses\nOpen: {}, Close: {}\n\nPlease fix before writing.",
            open_parens, close_parens
        ));
    }

    Ok(())
}

/// Basic Python syntax validation
async fn validate_python_syntax(content: &str) -> Result<(), String> {
    use tokio::process::Command;
    use tokio::io::AsyncWriteExt;

    // Create temp file
    let temp_path = std::env::temp_dir().join(format!("matrixcode_validate_{}.py", std::process::id()));

    // Write content to temp file
    let mut file = tokio::fs::File::create(&temp_path).await
        .map_err(|e| format!("Failed to create temp file: {}", e))?;
    file.write_all(content.as_bytes()).await
        .map_err(|e| format!("Failed to write temp file: {}", e))?;
    drop(file);

    // Use python -m py_compile for syntax check
    let output = Command::new("python")
        .args(["-m", "py_compile"])
        .arg(&temp_path)
        .output()
        .await;

    // Clean up
    let _ = tokio::fs::remove_file(&temp_path).await;

    match output {
        Ok(o) => {
            if !o.status.success() {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return Err(format!(
                    "Code gate blocked: Python syntax error\n{}\n\nPlease fix before writing.",
                    stderr.lines().next().unwrap_or("Syntax error detected")
                ));
            }
            Ok(())
        }
        Err(_) => Ok(()), // Python not available
    }
}

#[async_trait]
impl Tool for WriteTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "write".to_string(),
            description: "向文件写入内容，若文件不存在则创建。自动验证路径安全性，限制单次写入最大10MB。代码文件会经过语法检查门禁。".to_string(),
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

        // 2. CODE QUALITY GATE: Validate syntax before writing (for code files)
        if let Err(validation_error) = validate_code_syntax(path_str, content).await {
            return Err(anyhow::anyhow!("{}", validation_error));
        }

        // 3. Validate path security (prevent path traversal and system file writes)
        // For writes, we use strict validation (is_write=true)
        let validated_path = validate_path(path_str, None, true)?;

        // 4. Create parent directories if needed
        if let Some(parent) = validated_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // 5. Write the file with validated path
        let total_bytes = content.len();
        let size_mb = total_bytes as f64 / 1_000_000.0;

        // Write the file
        tokio::fs::write(&validated_path, content).await?;

        // 6. Provide helpful feedback based on file size
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
            "Code gate passed, write successful\nWrote {} bytes{} to {}\nPath validated: {}",
            total_bytes, size_feedback, path_str, validated_path.display()
        ))
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Mutating
    }
}