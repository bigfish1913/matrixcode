//! MCP Lazy Loader
//!
//! 懒加载 MCP 工具：只在首次调用时启动服务器

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

use super::client::McpClient;
use super::config::{McpConfig, McpServerConfig};
use super::proxy::McpToolWrapper;
use crate::approval::RiskLevel;
use crate::tools::{Tool, ToolDefinition};

// ============================================================================
// Lazy MCP Tool
// ============================================================================

/// 懒加载 MCP 工具 - 调用时才启动服务器
pub struct LazyMcpTool {
    /// 工具定义（预先知道）
    definition: ToolDefinition,

    /// 服务器名称
    server_name: String,

    /// 服务器配置
    server_config: McpServerConfig,

    /// 实际工具（启动后填充）
    actual_tool: Arc<Mutex<Option<Arc<McpToolWrapper>>>>,

    /// 客户端（启动后填充）
    client: Arc<Mutex<Option<Arc<McpClient>>>>,
}

impl LazyMcpTool {
    /// 创建懒加载工具
    pub fn new(
        server_name: String,
        server_config: McpServerConfig,
        tool_name: String,
        tool_description: String,
        tool_input_schema: Value,
    ) -> Self {
        // 创建工具定义（去掉服务器前缀）
        let definition = ToolDefinition {
            name: tool_name,
            description: tool_description,
            parameters: tool_input_schema,
            is_priority: false,
        };

        Self {
            definition,
            server_name,
            server_config,
            actual_tool: Arc::new(Mutex::new(None)),
            client: Arc::new(Mutex::new(None)),
        }
    }

    /// 启动服务器并获取实际工具
    async fn ensure_started(&self) -> Result<Arc<McpToolWrapper>> {
        // 检查是否已启动
        {
            let tool_lock = self.actual_tool.lock().await;
            if let Some(tool) = tool_lock.as_ref() {
                return Ok(tool.clone());
            }
        }

        // 启动服务器
        tracing::info!(
            "Starting lazy MCP server '{}' for tool '{}'",
            self.server_name,
            self.definition.name
        );

        let transport_config = self.server_config.to_transport_config()?;
        let client = Arc::new(McpClient::connect(&self.server_name, transport_config).await?);

        // 获取工具列表
        let mcp_tools = client.list_tools().await?;

        // 找到匹配的工具
        let tool_name_without_prefix = self.definition.name.clone();
        let mcp_tool = mcp_tools
            .into_iter()
            .find(|t| {
                // 匹配去掉前缀的名字
                let name = t.name.clone();
                name == tool_name_without_prefix
                    || name == format!("{}_{}", self.server_name, tool_name_without_prefix)
            })
            .ok_or_else(|| {
                anyhow!(
                    "Tool '{}' not found in MCP server '{}'",
                    self.definition.name,
                    self.server_name
                )
            })?;

        // 创建工具包装器
        let wrapper = Arc::new(McpToolWrapper::new(
            client.clone(),
            mcp_tool,
            self.server_name.clone(),
        ));

        // 缓存
        {
            let mut tool_lock = self.actual_tool.lock().await;
            *tool_lock = Some(wrapper.clone());
        }
        {
            let mut client_lock = self.client.lock().await;
            *client_lock = Some(client);
        }

        tracing::info!(
            "Lazy MCP server '{}' started successfully",
            self.server_name
        );

        Ok(wrapper)
    }

    /// 关闭服务器
    pub async fn shutdown(&self) -> Result<()> {
        let client_lock = self.client.lock().await;
        if let Some(client) = client_lock.as_ref() {
            client.shutdown().await?;
            tracing::info!("Lazy MCP server '{}' stopped", self.server_name);
        }
        Ok(())
    }
}

#[async_trait]
impl Tool for LazyMcpTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, params: Value) -> Result<String> {
        // 确保服务器已启动
        let tool = self.ensure_started().await?;

        // 执行工具
        tool.execute(params).await
    }

    fn risk_level(&self) -> RiskLevel {
        // MCP 工具默认为 Mutating（可能修改外部状态）
        RiskLevel::Mutating
    }
}

// ============================================================================
// Lazy MCP Loader
// ============================================================================

/// 懒加载 MCP 工具集 - 从配置预先创建工具定义，但不启动服务器
pub struct LazyMcpLoader {
    /// MCP 配置
    config: McpConfig,

    /// 预先创建的懒加载工具（预留字段）
    #[allow(dead_code)]
    tools: Vec<LazyMcpTool>,
}

impl LazyMcpLoader {
    /// 从配置创建懒加载工具集
    pub fn from_config(config: McpConfig) -> Self {
        let tools = Vec::new();

        // 为每个服务器预先创建占位工具
        // （实际工具定义需要查询服务器，这里无法预先知道）
        // 所以这个实现需要在首次查询时动态发现

        Self { config, tools }
    }

    /// 发现工具定义（不启动服务器）
    /// 返回占位符工具，调用时才启动
    pub async fn discover_tools(&self) -> Result<Vec<LazyMcpTool>> {
        // 这个方法需要实际连接服务器才能知道工具列表
        // 所以懒加载实现需要改变策略

        // 方案：提供一个特殊工具来触发服务器启动
        // 或者：在配置中预先声明工具

        // 这里返回空，实际实现需要其他策略
        Ok(Vec::new())
    }

    /// 获取配置
    pub fn config(&self) -> &McpConfig {
        &self.config
    }
}

// ============================================================================
// Alternative: MCP Tool Placeholder
// ============================================================================

/// MCP 工具占位符 - 用户显式调用时才启动服务器
pub struct McpToolPlaceholder {
    /// 服务器名称
    server_name: String,

    /// 服务器配置
    server_config: McpServerConfig,

    /// 已启动的工具（None 表示未启动）
    tools: Arc<RwLock<Option<Vec<Arc<McpToolWrapper>>>>>,

    /// 客户端（None 表示未启动）
    client: Arc<RwLock<Option<Arc<McpClient>>>>,
}

impl McpToolPlaceholder {
    /// 创建占位符
    pub fn new(server_name: String, server_config: McpServerConfig) -> Self {
        Self {
            server_name,
            server_config,
            tools: Arc::new(RwLock::new(None)),
            client: Arc::new(RwLock::new(None)),
        }
    }

    /// 启动服务器并返回工具列表
    pub async fn start(&self) -> Result<Vec<Arc<McpToolWrapper>>> {
        // 检查是否已启动
        {
            let tools_lock = self.tools.read().await;
            if let Some(tools) = tools_lock.as_ref() {
                return Ok(tools.clone());
            }
        }

        // 启动服务器
        tracing::info!("Starting MCP server '{}' on demand", self.server_name);

        let transport_config = self.server_config.to_transport_config()?;
        let client = Arc::new(McpClient::connect(&self.server_name, transport_config).await?);

        // 获取工具列表
        if !client.supports_tools().await {
            return Err(anyhow!(
                "MCP server '{}' does not support tools",
                self.server_name
            ));
        }

        let mcp_tools = client.list_tools().await?;

        // 创建工具包装器
        let tools: Vec<Arc<McpToolWrapper>> = mcp_tools
            .into_iter()
            .map(|tool| {
                Arc::new(McpToolWrapper::new(
                    client.clone(),
                    tool,
                    self.server_name.clone(),
                ))
            })
            .collect();

        // 缓存
        {
            let mut tools_lock = self.tools.write().await;
            *tools_lock = Some(tools.clone());
        }
        {
            let mut client_lock = self.client.write().await;
            *client_lock = Some(client);
        }

        tracing::info!(
            "MCP server '{}' started with {} tools",
            self.server_name,
            tools.len()
        );

        Ok(tools)
    }

    /// 关闭服务器
    pub async fn shutdown(&self) -> Result<()> {
        let client_lock = self.client.read().await;
        if let Some(client) = client_lock.as_ref() {
            client.shutdown().await?;
        }
        Ok(())
    }

    /// 是否已启动
    pub async fn is_started(&self) -> bool {
        self.tools.read().await.is_some()
    }

    /// 获取服务器名称
    pub fn server_name(&self) -> &str {
        &self.server_name
    }
}

// ============================================================================
// MCP Tool Registry (Dynamic + Lazy)
// ============================================================================

/// MCP 工具注册表 - 管理所有懒加载的 MCP 服务器
///
/// # 功能
/// - 从配置文件加载服务器（懒加载）
/// - 运行时动态添加/移除服务器
/// - 查询已连接服务器的状态
/// - 全局单例管理
pub struct McpToolRegistry {
    /// 服务器占位符（按名称索引）
    placeholders: HashMap<String, Arc<McpToolPlaceholder>>,
}

impl McpToolRegistry {
    /// 从配置创建注册表
    pub fn from_config(config: &McpConfig) -> Self {
        let placeholders = config
            .servers
            .iter()
            .filter(|(_, cfg)| cfg.enabled)
            .map(|(name, cfg)| {
                (
                    name.clone(),
                    Arc::new(McpToolPlaceholder::new(name.clone(), cfg.clone())),
                )
            })
            .collect();

        Self { placeholders }
    }

    /// 创建空注册表
    pub fn new() -> Self {
        Self {
            placeholders: HashMap::new(),
        }
    }

    /// 动态添加服务器（运行时）
    pub fn add_server(&mut self, name: String, config: McpServerConfig) {
        self.placeholders.insert(
            name.clone(),
            Arc::new(McpToolPlaceholder::new(name, config)),
        );
    }

    /// 移除服务器（如果已启动则先关闭）
    pub async fn remove_server(&mut self, name: &str) -> Result<()> {
        if let Some(placeholder) = self.placeholders.remove(name) {
            placeholder.shutdown().await?;
        }
        Ok(())
    }

    /// 获取服务器占位符
    pub fn get_server(&self, name: &str) -> Option<Arc<McpToolPlaceholder>> {
        self.placeholders.get(name).cloned()
    }

    /// 获取所有服务器名称
    pub fn server_names(&self) -> Vec<&String> {
        self.placeholders.keys().collect()
    }

    /// 获取已启动的服务器列表
    pub async fn started_servers(&self) -> Vec<String> {
        let mut started = Vec::new();
        for (name, placeholder) in &self.placeholders {
            if placeholder.is_started().await {
                started.push(name.clone());
            }
        }
        started
    }

    /// 获取服务器状态（用于 TUI 显示）
    pub async fn server_status(&self) -> HashMap<String, ServerStatus> {
        let mut status = HashMap::new();
        for (name, placeholder) in &self.placeholders {
            let is_started = placeholder.is_started().await;
            status.insert(
                name.clone(),
                ServerStatus {
                    name: name.clone(),
                    is_started,
                    tool_count: if is_started {
                        placeholder
                            .tools
                            .read()
                            .await
                            .as_ref()
                            .map(|t| t.len())
                            .unwrap_or(0)
                    } else {
                        0
                    },
                },
            );
        }
        status
    }

    /// 启动所有服务器
    pub async fn start_all(&self) -> Result<HashMap<String, Vec<Arc<McpToolWrapper>>>> {
        let mut results = HashMap::new();

        for (name, placeholder) in &self.placeholders {
            let tools = placeholder.start().await?;
            results.insert(name.clone(), tools);
        }

        Ok(results)
    }

    /// 关闭所有服务器
    pub async fn shutdown_all(&self) {
        for placeholder in self.placeholders.values() {
            if let Err(e) = placeholder.shutdown().await {
                tracing::error!(
                    "Failed to shutdown MCP server '{}': {}",
                    placeholder.server_name(),
                    e
                );
            }
        }
    }

    /// 从 CLI 参数添加服务器
    ///
    /// # 格式
    /// - `name:command args` → 自定义名称
    /// - `command args` → 自动推断名称（取命令第一部分）
    ///
    /// # Example
    /// ```ignore
    /// registry.add_from_cli_arg("playwright:npx -y @playwright/mcp@latest")?;
    /// registry.add_from_cli_arg("npx -y @modelcontextprotocol/server-filesystem")?;
    /// ```
    pub fn add_from_cli_arg(&mut self, arg: &str) -> Result<()> {
        let (name, command, args) = parse_cli_mcp_arg(arg)?;

        let config = McpServerConfig {
            command: Some(command),
            args,
            url: None,
            enabled: true,
            ..Default::default()
        };

        self.add_server(name, config);
        Ok(())
    }

    /// 从 CLI 参数批量添加服务器
    pub fn add_from_cli_args(&mut self, args: &[String]) -> Result<()> {
        for arg in args {
            self.add_from_cli_arg(arg)?;
        }
        Ok(())
    }
}

impl Default for McpToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 服务器状态（用于 UI 显示）
#[derive(Debug, Clone)]
pub struct ServerStatus {
    pub name: String,
    pub is_started: bool,
    pub tool_count: usize,
}

/// 解析 CLI MCP 参数
///
/// # 格式
/// - `name:command args` → (name, command, args)
/// - `command args` → (command, command, args)
///
/// # Example
/// ```
/// parse_cli_mcp_arg("playwright:npx -y @playwright/mcp@latest")
/// // → ("playwright", "npx", vec!["-y", "@playwright/mcp@latest"])
///
/// parse_cli_mcp_arg("npx -y @modelcontextprotocol/server-filesystem")
/// // → ("npx", "npx", vec!["-y", "@modelcontextprotocol/server-filesystem"])
/// ```
fn parse_cli_mcp_arg(arg: &str) -> Result<(String, String, Vec<String>)> {
    let arg = arg.trim();
    if arg.is_empty() {
        return Err(anyhow!("Empty MCP server argument"));
    }

    // 尝试解析 name:command args 格式
    if let Some((name, rest)) = arg.split_once(':') {
        let name = name.trim().to_string();
        let rest = rest.trim();

        // 解析命令和参数
        let parts =
            shell_words::split(rest).map_err(|e| anyhow!("Failed to parse command: {}", e))?;

        if parts.is_empty() {
            return Err(anyhow!("Missing command for MCP server '{}'", name));
        }

        let command = parts[0].clone();
        let args = parts[1..].to_vec();

        return Ok((name, command, args));
    }

    // 无名称前缀，使用命令作为名称
    let parts = shell_words::split(arg).map_err(|e| anyhow!("Failed to parse command: {}", e))?;

    if parts.is_empty() {
        return Err(anyhow!("Empty MCP server argument"));
    }

    let command = parts[0].clone();
    let name = command.clone(); // 使用命令名作为服务器名
    let args = parts[1..].to_vec();

    Ok((name, command, args))
}
