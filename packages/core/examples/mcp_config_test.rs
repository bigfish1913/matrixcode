//! MCP Configuration Integration Example
//!
//! Demonstrates how to connect MCP servers from config and use their tools.
//!
//! Usage:
//!   cargo run --example mcp_config_test
//!
//! Config example (~/.matrix/config.json):
//!   {
//!     "provider": "anthropic",
//!     "api_key": "your-key",
//!     "model": "claude-sonnet-4-20250514",
//!     "mcp_servers": {
//!       "playwright": {
//!         "command": "cmd",
//!         "args": ["/C", "npx", "-y", "@playwright/mcp@latest"],
//!         "enabled": true
//!       }
//!     }
//!   }

use matrixcode_core::mcp::connect_mcp_servers_from_config;
use matrixcode_core::config::MatrixConfig;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== MCP Config Integration Test ===\n");
    
    // Load config
    let config = MatrixConfig::load();
    
    // Check if MCP servers are configured
    let mcp_servers = config.mcp_servers.clone()
        .unwrap_or_else(|| {
            // Default: add Playwright MCP
            let mut servers = HashMap::new();
            servers.insert(
                "playwright".to_string(),
                matrixcode_core::mcp::McpServerConfig::stdio(
                    "cmd",
                    vec!["/C".into(), "npx".into(), "-y".into(), "@playwright/mcp@latest".into()]
                ),
            );
            servers
        });
    
    println!("MCP servers configured:");
    for (name, cfg) in &mcp_servers {
        let cmd = cfg.command.as_deref().unwrap_or("N/A");
        let args = cfg.args.join(" ");
        println!("  - {}: {} {}", name, cmd, args);
    }
    println!();
    
    // Connect MCP servers
    println!("Connecting MCP servers...");
    let (tools, manager) = connect_mcp_servers_from_config(&mcp_servers).await?;
    
    println!("✓ Connected {} MCP servers", manager.server_count().await);
    println!("✓ Loaded {} tools total\n", tools.len());
    
    // List all tools
    println!("Available MCP tools:");
    for tool in &tools {
        let def = tool.definition();
        println!("  - {}:", def.name);
        if let Some(desc) = def.description.lines().next() {
            println!("    {}", desc);
        }
    }
    println!();
    
    // Test tool call (browser_navigate)
    println!("=== Testing Tool Call ===");
    
    // Find browser_navigate tool
    let navigate_tool = tools.iter()
        .find(|t| t.definition().name.contains("browser_navigate"));
    
    if let Some(tool) = navigate_tool {
        println!("Calling browser_navigate with URL: https://example.com");
        
        let params = serde_json::json!({
            "url": "https://example.com"
        });
        
        let result = tool.execute(params).await;
        match result {
            Ok(output) => {
                println!("✓ Tool call result:");
                println!("{}", output);
            }
            Err(e) => {
                println!("✗ Tool call error: {}", e);
            }
        }
    } else {
        println!("browser_navigate tool not found");
    }
    
    // Shutdown
    println!("\nShutting down MCP servers...");
    manager.shutdown().await;
    println!("✓ Done!");
    
    Ok(())
}