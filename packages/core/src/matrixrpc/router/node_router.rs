//! Node Router Implementation
//!
//! Routes workflow node execution requests to the appropriate extension service.
//! Handles node discovery, capability checking, and callback endpoint setup.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use serde_json::Value as JsonValue;

use crate::matrixrpc::{
    ErrorCode, JsonRpcError, JsonRpcId, JsonRpcRequest, JsonRpcResponse,
    RegistryService, ServiceId, ServiceStatus,
};

/// Error type for node routing operations
#[derive(Debug, thiserror::Error)]
pub enum NodeRouterError {
    /// Node not found in any registered service
    #[error("Node '{0}' not found in any registered service")]
    NodeNotFound(String),

    /// Service that provides the node is not running
    #[error("Service '{service_id}' for node '{node_id}' is not running (status: {status:?})")]
    ServiceNotRunning {
        node_id: String,
        service_id: ServiceId,
        status: ServiceStatus,
    },

    /// No services registered
    #[error("No services registered in the registry")]
    NoServicesRegistered,

    /// Routing failed
    #[error("Routing failed: {0}")]
    RoutingFailed(String),

    /// Node does not support required capability
    #[error("Node '{node_id}' does not support capability '{capability}'")]
    CapabilityNotSupported { node_id: String, capability: String },

    /// Invalid node context
    #[error("Invalid context for node '{node_id}': {reason}")]
    InvalidContext { node_id: String, reason: String },

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result of a node routing operation
#[derive(Debug, Clone)]
pub struct NodeRouteResult {
    /// The service ID that should handle the node execution
    pub service_id: ServiceId,
    /// The node ID
    pub node_id: String,
    /// The execution context
    pub context: NodeContext,
    /// The original request ID for correlation
    pub request_id: JsonRpcId,
    /// Callback endpoint URL
    pub callback_endpoint: String,
    /// Timeout for node execution (milliseconds)
    pub timeout_ms: u64,
}

/// Node execution context
#[derive(Debug, Clone, Default)]
pub struct NodeContext {
    /// Input data for the node
    pub input_data: JsonValue,
    /// Workflow variables
    pub variables: HashMap<String, JsonValue>,
    /// Previous node results
    pub previous_results: HashMap<String, JsonValue>,
    /// Custom metadata
    pub metadata: HashMap<String, JsonValue>,
}

impl NodeContext {
    /// Create a new empty context
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a context with input data
    pub fn with_input(data: JsonValue) -> Self {
        Self {
            input_data: data,
            ..Default::default()
        }
    }

    /// Add a variable to the context
    pub fn variable(mut self, key: impl Into<String>, value: JsonValue) -> Self {
        self.variables.insert(key.into(), value);
        self
    }

    /// Add a previous result
    pub fn previous_result(mut self, node_id: impl Into<String>, result: JsonValue) -> Self {
        self.previous_results.insert(node_id.into(), result);
        self
    }

    /// Add metadata
    pub fn metadata(mut self, key: impl Into<String>, value: JsonValue) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Convert to JSON value
    pub fn to_json(&self) -> JsonValue {
        serde_json::json!({
            "input_data": self.input_data,
            "variables": self.variables,
            "previous_results": self.previous_results,
            "metadata": self.metadata
        })
    }
}

/// Node type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    /// Task node - executes a specific task
    Task,
    /// Condition node - evaluates conditions
    Condition,
    /// Validation node - validates data
    Validate,
    /// AI node - calls AI for processing
    Ai,
    /// Composite node - combines multiple operations
    Composite,
}

impl NodeType {
    /// Get the string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeType::Task => "task",
            NodeType::Condition => "condition",
            NodeType::Validate => "validate",
            NodeType::Ai => "ai",
            NodeType::Composite => "composite",
        }
    }

    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "task" => Some(NodeType::Task),
            "condition" => Some(NodeType::Condition),
            "validate" => Some(NodeType::Validate),
            "ai" => Some(NodeType::Ai),
            "composite" => Some(NodeType::Composite),
            _ => None,
        }
    }
}

/// Node capability flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeCapability {
    /// Can call AI
    AiExecution,
    /// Can execute tools
    ToolExecution,
    /// Can access workflow context
    ContextAccess,
}

impl NodeCapability {
    /// Get the string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeCapability::AiExecution => "ai_execution",
            NodeCapability::ToolExecution => "tool_execution",
            NodeCapability::ContextAccess => "context_access",
        }
    }

    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "ai_execution" => Some(NodeCapability::AiExecution),
            "tool_execution" => Some(NodeCapability::ToolExecution),
            "context_access" => Some(NodeCapability::ContextAccess),
            _ => None,
        }
    }
}

/// Node definition from an extension service
#[derive(Debug, Clone)]
pub struct NodeDefinition {
    /// Node ID
    pub id: String,
    /// Node name
    pub name: String,
    /// Service ID that provides this node
    pub service_id: ServiceId,
    /// Node type
    pub node_type: NodeType,
    /// Node description
    pub description: Option<String>,
    /// Supported capabilities
    pub capabilities: Vec<NodeCapability>,
    /// Timeout in milliseconds
    pub timeout_ms: Option<u64>,
    /// Parameter schema (JSON Schema)
    pub params_schema: Option<JsonValue>,
}

/// Node Router
///
/// Routes workflow node execution requests to the appropriate extension service.
/// Uses the registry service to discover which services provide each node.
#[derive(Debug)]
pub struct NodeRouter {
    /// Reference to the registry service
    registry: Arc<RegistryService>,
    /// Node ID to definition mapping (cached)
    node_index: Arc<RwLock<HashMap<String, NodeDefinition>>>,
    /// Default timeout for node execution (milliseconds)
    default_timeout_ms: u64,
    /// Default callback endpoint
    default_callback_endpoint: String,
}

impl NodeRouter {
    /// Create a new node router with a registry service
    pub fn new(registry: Arc<RegistryService>) -> Self {
        Self {
            registry,
            node_index: Arc::new(RwLock::new(HashMap::new())),
            default_timeout_ms: 60_000, // 60 seconds default for nodes
            default_callback_endpoint: "matrixcode://callback".to_string(),
        }
    }

    /// Set default timeout for node execution
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.default_timeout_ms = timeout_ms;
        self
    }

    /// Set default callback endpoint
    pub fn with_callback_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.default_callback_endpoint = endpoint.into();
        self
    }

    /// Register a node from a service
    pub async fn register_node(&self, _service_id: ServiceId, node_def: NodeDefinition) {
        let mut index = self.node_index.write().await;
        index.insert(node_def.id.clone(), node_def);
    }

    /// Unregister all nodes from a service
    pub async fn unregister_service_nodes(&self, service_id: &ServiceId) {
        let mut index = self.node_index.write().await;
        index.retain(|_, def| def.service_id != *service_id);
    }

    /// Rebuild the node index from the registry
    pub async fn rebuild_index(&self) -> Result<(), NodeRouterError> {
        let services = self.registry.list_all().await;
        let mut index = self.node_index.write().await;
        index.clear();

        for service in services {
            if service.status != ServiceStatus::Running {
                continue;
            }

            // Extract nodes from service capabilities
            for cap in &service.capabilities {
                if cap.name == "nodes" {
                    // Parse nodes from capability config
                    if let Some(nodes_json) = cap.config.get("nodes") {
                        if let Ok(nodes) = serde_json::from_value::<Vec<JsonValue>>(nodes_json.clone()) {
                            for node in nodes {
                                if let Some(id) = node.get("id").and_then(|n| n.as_str()) {
                                    let node_type = node
                                        .get("type")
                                        .and_then(|t| t.as_str())
                                        .and_then(NodeType::from_str)
                                        .unwrap_or(NodeType::Task);

                                    let capabilities: Vec<NodeCapability> = node
                                        .get("capabilities")
                                        .and_then(|c| c.as_array())
                                        .map(|arr| {
                                            arr.iter()
                                                .filter_map(|c| c.as_str().and_then(NodeCapability::from_str))
                                                .collect()
                                        })
                                        .unwrap_or_default();

                                    let def = NodeDefinition {
                                        id: id.to_string(),
                                        name: node.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()).unwrap_or_else(|| id.to_string()),
                                        service_id: service.id.clone(),
                                        node_type,
                                        description: node.get("description").and_then(|d| d.as_str()).map(|s| s.to_string()),
                                        capabilities,
                                        timeout_ms: node.get("timeout_ms").and_then(|t| t.as_u64()),
                                        params_schema: node.get("params_schema").cloned(),
                                    };
                                    index.insert(id.to_string(), def);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Route a node execution request
    pub async fn route(
        &self,
        node_id: &str,
        context: NodeContext,
        request_id: JsonRpcId,
        required_capabilities: Vec<NodeCapability>,
    ) -> Result<NodeRouteResult, NodeRouterError> {
        // Look up the node in the index
        let index = self.node_index.read().await;
        let node_def = index
            .get(node_id)
            .cloned()
            .ok_or_else(|| NodeRouterError::NodeNotFound(node_id.to_string()))?;

        // Check required capabilities
        for cap in required_capabilities {
            if !node_def.capabilities.contains(&cap) {
                return Err(NodeRouterError::CapabilityNotSupported {
                    node_id: node_id.to_string(),
                    capability: cap.as_str().to_string(),
                });
            }
        }

        // Check if the service is running
        let service = self.registry.get(&node_def.service_id).await;
        match service {
            Some(s) if s.status == ServiceStatus::Running => {
                // Service is healthy, proceed with routing
                let timeout = node_def.timeout_ms.unwrap_or(self.default_timeout_ms);
                Ok(NodeRouteResult {
                    service_id: node_def.service_id,
                    node_id: node_def.id,
                    context,
                    request_id,
                    callback_endpoint: self.default_callback_endpoint.clone(),
                    timeout_ms: timeout,
                })
            }
            Some(s) => {
                // Service exists but not running
                Err(NodeRouterError::ServiceNotRunning {
                    node_id: node_id.to_string(),
                    service_id: node_def.service_id,
                    status: s.status,
                })
            }
            None => {
                // Service not found in registry
                Err(NodeRouterError::NodeNotFound(node_id.to_string()))
            }
        }
    }

    /// Check if a node is available
    pub async fn has_node(&self, node_id: &str) -> bool {
        let index = self.node_index.read().await;
        index.contains_key(node_id)
    }

    /// List all available nodes
    pub async fn list_nodes(&self) -> Vec<NodeDefinition> {
        let index = self.node_index.read().await;
        index.values().cloned().collect()
    }

    /// Get node definition
    pub async fn get_node(&self, node_id: &str) -> Option<NodeDefinition> {
        let index = self.node_index.read().await;
        index.get(node_id).cloned()
    }

    /// Get nodes by type
    pub async fn get_nodes_by_type(&self, node_type: NodeType) -> Vec<NodeDefinition> {
        let index = self.node_index.read().await;
        index
            .values()
            .filter(|def| def.node_type == node_type)
            .cloned()
            .collect()
    }

    /// Get nodes by capability
    pub async fn get_nodes_by_capability(&self, capability: NodeCapability) -> Vec<NodeDefinition> {
        let index = self.node_index.read().await;
        index
            .values()
            .filter(|def| def.capabilities.contains(&capability))
            .cloned()
            .collect()
    }

    /// Create a JSON-RPC request for node execution
    pub fn create_node_request(&self, route_result: NodeRouteResult) -> JsonRpcRequest {
        JsonRpcRequest::with_id("node.execute", route_result.request_id)
            .params(serde_json::json!({
                "node_id": route_result.node_id,
                "context": route_result.context.to_json(),
                "callback_endpoint": route_result.callback_endpoint
            }))
    }

    /// Create an error response for routing failures
    pub fn create_error_response(
        &self,
        error: NodeRouterError,
        request_id: JsonRpcId,
    ) -> JsonRpcResponse {
        let (code, message, data) = match error {
            NodeRouterError::NodeNotFound(node) => {
                let available: Vec<String> = {
                    // We can't read async here, so use empty list for simplicity
                    Vec::new()
                };
                (
                    ErrorCode::RESOURCE_NOT_FOUND,
                    format!("Node '{}' not found", node),
                    Some(serde_json::json!({ "available_nodes": available })),
                )
            }
            NodeRouterError::ServiceNotRunning { node_id, service_id, status } => (
                ErrorCode::INVALID_STATE,
                format!("Service '{}' is not running", service_id),
                Some(serde_json::json!({
                    "node_id": node_id,
                    "service_id": service_id.to_string(),
                    "status": serde_json::to_string(&status).unwrap_or_default()
                })),
            ),
            NodeRouterError::CapabilityNotSupported { node_id, capability } => (
                ErrorCode::CAPABILITY_NOT_SUPPORTED,
                format!("Node '{}' does not support capability '{}'", node_id, capability),
                None,
            ),
            NodeRouterError::NoServicesRegistered => (
                ErrorCode::RESOURCE_NOT_FOUND,
                "No services registered".to_string(),
                None,
            ),
            NodeRouterError::InvalidContext { node_id, reason } => (
                ErrorCode::INVALID_PARAMS,
                format!("Invalid context for node '{}'", node_id),
                Some(serde_json::json!({ "reason": reason })),
            ),
            NodeRouterError::RoutingFailed(msg) | NodeRouterError::Internal(msg) => (
                ErrorCode::INTERNAL_ERROR,
                msg,
                None,
            ),
        };

        JsonRpcResponse::error(
            request_id,
            JsonRpcError::with_data(code, message, data.unwrap_or(JsonValue::Null)),
        )
    }

    /// Get the count of registered nodes
    pub async fn node_count(&self) -> usize {
        let index = self.node_index.read().await;
        index.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_node_router_creation() {
        let registry = Arc::new(RegistryService::new());
        let router = NodeRouter::new(registry);
        assert_eq!(router.default_timeout_ms, 60_000);
    }

    #[tokio::test]
    async fn test_register_node() {
        let registry = Arc::new(RegistryService::new());
        let router = NodeRouter::new(registry);

        let service_id = ServiceId::new("test-service");
        let node_def = NodeDefinition {
            id: "validate-node".to_string(),
            name: "Validate Node".to_string(),
            service_id: service_id.clone(),
            node_type: NodeType::Validate,
            description: Some("Validates input data".to_string()),
            capabilities: vec![NodeCapability::ContextAccess],
            timeout_ms: Some(10_000),
            params_schema: None,
        };

        router.register_node(service_id, node_def).await;
        assert!(router.has_node("validate-node").await);
    }

    #[tokio::test]
    async fn test_node_types() {
        assert_eq!(NodeType::Task.as_str(), "task");
        assert_eq!(NodeType::from_str("condition"), Some(NodeType::Condition));
        assert_eq!(NodeType::from_str("unknown"), None);
    }

    #[tokio::test]
    async fn test_node_capabilities() {
        assert_eq!(NodeCapability::AiExecution.as_str(), "ai_execution");
        assert_eq!(
            NodeCapability::from_str("tool_execution"),
            Some(NodeCapability::ToolExecution)
        );
    }

    #[tokio::test]
    async fn test_node_context() {
        let context = NodeContext::new()
            .variable("key", serde_json::json!("value"))
            .previous_result("prev-node", serde_json::json!({"result": "ok"}));

        let json = context.to_json();
        assert!(json.get("variables").is_some());
        assert!(json.get("previous_results").is_some());
    }

    #[tokio::test]
    async fn test_create_node_request() {
        let registry = Arc::new(RegistryService::new());
        let router = NodeRouter::new(registry);

        let route_result = NodeRouteResult {
            service_id: ServiceId::new("test-service"),
            node_id: "test-node".to_string(),
            context: NodeContext::new(),
            request_id: JsonRpcId::Number(1),
            callback_endpoint: "matrixcode://callback".to_string(),
            timeout_ms: 30_000,
        };

        let request = router.create_node_request(route_result);
        assert_eq!(request.method, "node.execute");
        assert!(request.params.is_some());
    }

    #[tokio::test]
    async fn test_route_node_not_found() {
        let registry = Arc::new(RegistryService::new());
        let router = NodeRouter::new(registry);

        let result = router.route(
            "unknown-node",
            NodeContext::new(),
            JsonRpcId::Number(1),
            vec![],
        ).await;

        assert!(matches!(result, Err(NodeRouterError::NodeNotFound(_))));
    }

    #[tokio::test]
    async fn test_list_nodes() {
        let registry = Arc::new(RegistryService::new());
        let router = NodeRouter::new(registry);

        let service_id = ServiceId::new("test-service");
        router.register_node(service_id.clone(), NodeDefinition {
            id: "node1".to_string(),
            name: "Node 1".to_string(),
            service_id: service_id.clone(),
            node_type: NodeType::Task,
            description: None,
            capabilities: vec![],
            timeout_ms: None,
            params_schema: None,
        }).await;

        router.register_node(service_id.clone(), NodeDefinition {
            id: "node2".to_string(),
            name: "Node 2".to_string(),
            service_id: service_id.clone(),
            node_type: NodeType::Condition,
            description: None,
            capabilities: vec![NodeCapability::AiExecution],
            timeout_ms: None,
            params_schema: None,
        }).await;

        let nodes = router.list_nodes().await;
        assert_eq!(nodes.len(), 2);

        let task_nodes = router.get_nodes_by_type(NodeType::Task).await;
        assert_eq!(task_nodes.len(), 1);

        let ai_nodes = router.get_nodes_by_capability(NodeCapability::AiExecution).await;
        assert_eq!(ai_nodes.len(), 1);
    }
}