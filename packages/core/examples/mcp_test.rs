//! MCP Integration Test
//!
//! 测试 MCP 客户端连接 Playwright MCP Server

use matrixcode_core::mcp::{Content, McpClient, McpServerConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== MCP Client Test ===\n");

    // 1. 创建配置
    // Windows 需要通过 cmd.exe 运行 npx.cmd
    let config = McpServerConfig::stdio(
        "cmd",
        vec![
            "/C".into(),
            "npx".into(),
            "-y".into(),
            "@playwright/mcp@latest".into(),
        ],
    );

    println!("Config: npx -y @playwright/mcp@latest");

    // 2. 创建传输配置
    let transport = config.to_transport_config()?;

    // 3. 连接 MCP 服务器
    println!("\nConnecting to Playwright MCP Server...");

    let client = McpClient::connect("playwright", transport).await?;

    println!("✓ Connected!");

    // 4. 获取服务器信息
    if let Some(info) = client.server_info().await {
        println!("  Server: {} v{}", info.name, info.version);
    }

    // 5. 检查工具支持
    if client.supports_tools().await {
        println!("  Supports tools: Yes");

        // 6. 列出工具
        println!("\nFetching tools...");
        let tools = client.list_tools().await?;

        println!("✓ Found {} tools:\n", tools.len());
        for tool in &tools {
            println!("  - {}:", tool.name);
            if let Some(desc) = &tool.description {
                println!("    {}", desc.lines().next().unwrap_or(""));
            }
        }

        // 7. 测试调用一个工具
        println!("\n=== Testing Tool Call ===");
        println!("Calling browser_navigate with URL: https://example.com");

        let result = client
            .call_tool(
                "browser_navigate",
                Some(serde_json::json!({
                    "url": "https://example.com"
                })),
            )
            .await?;

        println!("✓ Tool call result:");
        for content in &result.content {
            match content {
                Content::Text { text } => {
                    println!("  {}", text);
                }
                _ => {
                    println!("  [Non-text content]");
                }
            }
        }

        // 8. 截图测试
        println!("\nCalling browser_screenshot...");
        let result = client.call_tool("browser_screenshot", None).await?;

        println!("✓ Screenshot result:");
        for content in &result.content {
            match content {
                Content::Image { mime_type, .. } => {
                    println!("  Image captured: {}", mime_type);
                }
                Content::Text { text } => {
                    println!("  {}", text);
                }
                _ => {}
            }
        }

        // 9. 关闭浏览器
        println!("\nClosing browser...");
        client.call_tool("browser_close", None).await?;
        println!("✓ Browser closed");
    }

    // 10. 关闭连接
    println!("\nShutting down...");
    client.shutdown().await?;
    println!("✓ Done!");

    Ok(())
}
