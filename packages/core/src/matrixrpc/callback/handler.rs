//! Callback Handler
//!
//! Central handler for all callback types (AI, tool, context).
//! Coordinates callback requests from external extension services.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use super::ai::{AiCallbackHandler, AiCallbackRequest, AiCallbackError};
use super::tool::{ToolCallbackHandler, ToolCallbackRequest, ToolCallbackError};
use super::context::{ContextCallbackHandler, ContextCallbackRequest, ContextCallbackError};
use super::security::{SecurityValidator, ValidationResult};
use crate::matrixrpc::{
    ErrorCode, JsonRpcError, JsonRpcId, JsonRpcRequest, JsonRpcResponse, ServiceId,
    ToolRouter, NodeRouter,
};

/// Callback type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CallbackType {
    /// AI callback - call AI provider
    Ai,
    /// Tool callback - execute a tool
    Tool,
    /// Context callback - access workflow context
    Context,
}

impl CallbackType {
    /// Get the method name for this callback type
    pub fn method_name(&self) -> &'static str {
        match self {
            CallbackType::Ai => "callback.ai",
            CallbackType::Tool => "callback.tool",
            CallbackType::Context => "callback.context",
        }
    }

    /// Parse from method name
    pub fn from_method(method: &str) -> Option<Self> {
        match method {
            "callback.ai" => Some(CallbackType::Ai),
            "callback.tool" => Some(CallbackType::Tool),
            "callback.context" => Some(CallbackType::Context),
            _ => None,
        }
    }
}

/// Unified callback result
#[derive(Debug, Clone)]
pub enum CallbackResult {
    /// AI callback result
    Ai(super::ai::AiCallbackResult),
    /// Tool callback result
    Tool(super::tool::ToolCallbackResult),
    /// Context callback result
    Context(super::context::ContextCallbackResult),
}

impl CallbackResult {
    /// Convert to JSON value
    pub fn to_json(&self) -> JsonValue {
        match self {
            CallbackResult::Ai(result) => serde_json::to_value(result).unwrap_or(JsonValue::Null),
            CallbackResult::Tool(result) => serde_json::to_value(result).unwrap_or(JsonValue::Null),
            CallbackResult::Context(result) => serde_json::to_value(result).unwrap_or(JsonValue::Null),
        }
    }

    /// Get callback type
    pub fn callback_type(&self) -> CallbackType {
        match self {
            CallbackResult::Ai(_) => CallbackType::Ai,
            CallbackResult::Tool(_) => CallbackType::Tool,
            CallbackResult::Context(_) => CallbackType::Context,
        }
    }
}

/// Callback error
#[derive(Debug, thiserror::Error)]
pub enum CallbackError {
    /// Security validation failed
    #[error("Security validation failed: {0}")]
    SecurityFailed(String),

    /// Invalid callback type
    #[error("Invalid callback type: {0}")]
    InvalidType(String),

    /// AI callback error
    #[error("AI callback error: {0}")]
    Ai(#[from] AiCallbackError),

    /// Tool callback error
    #[error("Tool callback error: {0}")]
    Tool(#[from] ToolCallbackError),

    /// Context callback error
    #[error("Context callback error: {0}")]
    Context(#[from] ContextCallbackError),

    /// Missing required field
    #[error("Missing required field: {0}")]
    MissingField(String),

    /// Invalid JSON-RPC request
    #[error("Invalid JSON-RPC request: {0}")]
    InvalidRequest(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Callback configuration
#[derive(Debug, Clone)]
pub struct CallbackConfig {
    /// Enable AI callbacks
    pub enable_ai: bool,

    /// Enable tool callbacks
    pub enable_tool: bool,

    /// Enable context callbacks
    pub enable_context: bool,

    /// Default timeout for all callbacks (ms)
    pub default_timeout_ms: u64,

    /// Maximum concurrent callbacks
    pub max_concurrent: u32,

    /// Enable detailed logging
    pub detailed_logging: bool,
}

impl Default for CallbackConfig {
    fn default() -> Self {
        Self {
            enable_ai: true,
            enable_tool: true,
            enable_context: true,
            default_timeout_ms: 60_000,
            max_concurrent: 10,
            detailed_logging: false,
        }
    }
}

/// Callback Handler
///
/// Central handler for all callback requests from external extension services.
/// Coordinates AI, tool, and context callback handlers.
pub struct CallbackHandler {
    /// Security validator
    security: Arc<SecurityValidator>,

    /// AI callback handler
    ai_handler: Arc<AiCallbackHandler>,

    /// Tool callback handler
    tool_handler: Arc<ToolCallbackHandler>,

    /// Context callback handler
    context_handler: Arc<ContextCallbackHandler>,

    /// Configuration
    config: CallbackConfig,
}

impl CallbackHandler {
    /// Create a new callback handler
    pub fn new(
        security: Arc<SecurityValidator>,
        tool_router: Arc<ToolRouter>,
        node_router: Arc<NodeRouter>,
    ) -> Self {
        Self::with_config(
            security,
            tool_router,
            node_router,
            CallbackConfig::default(),
        )
    }

    /// Create a new callback handler with configuration
    pub fn with_config(
        security: Arc<SecurityValidator>,
        tool_router: Arc<ToolRouter>,
        _node_router: Arc<NodeRouter>,
        config: CallbackConfig,
    ) -> Self {
        let ai_handler = Arc::new(AiCallbackHandler::new(security.clone()));
        let tool_handler = Arc::new(ToolCallbackHandler::new(security.clone(), tool_router));
        let context_handler = Arc::new(ContextCallbackHandler::new(security.clone()));

        Self {
            security,
            ai_handler,
            tool_handler,
            context_handler,
            config,
        }
    }

    /// Handle a JSON-RPC callback request
    pub async fn handle_request(&self, request: JsonRpcRequest) -> Result<CallbackResult, CallbackError> {
        // Determine callback type from method
        let callback_type = CallbackType::from_method(&request.method)
            .ok_or_else(|| CallbackError::InvalidType(request.method.clone()))?;

        // Check if callback type is enabled
        self.check_callback_enabled(callback_type)?;

        // Get request ID
        let request_id = request.id.clone().unwrap_or_default();

        // Handle based on type
        match callback_type {
            CallbackType::Ai => {
                self.handle_ai_callback(request.params, request_id).await
            }
            CallbackType::Tool => {
                self.handle_tool_callback(request.params, request_id).await
            }
            CallbackType::Context => {
                self.handle_context_callback(request.params, request_id).await
            }
        }
    }

    /// Check if a callback type is enabled
    fn check_callback_enabled(&self, callback_type: CallbackType) -> Result<(), CallbackError> {
        let enabled = match callback_type {
            CallbackType::Ai => self.config.enable_ai,
            CallbackType::Tool => self.config.enable_tool,
            CallbackType::Context => self.config.enable_context,
        };

        if enabled {
            Ok(())
        } else {
            Err(CallbackError::InvalidType(format!(
                "{} callbacks are disabled",
                callback_type.method_name()
            )))
        }
    }

    /// Handle AI callback request
    async fn handle_ai_callback(
        &self,
        params: Option<JsonValue>,
        _request_id: JsonRpcId,
    ) -> Result<CallbackResult, CallbackError> {
        let params = params.ok_or_else(|| CallbackError::MissingField("params".to_string()))?;

        // Extract required fields
        let request_id = params
            .get("request_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CallbackError::MissingField("request_id".to_string()))?;

        let service_id = params
            .get("service_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CallbackError::MissingField("service_id".to_string()))?;

        let token = params
            .get("token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CallbackError::MissingField("token".to_string()))?;

        let prompt = params
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CallbackError::MissingField("prompt".to_string()))?;

        // Build AI callback request
        let ai_request = AiCallbackRequest {
            request_id: request_id.to_string(),
            service_id: ServiceId::new(service_id),
            token: token.to_string(),
            prompt: prompt.to_string(),
            context: params.get("context").cloned().unwrap_or(JsonValue::Null),
            model_config: params
                .get("model_config")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default(),
            timeout_ms: params
                .get("timeout_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(self.config.default_timeout_ms),
        };

        let result = self.ai_handler.handle(ai_request).await?;
        Ok(CallbackResult::Ai(result))
    }

    /// Handle tool callback request
    async fn handle_tool_callback(
        &self,
        params: Option<JsonValue>,
        _request_id: JsonRpcId,
    ) -> Result<CallbackResult, CallbackError> {
        let params = params.ok_or_else(|| CallbackError::MissingField("params".to_string()))?;

        // Extract required fields
        let request_id = params
            .get("request_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CallbackError::MissingField("request_id".to_string()))?;

        let service_id = params
            .get("service_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CallbackError::MissingField("service_id".to_string()))?;

        let token = params
            .get("token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CallbackError::MissingField("token".to_string()))?;

        let tool_name = params
            .get("tool_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CallbackError::MissingField("tool_name".to_string()))?;

        // Build tool callback request
        let tool_request = ToolCallbackRequest {
            request_id: request_id.to_string(),
            service_id: ServiceId::new(service_id),
            token: token.to_string(),
            tool_name: tool_name.to_string(),
            params: params.get("params").cloned().unwrap_or(JsonValue::Null),
            timeout_ms: params
                .get("timeout_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(self.config.default_timeout_ms),
            require_approval: params
                .get("require_approval")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        };

        let result = self.tool_handler.handle(tool_request).await?;
        Ok(CallbackResult::Tool(result))
    }

    /// Handle context callback request
    async fn handle_context_callback(
        &self,
        params: Option<JsonValue>,
        _request_id: JsonRpcId,
    ) -> Result<CallbackResult, CallbackError> {
        let params = params.ok_or_else(|| CallbackError::MissingField("params".to_string()))?;

        // Extract required fields
        let request_id = params
            .get("request_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CallbackError::MissingField("request_id".to_string()))?;

        let service_id = params
            .get("service_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CallbackError::MissingField("service_id".to_string()))?;

        let token = params
            .get("token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CallbackError::MissingField("token".to_string()))?;

        // Build context callback request
        let context_request = ContextCallbackRequest {
            request_id: request_id.to_string(),
            service_id: ServiceId::new(service_id),
            token: token.to_string(),
            operation: params
                .get("operation")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default(),
            key: params.get("key").and_then(|v| v.as_str()).map(|s| s.to_string()),
            value: params.get("value").cloned(),
            namespace: params.get("namespace").and_then(|v| v.as_str()).map(|s| s.to_string()),
        };

        let result = self.context_handler.handle(context_request).await?;
        Ok(CallbackResult::Context(result))
    }

    /// Create a JSON-RPC response for a callback result
    pub fn create_success_response(&self, id: JsonRpcId, result: CallbackResult) -> JsonRpcResponse {
        JsonRpcResponse::success(id, result.to_json())
    }

    /// Create a JSON-RPC error response for callback failures
    pub fn create_error_response(&self, error: CallbackError, id: JsonRpcId) -> JsonRpcResponse {
        let (code, message, data) = match error {
            CallbackError::SecurityFailed(msg) => (
                ErrorCode::PERMISSION_DENIED,
                "Security validation failed".to_string(),
                Some(serde_json::json!({ "reason": msg })),
            ),
            CallbackError::InvalidType(t) => (
                ErrorCode::METHOD_NOT_FOUND,
                format!("Invalid callback type: {}", t),
                None,
            ),
            CallbackError::Ai(ai_error) => {
                return self.ai_handler.create_error_response(ai_error, id);
            }
            CallbackError::Tool(tool_error) => {
                return self.tool_handler.create_error_response(tool_error, id);
            }
            CallbackError::Context(context_error) => {
                return self.context_handler.create_error_response(context_error, id);
            }
            CallbackError::MissingField(field) => (
                ErrorCode::INVALID_PARAMS,
                format!("Missing required field: {}", field),
                None,
            ),
            CallbackError::InvalidRequest(msg) => (
                ErrorCode::INVALID_REQUEST,
                msg,
                None,
            ),
            CallbackError::Internal(msg) => (
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

    /// Generate a callback token for a service
    pub async fn generate_token(
        &self,
        service_id: ServiceId,
        request_id: String,
        allowed_types: Vec<String>,
    ) -> Result<String, CallbackError> {
        self.security
            .generate_token(service_id, request_id, allowed_types)
            .await
            .map_err(|e| CallbackError::SecurityFailed(e.to_string()))
    }

    /// Validate a callback request (pre-validation)
    pub async fn validate(
        &self,
        token: &str,
        service_id: &ServiceId,
        request_id: &str,
        callback_type: &str,
    ) -> ValidationResult {
        self.security.validate(token, service_id, request_id, callback_type).await
    }

    /// Get the security validator
    pub fn security(&self) -> Arc<SecurityValidator> {
        self.security.clone()
    }

    /// Get the AI handler
    pub fn ai_handler(&self) -> Arc<AiCallbackHandler> {
        self.ai_handler.clone()
    }

    /// Get the tool handler
    pub fn tool_handler(&self) -> Arc<ToolCallbackHandler> {
        self.tool_handler.clone()
    }

    /// Get the context handler
    pub fn context_handler(&self) -> Arc<ContextCallbackHandler> {
        self.context_handler.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matrixrpc::{RegistryService, ToolRouter, NodeRouter};
    use super::ai::AiCallbackResult as TestAiCallbackResult;

    fn create_test_handlers() -> (
        Arc<SecurityValidator>,
        Arc<ToolRouter>,
        Arc<NodeRouter>,
        Arc<CallbackHandler>,
    ) {
        let security = Arc::new(SecurityValidator::new());
        let registry = Arc::new(RegistryService::new());
        let tool_router = Arc::new(ToolRouter::new(registry));
        let node_router = Arc::new(NodeRouter::new(registry));
        let callback = Arc::new(CallbackHandler::new(
            security.clone(),
            tool_router.clone(),
            node_router.clone(),
        ));

        (security, tool_router, node_router, callback)
    }

    #[tokio::test]
    async fn test_callback_handler_creation() {
        let (_, _, _, handler) = create_test_handlers();
        assert!(handler.config.enable_ai);
        assert!(handler.config.enable_tool);
        assert!(handler.config.enable_context);
    }

    #[tokio::test]
    async fn test_callback_type_detection() {
        assert_eq!(
            CallbackType::from_method("callback.ai"),
            Some(CallbackType::Ai)
        );
        assert_eq!(
            CallbackType::from_method("callback.tool"),
            Some(CallbackType::Tool)
        );
        assert_eq!(
            CallbackType::from_method("callback.context"),
            Some(CallbackType::Context)
        );
        assert_eq!(CallbackType::from_method("unknown"), None);
    }

    #[tokio::test]
    async fn test_invalid_callback_type() {
        let (_, _, _, handler) = create_test_handlers();

        let request = JsonRpcRequest::new("unknown.method");
        let result = handler.handle_request(request).await;

        assert!(matches!(result, Err(CallbackError::InvalidType(_))));
    }

    #[tokio::test]
    async fn test_missing_params() {
        let (_, _, _, handler) = create_test_handlers();

        let request = JsonRpcRequest::new("callback.ai"); // No params
        let result = handler.handle_request(request).await;

        assert!(matches!(result, Err(CallbackError::MissingField(_))));
    }

    #[tokio::test]
    async fn test_generate_token() {
        let (_, _, _, handler) = create_test_handlers();

        let service_id = ServiceId::new("test-service");
        let token = handler
            .generate_token(service_id, "req-001".to_string(), vec!["ai", "tool"])
            .await
            .unwrap();

        assert!(token.starts_with("cb_"));
    }

    #[tokio::test]
    async fn test_ai_callback_with_valid_token() {
        let (security, _, _, handler) = create_test_handlers();

        let service_id = ServiceId::new("test-service");
        let request_id = "req-001".to_string();
        let token = security
            .generate_token(service_id.clone(), request_id.clone(), vec!["ai"])
            .await
            .unwrap();

        let request = JsonRpcRequest::new("callback.ai")
            .params(serde_json::json!({
                "request_id": request_id,
                "service_id": service_id.to_string(),
                "token": token,
                "prompt": "Test prompt"
            }));

        let result = handler.handle_request(request).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_tool_callback_with_invalid_token() {
        let (_, _, _, handler) = create_test_handlers();

        let request = JsonRpcRequest::new("callback.tool")
            .params(serde_json::json!({
                "request_id": "req-001",
                "service_id": "test-service",
                "token": "invalid_token",
                "tool_name": "read"
            }));

        let result = handler.handle_request(request).await;
        assert!(matches!(result, Err(CallbackError::Tool(_))));
    }

    #[test]
    fn test_callback_result_to_json() {
        let ai_result = TestAiCallbackResult {
            content: "Test response".to_string(),
            model: "claude-sonnet-4".to_string(),
            input_tokens: 100,
            output_tokens: 50,
            duration_ms: 500,
            metadata: serde_json::json!({}),
        };

        let result = CallbackResult::Ai(ai_result);
        let json = result.to_json();

        assert!(json.get("content").is_some());
    }
}