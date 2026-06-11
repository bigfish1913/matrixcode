//! MCP Configuration
//!
//! 管理 MCP 服务器配置

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use super::transport::TransportConfig;

// ============================================================================
// Configuration Types
// ============================================================================

/// MCP 服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// 服务器名称（可选，默认使用配置键名）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// 启动命令（stdio 模式）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,

    /// 命令参数
    #[serde(default)]
    pub args: Vec<String>,

    /// 环境变量
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// SSE URL（HTTP 模式）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    /// 请求超时（毫秒）
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,

    /// 是否启用
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            name: None,
            command: None,
            args: Vec::new(),
            env: HashMap::new(),
            url: None,
            timeout_ms: default_timeout(),
            enabled: default_enabled(),
        }
    }
}

fn default_timeout() -> u64 {
    30000
}

fn default_enabled() -> bool {
    true
}

impl McpServerConfig {
    /// 创建 stdio 配置
    pub fn stdio(command: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            name: None,
            command: Some(command.into()),
            args,
            env: HashMap::new(),
            url: None,
            timeout_ms: default_timeout(),
            enabled: true,
        }
    }

    /// 创建 SSE 配置
    pub fn sse(url: impl Into<String>) -> Self {
        Self {
            name: None,
            command: None,
            args: Vec::new(),
            env: HashMap::new(),
            url: Some(url.into()),
            timeout_ms: default_timeout(),
            enabled: true,
        }
    }

    /// 设置名称
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// 设置环境变量
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// 设置超时
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// 设置启用状态
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// 转换为传输配置
    pub fn to_transport_config(&self) -> Result<TransportConfig> {
        if let Some(command) = &self.command {
            // Stdio 模式
            let env_vec: Vec<(String, String)> = self
                .env
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();

            Ok(TransportConfig::Stdio {
                command: command.clone(),
                args: self.args.clone(),
                env: if env_vec.is_empty() {
                    None
                } else {
                    Some(env_vec)
                },
            })
        } else if let Some(url) = &self.url {
            // SSE 模式
            Ok(TransportConfig::Sse {
                url: url.clone(),
                timeout_ms: Some(self.timeout_ms),
            })
        } else {
            Err(anyhow!(
                "MCP server config must have either 'command' or 'url'"
            ))
        }
    }

    /// 获取服务器名称
    pub fn get_name(&self, key: &str) -> String {
        self.name.clone().unwrap_or_else(|| key.to_string())
    }
}

/// MCP 配置文件
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpConfig {
    /// MCP 服务器配置
    #[serde(default)]
    pub servers: HashMap<String, McpServerConfig>,

    /// 全局设置
    #[serde(default)]
    pub settings: McpSettings,
}

/// MCP 全局设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpSettings {
    /// 自动发现 MCP 服务器
    #[serde(default = "default_auto_discover")]
    pub auto_discover: bool,

    /// 连接超时（毫秒）
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout_ms: u64,
}

fn default_auto_discover() -> bool {
    true
}

fn default_connect_timeout() -> u64 {
    10000
}

impl Default for McpSettings {
    fn default() -> Self {
        Self {
            auto_discover: default_auto_discover(),
            connect_timeout_ms: default_connect_timeout(),
        }
    }
}

impl McpConfig {
    /// 创建空配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加服务器配置
    pub fn add_server(mut self, key: impl Into<String>, config: McpServerConfig) -> Self {
        self.servers.insert(key.into(), config);
        self
    }

    /// 从文件加载配置
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| anyhow!("Failed to read MCP config file: {}", e))?;

        Self::from_str(&content)
    }

    /// 从字符串解析配置
    pub fn from_str(content: &str) -> Result<Self> {
        // 尝试解析为 TOML
        if let Ok(config) = toml::from_str(content) {
            return Ok(config);
        }

        // 尝试解析为 JSON
        if let Ok(config) = serde_json::from_str(content) {
            return Ok(config);
        }

        Err(anyhow!("Failed to parse MCP config as TOML or JSON"))
    }

    /// 保存到文件
    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| anyhow!("Failed to serialize MCP config: {}", e))?;

        std::fs::write(path.as_ref(), content)
            .map_err(|e| anyhow!("Failed to write MCP config file: {}", e))?;

        Ok(())
    }

    /// 获取所有启用的服务器配置
    pub fn enabled_servers(&self) -> Vec<(String, &McpServerConfig)> {
        self.servers
            .iter()
            .filter(|(_, config)| config.enabled)
            .map(|(key, config)| (key.clone(), config))
            .collect()
    }

    /// 合并两个配置（other 覆盖 self）
    pub fn merge(mut self, other: McpConfig) -> Self {
        // 合并服务器配置（other 覆盖同名服务器）
        for (key, config) in other.servers {
            self.servers.insert(key, config);
        }

        // 合并设置（other 的非默认值覆盖）
        if !other.settings.auto_discover {
            self.settings.auto_discover = false;
        }
        if other.settings.connect_timeout_ms != default_connect_timeout() {
            self.settings.connect_timeout_ms = other.settings.connect_timeout_ms;
        }

        self
    }
}

// ============================================================================
// Default Configurations
// ============================================================================

/// 创建 Playwright MCP 配置
pub fn playwright_config() -> McpConfig {
    McpConfig::new().add_server(
        "playwright",
        McpServerConfig::stdio("npx", vec!["-y".into(), "@playwright/mcp@latest".into()]),
    )
}

/// 创建常用 MCP 配置
pub fn default_mcp_config() -> McpConfig {
    McpConfig::new()
        // Playwright 浏览器自动化
        .add_server(
            "playwright",
            McpServerConfig::stdio("npx", vec!["-y".into(), "@playwright/mcp@latest".into()]),
        )
    // 文件系统（可选）
    // .add_server("filesystem", McpServerConfig::stdio(
    //     "npx",
    //     vec!["-y".into(), "@modelcontextprotocol/server-filesystem".into()]
    // ))
}

// ============================================================================
// Config File Discovery
// ============================================================================

/// MCP 配置文件名
pub const MCP_CONFIG_FILENAMES: &[&str] = &["mcp.toml", "mcp.json", ".mcp.toml", ".mcp.json"];

/// 查找工作目录中的 MCP 配置文件
pub fn find_mcp_config(start_dir: &Path) -> Option<std::path::PathBuf> {
    // 1. 项目级配置（优先）
    for filename in MCP_CONFIG_FILENAMES {
        let path = start_dir.join(filename);
        if path.exists() {
            return Some(path);
        }
    }

    // 2. 用户级配置目录 (~/.matrixcode/)
    if let Some(home) = dirs::home_dir() {
        let matrixcode_dir = home.join(".matrixcode");
        for filename in MCP_CONFIG_FILENAMES {
            let path = matrixcode_dir.join(filename);
            if path.exists() {
                return Some(path);
            }
        }

        // 3. 用户主目录 (~/.mcp.toml)
        for filename in MCP_CONFIG_FILENAMES {
            let path = home.join(filename);
            if path.exists() {
                return Some(path);
            }
        }
    }

    None
}

/// 加载 MCP 配置（合并项目级和用户级）
pub fn load_mcp_config(start_dir: &Path) -> McpConfig {
    let mut config = McpConfig::new();

    // 1. 加载用户级配置（基础）
    if let Some(home) = dirs::home_dir() {
        // ~/.matrixcode/ 目录
        let matrixcode_dir = home.join(".matrixcode");
        for filename in MCP_CONFIG_FILENAMES {
            let path = matrixcode_dir.join(filename);
            if path.exists()
                && let Ok(user_config) = McpConfig::from_file(&path) {
                    tracing::info!("Loaded user-level MCP config from {:?}", path);
                    config = config.merge(user_config);
                    break;
                }
        }

        // ~/.mcp.toml（备选）
        if config.servers.is_empty() {
            for filename in MCP_CONFIG_FILENAMES {
                let path = home.join(filename);
                if path.exists()
                    && let Ok(user_config) = McpConfig::from_file(&path) {
                        tracing::info!("Loaded user MCP config from {:?}", path);
                        config = config.merge(user_config);
                        break;
                    }
            }
        }
    }

    // 2. 加载项目级配置（覆盖）
    for filename in MCP_CONFIG_FILENAMES {
        let path = start_dir.join(filename);
        if path.exists()
            && let Ok(project_config) = McpConfig::from_file(&path) {
                tracing::info!("Loaded project-level MCP config from {:?}", path);
                config = config.merge(project_config);
                break;
            }
    }

    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_config_stdio() {
        let config = McpServerConfig::stdio("npx", vec!["-y".into(), "@playwright/mcp".into()]);

        assert!(config.command.is_some());
        assert!(config.url.is_none());
        assert!(config.enabled);

        let transport = config.to_transport_config().unwrap();
        match transport {
            TransportConfig::Stdio { command, args, .. } => {
                assert_eq!(command, "npx");
                assert_eq!(args.len(), 2);
            }
            _ => panic!("Expected Stdio transport"),
        }
    }

    #[test]
    fn test_server_config_sse() {
        let config = McpServerConfig::sse("http://localhost:3000");

        assert!(config.command.is_none());
        assert!(config.url.is_some());

        let transport = config.to_transport_config().unwrap();
        match transport {
            TransportConfig::Sse { url, .. } => {
                assert_eq!(url, "http://localhost:3000");
            }
            _ => panic!("Expected SSE transport"),
        }
    }

    #[test]
    fn test_config_serialization() {
        let config = McpConfig::new().add_server(
            "playwright",
            McpServerConfig::stdio("npx", vec!["-y".into(), "@playwright/mcp".into()]),
        );

        // TOML 序列化
        let toml = toml::to_string(&config).unwrap();
        assert!(toml.contains("[servers.playwright]"));

        // 反序列化
        let parsed: McpConfig = toml::from_str(&toml).unwrap();
        assert!(parsed.servers.contains_key("playwright"));
    }

    #[test]
    fn test_playwright_config() {
        let config = playwright_config();
        assert!(config.servers.contains_key("playwright"));

        let server = &config.servers["playwright"];
        assert_eq!(server.command, Some("npx".to_string()));
    }
}
