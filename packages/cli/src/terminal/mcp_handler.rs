//! MCP (Model Context Protocol) handler for terminal mode
//!
//! Handles MCP server startup, status, and lifecycle management.

use std::sync::Arc;
use matrixcode_core::{AgentEvent, mcp::McpToolRegistry};
use matrixcode_core::tools::Tool;

/// MCP manager that handles server lifecycle
pub struct McpManager {
    registry: Arc<tokio::sync::RwLock<McpToolRegistry>>,
}

impl McpManager {
    /// Create new MCP manager with servers config
    pub fn new() -> Self {
        Self {
            registry: Arc::new(tokio::sync::RwLock::new(McpToolRegistry::new())),
        }
    }
    
    /// Add servers to registry (async)
    pub async fn add_servers(&self, mcp_servers: Vec<(String, matrixcode_core::mcp::McpServerConfig)>) {
        let mut reg = self.registry.write().await;
        for (name, server_config) in mcp_servers {
            reg.add_server(name.clone(), server_config);
            log::info!("MCP server '{}' added to registry", name);
        }
    }
    
    /// Get registry reference for Agent
    pub fn registry(&self) -> Arc<tokio::sync::RwLock<McpToolRegistry>> {
        self.registry.clone()
    }
    
    /// Start all MCP servers and return tools
    pub async fn start_all(
        &self,
        event_tx: &tokio::sync::mpsc::Sender<AgentEvent>,
    ) -> Vec<Box<dyn Tool>> {
        let mut mcp_tools: Vec<Box<dyn Tool>> = Vec::new();
        
        let registry = self.registry.read().await;
        match registry.start_all().await {
            Ok(server_tools) => {
                for (name, tools) in server_tools {
                    log::info!("Connected to '{}' with {} tools", name, tools.len());
                    
                    // Send MCP server added event
                    let _ = event_tx.send(AgentEvent::mcp_server_added(
                        name.clone(),
                        tools.len(),
                    )).await;
                    
                    // Convert Arc<McpToolWrapper> to Box<dyn Tool>
                    for tool in tools {
                        mcp_tools.push(Box::new((*tool).clone()));
                    }
                }
                
                // Send overall MCP status after all servers started
                let statuses = registry.server_status().await;
                let mcp_infos: Vec<matrixcode_core::event::McpServerInfo> = statuses
                    .iter()
                    .map(|(_, s)| matrixcode_core::event::McpServerInfo::from_status(s))
                    .collect();
                let _ = event_tx.send(AgentEvent::mcp_server_status(mcp_infos)).await;
            }
            Err(e) => {
                log::error!("Failed to start MCP servers: {}", e);
                let _ = event_tx.send(AgentEvent::error(
                    format!("MCP 服务器启动失败: {}", e),
                    Some("mcp_error".to_string()),
                    None,
                )).await;
            }
        }
        
        mcp_tools
    }
}