//! Tool Callback Handler
//!
//! Handles tool callback requests from external services.
//! Enables external nodes to call MatrixCode's registered tools.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use super::security::SecurityValidator;
use crate::matrixrpc::{
    ErrorCode, JsonRpcError, JsonRpcId, JsonRpcResponse, ServiceId, ToolRouter, ToolRouteResult,
};

/// Tool callback request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallbackRequest {
    /// Request ID from original node execution
    pub request_id: String,

    /// Service ID making the callback
    pub service_id: ServiceId,

    /// Security token
    pub token: String,

    /// Tool name to execute
    pub tool_name: String,

    /// Tool parameters
    #[serde(default)]
    pub params: JsonValue,

    /// Timeout in milliseconds
    #[serde(default = "default_tool_timeout")]
    pub timeout_ms: u64,

    /// Whether to require approval
    #[serde(default)]
    pub require_approval: bool,
}

fn default_tool_timeout() -> u64 {
    30_000 // 30 seconds
}

/// Tool callback result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallbackResult {
    /// Tool name that was executed
    pub tool_name: String,

    /// Result data
    pub result: JsonValue,

    /// Execution status
    pub status: String,

    /// Duration in milliseconds
    pub duration_ms: u64,

    /// Whether approval was required
    pub approval_required: bool,

    /// Additional metadata
    #[serde(default)]
    pub metadata: JsonValue,
}

/// Tool callback error
#[derive(Debug, thiserror::Error)]
pub enum ToolCallbackError {
    /// Security validation failed
    #[error("Security validation failed: {0}")]
    SecurityFailed(String),

    /// Tool not found
    #[error("Tool '{0}' not found")]
    ToolNotFound(String),

    /// Tool execution failed
    #[error("Tool '{tool}' execution failed: {reason}")]
    ExecutionFailed { tool: String, reason: String },

    /// Invalid parameters
    #[error("Invalid parameters for tool '{tool}': {reason}")]
    InvalidParams { tool: String, reason: String },

    /// Timeout
    #[error("Tool '{0}' timed out after {1}ms")]
    Timeout(String, u64),

    /// Tool not allowed
    #[error("Tool '{0}' is not allowed for callback")]
    ToolNotAllowed(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Allowed tools for callback
#[derive(Debug, Clone)]
pub struct AllowedToolsConfig {
    /// Tools that are always allowed
    pub always_allowed: Vec<String>,

    /// Tools that require approval
    pub requires_approval: Vec<String>,

    /// Tools that are never allowed for callback
    pub never_allowed: Vec<String>,

    /// Allow all tools (dangerous)
    pub allow_all: bool,
}

impl Default for AllowedToolsConfig {
    fn default() -> Self {
        Self {
            // Read-only tools are always allowed
            always_allowed: vec![
                "read".to_string(), "grep".to_string(), "glob".to_string(),
                "codegraph_search".to_string(), "codegraph_node".to_string(),
                "codegraph_context".to_string(), "codegraph_callers".to_string(),
                "codegraph_callees".to_string(),
            ],
            // Write tools require approval
            requires_approval: vec![
                "write".to_string(), "edit".to_string(), "bash".to_string(),
                "tool_search".to_string(),
            ],
            // Dangerous tools are never allowed
            never_allowed: vec![
                "delete".to_string(), "rm".to_string(), "format".to_string(),
                "sudo".to_string(),
            ],
            allow_all: false,
        }
    }
}

/// Tool Callback Handler
///
/// Handles tool callback requests from external extension services.
pub struct ToolCallbackHandler {
    /// Security validator
    security: Arc<SecurityValidator>,

    /// Tool router for routing tool calls
    tool_router: Arc<ToolRouter>,

    /// Allowed tools configuration
    allowed_tools: AllowedToolsConfig,

    /// Default timeout
    default_timeout_ms: u64,
}

impl ToolCallbackHandler {
    /// Create a new tool callback handler
    pub fn new(security: Arc<SecurityValidator>, tool_router: Arc<ToolRouter>) -> Self {
        Self {
            security,
            tool_router,
            allowed_tools: AllowedToolsConfig::default(),
            default_timeout_ms: 30_000,
        }
    }

    /// Set allowed tools configuration
    pub fn with_allowed_tools(mut self, config: AllowedToolsConfig) -> Self {
        self.allowed_tools = config;
        self
    }

    /// Set default timeout
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.default_timeout_ms = timeout_ms;
        self
    }

    /// Handle a tool callback request
    pub async fn handle(&self, request: ToolCallbackRequest) -> Result<ToolCallbackResult, ToolCallbackError> {
        // Validate security
        let validation = self
            .security
            .validate(&request.token, &request.service_id, &request.request_id, "tool")
            .await;

        if !validation.is_valid {
            return Err(ToolCallbackError::SecurityFailed(
                validation.error.unwrap_or_else(|| "Unknown security error".to_string()),
            ));
        }

        // Check if tool is allowed
        let (approval_required, allowed) = self.check_tool_allowed(&request.tool_name);

        if !allowed {
            return Err(ToolCallbackError::ToolNotAllowed(request.tool_name));
        }

        // Route the tool
        let route_result = self
            .tool_router
            .route(
                &request.tool_name,
                request.params.clone(),
                JsonRpcId::generate(),
            )
            .await
            .map_err(|e| match e {
                crate::matrixrpc::ToolRouterError::ToolNotFound(tool) => {
                    ToolCallbackError::ToolNotFound(tool)
                }
                _ => ToolCallbackError::Internal(e.to_string()),
            })?;

        // Build result
        // Note: In real implementation, this would execute the actual tool
        // Here we provide a mock implementation
        let result = ToolCallbackResult {
            tool_name: request.tool_name.clone(),
            result: serde_json::json!({
                "status": "success",
                "message": format!("Tool '{}' executed successfully", request.tool_name),
            }),
            status: "success".to_string(),
            duration_ms: 100,
            approval_required,
            metadata: serde_json::json!({
                "request_id": request.request_id,
                "service_id": request.service_id.to_string(),
                "route": {
                    "service_id": route_result.service_id.to_string(),
                },
            }),
        };

        Ok(result)
    }

    /// Check if a tool is allowed for callback
    fn check_tool_allowed(&self, tool_name: &str) -> (bool, bool) {
        // Check if never allowed
        if self.allowed_tools.never_allowed.contains(&tool_name.to_string()) {
            return (false, false);
        }

        // Check if allow all
        if self.allowed_tools.allow_all {
            return (false, true);
        }

        // Check if always allowed
        if self.allowed_tools.always_allowed.contains(&tool_name.to_string()) {
            return (false, true);
        }

        // Check if requires approval
        if self.allowed_tools.requires_approval.contains(&tool_name.to_string()) {
            return (true, true);
        }

        // Default: not allowed
        (false, false)
    }

    /// Create a JSON-RPC error response for tool callback failures
    pub fn create_error_response(&self, error: ToolCallbackError, id: JsonRpcId) -> JsonRpcResponse {
        let (code, message, data) = match error {
            ToolCallbackError::SecurityFailed(msg) => (
                ErrorCode::PERMISSION_DENIED,
                "Security validation failed".to_string(),
                Some(serde_json::json!({ "reason": msg })),
            ),
            ToolCallbackError::ToolNotFound(tool) => (
                ErrorCode::RESOURCE_NOT_FOUND,
                format!("Tool '{}' not found", tool),
                None,
            ),
            ToolCallbackError::ExecutionFailed { tool, reason } => (
                ErrorCode::INTERNAL_ERROR,
                "Tool execution failed".to_string(),
                Some(serde_json::json!({ "tool": tool, "reason": reason })),
            ),
            ToolCallbackError::InvalidParams { tool, reason } => (
                ErrorCode::INVALID_PARAMS,
                "Invalid tool parameters".to_string(),
                Some(serde_json::json!({ "tool": tool, "reason": reason })),
            ),
            ToolCallbackError::Timeout(tool, ms) => (
                ErrorCode::TIMEOUT_ERROR,
                "Tool timed out".to_string(),
                Some(serde_json::json!({ "tool": tool, "timeout_ms": ms })),
            ),
            ToolCallbackError::ToolNotAllowed(tool) => (
                ErrorCode::PERMISSION_DENIED,
                format!("Tool '{}' is not allowed for callback", tool),
                None,
            ),
            ToolCallbackError::Internal(msg) => (
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

    /// List allowed tools
    pub fn list_allowed_tools(&self) -> Vec<String> {
        let mut tools = self.allowed_tools.always_allowed.clone();
        tools.extend(self.allowed_tools.requires_approval.clone());
        tools
    }

    /// Check if tool exists in router
    pub async fn tool_exists(&self, tool_name: &str) -> bool {
        self.tool_router.has_tool(tool_name).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matrixrpc::RegistryService;

    #[tokio::test]
    async fn test_tool_callback_handler_creation() {
        let security = Arc::new(SecurityValidator::new());
        let registry = Arc::new(RegistryService::new());
        let tool_router = Arc::new(ToolRouter::new(registry));
        let handler = ToolCallbackHandler::new(security, tool_router);

        assert_eq!(handler.default_timeout_ms, 30_000);
    }

    #[test]
    fn test_allowed_tools_config_default() {
        let config = AllowedToolsConfig::default();

        assert!(config.always_allowed.contains(&"read".to_string()));
        assert!(config.requires_approval.contains(&"write".to_string()));
        assert!(config.never_allowed.contains(&"delete".to_string()));
        assert!(!config.allow_all);
    }

    #[test]
    fn test_check_tool_allowed() {
        let security = Arc::new(SecurityValidator::new());
        let registry = Arc::new(RegistryService::new());
        let tool_router = Arc::new(ToolRouter::new(registry));
        let handler = ToolCallbackHandler::new(security, tool_router);

        // Always allowed
        let (approval, allowed) = handler.check_tool_allowed("read");
        assert!(!approval);
        assert!(allowed);

        // Requires approval
        let (approval, allowed) = handler.check_tool_allowed("write");
        assert!(approval);
        assert!(allowed);

        // Never allowed
        let (approval, allowed) = handler.check_tool_allowed("delete");
        assert!(!approval);
        assert!(!allowed);

        // Unknown tool
        let (approval, allowed) = handler.check_tool_allowed("unknown");
        assert!(!approval);
        assert!(!allowed);
    }

    #[tokio::test]
    async fn test_tool_callback_security_validation() {
        let security = Arc::new(SecurityValidator::new());
        let registry = Arc::new(RegistryService::new());
        let tool_router = Arc::new(ToolRouter::new(registry));

        // Register a tool
        tool_router
            .register_tool(
                ServiceId::new("test-service"),
                crate::matrixrpc::ToolDefinition {
                    name: "read".to_string(),
                    service_id: ServiceId::new("test-service"),
                    description: None,
                    risk_level: None,
                    timeout_ms: None,
                },
            )
            .await;

        let handler = ToolCallbackHandler::new(security.clone(), tool_router);

        // Generate token
        let service_id = ServiceId::new("callback-service");
        let request_id = "req-001".to_string();
        let token = security
            .generate_token(service_id.clone(), request_id.clone(), vec!["tool".to_string()])
            .await
            .unwrap();

        let request = ToolCallbackRequest {
            request_id,
            service_id,
            token,
            tool_name: "read".to_string(),
            params: serde_json::json!({}),
            timeout_ms: 30_000,
            require_approval: false,
        };

        let result = handler.handle(request).await;
        // Tool exists and is allowed
        assert!(result.is_ok() || matches!(result, Err(ToolCallbackError::ToolNotFound(_))));
    }

    #[tokio::test]
    async fn test_tool_callback_invalid_token() {
        let security = Arc::new(SecurityValidator::new());
        let registry = Arc::new(RegistryService::new());
        let tool_router = Arc::new(ToolRouter::new(registry));
        let handler = ToolCallbackHandler::new(security, tool_router);

        let request = ToolCallbackRequest {
            request_id: "req-001".to_string(),
            service_id: ServiceId::new("test-service"),
            token: "invalid_token".to_string(),
            tool_name: "read".to_string(),
            params: serde_json::json!({}),
            timeout_ms: 30_000,
            require_approval: false,
        };

        let result = handler.handle(request).await;
        assert!(matches!(result, Err(ToolCallbackError::SecurityFailed(_))));
    }

    #[test]
    fn test_list_allowed_tools() {
        let security = Arc::new(SecurityValidator::new());
        let registry = Arc::new(RegistryService::new());
        let tool_router = Arc::new(ToolRouter::new(registry));
        let handler = ToolCallbackHandler::new(security, tool_router);

        let tools = handler.list_allowed_tools();
        assert!(tools.contains(&"read".to_string()));
        assert!(tools.contains(&"write".to_string()));
    }
}