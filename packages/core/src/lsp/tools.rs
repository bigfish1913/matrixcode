//! LSP Tools for AI Agents
//!
//! Provides 4 LSP tools for AI to call:
//! - `lsp_hover`: Get type signature and documentation
//! - `lsp_definition`: Jump to definition
//! - `lsp_references`: Find all references
//! - `lsp_diagnostics`: Get diagnostics for a file

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use lsp_types::Position;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;

use crate::approval::RiskLevel;
use crate::tools::{Tool, ToolDefinition};
use super::client::{format_diagnostic, format_hover_result, format_location, path_to_uri, LspClient};
use super::registry::LspClientRegistry;

// ============================================================================
// Helper Functions
// ============================================================================

/// Detect language identifier from file path extension
pub fn detect_language_from_path(path: &PathBuf) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    let language = match ext.as_str() {
        // Rust
        "rs" => "rust",
        // TypeScript/JavaScript
        "ts" => "typescript",
        "tsx" => "typescript",
        "js" => "javascript",
        "jsx" => "javascript",
        "mjs" => "javascript",
        "cjs" => "javascript",
        // Python
        "py" => "python",
        "pyi" => "python",
        // Go
        "go" => "go",
        // C/C++
        "c" => "c",
        "h" => "c",
        "cpp" => "cpp",
        "hpp" => "cpp",
        "cc" => "cpp",
        "cxx" => "cpp",
        // Java
        "java" => "java",
        // C#
        "cs" => "csharp",
        // Ruby
        "rb" => "ruby",
        // PHP
        "php" => "php",
        // Swift
        "swift" => "swift",
        // Kotlin
        "kt" => "kotlin",
        "kts" => "kotlin",
        // Scala
        "scala" => "scala",
        // Lua
        "lua" => "lua",
        // Rust
        "vue" => "vue",
        // Svelte
        "svelte" => "svelte",
        // HTML/CSS
        "html" => "html",
        "htm" => "html",
        "css" => "css",
        "scss" => "scss",
        "less" => "less",
        // JSON/YAML/TOML
        "json" => "json",
        "yaml" => "yaml",
        "yml" => "yaml",
        "toml" => "toml",
        // Markdown
        "md" => "markdown",
        // Shell
        "sh" => "shell",
        "bash" => "shell",
        "zsh" => "shell",
        // Other
        _ => return None,
    };
    Some(language.to_string())
}

// ============================================================================
// LSP Hover Tool
// ============================================================================

/// Tool for getting hover information (type signature and documentation)
pub struct LspHoverTool {
    registry: Arc<LspClientRegistry>,
}

impl LspHoverTool {
    pub fn new(registry: Arc<LspClientRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl Tool for LspHoverTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "lsp_hover".to_string(),
            description: "获取指定位置的类型签名和文档。返回类型信息、函数签名、文档注释等。需要 LSP 服务器支持。".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file": {
                        "type": "string",
                        "description": "文件路径（绝对路径）"
                    },
                    "line": {
                        "type": "integer",
                        "description": "行号（0-based）"
                    },
                    "column": {
                        "type": "integer",
                        "description": "列号（0-based）"
                    }
                },
                "required": ["file", "line", "column"]
            }),
            is_priority: false,
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let file_path = params["file"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 'file' 参数"))?;
        let line = params["line"]
            .as_u64()
            .ok_or_else(|| anyhow!("缺少 'line' 参数"))? as u32;
        let column = params["column"]
            .as_u64()
            .ok_or_else(|| anyhow!("缺少 'column' 参数"))? as u32;

        let path = PathBuf::from(file_path);
        let language = detect_language_from_path(&path)
            .ok_or_else(|| anyhow!("无法识别文件类型: {}", file_path))?;

        let client = self.registry.get_client(&language).await
            .ok_or_else(|| anyhow!("语言 '{}' 的 LSP 客户端未启动", language))?;

        // Open file first
        let uri = path_to_uri(&path)?;
        let content = tokio::fs::read_to_string(&path).await
            .map_err(|e| anyhow!("读取文件失败: {}", e))?;
        client.open_file(&uri, &content).await?;

        // Get hover
        let position = Position { line, character: column };
        let hover = client.hover(&uri, position).await?
            .ok_or_else(|| anyhow!("该位置没有 hover 信息"))?;

        // Format output
        let mut result = format!("【类型】{}\n", hover.signature);
        if let Some(doc) = &hover.documentation {
            result.push_str(&format!("【文档】{}\n", doc));
        }
        result.push_str(&format!("【来源】{}", client.server_name()));

        Ok(result)
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }
}

// ============================================================================
// LSP Definition Tool
// ============================================================================

/// Tool for jumping to definition
pub struct LspDefinitionTool {
    registry: Arc<LspClientRegistry>,
}

impl LspDefinitionTool {
    pub fn new(registry: Arc<LspClientRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl Tool for LspDefinitionTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "lsp_definition".to_string(),
            description: "跳转到定义位置。返回符号定义的文件路径、行号和列号。需要 LSP 服务器支持。".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file": {
                        "type": "string",
                        "description": "文件路径（绝对路径）"
                    },
                    "line": {
                        "type": "integer",
                        "description": "行号（0-based）"
                    },
                    "column": {
                        "type": "integer",
                        "description": "列号（0-based）"
                    }
                },
                "required": ["file", "line", "column"]
            }),
            is_priority: false,
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let file_path = params["file"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 'file' 参数"))?;
        let line = params["line"]
            .as_u64()
            .ok_or_else(|| anyhow!("缺少 'line' 参数"))? as u32;
        let column = params["column"]
            .as_u64()
            .ok_or_else(|| anyhow!("缺少 'column' 参数"))? as u32;

        let path = PathBuf::from(file_path);
        let language = detect_language_from_path(&path)
            .ok_or_else(|| anyhow!("无法识别文件类型: {}", file_path))?;

        let client = self.registry.get_client(&language).await
            .ok_or_else(|| anyhow!("语言 '{}' 的 LSP 客户端未启动", language))?;

        // Open file first
        let uri = path_to_uri(&path)?;
        let content = tokio::fs::read_to_string(&path).await
            .map_err(|e| anyhow!("读取文件失败: {}", e))?;
        client.open_file(&uri, &content).await?;

        // Get definition
        let position = Position { line, character: column };
        let locations = client.definition(&uri, position).await?;

        if locations.is_empty() {
            return Ok("未找到定义位置".to_string());
        }

        let mut result = String::new();
        if locations.len() == 1 {
            result.push_str(&format!("定义位于: {}", format_location(&locations[0])));
        } else {
            result.push_str(&format!("找到 {} 个定义位置:\n", locations.len()));
            for (i, loc) in locations.iter().enumerate() {
                result.push_str(&format!("{}. {}\n", i + 1, format_location(loc)));
            }
        }

        Ok(result)
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }
}

// ============================================================================
// LSP References Tool
// ============================================================================

/// Tool for finding all references
pub struct LspReferencesTool {
    registry: Arc<LspClientRegistry>,
}

impl LspReferencesTool {
    pub fn new(registry: Arc<LspClientRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl Tool for LspReferencesTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "lsp_references".to_string(),
            description: "查找符号的所有引用位置。返回每个引用的文件路径、行号和列号。需要 LSP 服务器支持。".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file": {
                        "type": "string",
                        "description": "文件路径（绝对路径）"
                    },
                    "line": {
                        "type": "integer",
                        "description": "行号（0-based）"
                    },
                    "column": {
                        "type": "integer",
                        "description": "列号（0-based）"
                    },
                    "include_declaration": {
                        "type": "boolean",
                        "description": "是否包含声明位置（默认 true）",
                        "default": true
                    }
                },
                "required": ["file", "line", "column"]
            }),
            is_priority: false,
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let file_path = params["file"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 'file' 参数"))?;
        let line = params["line"]
            .as_u64()
            .ok_or_else(|| anyhow!("缺少 'line' 参数"))? as u32;
        let column = params["column"]
            .as_u64()
            .ok_or_else(|| anyhow!("缺少 'column' 参数"))? as u32;
        let include_declaration = params["include_declaration"].as_bool().unwrap_or(true);

        let path = PathBuf::from(file_path);
        let language = detect_language_from_path(&path)
            .ok_or_else(|| anyhow!("无法识别文件类型: {}", file_path))?;

        let client = self.registry.get_client(&language).await
            .ok_or_else(|| anyhow!("语言 '{}' 的 LSP 客户端未启动", language))?;

        // Open file first
        let uri = path_to_uri(&path)?;
        let content = tokio::fs::read_to_string(&path).await
            .map_err(|e| anyhow!("读取文件失败: {}", e))?;
        client.open_file(&uri, &content).await?;

        // Get references
        let position = Position { line, character: column };
        let locations = client.references(&uri, position, include_declaration).await?;

        if locations.is_empty() {
            return Ok("未找到引用".to_string());
        }

        let mut result = format!("找到 {} 个引用:\n", locations.len());
        for (i, loc) in locations.iter().enumerate() {
            result.push_str(&format!("{}. {}\n", i + 1, format_location(loc)));
        }

        Ok(result)
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }
}

// ============================================================================
// LSP Diagnostics Tool
// ============================================================================

/// Tool for getting diagnostics (errors, warnings) for a file
pub struct LspDiagnosticsTool {
    registry: Arc<LspClientRegistry>,
}

impl LspDiagnosticsTool {
    pub fn new(registry: Arc<LspClientRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl Tool for LspDiagnosticsTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "lsp_diagnostics".to_string(),
            description: "获取文件的诊断信息（错误、警告等）。返回诊断类型、消息和位置。需要 LSP 服务器支持。".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file": {
                        "type": "string",
                        "description": "文件路径（绝对路径）"
                    }
                },
                "required": ["file"]
            }),
            is_priority: false,
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let file_path = params["file"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 'file' 参数"))?;

        let path = PathBuf::from(file_path);
        let language = detect_language_from_path(&path)
            .ok_or_else(|| anyhow!("无法识别文件类型: {}", file_path))?;

        let client = self.registry.get_client(&language).await
            .ok_or_else(|| anyhow!("语言 '{}' 的 LSP 客户端未启动", language))?;

        // Open file first to trigger diagnostics
        let uri = path_to_uri(&path)?;
        let content = tokio::fs::read_to_string(&path).await
            .map_err(|e| anyhow!("读取文件失败: {}", e))?;
        client.open_file(&uri, &content).await?;

        // Get diagnostics
        let diagnostics = client.diagnostics(&uri).await?;

        if diagnostics.is_empty() {
            return Ok(format!("诊断结果 ({}): 无错误或警告", file_path));
        }

        let mut result = format!("诊断结果 ({}):\n", file_path);
        for diagnostic in &diagnostics {
            let severity_icon = match diagnostic.severity {
                Some(s) => match s {
                    lsp_types::DiagnosticSeverity::ERROR => "❌",
                    lsp_types::DiagnosticSeverity::WARNING => "⚠️",
                    lsp_types::DiagnosticSeverity::INFORMATION => "ℹ️",
                    lsp_types::DiagnosticSeverity::HINT => "💡",
                    _ => "•",
                },
                None => "•",
            };

            let formatted = format_diagnostic(diagnostic);
            let start = diagnostic.range.start;
            result.push_str(&format!(
                "{} {} 位置: {}:{}\n",
                severity_icon,
                formatted,
                start.line + 1,
                start.character + 1
            ));
        }

        Ok(result)
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }
}

// ============================================================================
// Factory Function
// ============================================================================

/// Create all LSP tools with the given registry
pub fn lsp_tools(registry: Arc<LspClientRegistry>) -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(LspHoverTool::new(Arc::clone(&registry))),
        Box::new(LspDefinitionTool::new(Arc::clone(&registry))),
        Box::new(LspReferencesTool::new(Arc::clone(&registry))),
        Box::new(LspDiagnosticsTool::new(registry)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_language_from_path() {
        assert_eq!(detect_language_from_path(&PathBuf::from("test.rs")), Some("rust".to_string()));
        assert_eq!(detect_language_from_path(&PathBuf::from("test.ts")), Some("typescript".to_string()));
        assert_eq!(detect_language_from_path(&PathBuf::from("test.tsx")), Some("typescript".to_string()));
        assert_eq!(detect_language_from_path(&PathBuf::from("test.js")), Some("javascript".to_string()));
        assert_eq!(detect_language_from_path(&PathBuf::from("test.py")), Some("python".to_string()));
        assert_eq!(detect_language_from_path(&PathBuf::from("test.go")), Some("go".to_string()));
        assert_eq!(detect_language_from_path(&PathBuf::from("test.java")), Some("java".to_string()));
        assert_eq!(detect_language_from_path(&PathBuf::from("test.unknown")), None);
    }

    #[test]
    fn test_lsp_hover_tool_definition() {
        let registry = Arc::new(LspClientRegistry::new());
        let tool = LspHoverTool::new(registry);
        let def = tool.definition();
        assert_eq!(def.name, "lsp_hover");
        assert!(def.description.contains("类型签名"));
        assert_eq!(def.risk_level, RiskLevel::Safe);
    }

    #[test]
    fn test_lsp_definition_tool_definition() {
        let registry = Arc::new(LspClientRegistry::new());
        let tool = LspDefinitionTool::new(registry);
        let def = tool.definition();
        assert_eq!(def.name, "lsp_definition");
        assert!(def.description.contains("定义"));
    }

    #[test]
    fn test_lsp_references_tool_definition() {
        let registry = Arc::new(LspClientRegistry::new());
        let tool = LspReferencesTool::new(registry);
        let def = tool.definition();
        assert_eq!(def.name, "lsp_references");
        assert!(def.description.contains("引用"));
    }

    #[test]
    fn test_lsp_diagnostics_tool_definition() {
        let registry = Arc::new(LspClientRegistry::new());
        let tool = LspDiagnosticsTool::new(registry);
        let def = tool.definition();
        assert_eq!(def.name, "lsp_diagnostics");
        assert!(def.description.contains("诊断"));
    }

    #[test]
    fn test_lsp_tools_creates_all_tools() {
        let registry = Arc::new(LspClientRegistry::new());
        let tools = lsp_tools(registry);
        assert_eq!(tools.len(), 4);
    }
}