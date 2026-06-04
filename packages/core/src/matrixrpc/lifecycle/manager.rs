//! Lifecycle Manager
//!
//! Manages the lifecycle of extension services including connection,
//! reconnection, heartbeat monitoring, and graceful shutdown.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, mpsc, RwLock};
use tokio::time::{interval, sleep};

use crate::matrixrpc::registry::RegistryService;
use crate::matrixrpc::service::{ExtensionService, ServiceId, ServiceStatus};

/// Lifecycle event
#[derive(Debug, Clone)]
pub enum LifecycleEvent {
    /// Service started
    Started(ServiceId),

    /// Service stopped
    Stopped(ServiceId),

    /// Service status changed
    StatusChanged {
        id: ServiceId,
        old_status: ServiceStatus,
        new_status: ServiceStatus,
    },

    /// Heartbeat received
    Heartbeat(ServiceId),

    /// Heartbeat timeout
    HeartbeatTimeout(ServiceId),

    /// Reconnection attempt
    Reconnecting {
        id: ServiceId,
        attempt: u32,
        max_attempts: u32,
    },

    /// Reconnection succeeded
    Reconnected(ServiceId),

    /// Reconnection failed
    ReconnectFailed(ServiceId),

    /// Service error
    Error {
        id: ServiceId,
        error: String,
    },
}

/// Lifecycle configuration
#[derive(Debug, Clone)]
pub struct LifecycleConfig {
    /// Heartbeat interval in seconds
    pub heartbeat_interval_secs: u64,

    /// Heartbeat timeout in seconds
    pub heartbeat_timeout_secs: u64,

    /// Maximum reconnection attempts
    pub max_reconnect_attempts: u32,

    /// Initial reconnection delay in milliseconds
    pub reconnect_delay_ms: u64,

    /// Maximum reconnection delay in milliseconds
    pub max_reconnect_delay_ms: u64,

    /// Backoff multiplier for reconnection delays
    pub reconnect_backoff_multiplier: f64,

    /// Enable auto-reconnection
    pub auto_reconnect: bool,
}

impl Default for LifecycleConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval_secs: 30,
            heartbeat_timeout_secs: 90,
            max_reconnect_attempts: 5,
            reconnect_delay_ms: 1000,
            max_reconnect_delay_ms: 30000,
            reconnect_backoff_multiplier: 2.0,
            auto_reconnect: true,
        }
    }
}

/// Lifecycle manager error
#[derive(Debug, thiserror::Error)]
pub enum LifecycleError {
    /// Service not found
    #[error("Service '{0}' not found")]
    NotFound(String),

    /// Connection failed
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    /// Reconnection failed
    #[error("Reconnection failed after {0} attempts")]
    ReconnectFailed(u32),

    /// Invalid state
    #[error("Invalid state: {0}")]
    InvalidState(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Service lifecycle state
#[derive(Debug, Clone)]
struct ServiceLifecycle {
#[allow(dead_code)]
    /// Service ID
    id: ServiceId,
#[allow(dead_code)]

    /// Current status
    status: ServiceStatus,

    /// Reconnection attempts
    reconnect_attempts: u32,

    /// Is auto-reconnect enabled for this service
    auto_reconnect: bool,

    /// Stop signal sender
    stop_tx: Option<mpsc::Sender<()>>,
}

/// Lifecycle Manager
///
/// Manages service lifecycle events including:
/// - Connection initialization
/// - Heartbeat monitoring
/// - Auto-reconnection with backoff
/// - Graceful shutdown
pub struct LifecycleManager {
    /// Registry service reference
    registry: Arc<RegistryService>,

    /// Lifecycle configuration
    config: LifecycleConfig,

    /// Service lifecycle states
    lifecycles: Arc<RwLock<HashMap<ServiceId, ServiceLifecycle>>>,

    /// Event broadcaster
    event_tx: broadcast::Sender<LifecycleEvent>,
}

impl LifecycleManager {
    /// Create a new lifecycle manager
    pub fn new(registry: Arc<RegistryService>) -> Self {
        Self::with_config(registry, LifecycleConfig::default())
    }

    /// Create a new lifecycle manager with configuration
    pub fn with_config(registry: Arc<RegistryService>, config: LifecycleConfig) -> Self {
        let (event_tx, _) = broadcast::channel(256);

        Self {
            registry,
            config,
            lifecycles: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
        }
    }

    /// Subscribe to lifecycle events
    pub fn subscribe(&self) -> broadcast::Receiver<LifecycleEvent> {
        self.event_tx.subscribe()
    }

    /// Start managing a service
    pub async fn start_service(&self, service: ExtensionService) -> Result<ServiceId, LifecycleError> {
        let auto_reconnect = service.transport.auto_reconnect;

        // Register the service
        let id = self
            .registry
            .register(service)
            .await
            .map_err(|e| LifecycleError::Internal(e.to_string()))?;

        // Set initial status
        self.registry
            .update_status(&id, ServiceStatus::Starting)
            .await
            .map_err(|e| LifecycleError::Internal(e.to_string()))?;

        // Create lifecycle state
        let (stop_tx, stop_rx) = mpsc::channel(1);
        let lifecycle = ServiceLifecycle {
            id: id.clone(),
            status: ServiceStatus::Starting,
            reconnect_attempts: 0,
            auto_reconnect,
            stop_tx: Some(stop_tx),
        };

        {
            let mut lifecycles = self.lifecycles.write().await;
            lifecycles.insert(id.clone(), lifecycle);
        }

        // Start heartbeat monitor task
        self.spawn_heartbeat_monitor(id.clone(), stop_rx);

        // Emit started event
        let _ = self.event_tx.send(LifecycleEvent::Started(id.clone()));

        // Transition to running
        self.transition_status(&id, ServiceStatus::Running).await?;

        Ok(id)
    }

    /// Stop managing a service
    pub async fn stop_service(&self, id: &ServiceId) -> Result<(), LifecycleError> {
        // Update status first before removing from lifecycles
        self.transition_status(id, ServiceStatus::Stopping).await?;

        // Get lifecycle and remove from tracking
        let lifecycle = {
            let mut lifecycles = self.lifecycles.write().await;
            lifecycles.remove(id).ok_or_else(|| LifecycleError::NotFound(id.to_string()))?
        };

        // Send stop signal
        if let Some(stop_tx) = lifecycle.stop_tx {
            let _ = stop_tx.send(()).await;
        }

        // Unregister from registry
        self.registry
            .unregister(id)
            .await
            .map_err(|e| LifecycleError::Internal(e.to_string()))?;

        // Emit stopped event
        let _ = self.event_tx.send(LifecycleEvent::Stopped(id.clone()));

        Ok(())
    }

    /// Handle heartbeat from a service
    pub async fn handle_heartbeat(&self, id: &ServiceId) -> Result<(), LifecycleError> {
        self.registry
            .heartbeat(id)
            .await
            .map_err(|e| LifecycleError::Internal(e.to_string()))?;

        // Reset reconnection attempts
        {
            let mut lifecycles = self.lifecycles.write().await;
            if let Some(lifecycle) = lifecycles.get_mut(id) {
                lifecycle.reconnect_attempts = 0;
            }
        }

        // Emit heartbeat event
        let _ = self.event_tx.send(LifecycleEvent::Heartbeat(id.clone()));

        Ok(())
    }

    /// Handle service error
    pub async fn handle_error(&self, id: &ServiceId, error: String) -> Result<(), LifecycleError> {
        // Emit error event
        let _ = self.event_tx.send(LifecycleEvent::Error {
            id: id.clone(),
            error,
        });

        // Transition to error status
        self.transition_status(id, ServiceStatus::Error).await?;

        // Attempt reconnection if enabled
        let should_reconnect = {
            let lifecycles = self.lifecycles.read().await;
            lifecycles
                .get(id)
                .map(|l| l.auto_reconnect)
                .unwrap_or(false)
        };

        if should_reconnect {
            self.attempt_reconnect(id).await?;
        }

        Ok(())
    }

    /// Attempt to reconnect a service
    async fn attempt_reconnect(&self, id: &ServiceId) -> Result<(), LifecycleError> {
        let (max_attempts, _delay_ms, backoff) = {
            let lifecycles = self.lifecycles.read().await;
            let lifecycle = lifecycles.get(id).ok_or_else(|| LifecycleError::NotFound(id.to_string()))?;
            (self.config.max_reconnect_attempts, self.config.reconnect_delay_ms, lifecycle.reconnect_attempts)
        };

        // Check max attempts
        if backoff >= max_attempts {
            let _ = self.event_tx.send(LifecycleEvent::ReconnectFailed(id.clone()));
            return Err(LifecycleError::ReconnectFailed(max_attempts));
        }

        // Update status and increment attempts
        {
            let mut lifecycles = self.lifecycles.write().await;
            if let Some(lifecycle) = lifecycles.get_mut(id) {
                lifecycle.reconnect_attempts += 1;
                lifecycle.status = ServiceStatus::Reconnecting;
            }
        }

        // Emit reconnecting event
        let _ = self.event_tx.send(LifecycleEvent::Reconnecting {
            id: id.clone(),
            attempt: backoff + 1,
            max_attempts: max_attempts,
        });

        self.registry
            .update_status(id, ServiceStatus::Reconnecting)
            .await
            .map_err(|e| LifecycleError::Internal(e.to_string()))?;

        // Calculate backoff delay
        let delay = self.calculate_reconnect_delay(backoff);

        // Wait for backoff
        sleep(Duration::from_millis(delay)).await;

        // Here you would attempt actual reconnection
        // This is a placeholder - actual implementation depends on transport

        Ok(())
    }

    /// Calculate reconnection delay with exponential backoff
    fn calculate_reconnect_delay(&self, attempt: u32) -> u64 {
        let base = self.config.reconnect_delay_ms as f64;
        let multiplier = self.config.reconnect_backoff_multiplier.powi(attempt as i32);
        let delay = base * multiplier;

        // Add jitter (10%)
        let jitter = delay * 0.1 * (rand_jitter() - 0.5) * 2.0;

        let final_delay = (delay + jitter) as u64;
        final_delay.min(self.config.max_reconnect_delay_ms)
    }

    /// Transition service status
    async fn transition_status(
        &self,
        id: &ServiceId,
        new_status: ServiceStatus,
    ) -> Result<(), LifecycleError> {
        let old_status = {
            let lifecycles = self.lifecycles.read().await;
            lifecycles
                .get(id)
                .map(|l| l.status)
                .ok_or_else(|| LifecycleError::NotFound(id.to_string()))?
        };

        if old_status == new_status {
            return Ok(());
        }

        // Update lifecycle state
        {
            let mut lifecycles = self.lifecycles.write().await;
            if let Some(lifecycle) = lifecycles.get_mut(id) {
                lifecycle.status = new_status;
            }
        }

        // Update registry
        self.registry
            .update_status(id, new_status)
            .await
            .map_err(|e| LifecycleError::Internal(e.to_string()))?;

        // Emit status changed event
        let _ = self.event_tx.send(LifecycleEvent::StatusChanged {
            id: id.clone(),
            old_status,
            new_status,
        });

        Ok(())
    }

    /// Spawn heartbeat monitor task
    fn spawn_heartbeat_monitor(&self, id: ServiceId, mut stop_rx: mpsc::Receiver<()>) {
        let registry = self.registry.clone();
        let event_tx = self.event_tx.clone();
        let timeout_secs = self.config.heartbeat_timeout_secs;

        tokio::spawn(async move {
            let mut check_interval = interval(Duration::from_secs(timeout_secs / 3));

            loop {
                tokio::select! {
                    _ = stop_rx.recv() => {
                        // Stop signal received
                        break;
                    }
                    _ = check_interval.tick() => {
                        // Check if service is healthy
                        if let Some(service) = registry.get(&id).await {
                            if !service.is_healthy(timeout_secs) {
                                if service.status == ServiceStatus::Running {
                                    // Emit heartbeat timeout event
                                    let _ = event_tx.send(LifecycleEvent::HeartbeatTimeout(id.clone()));

                                    // Update status to reconnecting
                                    let _ = registry.update_status(&id, ServiceStatus::Reconnecting).await;
                                }
                            }
                        } else {
                            // Service no longer exists
                            break;
                        }
                    }
                }
            }
        });
    }

    /// Stop all services
    pub async fn stop_all(&self) {
        let ids: Vec<ServiceId> = {
            let lifecycles = self.lifecycles.read().await;
            lifecycles.keys().cloned().collect()
        };

        for id in ids {
            let _ = self.stop_service(&id).await;
        }
    }

    /// Get service status
    pub async fn get_status(&self, id: &ServiceId) -> Option<ServiceStatus> {
        let lifecycles = self.lifecycles.read().await;
        lifecycles.get(id).map(|l| l.status)
    }

    /// Check if service is healthy
    pub async fn is_healthy(&self, id: &ServiceId) -> bool {
        let lifecycles = self.lifecycles.read().await;
        lifecycles
            .get(id)
            .map(|l| l.status == ServiceStatus::Running)
            .unwrap_or(false)
    }

    /// Get managed service count
    pub async fn count(&self) -> usize {
        self.lifecycles.read().await.len()
    }

    /// Trigger health check for all services
    pub async fn health_check(&self) -> Vec<ServiceId> {
        self.registry.health_check().await
    }
}

/// Simple random jitter generator
fn rand_jitter() -> f64 {
    // Use a simple deterministic jitter based on time
    // In production, use proper random number generator
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    nanos as f64 / u32::MAX as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_lifecycle_manager_creation() {
        let registry = Arc::new(RegistryService::new());
        let manager = LifecycleManager::new(registry);
        assert_eq!(manager.count().await, 0);
    }

    #[tokio::test]
    async fn test_start_stop_service() {
        let registry = Arc::new(RegistryService::new());
        let manager = LifecycleManager::new(registry);

        let service = ExtensionService::new("test", "1.0.0");
        let id = manager.start_service(service).await.unwrap();

        assert_eq!(manager.count().await, 1);
        assert_eq!(manager.get_status(&id).await, Some(ServiceStatus::Running));

        manager.stop_service(&id).await.unwrap();
        assert_eq!(manager.count().await, 0);
    }

    #[tokio::test]
    async fn test_handle_heartbeat() {
        let registry = Arc::new(RegistryService::new());
        let manager = LifecycleManager::new(registry);

        let service = ExtensionService::new("test", "1.0.0");
        let id = manager.start_service(service).await.unwrap();

        let result = manager.handle_heartbeat(&id).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_error() {
        let registry = Arc::new(RegistryService::new());
        let manager = LifecycleManager::new(registry.clone());

        let service = ExtensionService::new("test", "1.0.0");
        let id = manager.start_service(service).await.unwrap();

        // Disable auto-reconnect for this test
        {
            let mut lifecycles = manager.lifecycles.write().await;
            if let Some(l) = lifecycles.get_mut(&id) {
                l.auto_reconnect = false;
            }
        }

        manager
            .handle_error(&id, "Test error".to_string())
            .await
            .unwrap();

        assert_eq!(manager.get_status(&id).await, Some(ServiceStatus::Error));
    }

    #[tokio::test]
    async fn test_lifecycle_events() {
        let registry = Arc::new(RegistryService::new());
        let manager = LifecycleManager::new(registry);

        let mut event_rx = manager.subscribe();

        let service = ExtensionService::new("test", "1.0.0");
        let id = manager.start_service(service).await.unwrap();

        // Should receive Started and StatusChanged events
        let event1 = event_rx.try_recv();
        let event2 = event_rx.try_recv();

        assert!(event1.is_ok() || event2.is_ok());
    }

    #[tokio::test]
    async fn test_lifecycle_config() {
        let registry = Arc::new(RegistryService::new());
        let config = LifecycleConfig {
            heartbeat_interval_secs: 10,
            heartbeat_timeout_secs: 30,
            max_reconnect_attempts: 3,
            ..Default::default()
        };
        let manager = LifecycleManager::with_config(registry, config);

        assert_eq!(manager.config.heartbeat_interval_secs, 10);
        assert_eq!(manager.config.heartbeat_timeout_secs, 30);
        assert_eq!(manager.config.max_reconnect_attempts, 3);
    }

    #[test]
    fn test_calculate_reconnect_delay() {
        let registry = Arc::new(RegistryService::new());
        let manager = LifecycleManager::new(registry);

        let delay0 = manager.calculate_reconnect_delay(0);
        let delay1 = manager.calculate_reconnect_delay(1);
        let delay2 = manager.calculate_reconnect_delay(2);

        // Delay should increase with backoff
        assert!(delay1 > delay0);
        assert!(delay2 > delay1);
    }
}