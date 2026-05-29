//! LSP Handler
//!
//! Handles LSP server startup, status, and lifecycle management.

use std::sync::Arc;
use matrixcode_core::{AgentEvent, lsp::{LspManager, LspServerInfo}};

/// LSP manager that handles server lifecycle
pub struct LspHandler {
    manager: Arc<tokio::sync::RwLock<LspManager>>,
}

impl LspHandler {
    /// Create new LSP handler with servers config
    pub fn new() -> Self {
        Self {
            manager: Arc::new(tokio::sync::RwLock::new(LspManager::new())),
        }
    }

    /// Add servers from config
    pub async fn add_servers(&self, lsp_servers: Vec<(String, matrixcode_core::lsp::LspServerConfig)>) {
        let mut manager = self.manager.write().await;
        for (_name, config) in lsp_servers {
            manager.add_server(config);
            log::info!("LSP server '{}' added to manager", _name);
        }
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