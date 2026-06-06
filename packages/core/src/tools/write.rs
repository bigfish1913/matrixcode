use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};

use super::{Tool, ToolDefinition};
use super::tool_hooks::{HookRegistry, HookResult};
use super::code_quality_hook::{CodeQualityHook, VerificationStrategy};
use crate::approval::RiskLevel;
use crate::path_validator::{validate_content_size, validate_path};
use super::verify::{VerifyTool, ProjectType};

pub struct WriteTool {
    /// Hook registry for pre/post execution hooks
    hook_registry: HookRegistry,
}

impl Default for WriteTool {
    fn default() -> Self {
        Self::new()
    }
}

impl WriteTool {
    /// Create a new WriteTool with default hooks
    pub fn new() -> Self {
        Self::with_verification_strategy(VerificationStrategy::default())
    }

    /// Create WriteTool with specific verification strategy
    pub fn with_verification_strategy(strategy: VerificationStrategy) -> Self {
        let mut registry = HookRegistry::new();
        // Note: CodeQualityHook is added when strategy is not None
        if strategy != VerificationStrategy::None {
            registry.register(Box::new(CodeQualityHook::new(strategy)));
        }
        Self { hook_registry: registry }
    }

    /// Create WriteTool with custom hook registry
    pub fn with_hooks(registry: HookRegistry) -> Self {
        Self { hook_registry: registry }
    }

    /// Get the hook registry
    pub fn hook_registry(&self) -> &HookRegistry {
        &self.hook_registry
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

【代码质量验证】写入代码文件时会自动验证：
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
        // Extract path early (before potential modification by hooks)
        let path_str = params["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'path'"))?
            .to_string();  // Clone to avoid borrow issues

        // 1. Run pre-execute hooks (including code quality verification)
        let hook_result = self.hook_registry.pre_execute("write", &params).await?;

        // Check if hooks blocked execution
        let final_params = match hook_result {
            HookResult::Block { reason, details } => {
                // Return error to AI for correction
                let error_msg = if let Some(d) = details {
                    format!("{}\n\n详细信息:\n{}", reason, d)
                } else {
                    reason
                };
                return Err(anyhow::anyhow!(error_msg));
            }
            HookResult::Modify(new_params) => new_params,
            HookResult::Continue => params,
        };

        // 2. Validate content size (prevent accidental huge writes)
        let final_content = final_params["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'content' after hook modification"))?;
        validate_content_size(final_content)?;

        // 3. Validate path security (prevent path traversal and system file writes)
        // For writes, we use strict validation (is_write=true)
        let validated_path = validate_path(&path_str, None, true)?;

        // 4. Create parent directories if needed
        if let Some(parent) = validated_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // 5. Write the file with validated path
        let total_bytes = final_content.len();
        let size_mb = total_bytes as f64 / 1_000_000.0;

        // Write the file
        tokio::fs::write(&validated_path, final_content).await?;

        // 6. Run code verification for code files (post-write)
        let verify_result = self.run_code_verification(&validated_path, final_content).await;

        // 7. Provide helpful feedback based on file size
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

        let verify_feedback = match verify_result {
            Ok(msg) => msg,
            Err(e) => format!(" ⚠️ Code verification failed: {}", e),
        };

        // 8. Run post-execute hooks
        let base_result = format!(
            "Successfully wrote {} bytes{} to {}\nPath validated: {}\n{}",
            total_bytes,
            size_feedback,
            path_str,
            validated_path.display(),
            verify_feedback
        );

        let final_result = self.hook_registry.post_execute("write", &final_params, &base_result).await?;

        Ok(final_result)
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Mutating
    }
}

impl WriteTool {
    /// Run code verification after writing code files
    async fn run_code_verification(&self, path: &std::path::Path, _content: &str) -> Result<String> {
        // Only verify code files
        let extension = path.extension().and_then(|e| e.to_str());
        let is_code_file = matches!(extension, Some("rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go"));

        if !is_code_file {
            return Ok("(非代码文件，跳过代码检测)".to_string());
        }

        // Detect project type
        let project_root = std::env::current_dir()?;
        let verify_tool = VerifyTool::new(project_root);
        let project_type = verify_tool.project_type();

        // Run appropriate verification based on project type and file type
        match project_type {
            ProjectType::Rust if extension == Some("rs") => {
                self.verify_rust_code(path).await
            }
            ProjectType::NodeJs if matches!(extension, Some("ts" | "tsx")) => {
                self.verify_typescript_code(path).await
            }
            ProjectType::Python if extension == Some("py") => {
                self.verify_python_code(path).await
            }
            ProjectType::Go if extension == Some("go") => {
                self.verify_go_code(path).await
            }
            _ => Ok(format!("({} 文件，当前项目类型 {} 无自动检测)",
                extension.unwrap_or("unknown"),
                Self::project_type_str(project_type)))
        }
    }

    /// Get project type display name
    fn project_type_str(pt: ProjectType) -> &'static str {
        match pt {
            ProjectType::Rust => "Rust",
            ProjectType::NodeJs => "Node.js",
            ProjectType::Python => "Python",
            ProjectType::Go => "Go",
            ProjectType::Java => "Java",
            ProjectType::Unknown => "未知",
        }
    }

    /// Verify Rust code with cargo check
    async fn verify_rust_code(&self, _path: &std::path::Path) -> Result<String> {
        // Run cargo check in background
        let output = tokio::process::Command::new("cargo")
            .args(["check", "--quiet"])
            .output()
            .await;

        match output {
            Ok(o) if o.status.success() => {
                Ok("✅ cargo check 通过".to_string())
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                let errors = stderr.lines()
                    .filter(|l| l.contains("error") || l.contains("Error"))
                    .take(5)
                    .collect::<Vec<_>>()
                    .join("\n");
                if errors.is_empty() {
                    Ok("⚠️ cargo check 有警告，请检查".to_string())
                } else {
                    Ok(format!("❌ cargo check 失败:\n{}", errors))
                }
            }
            Err(e) => {
                // cargo not found or other error - don't fail the write
                Ok(format!("⚠️ 无法运行 cargo check: {}", e))
            }
        }
    }

    /// Verify TypeScript code with tsc --noEmit
    async fn verify_typescript_code(&self, _path: &std::path::Path) -> Result<String> {
        // Run tsc --noEmit in background
        let output = tokio::process::Command::new("npx")
            .args(["tsc", "--noEmit"])
            .output()
            .await;

        match output {
            Ok(o) if o.status.success() => {
                Ok("✅ tsc --noEmit 通过".to_string())
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                let errors = stderr.lines()
                    .filter(|l| l.contains("error"))
                    .take(5)
                    .collect::<Vec<_>>()
                    .join("\n");
                if errors.is_empty() {
                    Ok("⚠️ TypeScript 类型检查有警告".to_string())
                } else {
                    Ok(format!("❌ TypeScript 类型检查失败:\n{}", errors))
                }
            }
            Err(e) => {
                // tsc not found - don't fail the write
                Ok(format!("⚠️ 无法运行 tsc: {}", e))
            }
        }
    }

    /// Verify Python code with python -m py_compile
    async fn verify_python_code(&self, path: &std::path::Path) -> Result<String> {
        // Quick syntax check with python -m py_compile
        let output = tokio::process::Command::new("python")
            .args(["-m", "py_compile"])
            .arg(path)
            .output()
            .await;

        match output {
            Ok(o) if o.status.success() => {
                Ok("✅ Python 语法检查通过".to_string())
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                let errors = stderr.lines()
                    .filter(|l| l.contains("Error") || l.contains("SyntaxError"))
                    .take(3)
                    .collect::<Vec<_>>()
                    .join("\n");
                if errors.is_empty() {
                    Ok("⚠️ Python 检查有警告".to_string())
                } else {
                    Ok(format!("❌ Python 语法检查失败:\n{}", errors))
                }
            }
            Err(e) => {
                Ok(format!("⚠️ 无法运行 Python 检查: {}", e))
            }
        }
    }

    /// Verify Go code with go vet
    async fn verify_go_code(&self, _path: &std::path::Path) -> Result<String> {
        // Run go vet for quick syntax check
        let output = tokio::process::Command::new("go")
            .args(["vet"])
            .output()
            .await;

        match output {
            Ok(o) if o.status.success() => {
                Ok("✅ go vet 通过".to_string())
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                let errors = stderr.lines()
                    .filter(|l| l.contains("error") || l.contains("undefined"))
                    .take(3)
                    .collect::<Vec<_>>()
                    .join("\n");
                if errors.is_empty() {
                    Ok("⚠️ go vet 有警告".to_string())
                } else {
                    Ok(format!("❌ go vet 失败:\n{}", errors))
                }
            }
            Err(e) => {
                Ok(format!("⚠️ 无法运行 go vet: {}", e))
            }
        }
    }
}
