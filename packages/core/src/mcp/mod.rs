//! MCP (Model Context Protocol) Integration
//!
//! 提供完整的 MCP 协议支持，允许连接任意 MCP 服务器并将其工具映射为内置工具。
//!
//! # 架构概览
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  MatrixCode Agent                                           │
//! │  ┌─────────────────────────────────────────────────────┐   │
//! │  │  Tool System                                         │   │
//! │  │  ┌─────────┐ ┌─────────┐ ┌───────────────────────┐ │   │
//! │  │  │ Built-in│ │ CodeGraph│ │ MCP Tool Wrapper      │ │   │
//! │  │  │ Tools   │ │ Tools    │ │ (Transparent Proxy)   │ │   │
//! │  │  └─────────┘ └─────────┘ └───────────┬───────────┘ │   │
//! │  └─────────────────────────────────────│───────────────┘   │
//! └────────────────────────────────────────│───────────────────┘
//!                                          │
//! ┌─────────────────────────────────────────│───────────────────┐
//! │  MCP Module                            │                    │
//! │  ┌──────────────┐ ┌──────────────┐ ┌───┴───────┐           │
//! │  │ Config       │ │ Transport    │ │ McpClient │           │
//! │  │ (config.rs)  │ │ (transport.rs)│ │ (client.rs)│          │
//! │  └──────────────┘ └──────────────┘ └─────┬─────┘           │
//! └───────────────────────────────────────────│─────────────────┘
//!                                            │ MCP Protocol
//! ┌───────────────────────────────────────────│─────────────────┐
//! │  External MCP Servers                     │                 │
//! │  ┌─────────────┐ ┌─────────────┐ ┌───────┴───┐            │
//! │  │ Playwright  │ │ Filesystem  │ │  Other    │            │
//! │  │ MCP Server  │ │ MCP Server  │ │  MCP      │            │
//! │  └─────────────┘ └─────────────┘ └───────────┘            │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # 快速开始
//!
//! ## 1. 配置 MCP 服务器
//!
//! 创建 `mcp.toml` 文件：
//!
//! ```toml
//! [servers.playwright]
//! command = "npx"
//! args = ["-y", "@playwright/mcp@latest"]
//!
//! [servers.filesystem]
//! command = "npx"
//! args = ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/files"]
//! ```
//!
//! ## 2. 连接并使用工具
//!
//! ```ignore
//! use matrixcode_core::mcp::{McpConfig, McpToolManager};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // 加载配置
//!     let config = McpConfig::from_file("mcp.toml")?;
//!     
//!     // 创建工具管理器
//!     let manager = McpToolManager::new();
//!     
//!     // 连接所有服务器
//!     for (key, server_config) in config.enabled_servers() {
//!         let transport = server_config.to_transport_config()?;
//!         let tools = manager.connect_server(&server_config.get_name(&key), transport).await?;
//!         println!("Connected: {} tools", tools.len());
//!     }
//!     
//!     // 获取所有工具
//!     let tools = manager.get_tools().await;
//!     
//!     // 使用工具...
//!     
//!     // 关闭
//!     manager.shutdown().await;
//!     Ok(())
//! }
//! ```
//!
//! # 支持的传输方式
//!
//! - **Stdio**: 通过子进程 stdin/stdout 通信（推荐）
//! - **SSE**: 通过 HTTP Server-Sent Events 通信（远程服务器）
//!
//! # 内置 MCP 配置
//!
//! 模块提供常用 MCP 服务器的预设配置：
//!
//! - `playwright_config()`: Playwright 浏览器自动化
//! - `default_mcp_config()`: 常用配置组合

pub mod types;
pub mod transport;
pub mod client;
pub mod proxy;
pub mod config;
pub mod lazy;

// Re-export main types
pub use types::*;
pub use transport::{Transport, TransportConfig, StdioTransport, SseTransport};
pub use client::{McpClient, McpClientBuilder};
pub use proxy::{McpToolWrapper, McpToolManager, connect_mcp_server, connect_mcp_servers_from_config};
pub use config::{McpConfig, McpServerConfig, McpSettings, load_mcp_config, find_mcp_config};
pub use lazy::{McpToolPlaceholder, McpToolRegistry, ServerStatus};

// ============================================================================
// Convenience Functions
// ============================================================================

/// 连接 Playwright MCP 服务器
///
/// # Example
///
/// ```ignore
/// use matrixcode_core::mcp::connect_playwright;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let tools = connect_playwright().await?;
///     println!("Playwright tools: {:?}", tools.iter().map(|t| t.definition().name).collect::<Vec<_>>());
///     Ok(())
/// }
/// ```
pub async fn connect_playwright() -> anyhow::Result<Vec<Box<dyn crate::tools::Tool>>> {
    let config = config::playwright_config();
    let (key, server) = config.enabled_servers()
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("No playwright config found"))?;
    
    let transport = server.to_transport_config()?;
    connect_mcp_server(&server.get_name(&key), transport).await
}

/// 连接配置文件中的所有 MCP 服务器
///
/// 从当前目录或用户主目录查找 `mcp.toml` / `mcp.json` 配置文件，
/// 并连接所有启用的 MCP 服务器。
///
/// # Example
///
/// ```ignore
/// use matrixcode_core::mcp::connect_all_from_config;
/// use std::path::Path;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let tools = connect_all_from_config(Path::new(".")).await?;
///     println!("Total tools: {}", tools.len());
///     Ok(())
/// }
/// ```
pub async fn connect_all_from_config(
    start_dir: &std::path::Path,
) -> anyhow::Result<Vec<Box<dyn crate::tools::Tool>>> {
    let config = load_mcp_config(start_dir);
    let mut all_tools = Vec::new();
    
    for (key, server_config) in config.enabled_servers() {
        match server_config.to_transport_config() {
            Ok(transport) => {
                let name = server_config.get_name(&key);
                tracing::info!("Connecting to MCP server: {}", name);
                
                match connect_mcp_server(&name, transport).await {
                    Ok(tools) => {
                        tracing::info!("Connected to '{}' with {} tools", name, tools.len());
                        all_tools.extend(tools);
                    }
                    Err(e) => {
                        tracing::error!("Failed to connect to MCP server '{}': {}", name, e);
                    }
                }
            }
            Err(e) => {
                tracing::error!("Invalid config for server '{}': {}", key, e);
            }
        }
    }
    
    Ok(all_tools)
}