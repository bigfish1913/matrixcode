//! Registry Service Implementation
//!
//! Provides service registration, discovery, and management functionality.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::matrixrpc::service::{ExtensionService, RegistrationInfo, ServiceId, ServiceStatus};

/// Error type for registry operations
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    /// Service already exists
    #[error("Service '{0}' already exists in registry")]
    AlreadyExists(String),

    /// Service not found
    #[error("Service '{0}' not found in registry")]
    NotFound(String),

    /// Service is not running
    #[error("Service '{0}' is not running (status: {1:?})")]
    NotRunning(String, ServiceStatus),

    /// Invalid service state
    #[error("Invalid service state: {0}")]
    InvalidState(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Service query filter
#[derive(Debug, Clone, Default)]
pub struct ServiceFilter {
    /// Filter by status
    pub status: Option<ServiceStatus>,

    /// Filter by capability
    pub capability: Option<String>,

    /// Filter by transport type
    pub transport_type: Option<String>,
}

impl ServiceFilter {
    /// Create a new filter
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by status
    pub fn status(mut self, status: ServiceStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Filter by capability
    pub fn capability(mut self, cap: impl Into<String>) -> Self {
        self.capability = Some(cap.into());
        self
    }

    /// Filter by transport type
    pub fn transport_type(mut self, transport: impl Into<String>) -> Self {
        self.transport_type = Some(transport.into());
        self
    }

    /// Check if a service matches this filter
    pub fn matches(&self, service: &ExtensionService) -> bool {
        if let Some(status) = &self.status {
            if service.status != *status {
                return false;
            }
        }

        if let Some(cap) = &self.capability {
            if !service.has_capability(cap) {
                return false;
            }
        }

        true
    }
}

/// Registry statistics
#[derive(Debug, Clone, Default)]
pub struct RegistryStats {
    /// Total number of services
    pub total: usize,

    /// Number of running services
    pub running: usize,

    /// Number of stopped services
    pub stopped: usize,

    /// Number of error services
    pub errors: usize,

    /// Number of reconnecting services
    pub reconnecting: usize,
}

/// Service Registry
///
/// Manages registration, discovery, and lifecycle of extension services.
#[derive(Debug)]
pub struct RegistryService {
    /// Registered services
    services: Arc<RwLock<HashMap<ServiceId, RegistrationInfo>>>,

    /// Service name to ID mapping
    name_index: Arc<RwLock<HashMap<String, ServiceId>>>,

    /// Capability to service IDs mapping
    capability_index: Arc<RwLock<HashMap<String, Vec<ServiceId>>>>,

    /// Heartbeat timeout in seconds
    heartbeat_timeout_secs: u64,
}

impl Default for RegistryService {
    fn default() -> Self {
        Self::new()
    }
}

impl RegistryService {
    /// Create a new registry service
    pub fn new() -> Self {
        Self {
            services: Arc::new(RwLock::new(HashMap::new())),
            name_index: Arc::new(RwLock::new(HashMap::new())),
            capability_index: Arc::new(RwLock::new(HashMap::new())),
            heartbeat_timeout_secs: 60,
        }
    }

    /// Set heartbeat timeout
    pub fn with_heartbeat_timeout(mut self, secs: u64) -> Self {
        self.heartbeat_timeout_secs = secs;
        self
    }

    /// Register a new service
    pub async fn register(&self, service: ExtensionService) -> Result<ServiceId, RegistryError> {
        let name = service.name.clone();
        let id = service.id.clone();
        let capabilities: Vec<_> = service.capabilities.iter().map(|c| c.name.clone()).collect();

        // Check if service name already exists
        {
            let name_index = self.name_index.read().await;
            if name_index.contains_key(&name) {
                return Err(RegistryError::AlreadyExists(name));
            }
        }

        // Create registration info
        let registration = RegistrationInfo::new(service);

        // Add to services
        {
            let mut services = self.services.write().await;
            services.insert(id.clone(), registration);
        }

        // Update name index
        {
            let mut name_index = self.name_index.write().await;
            name_index.insert(name, id.clone());
        }

        // Update capability index
        {
            let mut cap_index = self.capability_index.write().await;
            for cap in capabilities {
                cap_index
                    .entry(cap)
                    .or_insert_with(Vec::new)
                    .push(id.clone());
            }
        }

        Ok(id)
    }

    /// Unregister a service by ID
    pub async fn unregister(&self, id: &ServiceId) -> Result<RegistrationInfo, RegistryError> {
        // Remove from services
        let registration = {
            let mut services = self.services.write().await;
            services
                .remove(id)
                .ok_or_else(|| RegistryError::NotFound(id.to_string()))?
        };

        // Remove from name index
        {
            let mut name_index = self.name_index.write().await;
            name_index.remove(&registration.service.name);
        }

        // Remove from capability index
        {
            let mut cap_index = self.capability_index.write().await;
            for cap in &registration.service.capabilities {
                if let Some(ids) = cap_index.get_mut(&cap.name) {
                    ids.retain(|sid| sid != id);
                }
            }
        }

        Ok(registration)
    }

    /// Get a service by ID
    pub async fn get(&self, id: &ServiceId) -> Option<ExtensionService> {
        let services = self.services.read().await;
        services.get(id).map(|r| r.service.clone())
    }

    /// Get a service by name
    pub async fn get_by_name(&self, name: &str) -> Option<ExtensionService> {
        let name_index = self.name_index.read().await;
        let id = name_index.get(name)?;

        let services = self.services.read().await;
        services.get(id).map(|r| r.service.clone())
    }

    /// Get service registration info by ID
    pub async fn get_registration(&self, id: &ServiceId) -> Option<RegistrationInfo> {
        let services = self.services.read().await;
        services.get(id).cloned()
    }

    /// Update service status
    pub async fn update_status(
        &self,
        id: &ServiceId,
        status: ServiceStatus,
    ) -> Result<(), RegistryError> {
        let mut services = self.services.write().await;
        let registration = services
            .get_mut(id)
            .ok_or_else(|| RegistryError::NotFound(id.to_string()))?;

        registration.service.set_status(status);
        registration.touch();
        Ok(())
    }

    /// Record a heartbeat for a service
    pub async fn heartbeat(&self, id: &ServiceId) -> Result<(), RegistryError> {
        let mut services = self.services.write().await;
        let registration = services
            .get_mut(id)
            .ok_or_else(|| RegistryError::NotFound(id.to_string()))?;

        registration.service.heartbeat();
        registration.touch();
        Ok(())
    }

    /// List all services
    pub async fn list_all(&self) -> Vec<ExtensionService> {
        let services = self.services.read().await;
        services.values().map(|r| r.service.clone()).collect()
    }

    /// List services matching a filter
    pub async fn list(&self, filter: &ServiceFilter) -> Vec<ExtensionService> {
        let services = self.services.read().await;
        services
            .values()
            .filter(|r| filter.matches(&r.service))
            .map(|r| r.service.clone())
            .collect()
    }

    /// Find services by capability
    pub async fn find_by_capability(&self, capability: &str) -> Vec<ExtensionService> {
        let cap_index = self.capability_index.read().await;
        let ids = cap_index.get(capability).cloned().unwrap_or_default();
        drop(cap_index);

        let services = self.services.read().await;
        ids.iter()
            .filter_map(|id| services.get(id).map(|r| r.service.clone()))
            .collect()
    }

    /// Get registry statistics
    pub async fn stats(&self) -> RegistryStats {
        let services = self.services.read().await;

        let mut stats = RegistryStats {
            total: services.len(),
            ..Default::default()
        };

        for registration in services.values() {
            match registration.service.status {
                ServiceStatus::Running => stats.running += 1,
                ServiceStatus::Stopped => stats.stopped += 1,
                ServiceStatus::Error => stats.errors += 1,
                ServiceStatus::Reconnecting => stats.reconnecting += 1,
                _ => {}
            }
        }

        stats
    }

    /// Check health of all services and update status
    pub async fn health_check(&self) -> Vec<ServiceId> {
        let timeout = self.heartbeat_timeout_secs;
        let mut unhealthy = Vec::new();

        let mut services = self.services.write().await;
        for (id, registration) in services.iter_mut() {
            if !registration.service.is_healthy(timeout) {
                if registration.service.status == ServiceStatus::Running {
                    registration.service.set_status(ServiceStatus::Reconnecting);
                    registration.touch();
                    unhealthy.push(id.clone());
                }
            }
        }

        unhealthy
    }

    /// Clear all registrations
    pub async fn clear(&self) {
        let mut services = self.services.write().await;
        let mut name_index = self.name_index.write().await;
        let mut cap_index = self.capability_index.write().await;

        services.clear();
        name_index.clear();
        cap_index.clear();
    }

    /// Get the number of registered services
    pub async fn count(&self) -> usize {
        let services = self.services.read().await;
        services.len()
    }
}

/// Builder for creating registry with custom configuration
#[derive(Debug)]
pub struct RegistryBuilder {
    heartbeat_timeout_secs: u64,
}

impl Default for RegistryBuilder {
    fn default() -> Self {
        Self {
            heartbeat_timeout_secs: 60,
        }
    }
}

impl RegistryBuilder {
    /// Create a new registry builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set heartbeat timeout
    pub fn heartbeat_timeout(mut self, secs: u64) -> Self {
        self.heartbeat_timeout_secs = secs;
        self
    }

    /// Build the registry service
    pub fn build(self) -> RegistryService {
        RegistryService::new().with_heartbeat_timeout(self.heartbeat_timeout_secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matrixrpc::Capability;

    #[tokio::test]
    async fn test_register_service() {
        let registry = RegistryService::new();
        let service = ExtensionService::new("test-service", "1.0.0");

        let id = registry.register(service).await.unwrap();
        assert!(registry.get(&id).await.is_some());
        assert_eq!(registry.count().await, 1);
    }

    #[tokio::test]
    async fn test_register_duplicate_name() {
        let registry = RegistryService::new();
        let service1 = ExtensionService::new("test", "1.0.0");
        let service2 = ExtensionService::new("test", "2.0.0");

        registry.register(service1).await.unwrap();
        let result = registry.register(service2).await;
        assert!(matches!(result, Err(RegistryError::AlreadyExists(_))));
    }

    #[tokio::test]
    async fn test_unregister_service() {
        let registry = RegistryService::new();
        let service = ExtensionService::new("test", "1.0.0");

        let id = registry.register(service).await.unwrap();
        registry.unregister(&id).await.unwrap();
        assert!(registry.get(&id).await.is_none());
    }

    #[tokio::test]
    async fn test_get_by_name() {
        let registry = RegistryService::new();
        let service = ExtensionService::new("test", "1.0.0");

        registry.register(service).await.unwrap();

        let found = registry.get_by_name("test").await;
        assert!(found.is_some());
        assert_eq!(found.unwrap().version, "1.0.0");
    }

    #[tokio::test]
    async fn test_update_status() {
        let registry = RegistryService::new();
        let service = ExtensionService::new("test", "1.0.0");

        let id = registry.register(service).await.unwrap();
        registry
            .update_status(&id, ServiceStatus::Running)
            .await
            .unwrap();

        let service = registry.get(&id).await.unwrap();
        assert_eq!(service.status, ServiceStatus::Running);
    }

    #[tokio::test]
    async fn test_find_by_capability() {
        let registry = RegistryService::new();

        let service1 = ExtensionService::new("service1", "1.0.0")
            .capability(Capability::new("tools"));

        let service2 = ExtensionService::new("service2", "1.0.0")
            .capability(Capability::new("resources"));

        let service3 = ExtensionService::new("service3", "1.0.0")
            .capability(Capability::new("tools"));

        registry.register(service1).await.unwrap();
        registry.register(service2).await.unwrap();
        registry.register(service3).await.unwrap();

        let tools_services = registry.find_by_capability("tools").await;
        assert_eq!(tools_services.len(), 2);

        let resources_services = registry.find_by_capability("resources").await;
        assert_eq!(resources_services.len(), 1);

        let prompts_services = registry.find_by_capability("prompts").await;
        assert!(prompts_services.is_empty());
    }

    #[tokio::test]
    async fn test_registry_stats() {
        let registry = RegistryService::new();

        let mut service1 = ExtensionService::new("s1", "1.0.0");
        service1.set_status(ServiceStatus::Running);

        let mut service2 = ExtensionService::new("s2", "1.0.0");
        service2.set_status(ServiceStatus::Stopped);

        let mut service3 = ExtensionService::new("s3", "1.0.0");
        service3.set_status(ServiceStatus::Error);

        registry.register(service1).await.unwrap();
        registry.register(service2).await.unwrap();
        registry.register(service3).await.unwrap();

        let stats = registry.stats().await;
        assert_eq!(stats.total, 3);
        assert_eq!(stats.running, 1);
        assert_eq!(stats.stopped, 1);
        assert_eq!(stats.errors, 1);
    }

    #[tokio::test]
    async fn test_service_filter() {
        let registry = RegistryService::new();

        let mut service1 = ExtensionService::new("s1", "1.0.0")
            .capability(Capability::new("tools"));
        service1.set_status(ServiceStatus::Running);

        let mut service2 = ExtensionService::new("s2", "1.0.0")
            .capability(Capability::new("tools"));
        service2.set_status(ServiceStatus::Stopped);

        registry.register(service1).await.unwrap();
        registry.register(service2).await.unwrap();

        let filter = ServiceFilter::new().status(ServiceStatus::Running);
        let services = registry.list(&filter).await;
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].name, "s1");
    }
}