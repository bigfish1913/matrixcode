//! MCP 核心功能测试

use matrixcode_core::mcp::{McpToolRegistry, McpServerConfig};
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::test]
async fn test_registry_basic() {
    let mut registry = McpToolRegistry::new();
    println!("✅ McpToolRegistry 创建成功");
    
    let config = McpServerConfig::stdio("npx", vec!["-y".into(), "@playwright/mcp@latest".into()]);
    registry.add_server("playwright".into(), config);
    println!("✅ 添加 server 成功");
    
    let status = registry.server_status().await;
    assert_eq!(status.len(), 1);
    assert!(status.contains_key("playwright"));
    println!("✅ server_status() 工作正常");
}

#[tokio::test]
async fn test_registry_multiple_servers() {
    let mut registry = McpToolRegistry::new();
    
    registry.add_server("playwright".into(), 
        McpServerConfig::stdio("npx", vec!["-y".into(), "@playwright/mcp@latest".into()]));
    
    registry.add_server("filesystem".into(),
        McpServerConfig::stdio("npx", vec!["-y".into(), "@modelcontextprotocol/server-filesystem".into()]));
    
    let status = registry.server_status().await;
    assert_eq!(status.len(), 2);
    println!("✅ 多 server 管理");
}

#[tokio::test]
async fn test_registry_remove_server() {
    let mut registry = McpToolRegistry::new();
    
    registry.add_server("playwright".into(),
        McpServerConfig::stdio("npx", vec!["-y".into(), "@playwright/mcp@latest".into()]));
    
    assert_eq!(registry.server_status().await.len(), 1);
    
    registry.remove_server("playwright").await;
    
    assert_eq!(registry.server_status().await.len(), 0);
    println!("✅ remove_server() 工作正常");
}

#[tokio::test]
async fn test_registry_wrapped() {
    let registry = Arc::new(RwLock::new(McpToolRegistry::new()));
    
    {
        let mut reg = registry.write().await;
        reg.add_server("playwright".into(),
            McpServerConfig::stdio("npx", vec!["-y".into(), "@playwright/mcp@latest".into()]));
    }
    
    {
        let reg = registry.read().await;
        assert_eq!(reg.server_status().await.len(), 1);
        println!("✅ Arc<RwLock<McpToolRegistry>> 包装工作正常");
    }
}

#[tokio::test]
async fn test_cli_arg_parsing() {
    let mut registry = McpToolRegistry::new();
    
    registry.add_from_cli_arg("playwright:npx -y @playwright/mcp@latest").unwrap();
    registry.add_from_cli_arg("npx -y @modelcontextprotocol/server-filesystem").unwrap();
    
    let status = registry.server_status().await;
    assert_eq!(status.len(), 2);
    println!("✅ CLI 参数解析工作正常");
}

#[tokio::test]
async fn test_event_system() {
    use matrixcode_core::event::{AgentEvent, McpServerInfo, EventType};
    
    let event = AgentEvent::mcp_server_added("test", 5);
    assert_eq!(event.event_type, EventType::McpServerAdded);
    
    let event = AgentEvent::mcp_server_removed("test");
    assert_eq!(event.event_type, EventType::McpServerRemoved);
    
    let infos = vec![
        McpServerInfo::new("playwright", true, 10),
        McpServerInfo::new("filesystem", false, 0),
    ];
    let event = AgentEvent::mcp_server_status(infos);
    assert_eq!(event.event_type, EventType::McpServerStatus);
    
    println!("✅ MCP 事件系统工作正常");
}