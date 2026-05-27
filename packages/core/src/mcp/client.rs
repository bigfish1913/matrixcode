//! MCP Client
//!
//! MCP 协议客户端，负责：
//! - 连接管理
//! - 协议握手
//! - 工具发现与调用

use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::collections::HashMap;
use tokio::sync::RwLock;

use super::transport::{create_transport, Transport, TransportConfig};
use super::types::*;

// ============================================================================
// MCP Client
// ============================================================================

/// MCP 客户端
pub struct McpClient {
    /// 服务器名称
    server_name: String,
    /// 传输层
    transport: Box<dyn Transport>,
    /// 服务器能力
    capabilities: RwLock<Option<ServerCapabilities>>,
    /// 服务器信息
    server_info: RwLock<Option<Implementation>>,
    /// 工具缓存
    tools_cache: RwLock<Vec<Tool>>,
    /// 请求 ID 计数器
    request_id: RwLock<i64>,
    /// 是否已初始化
    initialized: RwLock<bool>,
}

impl McpClient {
    /// 创建并初始化 MCP 客户端
    pub async fn connect(
        server_name: impl Into<String>,
        config: TransportConfig,
    ) -> Result<Self> {
        let server_name = server_name.into();
        let transport = create_transport(&server_name, &config).await?;
        
        let client = Self {
            server_name,
            transport,
            capabilities: RwLock::new(None),
            server_info: RwLock::new(None),
            tools_cache: RwLock::new(Vec::new()),
            request_id: RwLock::new(0),
            initialized: RwLock::new(false),
        };
        
        // 执行初始化握手
        client.initialize().await?;
        
        Ok(client)
    }
    
    /// 获取服务器名称
    pub fn server_name(&self) -> &str {
        &self.server_name
    }
    
    /// 是否已初始化
    pub async fn is_initialized(&self) -> bool {
        *self.initialized.read().await
    }
    
    /// 获取服务器能力
    pub async fn capabilities(&self) -> Option<ServerCapabilities> {
        self.capabilities.read().await.clone()
    }
    
    /// 获取服务器信息
    pub async fn server_info(&self) -> Option<Implementation> {
        self.server_info.read().await.clone()
    }
    
    // ========================================================================
    // Protocol Methods
    // ========================================================================
    
    /// 生成下一个请求 ID
    async fn next_request_id(&self) -> RequestId {
        let mut id = self.request_id.write().await;
        *id += 1;
        RequestId::Number(*id)
    }
    
    /// 发送请求并解析响应
    async fn send_request<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> Result<T> {
        let id = self.next_request_id().await;
        
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: id.clone(),
            method: method.to_string(),
            params,
        };
        
        let message = serde_json::to_string(&request)?;
        tracing::debug!("MCP request to '{}': {}", self.server_name, message);
        
        // 发送请求
        self.transport.notify(&message).await?;
        
        // 循环读取消息直到找到匹配的响应
        loop {
            let response = self.transport.receive().await?;
            tracing::debug!("MCP message from '{}': {}", self.server_name, response);
            
            // 尝试解析为服务端请求（如 roots/list）
            if let Ok(server_req) = serde_json::from_str::<JsonRpcRequest>(&response) {
                // 处理服务端请求
                self.handle_server_request(&server_req).await?;
                continue;
            }
            
            // 尝试解析为成功响应
            if let Ok(success) = serde_json::from_str::<JsonRpcResponse>(&response) {
                if success.id != id {
                    // 不是我们要的响应，继续等待
                    continue;
                }
                return serde_json::from_value(success.result)
                    .map_err(|e| anyhow!("Failed to parse result: {}", e));
            }
            
            // 尝试解析为错误响应
            if let Ok(error) = serde_json::from_str::<JsonRpcError>(&response) {
                if error.id != id {
                    continue;
                }
                return Err(anyhow!(
                    "MCP error from '{}': [{}] {}",
                    self.server_name,
                    error.error.code,
                    error.error.message
                ));
            }
            
            // 尝试解析为通知（无 id）
            if let Ok(notification) = serde_json::from_str::<JsonRpcNotification>(&response) {
                tracing::debug!("MCP notification from '{}': {}", self.server_name, notification.method);
                continue;
            }
            
            // 无法识别的消息格式
            tracing::warn!("Unexpected MCP message format: {}", response);
        }
    }
    
    /// 处理服务端发来的请求
    async fn handle_server_request(&self, request: &JsonRpcRequest) -> Result<()> {
        tracing::debug!("MCP server request '{}': {}", self.server_name, request.method);
        
        // 根据方法名处理
        match request.method.as_str() {
            "roots/list" => {
                // 返回空的 roots 列表
                let response = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id.clone(),
                    result: json!({ "roots": [] }),
                };
                let message = serde_json::to_string(&response)?;
                self.transport.notify(&message).await?;
            }
            "ping" => {
                // 响应 pong
                let response = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id.clone(),
                    result: json!({}),
                };
                let message = serde_json::to_string(&response)?;
                self.transport.notify(&message).await?;
            }
            _ => {
                tracing::warn!("Unhandled MCP server request: {}", request.method);
                // 返回方法不存在的错误
                let error_response = JsonRpcError {
                    jsonrpc: "2.0".to_string(),
                    id: request.id.clone(),
                    error: JsonRpcErrorDetail {
                        code: -32601,
                        message: "Method not found".to_string(),
                        data: None,
                    },
                };
                let message = serde_json::to_string(&error_response)?;
                self.transport.notify(&message).await?;
            }
        }
        
        Ok(())
    }
    
    /// 发送通知（无需响应）
    async fn send_notification(&self, method: &str, params: Option<Value>) -> Result<()> {
        let notification = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
        };
        
        let message = serde_json::to_string(&notification)?;
        self.transport.notify(&message).await?;
        Ok(())
    }
    
    // ========================================================================
    // Initialization
    // ========================================================================
    
    /// 执行初始化握手
    async fn initialize(&self) -> Result<()> {
        tracing::info!("Initializing MCP server '{}'", self.server_name);
        
        // 发送 initialize 请求
        let params = InitializeParams {
            capabilities: ClientCapabilities {
                roots: Some(RootsCapability {
                    list_changed: Some(false),
                }),
                ..Default::default()
            },
            client_info: Implementation::default(),
            protocol_version: Some("2024-11-05".to_string()),
        };
        
        let result: InitializeResult = self.send_request(
            "initialize",
            Some(serde_json::to_value(params)?),
        ).await?;
        
        // 保存服务器信息（先 clone 用于日志）
        let server_name = result.server_info.name.clone();
        let server_version = result.server_info.version.clone();
        
        *self.capabilities.write().await = Some(result.capabilities);
        *self.server_info.write().await = Some(result.server_info);
        
        tracing::info!(
            "MCP server '{}' initialized: {} v{}",
            self.server_name,
            server_name,
            server_version
        );
        
        // 发送 initialized 通知
        self.send_notification("notifications/initialized", None).await?;
        
        *self.initialized.write().await = true;
        Ok(())
    }
    
    // ========================================================================
    // Tools API
    // ========================================================================
    
    /// 列出所有工具
    pub async fn list_tools(&self) -> Result<Vec<Tool>> {
        if !self.is_initialized().await {
            return Err(anyhow!("MCP client not initialized"));
        }
        
        let result: ListToolsResult = self.send_request("tools/list", None).await?;
        
        // 缓存工具列表
        *self.tools_cache.write().await = result.tools.clone();
        
        Ok(result.tools)
    }
    
    /// 获取缓存的工具列表
    pub async fn cached_tools(&self) -> Vec<Tool> {
        self.tools_cache.read().await.clone()
    }
    
    /// 调用工具
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: Option<Value>,
    ) -> Result<CallToolResult> {
        if !self.is_initialized().await {
            return Err(anyhow!("MCP client not initialized"));
        }
        
        let params = CallToolParams {
            name: name.to_string(),
            arguments,
        };
        
        self.send_request("tools/call", Some(serde_json::to_value(params)?)).await
    }
    
    /// 检查服务器是否支持工具
    pub async fn supports_tools(&self) -> bool {
        self.capabilities.read().await
            .as_ref()
            .map(|c| c.tools.is_some())
            .unwrap_or(false)
    }
    
    // ========================================================================
    // Resources API (Optional)
    // ========================================================================
    
    /// 列出所有资源
    pub async fn list_resources(&self) -> Result<Vec<Resource>> {
        if !self.is_initialized().await {
            return Err(anyhow!("MCP client not initialized"));
        }
        
        let result: ListResourcesResult = self.send_request("resources/list", None).await?;
        Ok(result.resources)
    }
    
    /// 读取资源
    pub async fn read_resource(&self, uri: &str) -> Result<Value> {
        if !self.is_initialized().await {
            return Err(anyhow!("MCP client not initialized"));
        }
        
        self.send_request("resources/read", Some(json!({ "uri": uri }))).await
    }
    
    /// 检查服务器是否支持资源
    pub async fn supports_resources(&self) -> bool {
        self.capabilities.read().await
            .as_ref()
            .map(|c| c.resources.is_some())
            .unwrap_or(false)
    }
    
    // ========================================================================
    // Prompts API (Optional)
    // ========================================================================
    
    /// 列出所有 prompt
    pub async fn list_prompts(&self) -> Result<Vec<Prompt>> {
        if !self.is_initialized().await {
            return Err(anyhow!("MCP client not initialized"));
        }
        
        let result: ListPromptsResult = self.send_request("prompts/list", None).await?;
        Ok(result.prompts)
    }
    
    /// 获取 prompt
    pub async fn get_prompt(&self, name: &str, arguments: Option<HashMap<String, String>>) -> Result<Value> {
        if !self.is_initialized().await {
            return Err(anyhow!("MCP client not initialized"));
        }
        
        let mut params = json!({ "name": name });
        if let Some(args) = arguments {
            params["arguments"] = serde_json::to_value(args)?;
        }
        
        self.send_request("prompts/get", Some(params)).await
    }
    
    /// 检查服务器是否支持 prompt
    pub async fn supports_prompts(&self) -> bool {
        self.capabilities.read().await
            .as_ref()
            .map(|c| c.prompts.is_some())
            .unwrap_or(false)
    }
    
    // ========================================================================
    // Logging API
    // ========================================================================
    
    /// 设置日志级别
    pub async fn set_logging_level(&self, level: LogLevel) -> Result<()> {
        if !self.is_initialized().await {
            return Err(anyhow!("MCP client not initialized"));
        }
        
        let params = SetLoggingLevelParams { level };
        self.send_request("logging/setLevel", Some(serde_json::to_value(params)?)).await
    }
    
    // ========================================================================
    // Lifecycle
    // ========================================================================
    
    /// 关���连接
    pub async fn shutdown(&self) -> Result<()> {
        tracing::info!("Shutting down MCP server '{}'", self.server_name);
        self.transport.close().await
    }
}

// ============================================================================
// MCP Client Builder
// ============================================================================

/// MCP 客户端构建器
pub struct McpClientBuilder {
    server_name: String,
    config: TransportConfig,
}

impl McpClientBuilder {
    /// 创建构建器
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            server_name: name.into(),
            config: TransportConfig::stdio("", vec![]),
        }
    }
    
    /// 使用 stdio 传输
    pub fn stdio(mut self, command: impl Into<String>, args: Vec<String>) -> Self {
        self.config = TransportConfig::stdio(command, args);
        self
    }
    
    /// 使用 SSE 传输
    pub fn sse(mut self, url: impl Into<String>) -> Self {
        self.config = TransportConfig::sse(url);
        self
    }
    
    /// 构建并连接
    pub async fn connect(self) -> Result<McpClient> {
        McpClient::connect(self.server_name, self.config).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_client_builder() {
        let builder = McpClientBuilder::new("test")
            .stdio("npx", vec!["-y".into(), "@playwright/mcp".into()]);
        
        assert_eq!(builder.server_name, "test");
    }
}