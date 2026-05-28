//! LSP Types
//!
//! Language Server Protocol 相关类型定义

use serde::{Deserialize, Serialize};

/// LSP 服务器状态
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LspServerStatus {
    /// 未启动（灰色）
    NotStarted,
    /// 已连接，正常工作（绿色）
    Connected,
    /// 连接错误（红色）
    Error(String),
}

impl Default for LspServerStatus {
    fn default() -> Self {
        Self::NotStarted
    }
}

impl LspServerStatus {
    /// 获取状态显示文本
    pub fn label(&self) -> String {
        match self {
            LspServerStatus::NotStarted => "off".into(),
            LspServerStatus::Connected => "ok".into(),
            LspServerStatus::Error(msg) => format!("err: {}", msg),
        }
    }

    /// 是否正常工作
    pub fn is_ok(&self) -> bool {
        matches!(self, LspServerStatus::Connected)
    }

    /// 是否有错误
    pub fn is_error(&self) -> bool {
        matches!(self, LspServerStatus::Error(_))
    }
}

/// LSP 服务器信息（用于 TUI 显示）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LspServerInfo {
    /// 服务器名称（如 "rust-analyzer", "typescript"）
    pub name: String,
    /// 语言标识（如 "rust", "typescript"）
    pub language: String,
    /// 当前状态
    pub status: LspServerStatus,
}

impl LspServerInfo {
    /// 创建新的 LSP 服务器信息
    pub fn new(name: impl Into<String>, language: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            language: language.into(),
            status: LspServerStatus::NotStarted,
        }
    }

    /// 创建已连接状态
    pub fn connected(name: impl Into<String>, language: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            language: language.into(),
            status: LspServerStatus::Connected,
        }
    }

    /// 创建错误状态
    pub fn error(name: impl Into<String>, language: impl Into<String>, msg: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            language: language.into(),
            status: LspServerStatus::Error(msg.into()),
        }
    }

    /// 更新状态
    pub fn with_status(&self, status: LspServerStatus) -> Self {
        Self {
            name: self.name.clone(),
            language: self.language.clone(),
            status,
        }
    }
}

/// LSP 服务器配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LspServerConfig {
    /// 启动命令
    pub command: String,
    /// 命令参数
    #[serde(default)]
    pub args: Vec<String>,
    /// 语言标识
    pub language: String,
    /// 是否启用
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

impl LspServerConfig {
    /// 创建新的 LSP 服务器配置
    pub fn new(command: impl Into<String>, language: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            args: Vec::new(),
            language: language.into(),
            enabled: true,
        }
    }

    /// 添加参数
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }
}

/// LSP 配置文件
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct LspConfig {
    /// LSP 服务器配置映射
    #[serde(default)]
    pub servers: Vec<LspServerConfig>,
}

impl LspConfig {
    /// 创建空配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 从 TOML 文件加载
    pub fn from_file(path: &std::path::Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }

    /// 获取启用的服务器配置
    pub fn enabled_servers(&self) -> Vec<&LspServerConfig> {
        self.servers.iter().filter(|s| s.enabled).collect()
    }

    /// 添加服务器配置
    pub fn add_server(&mut self, config: LspServerConfig) {
        self.servers.push(config);
    }
}

/// 常用 LSP 服务器预设配置
pub fn default_rust_analyzer_config() -> LspServerConfig {
    LspServerConfig::new("rust-analyzer", "rust")
}

pub fn default_typescript_config() -> LspServerConfig {
    LspServerConfig::new("typescript-language-server", "typescript")
        .with_args(vec!["--stdio".into()])
}

pub fn default_python_config() -> LspServerConfig {
    LspServerConfig::new("pyright-langserver", "python")
        .with_args(vec!["--stdio".into()])
}

/// 默认 LSP 配置
pub fn default_lsp_config() -> LspConfig {
    LspConfig {
        servers: vec![
            default_rust_analyzer_config(),
        ],
    }
}