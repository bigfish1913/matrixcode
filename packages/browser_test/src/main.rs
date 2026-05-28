use matrixcode_core::mcp::{McpConfig, McpServerConfig, McpToolManager};
use matrixcode_core::tools::Tool;
use serde_json::json;
use std::time::Instant;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    
    println!("=== MCP Everything Search Test ===\n");
    
    let start = Instant::now();
    
    // Test with @modelcontextprotocol/server-everything (simple echo server)
    println!("1. Setting up MCP Everything Search...");
    let config = McpConfig::new()
        .add_server("everything", McpServerConfig::stdio(
            "npx",
            vec!["-y".into(), "@modelcontextprotocol/server-everything".into()]
        ));
    
    let manager = McpToolManager::new();
    
    let mut tools = Vec::new();
    for (key, server_config) in config.enabled_servers() {
        let name = server_config.get_name(&key);
        match server_config.to_transport_config() {
            Ok(transport) => {
                match manager.connect_server(&name, transport).await {
                    Ok(t) => tools.extend(t),
                    Err(e) => {
                        eprintln!("Failed to connect: {}", e);
                        return;
                    }
                }
            }
            Err(e) => {
                eprintln!("Transport config error: {}", e);
                return;
            }
        }
    }
    
    println!("   ✓ Connected with {} tools in {:.2}s", tools.len(), start.elapsed().as_secs_f64());
    
    // List tools
    println!("\n2. Available tools:");
    for tool in &tools {
        let def = tool.definition();
        println!("   - {} : {}", def.name, def.description.chars().take(60).collect::<String>());
    }
    
    // Try echo tool
    let echo_tool = tools.iter().find(|t| t.definition().name == "echo");
    if let Some(tool) = echo_tool {
        println!("\n3. Testing echo tool...");
        match tool.execute(json!({"message": "Hello from Rust MCP client!"})).await {
            Ok(result) => println!("   ✓ Result: {}", result),
            Err(e) => eprintln!("   ✗ Error: {}", e),
        }
    }
    
    manager.shutdown().await;
    println!("\n=== Test Complete ({:.2}s) ===", start.elapsed().as_secs_f64());
}
