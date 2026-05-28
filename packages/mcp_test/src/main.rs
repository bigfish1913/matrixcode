use matrixcode_core::mcp::{McpToolManager, McpServerConfig, McpConfig};
use std::time::Duration;
use tokio::time::timeout;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    
    println!("=== Playwright MCP Test ===\n");
    
    // 1. Create MCP config with Playwright
    println!("1. Creating Playwright MCP config...");
    let config = McpConfig::new()
        .add_server("playwright", McpServerConfig::stdio(
            "npx",
            vec!["-y".into(), "@playwright/mcp@latest".into()]
        ));
    
    println!("   Config: {} server(s)", config.servers.len());
    
    // 2. Create tool manager
    println!("\n2. Creating MCP Tool Manager...");
    let manager = McpToolManager::new();
    
    // 3. Connect to Playwright MCP server
    println!("\n3. Connecting to Playwright MCP server...");
    println!("   (This may take 10-20 seconds for first run)");
    
    let connect_result = timeout(
        Duration::from_secs(60),
        manager.connect_server("playwright", config.servers.get("playwright")
            .unwrap()
            .to_transport_config()
            .unwrap())
    ).await;
    
    match connect_result {
        Ok(Ok(tools)) => {
            println!("\n✓ Connected successfully!");
            println!("   Discovered {} tools:", tools.len());
            
            for (i, tool) in tools.iter().enumerate() {
                let def = tool.definition();
                let desc = if def.description.len() > 60 {
                    format!("{}...", &def.description[..57])
                } else {
                    def.description.clone()
                };
                println!("   {:2}. {} - {}", i + 1, def.name, desc);
            }
            
            // Shutdown
            println!("\n4. Shutting down...");
            manager.shutdown().await;
            println!("   ✓ MCP server stopped");
            
            println!("\n=== Test Complete ===");
            println!("Total tools: {}", tools.len());
        }
        Ok(Err(e)) => {
            eprintln!("\n✗ Connection failed: {}", e);
            manager.shutdown().await;
        }
        Err(_) => {
            eprintln!("\n✗ Connection timeout after 60 seconds");
            manager.shutdown().await;
        }
    }
}