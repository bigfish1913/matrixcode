//! LSP Handler
//!
//! Handles LSP server startup, status, and lifecycle management.

use std::path::PathBuf;
use std::sync::Arc;
use matrixcode_core::{AgentEvent, lsp::{LspClientRegistry, LspManager, LspServerInfo}};
use tokio::time::{Duration, timeout};

/// LSP 启动超时时间（秒）- 大型 Rust 项目需要较长时间索引
const LSP_STARTUP_TIMEOUT_SECS: u64 = 180;

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

    /// Add servers from config and start LSP clients in background (non-blocking)
    /// Agent can continue immediately, LSP tools will wait for clients to be ready
    pub async fn add_servers(&self, lsp_servers: Vec<(String, matrixcode_core::lsp::LspServerConfig)>, project_root: PathBuf, event_tx: tokio::sync::mpsc::Sender<AgentEvent>) {
        matrixcode_core::debug::debug_log().log("lsp", &format!("add_servers: {} servers (background mode)", lsp_servers.len()));

        // Add servers to manager and mark as starting
        {
            let mut manager = self.manager.write().await;
            for (name, config) in lsp_servers.iter() {
                manager.add_server(config.clone());
                manager.mark_starting(&config.language);
                matrixcode_core::debug::debug_log().log("lsp", &format!("Server '{}' marked as starting", name));
            }
        }

        // Start each server in background task
        for (name, config) in lsp_servers {
            let registry = self.registry.clone();
            let manager = self.manager.clone();
            let project_root_clone = project_root.clone();
            let language = config.language.clone();
            let event_tx_clone = event_tx.clone();

            matrixcode_core::debug::debug_log().log("lsp", &format!("Spawning background task for '{}'", name));

            tokio::spawn(async move {
                matrixcode_core::debug::debug_log().log("lsp", &format!("Background: starting '{}'...", name));

                let start_result = timeout(
                    Duration::from_secs(LSP_STARTUP_TIMEOUT_SECS),
                    registry.register(&config, &project_root_clone)
                ).await;

                // Update status after startup completes and notify UI
                match start_result {
                    Ok(Ok(_)) => {
                        matrixcode_core::debug::debug_log().log("lsp", &format!("Background: '{}' started OK", name));
                        manager.write().await.mark_connected(&language);
                    }
                    Ok(Err(e)) => {
                        matrixcode_core::debug::debug_log().log("lsp", &format!("Background: '{}' failed: {}", name, e));
                        manager.write().await.mark_error(&language, e.to_string());
                    }
                    Err(_) => {
                        matrixcode_core::debug::debug_log().log("lsp", &format!("Background: '{}' timeout", name));
                        manager.write().await.mark_error(&language, "Startup timeout".to_string());
                    }
                }
                
                // Send status update to UI after each server completes startup
                let servers = manager.read().await.server_infos();
                let _ = event_tx_clone.send(AgentEvent::lsp_server_status(servers)).await;
            });
        }

        matrixcode_core::debug::debug_log().log("lsp", "add_servers complete (background tasks running)");
    }

    /// Get registry for tool injection
    pub fn registry(&self) -> Arc<LspClientRegistry> {
        self.registry.clone()
    }

    /// Start all LSP servers and notify UI
    pub async fn start_all(&self, event_tx: &tokio::sync::mpsc::Sender<AgentEvent>) {
        let manager = self.manager.read().await;

        // Get current server status (should be "starting" after add_servers)
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