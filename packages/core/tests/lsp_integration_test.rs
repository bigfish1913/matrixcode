//! LSP Integration Test
//!
//! 测试 LSP 工具的实际执行能力（不需要真实 LSP 服务器）

use std::sync::Arc;
use std::path::PathBuf;
use matrixcode_core::lsp::{LspClientRegistry, LspServerConfig};
use matrixcode_core::tools::Tool;
use serde_json::json;

#[tokio::test]
async fn test_lsp_registry_creation() {
    let registry = Arc::new(LspClientRegistry::new());
    
    // 测试初始状态
    assert!(!registry.has_active_clients().await);
    assert_eq!(registry.active_languages().await.len(), 0);
}

#[test]
fn test_lsp_config_creation() {
    let config = LspServerConfig::new("rust-analyzer", "rust");
    
    assert_eq!(config.command, "rust-analyzer");
    assert_eq!(config.language, "rust");
    assert!(config.enabled);
    assert_eq!(config.args.len(), 0);
}

#[test]
fn test_lsp_config_with_args() {
    let config = LspServerConfig::new("typescript-language-server", "typescript")
        .with_args(vec!["--stdio".to_string()]);
    
    assert_eq!(config.args.len(), 1);
    assert_eq!(config.args[0], "--stdio");
}

#[tokio::test]
#[ignore] // 需要真实的 LSP 服务器，跳过
async fn test_lsp_registry_register() {
    let registry = Arc::new(LspClientRegistry::new());
    let config = LspServerConfig::new("rust-analyzer", "rust");
    let project_root = std::env::current_dir().unwrap();
    
    // 注册应该成功（即使 rust-analyzer 不存在，spawn 会失败）
    // 但这里我们测试 registry 的基本功能
    let result = registry.register(&config, &project_root).await;
    
    // 可能失败（如果 rust-analyzer 未安装），但 registry 应该能处理
    match result {
        Ok(_) => {
            // 成功启动
            assert!(registry.has_active_clients().await);
            assert!(registry.get_client("rust").await.is_some());
        }
        Err(_) => {
            // 启动失败（rust-analyzer 未安装或其他问题）
            // 这是正常的，不影响 registry 功能测试
        }
    }
}

#[tokio::test]
async fn test_lsp_hover_tool_without_client() {
    use matrixcode_core::lsp::LspHoverTool;
    
    let registry = Arc::new(LspClientRegistry::new());
    let tool = LspHoverTool::new(registry);
    
    // 测试没有 LSP 客户端时的行为
    let file = PathBuf::from("test.rs");
    let params = json!({
        "file": file.to_string_lossy(),
        "line": 0,
        "column": 0
    });
    
    let result = tool.execute(params).await;
    
    // 应该返回错误（没有可用的 LSP 客户端）
    assert!(result.is_err());
}

#[tokio::test]
async fn test_lsp_definition_tool_without_client() {
    use matrixcode_core::lsp::LspDefinitionTool;
    
    let registry = Arc::new(LspClientRegistry::new());
    let tool = LspDefinitionTool::new(registry);
    
    let file = PathBuf::from("test.rs");
    let params = json!({
        "file": file.to_string_lossy(),
        "line": 0,
        "column": 0
    });
    
    let result = tool.execute(params).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_lsp_references_tool_without_client() {
    use matrixcode_core::lsp::LspReferencesTool;
    
    let registry = Arc::new(LspClientRegistry::new());
    let tool = LspReferencesTool::new(registry);
    
    let file = PathBuf::from("test.rs");
    let params = json!({
        "file": file.to_string_lossy(),
        "line": 0,
        "column": 0,
        "include_declaration": true
    });
    
    let result = tool.execute(params).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_lsp_diagnostics_tool_without_client() {
    use matrixcode_core::lsp::LspDiagnosticsTool;
    
    let registry = Arc::new(LspClientRegistry::new());
    let tool = LspDiagnosticsTool::new(registry);
    
    let file = PathBuf::from("test.rs");
    let params = json!({
        "file": file.to_string_lossy()
    });
    
    let result = tool.execute(params).await;
    assert!(result.is_err());
}