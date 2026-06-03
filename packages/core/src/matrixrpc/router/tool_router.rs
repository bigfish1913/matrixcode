//! Tool Router Implementation
//!
//! Routes tool execution requests to the appropriate extension service.
//! Maintains a mapping from tool names to service IDs for efficient routing.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use serde_json::Value as JsonValue;

use crate::matrixrpc::{
    ErrorCode, JsonRpcError, JsonRpcId, JsonRpcRequest, JsonRpcResponse,
    RegistryService, ServiceId, ServiceStatus,
};

/// Error type for tool routing operations
#[derive(Debug, thiserror::Error)]
pub enum ToolRouterError {
    /// Tool not found in any registered service
    #[error("Tool '{0}' not found in any registered service")]
    ToolNotFound(String),

    /// Service that provides the tool is not running
    #[error("Service '{service_id}' for tool '{tool_name}' is not running (status: {status:?})")]
    ServiceNotRunning {
        tool_name: String,
        service_id: ServiceId,
        status: ServiceStatus,
    },

    /// No services registered
    #[error("No services registered in the registry")]
    NoServicesRegistered,

    /// Routing failed
    #[error("Routing failed: {0}")]
    RoutingFailed(String),

    /// Invalid tool parameters
    #[error("Invalid parameters for tool '{tool}': {reason}")]
    InvalidParams { tool: String, reason: String },

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result of a tool routing operation
#[derive(Debug, Clone)]
pub struct ToolRouteResult {
    /// The service ID that should handle the tool call
    pub service_id: ServiceId,
    /// The tool name (may be different if aliased)
    pub tool_name: String,
    /// The parameters to pass to the tool
    pub params: JsonValue,
    /// The original request ID for correlation
    pub request_id: JsonRpcId,
}

/// Tool definition from an extension service
#[derive(Debug, Clone)]
pub struct ToolDefinition {
    /// Tool name
    pub name: String,
    /// Service ID that provides this tool
    pub service_id: ServiceId,
    /// Tool description
    pub description: Option<String>,
    /// Risk level (safe, moderate, dangerous)
    pub risk_level: Option<String>,
    /// Timeout in milliseconds
    pub timeout_ms: Option<u64>,
}

/// Tool Router
///
/// Routes tool execution requests to the appropriate extension service.
/// Uses the registry service to discover which services provide each tool.
#[derive(Debug)]
pub struct ToolRouter {
    /// Reference to the registry service
    registry: Arc<RegistryService>,
    /// Tool name to service ID mapping (cached)
    tool_index: Arc<RwLock<HashMap<String, ToolDefinition>>>,
    /// Default timeout for tool execution (milliseconds)
    default_timeout_ms: u64,
}

impl ToolRouter {
    /// Create a new tool router with a registry service
    pub fn new(registry: Arc<RegistryService>) -> Self {
        Self {
            registry,
            tool_index: Arc::new(RwLock::new(HashMap::new())),
            default_timeout_ms: 30_000, // 30 seconds default
        }
    }

    /// Set default timeout for tool execution
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.default_timeout_ms = timeout_ms;
        self
    }

    /// Register a tool from a service
    ///
    /// This updates the internal tool index for fast routing.
    pub async fn register_tool(&self, service_id: ServiceId, tool_def: ToolDefinition) {
        let mut index = self.tool_index.write().await;
        index.insert(tool_def.name.clone(), tool_def);
    }

    /// Unregister all tools from a service
    ///
    /// Called when a service is unregistered or disconnected.
    pub async fn unregister_service_tools(&self, service_id: &ServiceId) {
        let mut index = self.tool_index.write().await;
        index.retain(|_, def| def.service_id != *service_id);
    }

    /// Rebuild the tool index from the registry
    ///
    /// Useful after bulk registration changes.
    pub async fn rebuild_index(&self) -> Result<(), ToolRouterError> {
        let services = self.registry.list_all().await;
        let mut index = self.tool_index.write().await;
        index.clear();

        for service in services {
            if service.status != ServiceStatus::Running {
                continue;
            }

            // Extract tools from service capabilities
            for cap in &service.capabilities {
                if cap.name == "tools" {
                    // Parse tools from capability config
                    if let Some(tools_json) = cap.config.get("tools") {
                        if let Ok(tools) = serde_json::from_value::<Vec<JsonValue>>(tools_json.clone()) {
                            for tool in tools {
                                if let Some(name) = tool.get("name").and_then(|n| n.as_str()) {
                                    let def = ToolDefinition {
                                        name: name.to_string(),
                                        service_id: service.id.clone(),
                                        description: tool.get("description").and_then(|d| d.as_str()).map(|s| s.to_string()),
                                        risk_level: tool.get("risk_level").and_then(|r| r.as_str()).map(|s| s.to_string()),
                                        timeout_ms: tool.get("timeout_ms").and_then(|t| t.as_u64()),
                                    };
                                    index.insert(name.to_string(), def);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Route a tool execution request
    ///
    /// Given a tool name and parameters, find the appropriate service
    /// and return routing information.
    pub async fn route(
        &self,
        tool_name: &str,
        params: JsonValue,
        request_id: JsonRpcId,
    ) -> Result<ToolRouteResult, ToolRouterError> {
        // Look up the tool in the index
        let index = self.tool_index.read().await;
        let tool_def = index
            .get(tool_name)
            .cloned()
            .ok_or_else(|| ToolRouterError::ToolNotFound(tool_name.to_string()))?;

        // Check if the service is running
        let service = self.registry.get(&tool_def.service_id).await;
        match service {
            Some(s) if s.status == ServiceStatus::Running => {
                // Service is healthy, proceed with routing
                Ok(ToolRouteResult {
                    service_id: tool_def.service_id,
                    tool_name: tool_def.name,
                    params,
                    request_id,
                })
            }
            Some(s) => {
                // Service exists but not running
                Err(ToolRouterError::ServiceNotRunning {
                    tool_name: tool_name.to_string(),
                    service_id: tool_def.service_id,
                    status: s.status,
                })
            }
            None => {
                // Service not found in registry (shouldn't happen if index is valid)
                Err(ToolRouterError::ToolNotFound(tool_name.to_string()))
            }
        }
    }

    /// Check if a tool is available
    pub async fn has_tool(&self, tool_name: &str) -> bool {
        let index = self.tool_index.read().await;
        index.contains_key(tool_name)
    }

    /// List all available tools
    pub async fn list_tools(&self) -> Vec<ToolDefinition> {
        let index = self.tool_index.read().await;
        index.values().cloned().collect()
    }

    /// Get tool definition
    pub async fn get_tool(&self, tool_name: &str) -> Option<ToolDefinition> {
        let index = self.tool_index.read().await;
        index.get(tool_name).cloned()
    }

    /// Create a JSON-RPC request for tool execution
    ///
    /// Creates the proper request format for the extension service.
    pub fn create_tool_request(&self, route_result: ToolRouteResult) -> JsonRpcRequest {
        JsonRpcRequest::with_id("tool.execute", route_result.request_id)
            .params(serde_json::json!({
                "tool_name": route_result.tool_name,
                "params": route_result.params
            }))
    }

    /// Create an error response for routing failures
    pub async fn create_error_response(
        &self,
        error: ToolRouterError,
        request_id: JsonRpcId,
    ) -> JsonRpcResponse {
        let (code, message, data) = match error {
            ToolRouterError::ToolNotFound(tool) => {
                let index = self.tool_index.read().await;
                let available: Vec<String> = index.keys().cloned().collect();
                (
                    ErrorCode::RESOURCE_NOT_FOUND,
                    format!("Tool '{}' not found", tool),
                    Some(serde_json::json!({ "available_tools": available })),
                )
            }
            ToolRouterError::ServiceNotRunning { tool_name, service_id, status } => (
                ErrorCode::INVALID_STATE,
                format!("Service '{}' is not running", service_id),
                Some(serde_json::json!({
                    "tool_name": tool_name,
                    "service_id": service_id.to_string(),
                    "status": serde_json::to_string(&status).unwrap_or_default()
                })),
            ),
            ToolRouterError::NoServicesRegistered => (
                ErrorCode::RESOURCE_NOT_FOUND,
                "No services registered".to_string(),
                None,
            ),
            ToolRouterError::InvalidParams { tool, reason } => (
                ErrorCode::INVALID_PARAMS,
                format!("Invalid parameters for tool '{}'", tool),
                Some(serde_json::json!({ "reason": reason })),
            ),
            ToolRouterError::RoutingFailed(msg) | ToolRouterError::Internal(msg) => (
                ErrorCode::INTERNAL_ERROR,
                msg,
                None,
            ),
        };

        JsonRpcResponse::error(request_id, JsonRpcError::with_data(code, message, data.unwrap_or(JsonValue::Null)))
    }

    /// Get the default timeout for a tool
    pub async fn get_timeout(&self, tool_name: &str) -> u64 {
        let index = self.tool_index.read().await;
        index
            .get(tool_name)
            .and_then(|def| def.timeout_ms)
            .unwrap_or(self.default_timeout_ms)
    }

    /// Get the count of registered tools
    pub async fn tool_count(&self) -> usize {
        let index = self.tool_index.read().await;
        index.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matrixrpc::{Capability, ExtensionService};

    #[tokio::test]
    async fn test_tool_router_creation() {
        let registry = Arc::new(RegistryService::new());
        let router = ToolRouter::new(registry);
        assert_eq!(router.default_timeout_ms, 30_000);
    }

    #[tokio::test]
    async fn test_register_tool() {
        let registry = Arc::new(RegistryService::new());
        let router = ToolRouter::new(registry);

        let service_id = ServiceId::new("test-service");
        let tool_def = ToolDefinition {
            name: "test_tool".to_string(),
            service_id: service_id.clone(),
            description: Some("A test tool".to_string()),
            risk_level: Some("safe".to_string()),
            timeout_ms: Some(5000),
        };

        router.register_tool(service_id, tool_def).await;
        assert!(router.has_tool("test_tool").await);
    }

    #[tokio::test]
    async fn test_list_tools() {
        let registry = Arc::new(RegistryService::new());
        let router = ToolRouter::new(registry);

        let service_id = ServiceId::new("test-service");
        router.register_tool(service_id.clone(), ToolDefinition {
            name: "tool1".to_string(),
            service_id: service_id.clone(),
            description: None,
            risk_level: None,
            timeout_ms: None,
        }).await;

        router.register_tool(service_id.clone(), ToolDefinition {
            name: "tool2".to_string(),
            service_id: service_id.clone(),
            description: None,
            risk_level: None,
            timeout_ms: None,
        }).await;

        let tools = router.list_tools().await;
        assert_eq!(tools.len(), 2);
    }

    #[tokio::test]
    async fn test_route_tool_not_found() {
        let registry = Arc::new(RegistryService::new());
        let router = ToolRouter::new(registry);

        let result = router.route(
            "unknown_tool",
            serde_json::json!({}),
            JsonRpcId::Number(1),
        ).await;

        assert!(matches!(result, Err(ToolRouterError::ToolNotFound(_))));
    }

    #[tokio::test]
    async fn test_create_tool_request() {
        let registry = Arc::new(RegistryService::new());
        let router = ToolRouter::new(registry);

        let route_result = ToolRouteResult {
            service_id: ServiceId::new("test-service"),
            tool_name: "test_tool".to_string(),
            params: serde_json::json!({"arg": "value"}),
            request_id: JsonRpcId::Number(1),
        };

        let request = router.create_tool_request(route_result);
        assert_eq!(request.method, "tool.execute");
        assert!(request.params.is_some());
    }

    #[tokio::test]
    async fn test_unregister_service_tools() {
        let registry = Arc::new(RegistryService::new());
        let router = ToolRouter::new(registry);

        let service_id = ServiceId::new("test-service");
        router.register_tool(service_id.clone(), ToolDefinition {
            name: "tool1".to_string(),
            service_id: service_id.clone(),
            description: None,
            risk_level: None,
            timeout_ms: None,
        }).await;

        assert!(router.has_tool("tool1").await);
        router.unregister_service_tools(&service_id).await;
        assert!(!router.has_tool("tool1").await);
    }
}