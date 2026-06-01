//! LSP Client Registry
//!
//! 管理多个语言的 LSP 客户端。

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::client::LspClient;
use super::types::LspServerConfig;

/// LSP 客户端注册表
pub struct LspClientRegistry {
    /// 语言 -> 客户端映射
    clients: Arc<RwLock<HashMap<String, Arc<LspClient>>>>,
}

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

    /// 获取指定语言的客户端
    pub async fn get_client(&self, language: &str) -> Option<Arc<LspClient>> {
        let clients = self.clients.read().await;
        clients.get(language).cloned()
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