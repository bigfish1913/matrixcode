use matrixcode_core::mcp::{McpConfig, McpToolManager, load_mcp_config};
use std::path::Path;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    
    println!("=== MCP Playwright Test ===\n");
    
    // 1. Load config
    println!("1. Loading MCP config...");
    let config = load_mcp_config(Path::new("."));
    
    println!("   Found {} MCP server(s) configured", config.servers.len());
    for (name, server) in &config.servers {
        println!("   - {}: {} (enabled: {})", name, server.command, server.enabled);
    }
    
    if config.servers.is_empty() {
        println!("\n   No MCP servers configured, using default Playwright config...");
        let config = McpConfig::new()
            .add_server("playwright", matrixcode_core::mcp::McpServerConfig::stdio(
                "npx",
                vec!["-y".into(), "@playwright/mcp@latest".into()]
            ));
        println!("   Created config with {} server(s)", config.servers.len());
    }
    
    // 2. Create tool manager
    println!("\n2. Creating MCP Tool Manager...");
    let manager = McpToolManager::new();
    
    // 3. Connect to servers
    println!("\n3. Connecting to MCP servers...");
    let mut total_tools = 0;
    
    for (key, server_config) in config.enabled_servers() {
        let name = server_config.get_name(&key);
        println!("   Connecting to '{}'...", name);
        
        match server_config.to_transport_config() {
            Ok(transport) => {
                match manager.connect_server(&name, transport).await {
                    Ok(tools) => {
                        println!("   ✓ Connected to '{}' with {} tools", name, tools.len());
                        total_tools += tools.len();
                    }
                    Err(e) => {
                        eprintln!("   ✗ Failed to connect to '{}': {}", name, e);
                    }
                }
            }
            Err(e) => {
                eprintln!("   ✗ Invalid config for '{}': {}", key, e);
            }
        }
    }
    
    // 4. List all discovered tools
    println!("\n4. Discovered tools:");
    let tools = manager.get_tools().await;
    
    if tools.is_empty() {
        println!("   No tools discovered");
    } else {
        println!("   Total {} tool(s):", tools.len());
        for tool in &tools {
            let def = tool.definition();
            let desc = &def.description;
            let short_desc = if desc.len() > 80 {
                format!("{}...", &desc[..77])
            } else {
                desc.clone()
            };
            println!("   - {} : {}", def.name, short_desc);
        }
    }
    
    // 5. Shutdown
    println!("\n5. Shutting down...");
    manager.shutdown().await;
    
    println!("\n=== Test Complete ===");
    println!("Total servers: {}", config.servers.len());
    println!("Total tools discovered: {}", total_tools);
}