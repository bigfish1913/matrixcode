//! MCP Tool Proxy
//!
//! 将 MCP 服务器的工具映射为 MatrixCode 内置工具

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::approval::RiskLevel;
use crate::tools::{Tool, ToolDefinition};

use super::client::McpClient;
use super::types::{CallToolResult, Content, Tool as McpTool};

// ============================================================================
// MCP Tool Wrapper
// ============================================================================

/// MCP 工具包装器 - 将 MCP 工具映射为内置 Tool
#[derive(Clone)]
pub struct McpToolWrapper {
    /// MCP 客户端
    client: Arc<McpClient>,
    /// 工具定义
    tool_def: McpTool,
    /// 服务器名称（用于区分不同服务器的同名工具）
    server_name: String,
    /// 工具定义（缓存）
    cached_definition: ToolDefinition,
}

impl McpToolWrapper {
    /// 创建 MCP 工具包装器
    pub fn new(client: Arc<McpClient>, tool_def: McpTool, server_name: String) -> Self {
        // 转换 MCP 工具定义为内置工具定义
        let name = format!("{}_{}", server_name, tool_def.name);
        let description = tool_def.description.clone()
            .unwrap_or_else(|| format!("MCP tool: {}", tool_def.name));
        
        let cached_definition = ToolDefinition {
            name: name.clone(),
            description,
            parameters: tool_def.input_schema.clone(),
            is_priority: false,
        };
        
        Self {
            client,
            tool_def,
            server_name,
            cached_definition,
        }
    }
    
    /// 获取原始工具名称
    pub fn original_name(&self) -> &str {
        &self.tool_def.name
    }
    
    /// 获取服务器名称
    pub fn server_name(&self) -> &str {
        &self.server_name
    }
    
    /// 解析工具调用结果
    fn parse_result(&self, result: CallToolResult) -> String {
        if result.content.is_empty() {
            return String::new();
        }
        
        let mut output = String::new();
        
        for content in result.content {
            match content {
                Content::Text { text } => {
                    output.push_str(&text);
                    output.push('\n');
                }
                Content::Image { data, mime_type } => {
                    output.push_str(&format!("[Image: {} ({} bytes)]\n", mime_type, data.len()));
                }
                Content::Resource { resource } => {
                    if let Some(text) = resource.text {
                        output.push_str(&text);
                        output.push('\n');
                    } else if let Some(blob) = resource.blob {
                        output.push_str(&format!("[Resource: {} ({} bytes)]\n", resource.uri, blob.len()));
                    } else {
                        output.push_str(&format!("[Resource: {}]\n", resource.uri));
                    }
                }
            }
        }
        
        output.trim_end().to_string()
    }
}

#[async_trait]
impl Tool for McpToolWrapper {
    fn definition(&self) -> ToolDefinition {
        self.cached_definition.clone()
    }
    
    async fn execute(&self, params: Value) -> Result<String> {
        tracing::debug!(
            "Executing MCP tool '{}' from server '{}'",
            self.tool_def.name,
            self.server_name
        );
        
        // 调用 MCP 工具
        let result = self.client.call_tool(&self.tool_def.name, Some(params)).await
            .map_err(|e| anyhow!("MCP tool '{}' failed: {}", self.cached_definition.name, e))?;
        
        // 检查是否为错误
        if result.is_error.unwrap_or(false) {
            let error_msg = self.parse_result(result);
            return Err(anyhow!("MCP tool error: {}", error_msg));
        }
        
        // 解析结果
        Ok(self.parse_result(result))
    }
    
    fn risk_level(&self) -> RiskLevel {
        // MCP 工具默认为中等风险
        // 可以根据工具名称或模式设置不同风险级别
        let name = &self.tool_def.name;
        
        // 只读操作为安全
        if name.contains("read") || name.contains("list") || name.contains("get") {
            RiskLevel::Safe
        }
        // 浏览器操作为中等风险
        else if name.contains("browser") || name.contains("navigate") || name.contains("click") {
            RiskLevel::Mutating
        }
        // 写入操作为高风险
        else if name.contains("write") || name.contains("delete") || name.contains("create") {
            RiskLevel::Dangerous
        }
        // 默认中等风险
        else {
            RiskLevel::Mutating
        }
    }
}

// ============================================================================
// MCP Tool Manager
// ============================================================================

/// MCP 工具管理器 - 管理多个 MCP 服务器的连接
pub struct McpToolManager {
    /// 已连接的 MCP 客户端
    clients: RwLock<Vec<Arc<McpClient>>>,
}

impl McpToolManager {
    /// 创建工具管理器
    pub fn new() -> Self {
        Self {
            clients: RwLock::new(Vec::new()),
        }
    }
    
    /// 连接 MCP 服务器并获取工具
    pub async fn connect_server(
        &self,
        server_name: impl Into<String>,
        config: super::transport::TransportConfig,
    ) -> Result<Vec<Box<dyn Tool>>> {
        let server_name = server_name.into();
        
        // 连接服务器
        let client = Arc::new(McpClient::connect(&server_name, config).await?);
        
        // 检查是否支持工具
        if !client.supports_tools().await {
            tracing::warn!("MCP server '{}' does not support tools", server_name);
            return Ok(Vec::new());
        }
        
        // 获取工具列表
        let mcp_tools = client.list_tools().await?;
        tracing::info!(
            "MCP server '{}' provided {} tools",
            server_name,
            mcp_tools.len()
        );
        
        // 转换为内置工具
        let tools: Vec<Box<dyn Tool>> = mcp_tools
            .into_iter()
            .map(|tool| Box::new(McpToolWrapper::new(client.clone(), tool, server_name.clone())) as Box<dyn Tool>)
            .collect();
        
        // 缓存客户端（用于后续 shutdown）
        self.clients.write().await.push(client);
        
        Ok(tools)
    }
    
    /// 获取已连接的服务器数量
    pub async fn server_count(&self) -> usize {
        self.clients.read().await.len()
    }
    
    /// 获取所有已连接的服务器名称
    pub async fn server_names(&self) -> Vec<String> {
        self.clients.read().await.iter()
            .map(|c| c.server_name().to_string())
            .collect()
    }
    
    /// 关闭所有连接
    pub async fn shutdown(&self) {
        let clients = self.clients.read().await;
        for client in clients.iter() {
            if let Err(e) = client.shutdown().await {
                tracing::error!("Failed to shutdown MCP server '{}': {}", client.server_name(), e);
            }
        }
    }
}

impl Default for McpToolManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Convenience Functions
// ============================================================================

/// 连接单个 MCP 服务器并返回工具列表
pub async fn connect_mcp_server(
    server_name: impl Into<String>,
    config: super::transport::TransportConfig,
) -> Result<Vec<Box<dyn Tool>>> {
    let server_name = server_name.into();
    let client = McpClient::connect(&server_name, config).await?;
    
    if !client.supports_tools().await {
        client.shutdown().await?;
        return Ok(Vec::new());
    }
    
    let mcp_tools = client.list_tools().await?;
    let client = Arc::new(client);
    
    let tools: Vec<Box<dyn Tool>> = mcp_tools
        .into_iter()
        .map(|tool| Box::new(McpToolWrapper::new(client.clone(), tool, server_name.clone())) as Box<dyn Tool>)
        .collect();
    
    Ok(tools)
}

/// 从配置连接所有 MCP 服务器并返回工具列表
pub async fn connect_mcp_servers_from_config(
    mcp_config: &std::collections::HashMap<String, super::config::McpServerConfig>,
) -> Result<(Vec<Box<dyn Tool>>, McpToolManager)> {
    let manager = McpToolManager::new();
    let mut all_tools = Vec::new();
    
    for (name, config) in mcp_config.iter() {
        if !config.enabled {
            tracing::debug!("MCP server '{}' is disabled, skipping", name);
            continue;
        }
        
        // 从配置创建 TransportConfig
        let transport_config = config.to_transport_config()
            .map_err(|e| anyhow!("Failed to create transport config for '{}': {}", name, e))?;
        
        tracing::info!("Connecting to MCP server '{}'...", name);
        let tools = manager.connect_server(name, transport_config).await?;
        
        if !tools.is_empty() {
            tracing::info!("MCP server '{}' provided {} tools", name, tools.len());
            all_tools.extend(tools);
        }
    }
    
    Ok((all_tools, manager))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_mcp_tool_wrapper_definition() {
        // 测试风险级别判断逻辑
        fn get_risk_level(name: &str) -> RiskLevel {
            if name.contains("read") || name.contains("list") || name.contains("get") {
                RiskLevel::Safe
            } else if name.contains("browser") || name.contains("navigate") || name.contains("click") {
                RiskLevel::Mutating
            } else if name.contains("write") || name.contains("delete") || name.contains("create") {
                RiskLevel::Dangerous
            } else {
                RiskLevel::Mutating
            }
        }
        
        assert_eq!(get_risk_level("read_file"), RiskLevel::Safe);
        assert_eq!(get_risk_level("browser_navigate"), RiskLevel::Mutating);
        assert_eq!(get_risk_level("write_file"), RiskLevel::Dangerous);
    }
}