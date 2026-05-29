//! LSP Manager
//!
//! 管理多个 LSP 服务器连接

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::types::{LspConfig, LspServerConfig, LspServerInfo, LspServerStatus};

/// LSP 管理器
///
/// 管理多个语言服务器的连接状态
pub struct LspManager {
    /// 服务器配置（按名称索引）
    configs: HashMap<String, LspServerConfig>,
    /// 服务器状态（按名称索引）
    statuses: Arc<RwLock<HashMap<String, LspServerStatus>>>,
}

impl LspManager {
    /// 创建新的 LSP 管理器
    pub fn new() -> Self {
        Self {
            configs: HashMap::new(),
            statuses: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 从配置创建 LSP 管理器
    pub fn from_config(config: &LspConfig) -> Self {
        let configs: HashMap<String, LspServerConfig> = config
            .servers
            .iter()
            .filter(|s| s.enabled)
            .map(|s| (s.language.clone(), s.clone()))
            .collect();

        // 初始化所有状态为 NotStarted
        let statuses = configs
            .keys()
            .map(|k| (k.clone(), LspServerStatus::NotStarted))
            .collect();

        Self {
            configs,
            statuses: Arc::new(RwLock::new(statuses)),
        }
    }

    /// 添加服务器配置
    pub fn add_server(&mut self, config: LspServerConfig) {
        let name = config.language.clone();
        self.configs.insert(name.clone(), config);
        if let Ok(mut statuses) = self.statuses.write() {
            statuses.insert(name, LspServerStatus::NotStarted);
        }
    }

    /// 移除服务器配置
    pub fn remove_server(&mut self, name: &str) {
        self.configs.remove(name);
        if let Ok(mut statuses) = self.statuses.write() {
            statuses.remove(name);
        }
    }

    /// 获取所有服务器信息（用于 TUI 显示）
    pub fn server_infos(&self) -> Vec<LspServerInfo> {
        if let Ok(statuses) = self.statuses.read() {
            self.configs
                .iter()
                .map(|(name, config)| {
                    let status = statuses
                        .get(name)
                        .cloned()
                        .unwrap_or(LspServerStatus::NotStarted);
                    LspServerInfo {
                        name: config.command.clone(),
                        language: config.language.clone(),
                        status,
                    }
                })
                .collect()
        } else {
            // 返回默认状态
            self.configs
                .iter()
                .map(|(_name, config)| LspServerInfo {
                    name: config.command.clone(),
                    language: config.language.clone(),
                    status: LspServerStatus::NotStarted,
                })
                .collect()
        }
    }

    /// 获取服务器状态
    pub fn get_status(&self, name: &str) -> LspServerStatus {
        self.statuses
            .read()
            .map(|s| s.get(name).cloned().unwrap_or(LspServerStatus::NotStarted))
            .unwrap_or(LspServerStatus::NotStarted)
    }

    /// 更新服务器状态
    pub fn set_status(&self, name: &str, status: LspServerStatus) {
        if let Ok(mut statuses) = self.statuses.write() {
            statuses.insert(name.to_string(), status);
        }
    }

    /// 标记服务器已连接
    pub fn mark_connected(&self, name: &str) {
        self.set_status(name, LspServerStatus::Connected);
    }

    /// 标记服务器错误
    pub fn mark_error(&self, name: &str, msg: impl Into<String>) {
        self.set_status(name, LspServerStatus::Error(msg.into()));
    }

    /// 获取服务器配置
    pub fn get_config(&self, name: &str) -> Option<&LspServerConfig> {
        self.configs.get(name)
    }

    /// 获取所有服务器名称
    pub fn server_names(&self) -> Vec<&String> {
        self.configs.keys().collect()
    }

    /// 获取已连接的服务器数量
    pub fn connected_count(&self) -> usize {
        self.statuses
            .read()
            .map(|s| s.values().filter(|s| s.is_ok()).count())
            .unwrap_or(0)
    }

    /// 获取有错误的服务器数量
    pub fn error_count(&self) -> usize {
        self.statuses
            .read()
            .map(|s| s.values().filter(|s| s.is_error()).count())
            .unwrap_or(0)
    }

    /// 是否有任何服务器配置
    pub fn has_servers(&self) -> bool {
        !self.configs.is_empty()
    }

    /// 重置所有状态为未启动
    pub fn reset_all(&self) {
        if let Ok(mut statuses) = self.statuses.write() {
            for name in self.configs.keys() {
                statuses.insert(name.clone(), LspServerStatus::NotStarted);
            }
        }
    }
}

impl Default for LspManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 查找 LSP 配置文件
///
/// 在当前目录或用户主目录查找 `lsp.toml`
pub fn find_lsp_config(start_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    // 当前目录
    let local_config = start_dir.join("lsp.toml");
    if local_config.exists() {
        return Some(local_config);
    }

    // 用户主目录
    if let Some(home) = dirs::home_dir() {
        let home_config = home.join(".config").join("matrixcode").join("lsp.toml");
        if home_config.exists() {
            return Some(home_config);
        }
    }

    None
}

/// 加载 LSP 配置
///
/// 自动查找配置文件，找不到则返回默认配置
pub fn load_lsp_config(start_dir: &std::path::Path) -> LspConfig {
    find_lsp_config(start_dir)
        .and_then(|p| LspConfig::from_file(&p).ok())
        .unwrap_or_default()
}
