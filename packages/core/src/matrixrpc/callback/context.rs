//! Context Callback Handler
//!
//! Handles context callback requests from external services.
//! Enables external nodes to access workflow context data.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use super::security::SecurityValidator;
use crate::matrixrpc::{ErrorCode, JsonRpcError, JsonRpcId, JsonRpcResponse, ServiceId};

/// Context operation type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ContextOperation {
    /// Get a value from context
    #[default]
    Get,

    /// Set a value in context
    Set,

    /// Delete a value from context
    Delete,

    /// List all keys
    List,

    /// Check if key exists
    Exists,

    /// Clear all context
    Clear,
}


impl ContextOperation {
    /// Get the string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            ContextOperation::Get => "get",
            ContextOperation::Set => "set",
            ContextOperation::Delete => "delete",
            ContextOperation::List => "list",
            ContextOperation::Exists => "exists",
            ContextOperation::Clear => "clear",
        }
    }
}

/// Context callback request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextCallbackRequest {
    /// Request ID from original node execution
    pub request_id: String,

    /// Service ID making the callback
    pub service_id: ServiceId,

    /// Security token
    pub token: String,

    /// Operation to perform
    #[serde(default)]
    pub operation: ContextOperation,

    /// Key to access
    #[serde(default)]
    pub key: Option<String>,

    /// Value to set (for Set operation)
    #[serde(default)]
    pub value: Option<JsonValue>,

    /// Namespace for the key
    #[serde(default)]
    pub namespace: Option<String>,
}

/// Context callback result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextCallbackResult {
    /// Operation that was performed
    pub operation: String,

    /// Key that was accessed
    #[serde(default)]
    pub key: Option<String>,

    /// Value (for Get operation)
    #[serde(default)]
    pub value: Option<JsonValue>,

    /// Keys (for List operation)
    #[serde(default)]
    pub keys: Vec<String>,

    /// Whether key exists (for Exists operation)
    #[serde(default)]
    pub exists: Option<bool>,

    /// Status message
    pub status: String,

    /// Additional metadata
    #[serde(default)]
    pub metadata: JsonValue,
}

/// Context callback error
#[derive(Debug, thiserror::Error)]
pub enum ContextCallbackError {
    /// Security validation failed
    #[error("Security validation failed: {0}")]
    SecurityFailed(String),

    /// Key not found
    #[error("Context key '{0}' not found")]
    KeyNotFound(String),

    /// Key already exists
    #[error("Context key '{0}' already exists")]
    KeyExists(String),

    /// Invalid operation
    #[error("Invalid context operation: {0}")]
    InvalidOperation(String),

    /// Missing key
    #[error("Missing key for context operation")]
    MissingKey,

    /// Missing value
    #[error("Missing value for Set operation")]
    MissingValue,

    /// Namespace not accessible
    #[error("Namespace '{0}' is not accessible")]
    NamespaceNotAccessible(String),

    /// Read-only context
    #[error("Context is read-only, cannot perform {0} operation")]
    ReadOnly(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Context namespace configuration
#[derive(Debug, Clone)]
pub struct ContextNamespaceConfig {
    /// Public namespaces (accessible to all)
    pub public: Vec<String>,

    /// Service-specific namespaces
    pub service_namespaces: HashMap<ServiceId, Vec<String>>,

    /// Read-only namespaces
    pub readonly: Vec<String>,

    /// Maximum context size per namespace
    pub max_size: usize,
}

impl Default for ContextNamespaceConfig {
    fn default() -> Self {
        Self {
            public: vec![
                "workflow".to_string(), "input".to_string(),
                "output".to_string(), "variables".to_string(),
            ],
            service_namespaces: HashMap::new(),
            readonly: vec![
                "input".to_string(), "system".to_string(),
            ],
            max_size: 1024,
        }
    }
}

/// Context store
#[derive(Debug, Default)]
struct ContextStore {
    /// Data store by namespace
    namespaces: HashMap<String, HashMap<String, JsonValue>>,
}

impl ContextStore {
    fn new() -> Self {
        Self::default()
    }

    fn get(&self, namespace: &str, key: &str) -> Option<&JsonValue> {
        self.namespaces.get(namespace)?.get(key)
    }

    fn set(&mut self, namespace: &str, key: &str, value: JsonValue) {
        self.namespaces
            .entry(namespace.to_string())
            .or_default()
            .insert(key.to_string(), value);
    }

    fn delete(&mut self, namespace: &str, key: &str) -> Option<JsonValue> {
        self.namespaces.get_mut(namespace)?.remove(key)
    }

    fn list(&self, namespace: &str) -> Vec<String> {
        self.namespaces
            .get(namespace)
            .map(|ns| ns.keys().cloned().collect())
            .unwrap_or_default()
    }

    fn exists(&self, namespace: &str, key: &str) -> bool {
        self.namespaces
            .get(namespace)
            .map(|ns| ns.contains_key(key))
            .unwrap_or(false)
    }

    fn clear(&mut self, namespace: &str) {
        if let Some(ns) = self.namespaces.get_mut(namespace) {
            ns.clear();
        }
    }
}

/// Context Callback Handler
///
/// Handles context callback requests from external extension services.
pub struct ContextCallbackHandler {
    /// Security validator
    security: Arc<SecurityValidator>,

    /// Context store
    store: Arc<tokio::sync::RwLock<ContextStore>>,

    /// Namespace configuration
    namespace_config: ContextNamespaceConfig,
}

impl ContextCallbackHandler {
    /// Create a new context callback handler
    pub fn new(security: Arc<SecurityValidator>) -> Self {
        Self {
            security,
            store: Arc::new(tokio::sync::RwLock::new(ContextStore::new())),
            namespace_config: ContextNamespaceConfig::default(),
        }
    }

    /// Set namespace configuration
    pub fn with_namespace_config(mut self, config: ContextNamespaceConfig) -> Self {
        self.namespace_config = config;
        self
    }

    /// Initialize with existing context
    pub async fn initialize_context(&self, namespace: &str, data: HashMap<String, JsonValue>) {
        let mut store = self.store.write().await;
        store.namespaces.insert(namespace.to_string(), data);
    }

    /// Handle a context callback request
    pub async fn handle(&self, request: ContextCallbackRequest) -> Result<ContextCallbackResult, ContextCallbackError> {
        // Validate security
        let validation = self
            .security
            .validate(&request.token, &request.service_id, &request.request_id, "context")
            .await;

        if !validation.is_valid {
            return Err(ContextCallbackError::SecurityFailed(
                validation.error.unwrap_or_else(|| "Unknown security error".to_string()),
            ));
        }

        // Get namespace (default to "workflow")
        let namespace = request.namespace.clone().unwrap_or_else(|| "workflow".to_string());

        // Check namespace accessibility
        if !self.is_namespace_accessible(&namespace, &request.service_id) {
            return Err(ContextCallbackError::NamespaceNotAccessible(namespace));
        }

        // Check if read-only namespace for write operations
        if self.namespace_config.readonly.contains(&namespace)
            && matches!(
                request.operation,
                ContextOperation::Set | ContextOperation::Delete | ContextOperation::Clear
            )
        {
            return Err(ContextCallbackError::ReadOnly(request.operation.as_str().to_string()));
        }

        let mut store = self.store.write().await;

        match request.operation {
            ContextOperation::Get => {
                let key = request.key.clone().ok_or(ContextCallbackError::MissingKey)?;
                let value = store
                    .get(&namespace, &key)
                    .cloned()
                    .ok_or_else(|| ContextCallbackError::KeyNotFound(key.clone()))?;

                Ok(ContextCallbackResult {
                    operation: "get".to_string(),
                    key: Some(key),
                    value: Some(value),
                    keys: vec![],
                    exists: None,
                    status: "success".to_string(),
                    metadata: serde_json::json!({
                        "namespace": namespace,
                        "request_id": request.request_id,
                    }),
                })
            }

            ContextOperation::Set => {
                let key = request.key.clone().ok_or(ContextCallbackError::MissingKey)?;
                let value = request.value.clone().ok_or(ContextCallbackError::MissingValue)?;

                store.set(&namespace, &key, value.clone());

                Ok(ContextCallbackResult {
                    operation: "set".to_string(),
                    key: Some(key),
                    value: Some(value),
                    keys: vec![],
                    exists: None,
                    status: "success".to_string(),
                    metadata: serde_json::json!({
                        "namespace": namespace,
                        "request_id": request.request_id,
                    }),
                })
            }

            ContextOperation::Delete => {
                let key = request.key.clone().ok_or(ContextCallbackError::MissingKey)?;
                let existed = store.delete(&namespace, &key).is_some();

                Ok(ContextCallbackResult {
                    operation: "delete".to_string(),
                    key: Some(key),
                    value: None,
                    keys: vec![],
                    exists: Some(existed),
                    status: if existed { "success" } else { "not_found" }.to_string(),
                    metadata: serde_json::json!({
                        "namespace": namespace,
                        "request_id": request.request_id,
                    }),
                })
            }

            ContextOperation::List => {
                let keys = store.list(&namespace);
                let keys_count = keys.len();

                Ok(ContextCallbackResult {
                    operation: "list".to_string(),
                    key: None,
                    value: None,
                    keys,
                    exists: None,
                    status: "success".to_string(),
                    metadata: serde_json::json!({
                        "namespace": namespace,
                        "request_id": request.request_id,
                        "count": keys_count,
                    }),
                })
            }

            ContextOperation::Exists => {
                let key = request.key.clone().ok_or(ContextCallbackError::MissingKey)?;
                let exists = store.exists(&namespace, &key);

                Ok(ContextCallbackResult {
                    operation: "exists".to_string(),
                    key: Some(key),
                    value: None,
                    keys: vec![],
                    exists: Some(exists),
                    status: "success".to_string(),
                    metadata: serde_json::json!({
                        "namespace": namespace,
                        "request_id": request.request_id,
                    }),
                })
            }

            ContextOperation::Clear => {
                store.clear(&namespace);

                Ok(ContextCallbackResult {
                    operation: "clear".to_string(),
                    key: None,
                    value: None,
                    keys: vec![],
                    exists: None,
                    status: "success".to_string(),
                    metadata: serde_json::json!({
                        "namespace": namespace,
                        "request_id": request.request_id,
                    }),
                })
            }
        }
    }

    /// Check if namespace is accessible for a service
    fn is_namespace_accessible(&self, namespace: &str, service_id: &ServiceId) -> bool {
        // Public namespaces are accessible to all
        if self.namespace_config.public.contains(&namespace.to_string()) {
            return true;
        }

        // Check service-specific namespaces
        if let Some(namespaces) = self.namespace_config.service_namespaces.get(service_id)
            && namespaces.contains(&namespace.to_string()) {
                return true;
            }

        false
    }

    /// Create a JSON-RPC error response for context callback failures
    pub fn create_error_response(&self, error: ContextCallbackError, id: JsonRpcId) -> JsonRpcResponse {
        let (code, message, data) = match error {
            ContextCallbackError::SecurityFailed(msg) => (
                ErrorCode::PERMISSION_DENIED,
                "Security validation failed".to_string(),
                Some(serde_json::json!({ "reason": msg })),
            ),
            ContextCallbackError::KeyNotFound(key) => (
                ErrorCode::RESOURCE_NOT_FOUND,
                format!("Context key '{}' not found", key),
                None,
            ),
            ContextCallbackError::KeyExists(key) => (
                ErrorCode::RESOURCE_EXISTS,
                format!("Context key '{}' already exists", key),
                None,
            ),
            ContextCallbackError::InvalidOperation(op) => (
                ErrorCode::INVALID_PARAMS,
                format!("Invalid context operation: {}", op),
                None,
            ),
            ContextCallbackError::MissingKey => (
                ErrorCode::INVALID_PARAMS,
                "Missing key for context operation".to_string(),
                None,
            ),
            ContextCallbackError::MissingValue => (
                ErrorCode::INVALID_PARAMS,
                "Missing value for Set operation".to_string(),
                None,
            ),
            ContextCallbackError::NamespaceNotAccessible(ns) => (
                ErrorCode::PERMISSION_DENIED,
                format!("Namespace '{}' is not accessible", ns),
                None,
            ),
            ContextCallbackError::ReadOnly(op) => (
                ErrorCode::PERMISSION_DENIED,
                format!("Context is read-only, cannot perform {} operation", op),
                None,
            ),
            ContextCallbackError::Internal(msg) => (
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

    /// Get all available namespaces for a service
    pub fn get_available_namespaces(&self, service_id: &ServiceId) -> Vec<String> {
        let mut namespaces = self.namespace_config.public.clone();

        if let Some(service_ns) = self.namespace_config.service_namespaces.get(service_id) {
            namespaces.extend(service_ns.clone());
        }

        namespaces
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_context_callback_handler_creation() {
        let security = Arc::new(SecurityValidator::new());
        let handler = ContextCallbackHandler::new(security);

        assert!(!handler.namespace_config.public.is_empty());
    }

    #[tokio::test]
    async fn test_initialize_context() {
        let security = Arc::new(SecurityValidator::new());
        let handler = ContextCallbackHandler::new(security);

        let data = HashMap::from([
            ("key1".to_string(), serde_json::json!("value1")),
            ("key2".to_string(), serde_json::json!(42)),
        ]);

        handler.initialize_context("workflow", data).await;

        // Verify by doing a List operation
        let store = handler.store.read().await;
        let keys = store.list("workflow");
        assert_eq!(keys.len(), 2);
    }

    #[tokio::test]
    async fn test_context_get() {
        let security = Arc::new(SecurityValidator::new());
        let handler = ContextCallbackHandler::new(security.clone());

        // Initialize context
        handler
            .initialize_context(
                "workflow",
                HashMap::from([("test_key".to_string(), serde_json::json!("test_value"))]),
            )
            .await;

        // Generate token
        let service_id = ServiceId::new("test-service");
        let request_id = "req-001".to_string();
        let token = security
            .generate_token(service_id.clone(), request_id.clone(), vec!["context".to_string()])
            .await
            .unwrap();

        let request = ContextCallbackRequest {
            request_id,
            service_id,
            token,
            operation: ContextOperation::Get,
            key: Some("test_key".to_string()),
            value: None,
            namespace: Some("workflow".to_string()),
        };

        let result = handler.handle(request).await.unwrap();
        assert_eq!(result.operation, "get");
        assert_eq!(result.key, Some("test_key".to_string()));
        assert_eq!(result.value, Some(serde_json::json!("test_value")));
    }

    #[tokio::test]
    async fn test_context_set() {
        let security = Arc::new(SecurityValidator::new());
        let handler = ContextCallbackHandler::new(security.clone());

        // Generate token
        let service_id = ServiceId::new("test-service");
        let request_id = "req-001".to_string();
        let token = security
            .generate_token(service_id.clone(), request_id.clone(), vec!["context".to_string()])
            .await
            .unwrap();

        let request = ContextCallbackRequest {
            request_id,
            service_id,
            token,
            operation: ContextOperation::Set,
            key: Some("new_key".to_string()),
            value: Some(serde_json::json!("new_value")),
            namespace: Some("workflow".to_string()),
        };

        let result = handler.handle(request).await.unwrap();
        assert_eq!(result.operation, "set");
        assert_eq!(result.status, "success");
    }

    #[tokio::test]
    async fn test_context_list() {
        let security = Arc::new(SecurityValidator::new());
        let handler = ContextCallbackHandler::new(security.clone());

        handler
            .initialize_context(
                "workflow",
                HashMap::from([
                    ("key1".to_string(), serde_json::json!(1)),
                    ("key2".to_string(), serde_json::json!(2)),
                ]),
            )
            .await;

        let service_id = ServiceId::new("test-service");
        let request_id = "req-001".to_string();
        let token = security
            .generate_token(service_id.clone(), request_id.clone(), vec!["context".to_string()])
            .await
            .unwrap();

        let request = ContextCallbackRequest {
            request_id,
            service_id,
            token,
            operation: ContextOperation::List,
            key: None,
            value: None,
            namespace: Some("workflow".to_string()),
        };

        let result = handler.handle(request).await.unwrap();
        assert_eq!(result.keys.len(), 2);
    }

    #[tokio::test]
    async fn test_context_exists() {
        let security = Arc::new(SecurityValidator::new());
        let handler = ContextCallbackHandler::new(security.clone());

        handler
            .initialize_context(
                "workflow",
                HashMap::from([("existing_key".to_string(), serde_json::json!("value"))]),
            )
            .await;

        let service_id = ServiceId::new("test-service");
        let request_id = "req-001".to_string();
        let token = security
            .generate_token(service_id.clone(), request_id.clone(), vec!["context".to_string()])
            .await
            .unwrap();

        // Test existing key
        let request = ContextCallbackRequest {
            request_id,
            service_id,
            token,
            operation: ContextOperation::Exists,
            key: Some("existing_key".to_string()),
            value: None,
            namespace: Some("workflow".to_string()),
        };

        let result = handler.handle(request).await.unwrap();
        assert_eq!(result.exists, Some(true));
    }

    #[tokio::test]
    async fn test_context_readonly_namespace() {
        let security = Arc::new(SecurityValidator::new());
        let handler = ContextCallbackHandler::new(security.clone());

        let service_id = ServiceId::new("test-service");
        let request_id = "req-001".to_string();
        let token = security
            .generate_token(service_id.clone(), request_id.clone(), vec!["context".to_string()])
            .await
            .unwrap();

        // "input" is read-only by default
        let request = ContextCallbackRequest {
            request_id,
            service_id,
            token,
            operation: ContextOperation::Set,
            key: Some("key".to_string()),
            value: Some(serde_json::json!("value")),
            namespace: Some("input".to_string()),
        };

        let result = handler.handle(request).await;
        assert!(matches!(result, Err(ContextCallbackError::ReadOnly(_))));
    }

    #[test]
    fn test_namespace_accessible() {
        let security = Arc::new(SecurityValidator::new());
        let handler = ContextCallbackHandler::new(security);

        // Public namespace
        assert!(handler.is_namespace_accessible("workflow", &ServiceId::new("any")));

        // Not accessible (not public and not in service namespaces)
        assert!(!handler.is_namespace_accessible("private", &ServiceId::new("any")));
    }

    #[test]
    fn test_get_available_namespaces() {
        let security = Arc::new(SecurityValidator::new());
        let handler = ContextCallbackHandler::new(security);

        let namespaces = handler.get_available_namespaces(&ServiceId::new("test"));
        assert!(namespaces.contains(&"workflow".to_string()));
        assert!(namespaces.contains(&"input".to_string()));
    }
}