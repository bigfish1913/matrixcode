//! MatrixRPC Configuration
//!
//! Handles loading and parsing of matrixrpc.toml configuration files.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

use super::service::{ExtensionService, TransportConfig, TransportType};

/// Configuration file name
pub const CONFIG_FILE_NAME: &str = "matrixrpc.toml";

/// Configuration error
#[derive(Debug, Error)]
pub enum ConfigError {
    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// TOML parse error
    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),

    /// Validation error
    #[error("Configuration validation error: {0}")]
    Validation(String),

    /// Service not found
    #[error("Service '{0}' not found in configuration")]
    ServiceNotFound(String),
}

/// Root configuration structure for matrixrpc.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixRpcConfig {
    /// Global settings
    #[serde(default)]
    pub global: GlobalConfig,

    /// Service definitions
    #[serde(default)]
    pub services: HashMap<String, ServiceDefinition>,
}

impl MatrixRpcConfig {
    /// Create an empty configuration
    pub fn new() -> Self {
        Self {
            global: GlobalConfig::default(),
            services: HashMap::new(),
        }
    }

    /// Load configuration from a file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path.as_ref())?;
        let config: Self = toml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }

    /// Load configuration from the default location
    pub fn load_default() -> Result<Self, ConfigError> {
        // Try in order:
        // 1. ./matrixrpc.toml
        // 2. ./.matrix/matrixrpc.toml
        // 3. ~/.matrix/matrixrpc.toml
        let candidates = vec![
            PathBuf::from(CONFIG_FILE_NAME),
            PathBuf::from(".matrix").join(CONFIG_FILE_NAME),
            dirs::config_dir()
                .map(|p| p.join("matrix").join(CONFIG_FILE_NAME))
                .unwrap_or_default(),
        ];

        for path in candidates {
            if path.exists() {
                return Self::load(&path);
            }
        }

        // Return empty config if no file found
        Ok(Self::new())
    }

    /// Save configuration to a file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), ConfigError> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| ConfigError::Validation(e.to_string()))?;
        std::fs::write(path.as_ref(), content)?;
        Ok(())
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        for (name, service) in &self.services {
            if service.command.is_none() && service.address.is_none() {
                return Err(ConfigError::Validation(format!(
                    "Service '{}' must have either 'command' or 'address' configured",
                    name
                )));
            }
        }
        Ok(())
    }

    /// Get a service definition by name
    pub fn get_service(&self, name: &str) -> Option<&ServiceDefinition> {
        self.services.get(name)
    }

    /// Add a service definition
    pub fn add_service(&mut self, name: impl Into<String>, service: ServiceDefinition) {
        self.services.insert(name.into(), service);
    }

    /// Convert service definition to ExtensionService
    pub fn create_service(&self, name: &str) -> Result<ExtensionService, ConfigError> {
        let def = self
            .get_service(name)
            .ok_or_else(|| ConfigError::ServiceNotFound(name.to_string()))?;

        let transport = def.to_transport_config();

        let mut service = ExtensionService::new(name, &def.version);
        service = service.description(&def.description);
        service = service.transport(transport);

        for cap in &def.capabilities {
            service = service.capability(super::service::Capability::new(cap));
        }

        Ok(service)
    }

    /// List all service names
    pub fn service_names(&self) -> Vec<&String> {
        self.services.keys().collect()
    }
}

impl Default for MatrixRpcConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Global configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// Default connection timeout in seconds
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,

    /// Default heartbeat interval in seconds
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval_secs: u64,

    /// Maximum retry attempts for connections
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Enable debug logging
    #[serde(default)]
    pub debug: bool,

    /// Log level (trace, debug, info, warn, error)
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

fn default_timeout_secs() -> u64 {
    30
}

fn default_heartbeat_interval() -> u64 {
    30
}

fn default_max_retries() -> u32 {
    3
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_true() -> bool {
    true
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            timeout_secs: default_timeout_secs(),
            heartbeat_interval_secs: default_heartbeat_interval(),
            max_retries: default_max_retries(),
            debug: false,
            log_level: default_log_level(),
        }
    }
}

/// Service definition in configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServiceDefinition {
    /// Service version
    #[serde(default)]
    pub version: String,

    /// Service description
    #[serde(default)]
    pub description: String,

    /// Transport type
    #[serde(rename = "type", default)]
    pub transport_type: ServiceTransportType,

    /// Command to execute (for stdio transport)
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

    /// Network address (for TCP/WebSocket transport)
    #[serde(default)]
    pub address: Option<String>,

    /// Port number (for TCP transport)
    #[serde(default)]
    pub port: Option<u16>,

    /// Connection timeout in seconds
    #[serde(default)]
    pub timeout_secs: Option<u64>,

    /// Enable auto-reconnect
    #[serde(default = "default_true")]
    pub auto_reconnect: bool,

    /// Maximum retry attempts
    #[serde(default)]
    pub max_retries: Option<u32>,

    /// Heartbeat interval in seconds
    #[serde(default)]
    pub heartbeat_interval_secs: Option<u64>,

    /// Capabilities provided by this service
    #[serde(default)]
    pub capabilities: Vec<String>,

    /// Service-specific configuration
    #[serde(default)]
    pub config: HashMap<String, serde_json::Value>,
}

impl ServiceDefinition {
    /// Create a new stdio service definition
    pub fn stdio(command: impl Into<String>) -> Self {
        Self {
            version: String::new(),
            description: String::new(),
            transport_type: ServiceTransportType::Stdio,
            command: Some(command.into()),
            args: Vec::new(),
            env: HashMap::new(),
            cwd: None,
            address: None,
            port: None,
            timeout_secs: None,
            auto_reconnect: true,
            max_retries: None,
            heartbeat_interval_secs: None,
            capabilities: Vec::new(),
            config: HashMap::new(),
        }
    }

    /// Create a new TCP service definition
    pub fn tcp(address: impl Into<String>, port: u16) -> Self {
        Self {
            version: String::new(),
            description: String::new(),
            transport_type: ServiceTransportType::Tcp,
            command: None,
            args: Vec::new(),
            env: HashMap::new(),
            cwd: None,
            address: Some(address.into()),
            port: Some(port),
            timeout_secs: None,
            auto_reconnect: true,
            max_retries: None,
            heartbeat_interval_secs: None,
            capabilities: Vec::new(),
            config: HashMap::new(),
        }
    }

    /// Add a command argument
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Add an environment variable
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Set version
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    /// Set description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Add a capability
    pub fn capability(mut self, cap: impl Into<String>) -> Self {
        self.capabilities.push(cap.into());
        self
    }

    /// Convert to TransportConfig
    pub fn to_transport_config(&self) -> TransportConfig {
        TransportConfig {
            transport_type: self.transport_type.into(),
            address: self.address.clone(),
            port: self.port,
            command: self.command.clone(),
            args: self.args.clone(),
            env: self.env.clone(),
            cwd: self.cwd.clone(),
            timeout_secs: self.timeout_secs.unwrap_or(default_timeout_secs()),
            auto_reconnect: self.auto_reconnect,
            max_retries: self.max_retries.unwrap_or(default_max_retries()),
            heartbeat_interval_secs: self.heartbeat_interval_secs
                .unwrap_or(default_heartbeat_interval()),
        }
    }
}

/// Service transport type (for TOML serialization)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceTransportType {
    /// Stdio transport
    Stdio,
    /// TCP transport
    Tcp,
    /// WebSocket transport
    WebSocket,
    /// Unix socket transport (Unix only)
    #[cfg(unix)]
    Unix,
}

impl Default for ServiceTransportType {
    fn default() -> Self {
        Self::Stdio
    }
}

impl From<ServiceTransportType> for TransportType {
    fn from(value: ServiceTransportType) -> Self {
        match value {
            ServiceTransportType::Stdio => TransportType::Stdio,
            ServiceTransportType::Tcp => TransportType::Tcp,
            ServiceTransportType::WebSocket => TransportType::WebSocket,
            #[cfg(unix)]
            ServiceTransportType::Unix => TransportType::Unix,
        }
    }
}

impl From<TransportType> for ServiceTransportType {
    fn from(value: TransportType) -> Self {
        match value {
            TransportType::Stdio => ServiceTransportType::Stdio,
            TransportType::Tcp => ServiceTransportType::Tcp,
            TransportType::WebSocket => ServiceTransportType::WebSocket,
            #[cfg(unix)]
            TransportType::Unix => ServiceTransportType::Unix,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = MatrixRpcConfig::new();
        assert!(config.services.is_empty());
        assert_eq!(config.global.timeout_secs, 30);
    }

    #[test]
    fn test_service_definition_stdio() {
        let def = ServiceDefinition::stdio("my-server")
            .arg("--port")
            .arg("8080")
            .env("DEBUG", "1")
            .version("1.0.0");

        assert_eq!(def.command, Some("my-server".to_string()));
        assert_eq!(def.args, vec!["--port", "8080"]);
        assert_eq!(def.env.get("DEBUG"), Some(&"1".to_string()));
        assert_eq!(def.version, "1.0.0");
    }

    #[test]
    fn test_service_definition_tcp() {
        let def = ServiceDefinition::tcp("localhost", 8080)
            .version("2.0.0")
            .capability("tools");

        assert_eq!(def.address, Some("localhost".to_string()));
        assert_eq!(def.port, Some(8080));
        assert_eq!(def.version, "2.0.0");
        assert!(def.capabilities.contains(&"tools".to_string()));
    }

    #[test]
    fn test_parse_toml() {
        let toml = r#"
[global]
timeout_secs = 60
debug = true

[services.my-server]
version = "1.0.0"
description = "My test server"
type = "stdio"
command = "my-server"
args = ["--verbose"]

[services.my-server.env]
DEBUG = "1"

[services.tcp-server]
type = "tcp"
address = "127.0.0.1"
port = 9000
"#;

        let config: MatrixRpcConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.global.timeout_secs, 60);
        assert!(config.global.debug);
        assert!(config.services.contains_key("my-server"));
        assert!(config.services.contains_key("tcp-server"));

        let my_server = &config.services["my-server"];
        assert_eq!(my_server.version, "1.0.0");
        assert_eq!(my_server.command, Some("my-server".to_string()));
        assert_eq!(my_server.env.get("DEBUG"), Some(&"1".to_string()));

        let tcp_server = &config.services["tcp-server"];
        assert_eq!(tcp_server.address, Some("127.0.0.1".to_string()));
        assert_eq!(tcp_server.port, Some(9000));
    }

    #[test]
    fn test_config_validation() {
        let mut config = MatrixRpcConfig::new();

        // Valid stdio service
        config.add_service("valid", ServiceDefinition::stdio("server"));
        assert!(config.validate().is_ok());

        // Invalid service (no command or address)
        config.services.insert(
            "invalid".to_string(),
            ServiceDefinition {
                command: None,
                address: None,
                ..Default::default()
            },
        );
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_create_service() {
        let mut config = MatrixRpcConfig::new();
        config.add_service(
            "test",
            ServiceDefinition::stdio("test-server")
                .version("1.0.0")
                .capability("tools"),
        );

        let service = config.create_service("test").unwrap();
        assert_eq!(service.name, "test");
        assert_eq!(service.version, "1.0.0");
        assert!(service.has_capability("tools"));
    }
}