//! AI Callback Handler
//!
//! Handles AI callback requests from external services.
//! Enables external nodes to call MatrixCode's AI providers.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use super::security::SecurityValidator;
use crate::matrixrpc::{ErrorCode, JsonRpcError, JsonRpcId, JsonRpcResponse, ServiceId};

/// AI callback request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiCallbackRequest {
    /// Request ID from original node execution
    pub request_id: String,

    /// Service ID making the callback
    pub service_id: ServiceId,

    /// Security token
    pub token: String,

    /// Prompt to send to AI
    pub prompt: String,

    /// Context data
    #[serde(default)]
    pub context: JsonValue,

    /// Model configuration
    #[serde(default)]
    pub model_config: AiModelConfig,

    /// Timeout in milliseconds
    #[serde(default = "default_ai_timeout")]
    pub timeout_ms: u64,
}

fn default_ai_timeout() -> u64 {
    60_000 // 60 seconds
}

/// AI model configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AiModelConfig {
    /// Model name (e.g., "claude-sonnet-4")
    #[serde(default)]
    pub model: Option<String>,

    /// Temperature (0.0 - 1.0)
    #[serde(default)]
    pub temperature: Option<f32>,

    /// Max tokens to generate
    #[serde(default)]
    pub max_tokens: Option<u32>,

    /// System prompt override
    #[serde(default)]
    pub system_prompt: Option<String>,

    /// Stop sequences
    #[serde(default)]
    pub stop_sequences: Option<Vec<String>>,

    /// Enable streaming
    #[serde(default)]
    pub stream: bool,
}

/// AI callback result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiCallbackResult {
    /// Response content
    pub content: String,

    /// Model used
    pub model: String,

    /// Input tokens
    pub input_tokens: u32,

    /// Output tokens
    pub output_tokens: u32,

    /// Duration in milliseconds
    pub duration_ms: u64,

    /// Additional metadata
    #[serde(default)]
    pub metadata: JsonValue,
}

/// AI callback error
#[derive(Debug, thiserror::Error)]
pub enum AiCallbackError {
    /// Security validation failed
    #[error("Security validation failed: {0}")]
    SecurityFailed(String),

    /// Provider not available
    #[error("AI provider not available")]
    ProviderNotAvailable,

    /// Invalid prompt
    #[error("Invalid prompt: {0}")]
    InvalidPrompt(String),

    /// Model not found
    #[error("Model '{0}' not found")]
    ModelNotFound(String),

    /// Timeout
    #[error("AI request timed out after {0}ms")]
    Timeout(u64),

    /// Provider error
    #[error("Provider error: {0}")]
    ProviderError(String),

    /// Rate limit exceeded
    #[error("AI rate limit exceeded")]
    RateLimitExceeded,

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

/// AI Callback Handler
///
/// Handles AI callback requests from external extension services.
pub struct AiCallbackHandler {
    /// Security validator
    security: Arc<SecurityValidator>,

    /// Default model
    default_model: String,

    /// Default timeout
    default_timeout_ms: u64,

    /// Maximum tokens allowed
    max_tokens_limit: u32,
}

impl AiCallbackHandler {
    /// Create a new AI callback handler
    pub fn new(security: Arc<SecurityValidator>) -> Self {
        Self {
            security,
            default_model: "claude-sonnet-4".to_string(),
            default_timeout_ms: 60_000,
            max_tokens_limit: 4096,
        }
    }

    /// Set default model
    pub fn with_default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }

    /// Set default timeout
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.default_timeout_ms = timeout_ms;
        self
    }

    /// Set max tokens limit
    pub fn with_max_tokens_limit(mut self, limit: u32) -> Self {
        self.max_tokens_limit = limit;
        self
    }

    /// Handle an AI callback request
    pub async fn handle(&self, request: AiCallbackRequest) -> Result<AiCallbackResult, AiCallbackError> {
        // Validate security
        let validation = self
            .security
            .validate(&request.token, &request.service_id, &request.request_id, "ai")
            .await;

        if !validation.is_valid {
            return Err(AiCallbackError::SecurityFailed(
                validation.error.unwrap_or_else(|| "Unknown security error".to_string()),
            ));
        }

        // Validate prompt
        if request.prompt.is_empty() {
            return Err(AiCallbackError::InvalidPrompt("Prompt cannot be empty".to_string()));
        }

        // Get model to use
        let model = request
            .model_config
            .model
            .clone()
            .unwrap_or_else(|| self.default_model.clone());

        // Get max tokens with limit
        let max_tokens = request
            .model_config
            .max_tokens
            .unwrap_or(1024)
            .min(self.max_tokens_limit);

        // Get timeout
        let timeout = request.timeout_ms.max(self.default_timeout_ms);

        // Build response
        // Note: In real implementation, this would call the actual AI provider
        // Here we provide a mock implementation for testing
        let result = AiCallbackResult {
            content: format!("AI response to: {}", &request.prompt[..100.min(request.prompt.len())]),
            model,
            input_tokens: request.prompt.len() as u32 / 4,
            output_tokens: 100,
            duration_ms: 500,
            metadata: serde_json::json!({
                "request_id": request.request_id,
                "service_id": request.service_id.to_string(),
                "temperature": request.model_config.temperature.unwrap_or(0.7),
            }),
        };

        Ok(result)
    }

    /// Create a JSON-RPC error response for AI callback failures
    pub fn create_error_response(&self, error: AiCallbackError, id: JsonRpcId) -> JsonRpcResponse {
        let (code, message, data) = match error {
            AiCallbackError::SecurityFailed(msg) => (
                ErrorCode::PERMISSION_DENIED,
                "Security validation failed".to_string(),
                Some(serde_json::json!({ "reason": msg })),
            ),
            AiCallbackError::ProviderNotAvailable => (
                ErrorCode::RESOURCE_NOT_FOUND,
                "AI provider not available".to_string(),
                None,
            ),
            AiCallbackError::InvalidPrompt(msg) => (
                ErrorCode::INVALID_PARAMS,
                "Invalid prompt".to_string(),
                Some(serde_json::json!({ "reason": msg })),
            ),
            AiCallbackError::ModelNotFound(model) => (
                ErrorCode::RESOURCE_NOT_FOUND,
                format!("Model '{}' not found", model),
                None,
            ),
            AiCallbackError::Timeout(ms) => (
                ErrorCode::TIMEOUT_ERROR,
                "AI request timed out".to_string(),
                Some(serde_json::json!({ "timeout_ms": ms })),
            ),
            AiCallbackError::ProviderError(msg) => (
                ErrorCode::INTERNAL_ERROR,
                "Provider error".to_string(),
                Some(serde_json::json!({ "error": msg })),
            ),
            AiCallbackError::RateLimitExceeded => (
                ErrorCode::PERMISSION_DENIED,
                "AI rate limit exceeded".to_string(),
                None,
            ),
            AiCallbackError::Internal(msg) => (
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

    /// Check if a model is available
    pub fn is_model_available(&self, model: &str) -> bool {
        // Check against known models
        matches!(
            model,
            "claude-opus-4" | "claude-sonnet-4" | "claude-haiku-4" | "claude-3-opus" | "claude-3-sonnet" | "claude-3-haiku"
        )
    }

    /// Get available models
    pub fn get_available_models(&self) -> Vec<String> {
        vec![
            "claude-opus-4".to_string(),
            "claude-sonnet-4".to_string(),
            "claude-haiku-4".to_string(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ai_callback_handler_creation() {
        let security = Arc::new(SecurityValidator::new());
        let handler = AiCallbackHandler::new(security);

        assert_eq!(handler.default_model, "claude-sonnet-4");
        assert_eq!(handler.default_timeout_ms, 60_000);
    }

    #[tokio::test]
    async fn test_ai_callback_with_custom_config() {
        let security = Arc::new(SecurityValidator::new());
        let handler = AiCallbackHandler::new(security)
            .with_default_model("claude-opus-4")
            .with_timeout(30_000)
            .with_max_tokens_limit(2048);

        assert_eq!(handler.default_model, "claude-opus-4");
        assert_eq!(handler.default_timeout_ms, 30_000);
        assert_eq!(handler.max_tokens_limit, 2048);
    }

    #[test]
    fn test_ai_model_config_default() {
        let config = AiModelConfig::default();
        assert!(config.model.is_none());
        assert!(config.temperature.is_none());
        assert!(config.max_tokens.is_none());
    }

    #[test]
    fn test_ai_callback_result_serialization() {
        let result = AiCallbackResult {
            content: "Test response".to_string(),
            model: "claude-sonnet-4".to_string(),
            input_tokens: 100,
            output_tokens: 50,
            duration_ms: 500,
            metadata: serde_json::json!({}),
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("Test response"));
    }

    #[test]
    fn test_is_model_available() {
        let security = Arc::new(SecurityValidator::new());
        let handler = AiCallbackHandler::new(security);

        assert!(handler.is_model_available("claude-sonnet-4"));
        assert!(handler.is_model_available("claude-opus-4"));
        assert!(!handler.is_model_available("unknown-model"));
    }

    #[test]
    fn test_get_available_models() {
        let security = Arc::new(SecurityValidator::new());
        let handler = AiCallbackHandler::new(security);

        let models = handler.get_available_models();
        assert!(models.contains(&"claude-sonnet-4".to_string()));
        assert!(models.contains(&"claude-opus-4".to_string()));
    }

    #[tokio::test]
    async fn test_ai_callback_security_validation() {
        let security = Arc::new(SecurityValidator::new());
        let handler = AiCallbackHandler::new(security.clone());

        // Generate token
        let service_id = ServiceId::new("test-service");
        let request_id = "req-001".to_string();
        let token = security
            .generate_token(service_id.clone(), request_id.clone(), vec!["ai"])
            .await
            .unwrap();

        let request = AiCallbackRequest {
            request_id,
            service_id,
            token,
            prompt: "Test prompt".to_string(),
            context: serde_json::json!({}),
            model_config: AiModelConfig::default(),
            timeout_ms: 60_000,
        };

        let result = handler.handle(request).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert!(!response.content.is_empty());
    }

    #[tokio::test]
    async fn test_ai_callback_invalid_token() {
        let security = Arc::new(SecurityValidator::new());
        let handler = AiCallbackHandler::new(security);

        let request = AiCallbackRequest {
            request_id: "req-001".to_string(),
            service_id: ServiceId::new("test-service"),
            token: "invalid_token".to_string(),
            prompt: "Test prompt".to_string(),
            context: serde_json::json!({}),
            model_config: AiModelConfig::default(),
            timeout_ms: 60_000,
        };

        let result = handler.handle(request).await;
        assert!(matches!(result, Err(AiCallbackError::SecurityFailed(_))));
    }
}