//! LSP Integration Demo
//!
//! 演示如何使用 LSP 工具：
//! - lsp_hover: 获取类型签名和文档
//! - lsp_definition: 跳转到定义
//! - lsp_references: 查找所有引用
//! - lsp_diagnostics: 获取诊断信息

use std::path::PathBuf;
use std::sync::Arc;

use matrixcode_core::lsp::{LspClientRegistry, LspServerConfig, lsp_tools};
use matrixcode_core::tools::Tool;

#[tokio::main]
async fn main() {
    println!("=== LSP Integration Demo ===\n");

    // 1. 创建 LSP Registry
    let registry = Arc::new(LspClientRegistry::new());
    println!("✓ LSP Client Registry created");

    // 2. 配置 rust-analyzer
    let config = LspServerConfig::new("rust-analyzer", "rust");
    
    // 使用绝对路径（项目根目录）
    let project_root = std::env::current_dir()
        .expect("Failed to get current directory")
        .parent()
        .expect("Failed to get parent directory")
        .to_path_buf();
    
    println!("📁 Project root: {}", project_root.display());
    println!("⏳ Starting rust-analyzer...");
    
    // 3. 启动 LSP 服务器（可能需要等待）
    println!("⏳ Starting rust-analyzer...");
    let start = std::time::Instant::now();
    
    match registry.register(&config, &project_root).await {
        Ok(_) => {
            let elapsed = start.elapsed();
            println!("✓ rust-analyzer started successfully in {:.2}s", elapsed.as_secs_f64());
        }
        Err(e) => {
            println!("✗ Failed to start rust-analyzer: {}", e);
            println!("  提示：确保 rust-analyzer 已安装");
            return;
        }
    }

    // 4. 创建 LSP 工具
    let tools: Vec<Box<dyn Tool>> = lsp_tools(registry.clone());
    println!("✓ Created {} LSP tools", tools.len());

    // 5. 等待 LSP 就绪（rust-analyzer 需要时间初始化项目）
    println!("⏳ Waiting for rust-analyzer to initialize...");
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // 6. 测试 hover - 获取类型信息
    println!("\n=== Test 1: lsp_hover ===");
    test_hover(&registry).await;

    // 7. 测试 definition - 跳转到定义
    println!("\n=== Test 2: lsp_definition ===");
    test_definition(&registry).await;

    // 8. 测试 references - 查找引用
    println!("\n=== Test 3: lsp_references ===");
    test_references(&registry).await;

    // 9. 测试 diagnostics - 获取诊断
    println!("\n=== Test 4: lsp_diagnostics ===");
    test_diagnostics(&registry).await;

    // 10. 关闭所有 LSP 客户端
    println!("\n=== Cleanup ===");
    match registry.shutdown_all().await {
        Ok(_) => println!("✓ All LSP clients shutdown"),
        Err(e) => println!("✗ Shutdown error: {}", e),
    }

    println!("\n=== Demo Complete ===");
}

async fn test_hover(registry: &Arc<LspClientRegistry>) {
    use matrixcode_core::lsp::LspHoverTool;
    use matrixcode_core::tools::Tool;
    use serde_json::json;

    let tool = LspHoverTool::new(registry.clone());
    
    // 测试 core/src/lib.rs 第 33 行第 8 列的 Agent
    let file = PathBuf::from("core/src/lib.rs");
    let params = json!({
        "file": file.to_string_lossy(),
        "line": 33,  // pub use agent::{Agent, AgentBuilder};
        "column": 8  // Agent 的位置
    });

    println!("  Querying hover info for Agent in core/src/lib.rs:33:8");
    
    match tool.execute(params).await {
        Ok(result) => {
            println!("  ✓ Hover result:");
            println!("    {}", result);
        }
        Err(e) => {
            println!("  ✗ Hover failed: {}", e);
            println!("    可能原因：LSP 还在初始化或位置无效");
        }
    }
}

async fn test_definition(registry: &Arc<LspClientRegistry>) {
    use matrixcode_core::lsp::LspDefinitionTool;
    use serde_json::json;

    let tool = LspDefinitionTool::new(registry.clone());
    
    // 测试 core/src/lib.rs 第 33 行的 Agent
    let file = PathBuf::from("core/src/lib.rs");
    let params = json!({
        "file": file.to_string_lossy(),
        "line": 33,
        "column": 8
    });

    println!("  Finding definition of Agent in core/src/lib.rs:33:8");
    
    match tool.execute(params).await {
        Ok(result) => {
            println!("  ✓ Definition location:");
            println!("    {}", result);
        }
        Err(e) => {
            println!("  ✗ Definition failed: {}", e);
        }
    }
}

async fn test_references(registry: &Arc<LspClientRegistry>) {
    use matrixcode_core::lsp::LspReferencesTool;
    use serde_json::json;

    let tool = LspReferencesTool::new(registry.clone());
    
    // 测试查找 Agent 的所有引用
    let file = PathBuf::from("core/src/agent/mod.rs");
    let params = json!({
        "file": file.to_string_lossy(),
        "line": 45,  // pub use types::{Agent, AgentBuilder};
        "column": 12,
        "include_declaration": true
    });

    println!("  Finding all references to Agent");
    
    match tool.execute(params).await {
        Ok(result) => {
            println!("  ✓ References found:");
            println!("    {}", result);
        }
        Err(e) => {
            println!("  ✗ References failed: {}", e);
        }
    }
}

async fn test_diagnostics(registry: &Arc<LspClientRegistry>) {
    use matrixcode_core::lsp::LspDiagnosticsTool;
    use serde_json::json;

    let tool = LspDiagnosticsTool::new(registry.clone());
    
    // 测试获取诊断信息
    let file = PathBuf::from("core/src/lib.rs");
    let params = json!({
        "file": file.to_string_lossy()
    });

    println!("  Getting diagnostics for core/src/lib.rs");
    
    match tool.execute(params).await {
        Ok(result) => {
            println!("  ✓ Diagnostics:");
            println!("    {}", result);
        }
        Err(e) => {
            println!("  ✗ Diagnostics failed: {}", e);
        }
    }
}