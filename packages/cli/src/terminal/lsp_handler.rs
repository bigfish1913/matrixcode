//! LSP Handler
//!
//! Handles LSP server startup, status, and lifecycle management.

use std::path::PathBuf;
use std::sync::Arc;
use matrixcode_core::{AgentEvent, lsp::{LspClientRegistry, LspManager, LspServerInfo}};

/// LSP manager that handles server lifecycle
pub struct LspHandler {
    manager: Arc<tokio::sync::RwLock<LspManager>>,
    registry: Arc<LspClientRegistry>,
}

impl LspHandler {
    /// Create new LSP handler with servers config
    pub fn new() -> Self {
        Self {
            manager: Arc::new(tokio::sync::RwLock::new(LspManager::new())),
            registry: Arc::new(LspClientRegistry::new()),
        }
    }

    /// Add servers from config and spawn actual LSP clients
    pub async fn add_servers(&self, lsp_servers: Vec<(String, matrixcode_core::lsp::LspServerConfig)>, project_root: PathBuf) {
        let mut manager = self.manager.write().await;
        for (name, config) in lsp_servers {
            manager.add_server(config.clone());
            log::info!("LSP server '{}' added to manager", name);

            // Spawn actual LSP client
            if let Err(e) = self.registry.register(&config, &project_root).await {
                log::warn!("Failed to start LSP client '{}': {}", name, e);
                manager.mark_error(&config.language, e.to_string());
            } else {
                log::info!("LSP client '{}' started", name);
            }
        }
    }

    /// Get registry for tool injection
    pub fn registry(&self) -> Arc<LspClientRegistry> {
        self.registry.clone()
    }

    /// Start all LSP servers and notify UI
    pub async fn start_all(&self, event_tx: &tokio::sync::mpsc::Sender<AgentEvent>) {
        let manager = self.manager.write().await;

        // Get all server configs and mark them as connected
        let servers: Vec<_> = manager.server_infos();
        
        // Mark all detected servers as connected (they passed detection)
        for server in &servers {
            manager.mark_connected(&server.language);
        }
        
        // Get updated statuses
        let servers = manager.server_infos();

        // Notify UI about each server
        for server in &servers {
            let _ = event_tx.send(AgentEvent::lsp_server_added(
                server.name.clone(),
                server.language.clone(),
            )).await;
        }

        // Send overall status
        let _ = event_tx.send(AgentEvent::lsp_server_status(servers)).await;
    }

    /// Get server statuses
    #[allow(dead_code)]
    pub async fn get_status(&self) -> Vec<LspServerInfo> {
        self.manager.read().await.server_infos()
    }
}

impl Default for LspHandler {
    fn default() -> Self {
        Self::new()
    }
}