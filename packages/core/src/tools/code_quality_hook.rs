//! Code Quality Verification Hook
//!
//! This hook verifies code quality before writing files, preventing
//! invalid code from being written and returning errors to AI for correction.
//!
//! # Verification Strategy
//!
//! - `none`: No verification
//! - `post`: Verify after write (default, current behavior)
//! - `pre`: Verify before write, block if errors
//! - `pre-quick`: Quick syntax check before write, full check after
//!
//! # Workflow
//!
//! 1. Detect file type (Rust, TypeScript, Python, Go)
//! 2. Write to temporary file for verification
//! 3. Run appropriate verification command
//! 4. If errors found, block write and return errors to AI
//! 5. AI corrects code and tries again

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;

use super::tool_hooks::{HookResult, ToolHook};
use crate::tools::verify::{ProjectType, VerifyTool};

/// Code quality verification hook
pub struct CodeQualityHook {
    /// Verification strategy
    strategy: VerificationStrategy,
    /// Whether hook is enabled
    enabled: bool,
    /// Project root for detection
    project_root: Option<Arc<std::path::PathBuf>>,
}

/// Verification strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VerificationStrategy {
    /// No verification
    None,
    /// Verify after write (current behavior)
    #[default]
    Post,
    /// Verify before write, block if errors
    Pre,
    /// Quick syntax check before, full check after
    PreQuick,
}

impl VerificationStrategy {
    /// Parse from string
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "none" => Self::None,
            "post" => Self::Post,
            "pre" => Self::Pre,
            "pre-quick" | "prequick" => Self::PreQuick,
            _ => Self::Post,
        }
    }

    /// Convert to string
    pub fn to_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Post => "post",
            Self::Pre => "pre",
            Self::PreQuick => "pre-quick",
        }
    }
}

impl Default for CodeQualityHook {
    fn default() -> Self {
        Self::new(VerificationStrategy::default())
    }
}

impl CodeQualityHook {
    /// Create with strategy
    pub fn new(strategy: VerificationStrategy) -> Self {
        Self {
            strategy,
            enabled: strategy != VerificationStrategy::None,
            project_root: None,
        }
    }

    /// Create with strategy string
    pub fn from_strategy_str(strategy: &str) -> Self {
        Self::new(VerificationStrategy::from_str(strategy))
    }

    /// Set project root
    pub fn with_project_root(mut self, root: Arc<std::path::PathBuf>) -> Self {
        self.project_root = Some(root);
        self
    }

    /// Set enabled status
    pub fn set_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Get verification strategy
    pub fn strategy(&self) -> VerificationStrategy {
        self.strategy
    }

    /// Check if file is a code file that needs verification
    fn is_code_file(path: &str) -> bool {
        let ext = Path::new(path)
            .extension()
            .and_then(|e| e.to_str());
        matches!(ext, Some("rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go"))
    }

    /// Get file extension
    fn get_extension(path: &str) -> Option<&str> {
        Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
    }

    /// Detect project type
    fn detect_project_type(&self) -> ProjectType {
        if let Some(root) = &self.project_root {
            VerifyTool::detect_project_type(root.as_ref())
        } else {
            // Fallback: detect from current directory
            let current_dir = std::env::current_dir().ok();
            current_dir
                .as_ref()
                .map(|d| VerifyTool::detect_project_type(d))
                .unwrap_or(ProjectType::Unknown)
        }
    }

    /// Run pre-write verification
    async fn verify_before_write(&self, path: &str, content: &str) -> Result<HookResult> {
        // Only verify code files
        if !Self::is_code_file(path) {
            return Ok(HookResult::Continue);
        }

        // Create temporary directory for verification
        let temp_dir = TempDir::new()?;
        let temp_path = temp_dir.path().join(Path::new(path).file_name().unwrap_or_default());

        // Write content to temp file
        tokio::fs::write(&temp_path, content).await?;

        // Detect project type and run appropriate verification
        let project_type = self.detect_project_type();
        let extension = Self::get_extension(path);

        let verify_result = match project_type {
            ProjectType::Rust if extension == Some("rs") => {
                self.verify_rust(&temp_path).await
            }
            ProjectType::NodeJs if matches!(extension, Some("ts" | "tsx")) => {
                self.verify_typescript(&temp_path).await
            }
            ProjectType::Python if extension == Some("py") => {
                self.verify_python(&temp_path).await
            }
            ProjectType::Go if extension == Some("go") => {
                self.verify_go(&temp_path).await
            }
            _ => {
                // No verification for mismatched types
                return Ok(HookResult::Continue);
            }
        };

        match verify_result {
            Ok(VerifyOutcome::Pass) => {
                Ok(HookResult::Continue)
            }
            Ok(VerifyOutcome::Fail { errors, warnings }) => {
                // Build detailed error message for AI correction
                let reason = if errors.is_empty() {
                    format!("⚠️ 代码验证发现警告，建议检查：\n{}", warnings.join("\n"))
                } else {
                    format!("❌ 代码验证失败，请修正以下错误后再写入：\n{}", errors.join("\n"))
                };

                let details = if !warnings.is_empty() && !errors.is_empty() {
                    Some(format!("警告:\n{}\n\n错误:\n{}",
                        warnings.join("\n"),
                        errors.join("\n")))
                } else if !warnings.is_empty() {
                    Some(format!("警告:\n{}", warnings.join("\n")))
                } else {
                    None
                };

                Ok(HookResult::Block { reason, details })
            }
            Err(e) => {
                // Verification tool not available - don't block
                log::warn!("Code verification failed: {}", e);
                Ok(HookResult::Continue)
            }
        }
    }

    /// Verify Rust code with rustfmt and rustc
    async fn verify_rust(&self, path: &Path) -> Result<VerifyOutcome> {
        // 1. Quick syntax check with rustfmt --check
        let fmt_output = tokio::process::Command::new("rustfmt")
            .arg("--check")
            .arg(path)
            .output()
            .await;

        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Check formatting
        match fmt_output {
            Ok(o) if !o.status.success() => {
                // Format issues - treat as warning, not blocking
                let stderr = String::from_utf8_lossy(&o.stderr);
                if !stderr.is_empty() {
                    warnings.push(format!("格式问题: 建议运行 rustfmt"));
                }
            }
            Err(_) => {
                // rustfmt not available - skip format check
            }
            _ => {}
        }

        // 2. Syntax check with rustc (fast, no full compilation)
        // For single file, we can't do full cargo check, but we can catch syntax errors
        let syntax_output = tokio::process::Command::new("rustc")
            .arg("--edition=2021")
            .arg("--emit=metadata")
            .arg("-o")
            .arg("/dev/null")  // We just want to check syntax
            .arg(path)
            .output()
            .await;

        match syntax_output {
            Ok(o) if !o.status.success() => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                for line in stderr.lines() {
                    if line.contains("error") {
                        errors.push(line.to_string());
                    } else if line.contains("warning") {
                        warnings.push(line.to_string());
                    }
                }
            }
            Err(_) => {
                // rustc not available - try cargo check
                // This might not work for single file, but let's try
            }
            _ => {}
        }

        // 3. If we have project context, run cargo check
        if errors.is_empty() {
            if let Some(root) = &self.project_root {
                let cargo_output = tokio::process::Command::new("cargo")
                    .args(["check", "--quiet"])
                    .current_dir(root.as_ref())
                    .output()
                    .await;

                match cargo_output {
                    Ok(o) if !o.status.success() => {
                        let stderr = String::from_utf8_lossy(&o.stderr);
                        for line in stderr.lines().filter(|l| l.contains("error")) {
                            errors.push(line.to_string());
                        }
                    }
                    Err(_) => {}
                    _ => {}
                }
            }
        }

        if errors.is_empty() && warnings.is_empty() {
            Ok(VerifyOutcome::Pass)
        } else {
            Ok(VerifyOutcome::Fail { errors, warnings })
        }
    }

    /// Verify TypeScript code with tsc
    async fn verify_typescript(&self, path: &Path) -> Result<VerifyOutcome> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // For single file verification, we use tsc with the file
        // Note: This may require a tsconfig.json in the temp directory
        // For quick check, we just verify syntax

        // Try to run tsc
        let tsc_output = tokio::process::Command::new("npx")
            .args(["tsc", "--noEmit", "--skipLibCheck"])
            .arg(path)
            .output()
            .await;

        match tsc_output {
            Ok(o) if !o.status.success() => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                let stdout = String::from_utf8_lossy(&o.stdout);

                for line in stderr.lines().chain(stdout.lines()) {
                    if line.contains("error TS") {
                        errors.push(line.to_string());
                    }
                }
            }
            Err(_) => {
                // tsc not available - skip
                warnings.push("tsc 不可用，跳过 TypeScript 验证".to_string());
            }
            _ => {}
        }

        // If we have project context, run project-level check
        if errors.is_empty() {
            if let Some(root) = &self.project_root {
                let project_output = tokio::process::Command::new("npx")
                    .args(["tsc", "--noEmit"])
                    .current_dir(root.as_ref())
                    .output()
                    .await;

                match project_output {
                    Ok(o) if !o.status.success() => {
                        let stderr = String::from_utf8_lossy(&o.stderr);
                        for line in stderr.lines().filter(|l| l.contains("error TS")) {
                            errors.push(line.to_string());
                        }
                    }
                    Err(_) => {}
                    _ => {}
                }
            }
        }

        if errors.is_empty() && warnings.is_empty() {
            Ok(VerifyOutcome::Pass)
        } else {
            Ok(VerifyOutcome::Fail { errors, warnings })
        }
    }

    /// Verify Python code with python -m py_compile
    async fn verify_python(&self, path: &Path) -> Result<VerifyOutcome> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Quick syntax check
        let output = tokio::process::Command::new("python")
            .args(["-m", "py_compile"])
            .arg(path)
            .output()
            .await;

        match output {
            Ok(o) if !o.status.success() => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                for line in stderr.lines() {
                    if line.contains("SyntaxError") || line.contains("Error") {
                        errors.push(line.to_string());
                    }
                }
            }
            Err(_) => {
                warnings.push("python 不可用，跳过语法验证".to_string());
            }
            _ => {}
        }

        if errors.is_empty() && warnings.is_empty() {
            Ok(VerifyOutcome::Pass)
        } else {
            Ok(VerifyOutcome::Fail { errors, warnings })
        }
    }

    /// Verify Go code with go vet
    async fn verify_go(&self, path: &Path) -> Result<VerifyOutcome> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Go vet for single file
        let output = tokio::process::Command::new("go")
            .args(["vet"])
            .arg(path)
            .output()
            .await;

        match output {
            Ok(o) if !o.status.success() => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                for line in stderr.lines() {
                    if line.contains("error") || line.contains("undefined") {
                        errors.push(line.to_string());
                    }
                }
            }
            Err(_) => {
                warnings.push("go vet 不可用，跳过验证".to_string());
            }
            _ => {}
        }

        // gofmt check (formatting)
        let fmt_output = tokio::process::Command::new("gofmt")
            .args(["-l"])
            .arg(path)
            .output()
            .await;

        match fmt_output {
            Ok(o) if !o.stdout.is_empty() => {
                warnings.push("格式问题: 建议运行 gofmt".to_string());
            }
            Err(_) => {}
            _ => {}
        }

        if errors.is_empty() && warnings.is_empty() {
            Ok(VerifyOutcome::Pass)
        } else {
            Ok(VerifyOutcome::Fail { errors, warnings })
        }
    }
}

/// Verification outcome
#[derive(Debug, Clone)]
enum VerifyOutcome {
    /// Verification passed
    Pass,
    /// Verification failed with errors/warnings
    Fail {
        errors: Vec<String>,
        warnings: Vec<String>,
    },
}

#[async_trait]
impl ToolHook for CodeQualityHook {
    fn name(&self) -> &str {
        "code_quality"
    }

    fn is_enabled(&self) -> bool {
        self.enabled && self.strategy != VerificationStrategy::None
    }

    fn applies_to(&self) -> Vec<&str> {
        vec!["write", "edit", "multi_edit"]
    }

    async fn pre_execute(&self, tool_name: &str, params: &Value) -> Result<HookResult> {
        // Only run pre-verification for write tool with pre strategy
        if self.strategy != VerificationStrategy::Pre &&
           self.strategy != VerificationStrategy::PreQuick {
            return Ok(HookResult::Continue);
        }

        // Get path and content from params
        let path = params["path"].as_str().ok_or_else(||
            anyhow::anyhow!("missing 'path' in params"))?;

        let content = params["content"].as_str().ok_or_else(||
            anyhow::anyhow!("missing 'content' in params"))?;

        // For edit/multi_edit, we need to apply the edit first to get full content
        // This is complex, so we skip pre-verification for edits
        if tool_name != "write" {
            return Ok(HookResult::Continue);
        }

        self.verify_before_write(path, content).await
    }

    async fn post_execute(&self, _tool_name: &str, _params: &Value, result: &str) -> Result<String> {
        // Post-verification is handled by WriteTool's own run_code_verification
        // This hook just adds additional context if needed

        if self.strategy == VerificationStrategy::None {
            return Ok(result.to_string());
        }

        // Add hook signature to result (for debugging)
        Ok(format!("{}\n[code_quality_hook: strategy={}]", result, self.strategy.to_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verification_strategy_parse() {
        assert_eq!(VerificationStrategy::from_str("none"), VerificationStrategy::None);
        assert_eq!(VerificationStrategy::from_str("post"), VerificationStrategy::Post);
        assert_eq!(VerificationStrategy::from_str("pre"), VerificationStrategy::Pre);
        assert_eq!(VerificationStrategy::from_str("pre-quick"), VerificationStrategy::PreQuick);
        assert_eq!(VerificationStrategy::from_str("invalid"), VerificationStrategy::Post);
    }

    #[test]
    fn test_is_code_file() {
        assert!(CodeQualityHook::is_code_file("test.rs"));
        assert!(CodeQualityHook::is_code_file("test.ts"));
        assert!(CodeQualityHook::is_code_file("test.py"));
        assert!(CodeQualityHook::is_code_file("test.go"));
        assert!(!CodeQualityHook::is_code_file("test.txt"));
        assert!(!CodeQualityHook::is_code_file("test.md"));
    }

    #[test]
    fn test_hook_applies_to() {
        let hook = CodeQualityHook::default();
        let applies_to = hook.applies_to();
        assert!(applies_to.contains(&"write"));
        assert!(applies_to.contains(&"edit"));
        assert!(applies_to.contains(&"multi_edit"));
        assert!(!applies_to.contains(&"read"));
    }

    #[tokio::test]
    async fn test_hook_disabled() {
        let hook = CodeQualityHook::new(VerificationStrategy::None);
        assert!(!hook.is_enabled());

        let result = hook.pre_execute("write", &serde_json::json!({
            "path": "test.rs",
            "content": "fn main() {}"
        })).await;

        assert!(matches!(result.unwrap(), HookResult::Continue));
    }
}