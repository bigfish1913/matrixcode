//! MatrixRPC Extension Service Data Models
//!
//! Defines the core data structures for extension services including
//! service metadata, capabilities, and registration info.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

/// Unique identifier for an extension service
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ServiceId(pub String);

impl ServiceId {
    /// Create a new service ID
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Generate a new unique service ID
    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl std::fmt::Display for ServiceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for ServiceId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for ServiceId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Extension service status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceStatus {
    /// Service is starting up
    Starting,
    /// Service is running and healthy
    Running,
    /// Service is stopping
    Stopping,
    /// Service has stopped
    Stopped,
    /// Service encountered an error
    Error,
    /// Service is reconnecting
    Reconnecting,
}

impl Default for ServiceStatus {
    fn default() -> Self {
        Self::Starting
    }
}

/// Extension service metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionService {
    /// Unique service identifier
    pub id: ServiceId,
    /// Human-readable service name
    pub name: String,
    /// Service version (semver)
    pub version: String,
    /// Service description
    #[serde(default)]
    pub description: String,
    /// Supported capabilities
    pub capabilities: Vec<Capability>,
    /// Transport configuration
    pub transport: TransportConfig,
    /// Custom metadata
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
    /// Current status
    #[serde(default)]
    pub status: ServiceStatus,
    /// Time of last heartbeat
    #[serde(skip)]
    pub last_heartbeat: Option<Instant>,
    /// Connection retry count
    #[serde(default)]
    pub retry_count: u32,
}

impl ExtensionService {
    /// Create a new extension service
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            id: ServiceId::generate(),
            name: name.into(),
            version: version.into(),
            description: String::new(),
            capabilities: Vec::new(),
            transport: TransportConfig::default(),
            metadata: HashMap::new(),
            status: ServiceStatus::Starting,
            last_heartbeat: None,
            retry_count: 0,
        }
    }

    /// Set service description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Add a capability
    pub fn capability(mut self, cap: Capability) -> Self {
        self.capabilities.push(cap);
        self
    }

    /// Set transport configuration
    pub fn transport(mut self, transport: TransportConfig) -> Self {
        self.transport = transport;
        self
    }

    /// Add custom metadata
    pub fn metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Check if service has a specific capability
    pub fn has_capability(&self, name: &str) -> bool {
        self.capabilities.iter().any(|c| c.name == name)
    }

    /// Get capability by name
    pub fn get_capability(&self, name: &str) -> Option<&Capability> {
        self.capabilities.iter().find(|c| c.name == name)
    }

    /// Update service status
    pub fn set_status(&mut self, status: ServiceStatus) {
        self.status = status;
    }

    /// Record a heartbeat
    pub fn heartbeat(&mut self) {
        self.last_heartbeat = Some(Instant::now());
        self.retry_count = 0;
    }

    /// Check if service is healthy (received heartbeat recently)
    pub fn is_healthy(&self, timeout_secs: u64) -> bool {
        match self.last_heartbeat {
            Some(last) => {
                last.elapsed().as_secs() < timeout_secs
                    && self.status == ServiceStatus::Running
            }
            None => false,
        }
    }
}

/// Capability provided by an extension service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    /// Capability name (e.g., "tools", "resources", "prompts")
    pub name: String,
    /// Capability version
    #[serde(default)]
    pub version: String,
    /// Capability-specific configuration
    #[serde(default)]
    pub config: HashMap<String, serde_json::Value>,
}

impl Capability {
    /// Create a new capability
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: String::new(),
            config: HashMap::new(),
        }
    }

    /// Set capability version
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    /// Add configuration
    pub fn config(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.config.insert(key.into(), value);
        self
    }
}

/// Transport configuration for extension services
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    /// Transport type
    #[serde(rename = "type")]
    pub transport_type: TransportType,
    /// Connection address (for TCP/UDP)
    #[serde(default)]
    pub address: Option<String>,
    /// Port number (for TCP/UDP)
    #[serde(default)]
    pub port: Option<u16>,
    /// Command to execute (for stdio)
    #[serde(default)]
    pub command: Option<String>,
    /// Command arguments
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Working directory
    #[serde(default)]
    pub cwd: Option<String>,
    /// Connection timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    /// Enable auto-reconnect
    #[serde(default = "default_true")]
    pub auto_reconnect: bool,
    /// Maximum retry attempts
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Heartbeat interval in seconds
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval_secs: u64,
}

fn default_timeout() -> u64 {
    30
}

fn default_true() -> bool {
    true
}

fn default_max_retries() -> u32 {
    3
}

fn default_heartbeat_interval() -> u64 {
    30
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            transport_type: TransportType::Stdio,
            address: None,
            port: None,
            command: None,
            args: Vec::new(),
            env: HashMap::new(),
            cwd: None,
            timeout_secs: default_timeout(),
            auto_reconnect: true,
            max_retries: default_max_retries(),
            heartbeat_interval_secs: default_heartbeat_interval(),
        }
    }
}

/// Transport type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransportType {
    /// Standard input/output transport
    Stdio,
    /// TCP socket transport
    Tcp,
    /// Unix domain socket transport
    #[cfg(unix)]
    Unix,
    /// WebSocket transport
    WebSocket,
}

/// Service registration info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationInfo {
    /// Service being registered
    pub service: ExtensionService,
    /// Registration timestamp
    pub registered_at: chrono::DateTime<chrono::Utc>,
    /// Last update timestamp
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl RegistrationInfo {
    /// Create a new registration
    pub fn new(service: ExtensionService) -> Self {
        let now = chrono::Utc::now();
        Self {
            service,
            registered_at: now,
            updated_at: now,
        }
    }

    /// Update the timestamp
    pub fn touch(&mut self) {
        self.updated_at = chrono::Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_id() {
        let id = ServiceId::new("test-service");
        assert_eq!(id.to_string(), "test-service");

        let generated = ServiceId::generate();
        assert!(!generated.0.is_empty());
    }

    #[test]
    fn test_extension_service_creation() {
        let service = ExtensionService::new("test-service", "1.0.0")
            .description("A test service")
            .capability(Capability::new("tools"));

        assert_eq!(service.name, "test-service");
        assert_eq!(service.version, "1.0.0");
        assert_eq!(service.description, "A test service");
        assert!(service.has_capability("tools"));
        assert!(!service.has_capability("resources"));
    }

    #[test]
    fn test_service_status() {
        let mut service = ExtensionService::new("test", "1.0.0");
        assert_eq!(service.status, ServiceStatus::Starting);

        service.set_status(ServiceStatus::Running);
        assert_eq!(service.status, ServiceStatus::Running);
    }

    #[test]
    fn test_service_heartbeat() {
        let mut service = ExtensionService::new("test", "1.0.0");
        assert!(!service.is_healthy(30));

        service.set_status(ServiceStatus::Running);
        service.heartbeat();
        assert!(service.is_healthy(30));
    }

    #[test]
    fn test_capability() {
        let cap = Capability::new("tools")
            .version("1.0")
            .config("max_items".to_string(), serde_json::json!(100));

        assert_eq!(cap.name, "tools");
        assert_eq!(cap.version, "1.0");
        assert_eq!(cap.config.get("max_items"), Some(&serde_json::json!(100)));
    }

    #[test]
    fn test_transport_config_defaults() {
        let config = TransportConfig::default();
        assert_eq!(config.transport_type, TransportType::Stdio);
        assert!(config.auto_reconnect);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.heartbeat_interval_secs, 30);
    }

    #[test]
    fn test_registration_info() {
        let service = ExtensionService::new("test", "1.0.0");
        let reg = RegistrationInfo::new(service);

        assert!(reg.registered_at <= chrono::Utc::now());
        assert!(reg.updated_at <= chrono::Utc::now());
    }
}