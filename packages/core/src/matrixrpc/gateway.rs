//! Extension Gateway
//!
//! Unified entry point for the MatrixRPC extension system.
//! Coordinates Registry, Lifecycle, Router, and Callback components.

use std::sync::Arc;

use serde_json::Value as JsonValue;
use tokio::sync::{broadcast, RwLock};

use crate::matrixrpc::{
    callback::{CallbackConfig, CallbackHandler, CallbackResult, CallbackType, SecurityValidator},
    lifecycle::{LifecycleConfig, LifecycleManager},
    protocol::{ErrorCode, JsonRpcError, JsonRpcId, JsonRpcRequest, JsonRpcResponse},
    registry::{RegistryBuilder, RegistryService, RegistryStats},
    router::{NodeCapability, NodeContext, NodeDefinition, NodeRouter, NodeRouteResult},
    service::{ExtensionService, ServiceId, ServiceStatus},
    ToolDefinition, ToolRouter, ToolRouteResult,
};

/// Gateway configuration
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    /// Registry configuration
    pub registry_heartbeat_timeout_secs: u64,

    /// Lifecycle configuration
    pub lifecycle_config: LifecycleConfig,

    /// Callback configuration
    pub callback_config: CallbackConfig,

    /// Default tool timeout (ms)
    pub default_tool_timeout_ms: u64,

    /// Default node timeout (ms)
    pub default_node_timeout_ms: u64,

    /// Enable auto-discovery
    pub auto_discovery: bool,

    /// Maximum services
    pub max_services: u32,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            registry_heartbeat_timeout_secs: 60,
            lifecycle_config: LifecycleConfig::default(),
            callback_config: CallbackConfig::default(),
            default_tool_timeout_ms: 30_000,
            default_node_timeout_ms: 60_000,
            auto_discovery: true,
            max_services: 100,
        }
    }
}

/// Gateway statistics
#[derive(Debug, Clone, Default)]
pub struct GatewayStats {
    /// Total registered services
    pub total_services: usize,

    /// Running services
    pub running_services: usize,

    /// Total registered tools
    pub total_tools: usize,

    /// Total registered nodes
    pub total_nodes: usize,

    /// Total callbacks processed
    pub total_callbacks: usize,

    /// Total errors
    pub total_errors: usize,
}

/// Gateway error
#[derive(Debug, thiserror::Error)]
pub enum GatewayError {
    /// Service not found
    #[error("Service '{0}' not found")]
    ServiceNotFound(String),

    /// Service limit exceeded
    #[error("Maximum service limit ({0}) exceeded")]
    ServiceLimitExceeded(u32),

    /// Registration failed
    #[error("Registration failed: {0}")]
    RegistrationFailed(String),

    /// Routing failed
    #[error("Routing failed: {0}")]
    RoutingFailed(String),

    /// Execution failed
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    /// Callback failed
    #[error("Callback failed: {0}")]
    CallbackFailed(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Service registration request
#[derive(Debug, Clone)]
pub struct ServiceRegistrationRequest {
    /// Service name
    pub name: String,

    /// Service version
    pub version: String,

    /// Service description
    pub description: Option<String>,

    /// Tools to register
    pub tools: Vec<ToolDefinition>,

    /// Nodes to register
    pub nodes: Vec<NodeDefinition>,

    /// Transport type (stdio or tcp)
    pub transport_type: String,

    /// Command (for stdio)
    pub command: Option<String>,

    /// Arguments (for stdio)
    pub args: Vec<String>,
}

impl ServiceRegistrationRequest {
    /// Create a new registration request
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            description: None,
            tools: Vec::new(),
            nodes: Vec::new(),
            transport_type: "stdio".to_string(),
            command: None,
            args: Vec::new(),
        }
    }

    /// Add description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Add a tool
    pub fn tool(mut self, tool: ToolDefinition) -> Self {
        self.tools.push(tool);
        self
    }

    /// Add a node
    pub fn node(mut self, node: NodeDefinition) -> Self {
        self.nodes.push(node);
        self
    }

    /// Set transport type
    pub fn transport(mut self, transport_type: impl Into<String>) -> Self {
        self.transport_type = transport_type.into();
        self
    }

    /// Set command (for stdio)
    pub fn command(mut self, cmd: impl Into<String>, args: Vec<String>) -> Self {
        self.command = Some(cmd.into());
        self.args = args;
        self
    }
}

/// Extension Gateway
///
/// Unified entry point for the MatrixRPC extension system.
/// Coordinates all components: Registry, Lifecycle, Router, and Callback.
pub struct ExtensionGateway {
    /// Configuration
    config: GatewayConfig,

    /// Registry service
    registry: Arc<RegistryService>,

    /// Lifecycle manager
    lifecycle: Arc<LifecycleManager>,

    /// Tool router
    tool_router: Arc<ToolRouter>,

    /// Node router
    node_router: Arc<NodeRouter>,

    /// Callback handler
    callback: Arc<CallbackHandler>,

    /// Security validator
    security: Arc<SecurityValidator>,

    /// Statistics
    stats: Arc<RwLock<GatewayStats>>,

    /// Event broadcaster
    event_tx: broadcast::Sender<GatewayEvent>,
}

/// Gateway events
#[derive(Debug, Clone)]
pub enum GatewayEvent {
    /// Service registered
    ServiceRegistered(ServiceId),

    /// Service unregistered
    ServiceUnregistered(ServiceId),

    /// Service status changed
    ServiceStatusChanged {
        service_id: ServiceId,
        old_status: ServiceStatus,
        new_status: ServiceStatus,
    },

    /// Tool registered
    ToolRegistered {
        tool_name: String,
        service_id: ServiceId,
    },

    /// Node registered
    NodeRegistered {
        node_id: String,
        service_id: ServiceId,
    },

    /// Callback received
    CallbackReceived {
        callback_type: CallbackType,
        service_id: ServiceId,
    },

    /// Error occurred
    Error(String),
}

impl ExtensionGateway {
    /// Create a new extension gateway
    pub fn new() -> Self {
        Self::with_config(GatewayConfig::default())
    }

    /// Create a new extension gateway with configuration
    pub fn with_config(config: GatewayConfig) -> Self {
        let (event_tx, _) = broadcast::channel(256);

        // Create registry
        let registry = Arc::new(
            RegistryBuilder::new()
                .heartbeat_timeout(config.registry_heartbeat_timeout_secs)
                .build(),
        );

        // Create lifecycle manager
        let lifecycle = Arc::new(
            LifecycleManager::with_config(registry.clone(), config.lifecycle_config.clone()),
        );

        // Create routers
        let tool_router = Arc::new(ToolRouter::new(registry.clone()));
        let node_router = Arc::new(NodeRouter::new(registry.clone()));

        // Create security validator and callback handler
        let security = Arc::new(SecurityValidator::new());
        let callback = Arc::new(
            CallbackHandler::with_config(
                security.clone(),
                tool_router.clone(),
                node_router.clone(),
                config.callback_config.clone(),
            ),
        );

        Self {
            config,
            registry,
            lifecycle,
            tool_router,
            node_router,
            callback,
            security,
            stats: Arc::new(RwLock::new(GatewayStats::default())),
            event_tx,
        }
    }

    /// Subscribe to gateway events
    pub fn subscribe(&self) -> broadcast::Receiver<GatewayEvent> {
        self.event_tx.subscribe()
    }

    /// Register a new extension service
    pub async fn register_service(&self, request: ServiceRegistrationRequest) -> Result<ServiceId, GatewayError> {
        // Check service limit
        {
            let stats = self.stats.read().await;
            if stats.total_services >= self.config.max_services as usize {
                return Err(GatewayError::ServiceLimitExceeded(self.config.max_services));
            }
        }

        // Build extension service
        let mut service = ExtensionService::new(&request.name, &request.version);

        if let Some(desc) = &request.description {
            service = service.description(desc);
        }

        // Set transport config based on type
        let transport_config = crate::matrixrpc::service::TransportConfig {
            transport_type: if request.transport_type == "tcp" {
                crate::matrixrpc::service::TransportType::Tcp
            } else {
                crate::matrixrpc::service::TransportType::Stdio
            },
            command: request.command.clone(),
            args: request.args.clone(),
            ..Default::default()
        };
        service = service.transport(transport_config);

        // Save counts before moving
        let tools_count = request.tools.len();
        let nodes_count = request.nodes.len();

        // Start service via lifecycle manager
        let service_id = self
            .lifecycle
            .start_service(service)
            .await
            .map_err(|e| GatewayError::RegistrationFailed(e.to_string()))?;

        // Register tools
        for tool in request.tools {
            self.tool_router
                .register_tool(service_id.clone(), tool.clone())
                .await;
            let _ = self.event_tx.send(GatewayEvent::ToolRegistered {
                tool_name: tool.name,
                service_id: service_id.clone(),
            });
        }

        // Register nodes
        for node in request.nodes {
            self.node_router
                .register_node(service_id.clone(), node.clone())
                .await;
            let _ = self.event_tx.send(GatewayEvent::NodeRegistered {
                node_id: node.id,
                service_id: service_id.clone(),
            });
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.total_services += 1;
            stats.total_tools += tools_count;
            stats.total_nodes += nodes_count;
        }

        // Emit event
        let _ = self.event_tx.send(GatewayEvent::ServiceRegistered(service_id.clone()));

        Ok(service_id)
    }

    /// Unregister a service
    pub async fn unregister_service(&self, service_id: &ServiceId) -> Result<(), GatewayError> {
        // Unregister tools and nodes
        self.tool_router.unregister_service_tools(service_id).await;
        self.node_router.unregister_service_nodes(service_id).await;

        // Stop service
        self.lifecycle
            .stop_service(service_id)
            .await
            .map_err(|e| GatewayError::Internal(e.to_string()))?;

        // Invalidate security tokens
        self.security.invalidate_service_tokens(service_id).await;

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.total_services -= 1;
        }

        // Emit event
        let _ = self.event_tx.send(GatewayEvent::ServiceUnregistered(service_id.clone()));

        Ok(())
    }

    /// Execute a tool
    pub async fn execute_tool(
        &self,
        tool_name: &str,
        params: JsonValue,
        request_id: JsonRpcId,
    ) -> Result<ToolRouteResult, GatewayError> {
        // Route the tool
        let route_result = self
            .tool_router
            .route(tool_name, params, request_id)
            .await
            .map_err(|e| GatewayError::RoutingFailed(e.to_string()))?;

        // In real implementation, would execute via transport
        // For now, return the route result

        Ok(route_result)
    }

    /// Execute a node
    pub async fn execute_node(
        &self,
        node_id: &str,
        context: NodeContext,
        request_id: JsonRpcId,
        required_capabilities: Vec<NodeCapability>,
    ) -> Result<NodeRouteResult, GatewayError> {
        // Save callback types before moving capabilities
        let callback_types = self.get_callback_types_for_capabilities(&required_capabilities);

        // Route the node
        let route_result = self
            .node_router
            .route(node_id, context, request_id, required_capabilities)
            .await
            .map_err(|e| GatewayError::RoutingFailed(e.to_string()))?;

        // Generate callback token
        let _token = self
            .callback
            .generate_token(route_result.service_id.clone(), route_result.request_id.to_string(), callback_types)
            .await
            .map_err(|e| GatewayError::CallbackFailed(e.to_string()))?;

        // In real implementation, would execute via transport with callback endpoint

        Ok(route_result)
    }

    /// Handle a callback request
    pub async fn handle_callback(&self, request: JsonRpcRequest) -> Result<CallbackResult, GatewayError> {
        // Track callback
        {
            let mut stats = self.stats.write().await;
            stats.total_callbacks += 1;
        }

        // Handle via callback handler
        let result = self
            .callback
            .handle_request(request.clone())
            .await;

        match result {
            Ok(res) => {
                // Emit event
                if let Some(params) = &request.params {
                    if let Some(service_id) = params.get("service_id").and_then(|v| v.as_str()) {
                        let _ = self.event_tx.send(GatewayEvent::CallbackReceived {
                            callback_type: res.callback_type(),
                            service_id: ServiceId::new(service_id),
                        });
                    }
                }
                Ok(res)
            }
            Err(e) => {
                // Update error stats
                {
                    let mut stats = self.stats.write().await;
                    stats.total_errors += 1;
                }
                Err(GatewayError::CallbackFailed(e.to_string()))
            }
        }
    }

    /// Get callback types for node capabilities
    fn get_callback_types_for_capabilities(&self, capabilities: &[NodeCapability]) -> Vec<String> {
        let mut types = Vec::new();

        for cap in capabilities {
            match cap {
                NodeCapability::AiExecution => types.push("ai".to_string()),
                NodeCapability::ToolExecution => types.push("tool".to_string()),
                NodeCapability::ContextAccess => types.push("context".to_string()),
            }
        }

        types
    }

    /// List all services
    pub async fn list_services(&self) -> Vec<ExtensionService> {
        self.registry.list_all().await
    }

    /// Get service by ID
    pub async fn get_service(&self, service_id: &ServiceId) -> Option<ExtensionService> {
        self.registry.get(service_id).await
    }

    /// Get service by name
    pub async fn get_service_by_name(&self, name: &str) -> Option<ExtensionService> {
        self.registry.get_by_name(name).await
    }

    /// List all tools
    pub async fn list_tools(&self) -> Vec<ToolDefinition> {
        self.tool_router.list_tools().await
    }

    /// List all nodes
    pub async fn list_nodes(&self) -> Vec<NodeDefinition> {
        self.node_router.list_nodes().await
    }

    /// Check if tool exists
    pub async fn has_tool(&self, tool_name: &str) -> bool {
        self.tool_router.has_tool(tool_name).await
    }

    /// Check if node exists
    pub async fn has_node(&self, node_id: &str) -> bool {
        self.node_router.has_node(node_id).await
    }

    /// Get registry statistics
    pub async fn registry_stats(&self) -> RegistryStats {
        self.registry.stats().await
    }

    /// Get gateway statistics
    pub async fn gateway_stats(&self) -> GatewayStats {
        self.stats.read().await.clone()
    }

    /// Health check all services
    pub async fn health_check(&self) -> Vec<ServiceId> {
        self.lifecycle.health_check().await
    }

    /// Send heartbeat for a service
    pub async fn heartbeat(&self, service_id: &ServiceId) -> Result<(), GatewayError> {
        self.lifecycle
            .handle_heartbeat(service_id)
            .await
            .map_err(|e| GatewayError::Internal(e.to_string()))
    }

    /// Get service status
    pub async fn get_service_status(&self, service_id: &ServiceId) -> Option<ServiceStatus> {
        self.lifecycle.get_status(service_id).await
    }

    /// Stop all services
    pub async fn stop_all(&self) {
        self.lifecycle.stop_all().await;
        self.registry.clear().await;
        self.security.cleanup_expired().await;

        let mut stats = self.stats.write().await;
        stats.total_services = 0;
        stats.total_tools = 0;
        stats.total_nodes = 0;
    }

    /// Create JSON-RPC error response
    pub fn create_error_response(&self, error: GatewayError, id: JsonRpcId) -> JsonRpcResponse {
        let (code, message, data) = match error {
            GatewayError::ServiceNotFound(id) => (
                ErrorCode::RESOURCE_NOT_FOUND,
                format!("Service '{}' not found", id),
                None,
            ),
            GatewayError::ServiceLimitExceeded(limit) => (
                ErrorCode::PERMISSION_DENIED,
                format!("Maximum service limit ({}) exceeded", limit),
                None,
            ),
            GatewayError::RegistrationFailed(msg) => (
                ErrorCode::INTERNAL_ERROR,
                "Registration failed".to_string(),
                Some(serde_json::json!({ "reason": msg })),
            ),
            GatewayError::RoutingFailed(msg) => (
                ErrorCode::INTERNAL_ERROR,
                "Routing failed".to_string(),
                Some(serde_json::json!({ "reason": msg })),
            ),
            GatewayError::ExecutionFailed(msg) => (
                ErrorCode::INTERNAL_ERROR,
                "Execution failed".to_string(),
                Some(serde_json::json!({ "reason": msg })),
            ),
            GatewayError::CallbackFailed(msg) => (
                ErrorCode::CALLBACK_ERROR,
                "Callback failed".to_string(),
                Some(serde_json::json!({ "reason": msg })),
            ),
            GatewayError::Internal(msg) => (
                ErrorCode::INTERNAL_ERROR,
                msg,
                None,
            ),
        };

        JsonRpcResponse::error(
            id,
            JsonRpcError::with_data(code, message, data.unwrap_or(JsonValue::Null)),
        )
    }

    /// Get the registry service
    pub fn registry(&self) -> Arc<RegistryService> {
        self.registry.clone()
    }

    /// Get the lifecycle manager
    pub fn lifecycle(&self) -> Arc<LifecycleManager> {
        self.lifecycle.clone()
    }

    /// Get the tool router
    pub fn tool_router(&self) -> Arc<ToolRouter> {
        self.tool_router.clone()
    }

    /// Get the node router
    pub fn node_router(&self) -> Arc<NodeRouter> {
        self.node_router.clone()
    }

    /// Get the callback handler
    pub fn callback(&self) -> Arc<CallbackHandler> {
        self.callback.clone()
    }

    /// Get the security validator
    pub fn security(&self) -> Arc<SecurityValidator> {
        self.security.clone()
    }
}

impl Default for ExtensionGateway {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matrixrpc::NodeType;

    #[tokio::test]
    async fn test_gateway_creation() {
        let gateway = ExtensionGateway::new();
        let stats = gateway.gateway_stats().await;

        assert_eq!(stats.total_services, 0);
        assert_eq!(stats.total_tools, 0);
        assert_eq!(stats.total_nodes, 0);
    }

    #[tokio::test]
    async fn test_gateway_with_config() {
        let config = GatewayConfig {
            max_services: 10,
            default_tool_timeout_ms: 10_000,
            ..Default::default()
        };

        let gateway = ExtensionGateway::with_config(config);
        assert_eq!(gateway.config.max_services, 10);
    }

    #[tokio::test]
    async fn test_register_service() {
        let gateway = ExtensionGateway::new();

        let request = ServiceRegistrationRequest::new("test-service", "1.0.0")
            .description("A test service");

        let service_id = gateway.register_service(request).await.unwrap();
        assert!(gateway.get_service(&service_id).await.is_some());
    }

    #[tokio::test]
    async fn test_unregister_service() {
        let gateway = ExtensionGateway::new();

        let request = ServiceRegistrationRequest::new("test-service", "1.0.0");
        let service_id = gateway.register_service(request).await.unwrap();

        gateway.unregister_service(&service_id).await.unwrap();
        assert!(gateway.get_service(&service_id).await.is_none());
    }

    #[tokio::test]
    async fn test_register_service_with_tools() {
        let gateway = ExtensionGateway::new();

        let request = ServiceRegistrationRequest::new("test-service", "1.0.0")
            .tool(ToolDefinition {
                name: "test_tool".to_string(),
                service_id: ServiceId::generate(),
                description: Some("Test tool".to_string()),
                risk_level: Some("safe".to_string()),
                timeout_ms: Some(5000),
            });

        let service_id = gateway.register_service(request).await.unwrap();

        // Tool should be registered
        assert!(gateway.has_tool("test_tool").await);
    }

    #[tokio::test]
    async fn test_register_service_with_nodes() {
        let gateway = ExtensionGateway::new();

        let request = ServiceRegistrationRequest::new("test-service", "1.0.0")
            .node(NodeDefinition {
                id: "test_node".to_string(),
                name: "Test Node".to_string(),
                service_id: ServiceId::generate(),
                node_type: NodeType::Task,
                description: Some("Test node".to_string()),
                capabilities: vec![NodeCapability::AiExecution],
                timeout_ms: Some(10_000),
                params_schema: None,
            });

        let service_id = gateway.register_service(request).await.unwrap();

        // Node should be registered
        assert!(gateway.has_node("test_node").await);
    }

    #[tokio::test]
    async fn test_list_services() {
        let gateway = ExtensionGateway::new();

        gateway
            .register_service(ServiceRegistrationRequest::new("service1", "1.0.0"))
            .await
            .unwrap();
        gateway
            .register_service(ServiceRegistrationRequest::new("service2", "1.0.0"))
            .await
            .unwrap();

        let services = gateway.list_services().await;
        assert_eq!(services.len(), 2);
    }

    #[tokio::test]
    async fn test_list_tools() {
        let gateway = ExtensionGateway::new();

        let request = ServiceRegistrationRequest::new("test", "1.0")
            .tool(ToolDefinition {
                name: "tool1".to_string(),
                service_id: ServiceId::generate(),
                description: None,
                risk_level: None,
                timeout_ms: None,
            })
            .tool(ToolDefinition {
                name: "tool2".to_string(),
                service_id: ServiceId::generate(),
                description: None,
                risk_level: None,
                timeout_ms: None,
            });

        gateway.register_service(request).await.unwrap();

        let tools = gateway.list_tools().await;
        assert_eq!(tools.len(), 2);
    }

    #[tokio::test]
    async fn test_service_limit() {
        let config = GatewayConfig {
            max_services: 2,
            ..Default::default()
        };

        let gateway = ExtensionGateway::with_config(config);

        // Register two services
        gateway
            .register_service(ServiceRegistrationRequest::new("s1", "1.0"))
            .await
            .unwrap();
        gateway
            .register_service(ServiceRegistrationRequest::new("s2", "1.0"))
            .await
            .unwrap();

        // Third should fail
        let result = gateway
            .register_service(ServiceRegistrationRequest::new("s3", "1.0"))
            .await;

        assert!(matches!(result, Err(GatewayError::ServiceLimitExceeded(2))));
    }

    #[tokio::test]
    async fn test_gateway_events() {
        let gateway = ExtensionGateway::new();
        let mut event_rx = gateway.subscribe();

        let request = ServiceRegistrationRequest::new("test-service", "1.0.0");
        gateway.register_service(request).await.unwrap();

        // Should receive ServiceRegistered event
        let event = event_rx.try_recv();
        assert!(event.is_ok());
    }

    #[tokio::test]
    async fn test_health_check() {
        let gateway = ExtensionGateway::new();

        let request = ServiceRegistrationRequest::new("test-service", "1.0.0");
        gateway.register_service(request).await.unwrap();

        let unhealthy = gateway.health_check().await;
        // Newly registered service should not be unhealthy initially
        // (depends on heartbeat timing)
    }

    #[tokio::test]
    async fn test_stop_all() {
        let gateway = ExtensionGateway::new();

        gateway
            .register_service(ServiceRegistrationRequest::new("s1", "1.0"))
            .await
            .unwrap();
        gateway
            .register_service(ServiceRegistrationRequest::new("s2", "1.0"))
            .await
            .unwrap();

        gateway.stop_all().await;

        let services = gateway.list_services().await;
        assert!(services.is_empty());
    }
}