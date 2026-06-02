//! LSP Client Registry
//!
//! 管理多个语言的 LSP 客户端。

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, timeout};

use super::client::LspClient;
use super::types::LspServerConfig;

/// LSP 客户端注册表
pub struct LspClientRegistry {
    /// 语言 -> 客户端映射
    clients: Arc<RwLock<HashMap<String, Arc<LspClient>>>>,
}

/// 默认等待 LSP 客户端启动的时间（与启动超时一致）
const LSP_WAIT_TIMEOUT_SECS: u64 = 180;

impl LspClientRegistry {
    /// 创建空注册表
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 启动并注册 LSP 客户端
    pub async fn register(&self, config: &LspServerConfig, project_root: &Path) -> Result<()> {
        let client = LspClient::from_config(config, project_root.to_path_buf());
        client.spawn(config).await?;
        let mut clients = self.clients.write().await;
        clients.insert(config.language.clone(), Arc::new(client));
        log::info!("LSP client registered for language: {}", config.language);
        Ok(())
    }

    /// 获取指定语言的客户端（立即返回，不等待）
    pub async fn get_client(&self, language: &str) -> Option<Arc<LspClient>> {
        let clients = self.clients.read().await;
        clients.get(language).cloned()
    }

    /// 获取指定语言的客户端，等待启动完成（最多 30 秒）
    pub async fn get_client_or_wait(&self, language: &str) -> Result<Arc<LspClient>> {
        // 先尝试立即获取
        if let Some(client) = self.get_client(language).await {
            return Ok(client);
        }

        // 等待客户端启动
        log::info!("Waiting for LSP client '{}' to start...", language);
        let wait_duration = Duration::from_secs(LSP_WAIT_TIMEOUT_SECS);

        timeout(wait_duration, async {
            loop {
                if let Some(client) = self.get_client(language).await {
                    log::info!("LSP client '{}' is now available", language);
                    return Ok(client);
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        })
        .await
        .map_err(|_| anyhow::anyhow!(
            "LSP 客户端 '{}' 启动超时（{}秒）。\n\
            提示：LSP 服务器可能正在后台启动，请稍后再试。\n\
            状态：检查 TUI 状态栏 LSP 是否显示 'starting...'",
            language, LSP_WAIT_TIMEOUT_SECS
        ))?
    }

    /// 是否有活跃客户端
    pub async fn has_active_clients(&self) -> bool {
        let clients = self.clients.read().await;
        !clients.is_empty()
    }

    /// 获取所有活跃语言
    pub async fn active_languages(&self) -> Vec<String> {
        let clients = self.clients.read().await;
        clients.keys().cloned().collect()
    }

    /// 关闭所有客户端
    pub async fn shutdown_all(&self) -> Result<()> {
        let mut clients = self.clients.write().await;
        for (language, client) in clients.iter() {
            if let Err(e) = client.shutdown().await {
                log::warn!("Failed to shutdown LSP client '{}': {}", language, e);
            }
        }
        clients.clear();
        Ok(())
    }
}

impl Default for LspClientRegistry {
    fn default() -> Self {
        Self::new()
    }
}