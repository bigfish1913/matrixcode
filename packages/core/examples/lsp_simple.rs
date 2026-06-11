//! Simple LSP Test
//!
//! 简单测试 LSP 启动和初始化

use matrixcode_core::lsp::{LspClient, LspServerConfig};

#[tokio::main]
async fn main() {
    println!("=== Simple LSP Test ===\n");

    // 1. 创建 LSP 客户端
    let project_root = std::env::current_dir()
        .expect("Failed to get current directory")
        .parent()
        .expect("Failed to get parent directory")
        .to_path_buf();
    
    println!("📁 Project root: {}", project_root.display());
    
    let config = LspServerConfig::new("rust-analyzer", "rust");
    let client = LspClient::from_config(&config, project_root);
    
    // 2. 启动并初始化
    println!("⏳ Starting rust-analyzer...");
    let start = std::time::Instant::now();
    
    match client.spawn(&config).await {
        Ok(_) => {
            let elapsed = start.elapsed();
            println!("✓ rust-analyzer initialized in {:.2}s", elapsed.as_secs_f64());
        }
        Err(e) => {
            println!("✗ Failed to start: {}", e);
            println!("\n错误详情:");
            for cause in e.chain() {
                println!("  - {}", cause);
            }
            return;
        }
    }

    // 3. 关闭
    println!("\n⏳ Shutting down...");
    match client.shutdown().await {
        Ok(_) => println!("✓ Shutdown successful"),
        Err(e) => println!("✗ Shutdown error: {}", e),
    }

    println!("\n=== Test Complete ===");
}