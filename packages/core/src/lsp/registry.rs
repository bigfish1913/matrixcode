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
use super::constants::LSP_WAIT_TIMEOUT_SECS;
use super::progress::LspProgressCallback;
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

    /// 启动并注册 LSP 客户端（带进度回调）
    ///
    /// 使用异步初始化 + 进度回调，适合 TUI 环境显示进度条。
    ///
    /// # Arguments
    /// - `config`: LSP 服务器配置
    /// - `project_root`: 项目根目录
    /// - `progress_callback`: 进度回调（可以使用 `NoOpProgressCallback` 如果不需要进度显示）
    ///
    /// # Progress Flow
    /// - 0.1: Starting process...
    /// - 0.3: Process started, initializing server...
    /// - 0.5-0.9: Loading workspace...
    /// - 1.0: Ready
    ///
    /// # Timeouts
    /// - 进程启动：5 秒（快速失败）
    /// - 服务器初始化：60 秒（大型项目可能需要）
    pub async fn register_with_progress(
        &self,
        config: &LspServerConfig,
        project_root: &Path,
        progress_callback: Arc<dyn LspProgressCallback>,
    ) -> Result<()> {
        let client = LspClient::from_config(config, project_root.to_path_buf());
        
        // 使用异步初始化 + 进度回调
        client.spawn_async(config, progress_callback.clone()).await?;
        
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