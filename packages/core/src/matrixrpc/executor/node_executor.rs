//! Node Executor Implementation
//!
//! Executes workflow node calls via JSON-RPC transport with timeout and retry support.
//! Handles callback endpoints and execution result processing.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;

use crate::matrixrpc::{
    ErrorCode, JsonRpcError, JsonRpcId, JsonRpcResponse,
    ServiceId, ServiceStatus, RegistryService,
};
use crate::matrixrpc::transport::{StdioTransport, TransportConfig as TransportSettings};
use crate::matrixrpc::router::{NodeRouter, NodeRouteResult, NodeRouterError, NodeContext};

/// Error type for node execution operations
#[derive(Debug, thiserror::Error)]
pub enum NodeExecutorError {
    /// Transport error during execution
    #[error("Transport error: {0}")]
    TransportError(String),

    /// Execution timeout
    #[error("Node '{node_id}' execution timed out after {timeout_ms}ms")]
    Timeout { node_id: String, timeout_ms: u64 },

    /// Retry exhausted
    #[error("Node '{node_id}' execution failed after {attempts} attempts")]
    RetryExhausted { node_id: String, attempts: u32, last_error: String },

    /// Service not connected
    #[error("Service '{0}' is not connected")]
    ServiceNotConnected(ServiceId),

    /// Invalid response
    #[error("Invalid response from service: {0}")]
    InvalidResponse(String),

    /// Node execution failed
    #[error("Node '{node_id}' execution failed: {message}")]
    ExecutionFailed { node_id: String, message: String, data: Option<serde_json::Value> },

    /// Callback failed
    #[error("Callback '{callback_type}' failed for node '{node_id}': {message}")]
    CallbackFailed { node_id: String, callback_type: String, message: String },

    /// Routing error
    #[error("Routing error: {0}")]
    RoutingError(#[from] NodeRouterError),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result of a node execution
#[derive(Debug, Clone)]
pub struct NodeExecutionResult {
    /// The node ID that was executed
    pub node_id: String,
    /// The execution result data
    pub result: serde_json::Value,
    /// Execution status
    pub status: NodeExecutionStatus,
    /// Execution metadata
    pub metadata: serde_json::Value,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    /// Callbacks made during execution (if any)
    pub callbacks: Vec<CallbackRecord>,
}

/// Status of node execution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeExecutionStatus {
    /// Node executed successfully
    Success,
    /// Node execution failed
    Failed,
    /// Node was skipped (condition not met)
    Skipped,
    /// Node is pending (async execution)
    Pending,
    /// Node requires retry
    Retry,
}

impl NodeExecutionStatus {
    /// Check if the status indicates success
    pub fn is_success(&self) -> bool {
        matches!(self, NodeExecutionStatus::Success)
    }

    /// Check if the status indicates failure
    pub fn is_failure(&self) -> bool {
        matches!(self, NodeExecutionStatus::Failed)
    }

    /// Check if the node was skipped
    pub fn is_skipped(&self) -> bool {
        matches!(self, NodeExecutionStatus::Skipped)
    }
}

/// Record of a callback made during node execution
#[derive(Debug, Clone)]
pub struct CallbackRecord {
    /// Callback type (ai, tool, context)
    pub callback_type: String,
    /// Callback request ID
    pub request_id: String,
    /// Callback parameters
    pub params: serde_json::Value,
    /// Callback result
    pub result: serde_json::Value,
    /// Callback duration in milliseconds
    pub duration_ms: u64,
}

/// Node execution configuration
#[derive(Debug, Clone)]
pub struct NodeExecutionConfig {
    /// Timeout for node execution (milliseconds)
    pub timeout_ms: u64,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Retry interval base (milliseconds)
    pub retry_interval_ms: u64,
    /// Enable callback handling
    pub enable_callbacks: bool,
    /// Callback timeout (milliseconds)
    pub callback_timeout_ms: u64,
    /// Transport settings
    pub transport: TransportSettings,
}

impl Default for NodeExecutionConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 60_000,
            max_retries: 3,
            retry_interval_ms: 2000,
            enable_callbacks: true,
            callback_timeout_ms: 30_000,
            transport: TransportSettings::default(),
        }
    }
}

impl NodeExecutionConfig {
    /// Create a new configuration
    pub fn new(timeout_ms: u64) -> Self {
        Self {
            timeout_ms,
            ..Default::default()
        }
    }

    /// Set maximum retries
    pub fn max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// Disable callbacks
    pub fn no_callbacks(mut self) -> Self {
        self.enable_callbacks = false;
        self
    }

    /// Set callback timeout
    pub fn callback_timeout(mut self, timeout_ms: u64) -> Self {
        self.callback_timeout_ms = timeout_ms;
        self
    }
}

/// Transport connection wrapper for node execution
#[derive(Debug)]
struct NodeTransportConnection {
    service_id: ServiceId,
    connected: RwLock<bool>,
}

impl NodeTransportConnection {
    fn new(service_id: ServiceId) -> Self {
        Self {
            service_id,
            connected: RwLock::new(false),
        }
    }

    async fn is_connected(&self) -> bool {
        *self.connected.read().await
    }
}

use std::collections::HashMap;

/// Node Executor
///
/// Executes workflow node calls through the transport layer with support for
/// timeout, retry, and callback handling.
#[derive(Debug)]
pub struct NodeExecutor {
    /// Node router for finding services
    router: Arc<NodeRouter>,
    /// Registry service for status checks
    registry: Arc<RegistryService>,
    /// Execution configuration
    config: NodeExecutionConfig,
    /// Active transport connections
    connections: RwLock<HashMap<ServiceId, Arc<NodeTransportConnection>>>,
    /// Pending callbacks waiting for response
    pending_callbacks: RwLock<HashMap<String, CallbackContext>>,
}

/// Context for a pending callback
#[derive(Debug, Clone)]
struct CallbackContext {
    /// The node execution that made the callback
    node_id: String,
    /// Original request ID
    original_request_id: JsonRpcId,
    /// Callback request ID
    callback_request_id: String,
    /// Timestamp when callback was initiated
    initiated_at: std::time::Instant,
}

impl NodeExecutor {
    /// Create a new node executor
    pub fn new(router: Arc<NodeRouter>, registry: Arc<RegistryService>) -> Self {
        Self {
            router,
            registry,
            config: NodeExecutionConfig::default(),
            connections: RwLock::new(HashMap::new()),
            pending_callbacks: RwLock::new(HashMap::new()),
        }
    }

    /// Create with custom configuration
    pub fn with_config(
        router: Arc<NodeRouter>,
        registry: Arc<RegistryService>,
        config: NodeExecutionConfig,
    ) -> Self {
        Self {
            router,
            registry,
            config,
            connections: RwLock::new(HashMap::new()),
            pending_callbacks: RwLock::new(HashMap::new()),
        }
    }

    /// Execute a node
    ///
    /// Routes the execution to the appropriate service and handles callbacks.
    pub async fn execute(
        &self,
        node_id: &str,
        context: NodeContext,
    ) -> Result<NodeExecutionResult, NodeExecutorError> {
        let request_id = JsonRpcId::generate();

        // Route the execution
        let route_result = self.router
            .route(node_id, context.clone(), request_id.clone(), vec![])
            .await?;

        // Execute with retry
        self.execute_with_retry(route_result).await
    }

    /// Execute a node with required capabilities
    pub async fn execute_with_capabilities(
        &self,
        node_id: &str,
        context: NodeContext,
        required_capabilities: Vec<String>,
    ) -> Result<NodeExecutionResult, NodeExecutorError> {
        let request_id = JsonRpcId::generate();

        // Convert capabilities
        let caps: Vec<crate::matrixrpc::router::NodeCapability> = required_capabilities
            .iter()
            .filter_map(|c| crate::matrixrpc::router::NodeCapability::from_str(c))
            .collect();

        // Route the execution
        let route_result = self.router
            .route(node_id, context.clone(), request_id.clone(), caps)
            .await?;

        // Execute with retry
        self.execute_with_retry(route_result).await
    }

    /// Execute with retry strategy
    async fn execute_with_retry(
        &self,
        route_result: NodeRouteResult,
    ) -> Result<NodeExecutionResult, NodeExecutorError> {
        let mut attempts = 0;
        let mut last_error: Option<String> = None;
        let start_time = std::time::Instant::now();

        while attempts < self.config.max_retries {
            attempts += 1;

            // Apply retry delay (skip for first attempt)
            if attempts > 1 {
                let delay = self.config.retry_interval_ms * 2u64.pow(attempts - 2);
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }

            // Check service status
            let service = self.registry.get(&route_result.service_id).await;
            match service {
                Some(s) if s.status == ServiceStatus::Running => {}
                Some(s) => {
                    last_error = Some(format!("Service status: {:?}", s.status));
                    continue;
                }
                None => {
                    return Err(NodeExecutorError::ServiceNotConnected(route_result.service_id.clone()));
                }
            }

            // Execute single attempt
            let result = self.execute_single(&route_result).await;

            match result {
                Ok(execution_result) => {
                    // Update duration
                    let duration_ms = start_time.elapsed().as_millis() as u64;
                    return Ok(NodeExecutionResult {
                        node_id: execution_result.node_id,
                        result: execution_result.result,
                        status: execution_result.status,
                        metadata: execution_result.metadata,
                        duration_ms,
                        callbacks: execution_result.callbacks,
                    });
                }
                Err(NodeExecutorError::Timeout { .. }) => {
                    last_error = Some("Timeout".to_string());
                    // Retry on timeout
                }
                Err(NodeExecutorError::TransportError(_)) => {
                    last_error = Some(result.unwrap_err().to_string());
                    // Retry on transport error
                }
                Err(e) => {
                    // Don't retry on other errors
                    return Err(e);
                }
            }
        }

        // All retries exhausted
        Err(NodeExecutorError::RetryExhausted {
            node_id: route_result.node_id.clone(),
            attempts,
            last_error: last_error.unwrap_or_else(|| "Unknown error".to_string()),
        })
    }

    /// Execute a single attempt
    async fn execute_single(
        &self,
        route_result: &NodeRouteResult,
    ) -> Result<NodeExecutionResult, NodeExecutorError> {
        // Create the request
        let request = self.router.create_node_request(route_result.clone());

        // Get connection
        let _connection = self.get_connection(&route_result.service_id).await?;

        // In real implementation, send request via transport
        // For now, return a placeholder since actual connection requires service config
        Err(NodeExecutorError::ServiceNotConnected(route_result.service_id.clone()))
    }

    /// Get or create a transport connection for a service
    async fn get_connection(
        &self,
        service_id: &ServiceId,
    ) -> Result<Arc<NodeTransportConnection>, NodeExecutorError> {
        let connections = self.connections.read().await;

        if let Some(conn) = connections.get(service_id) {
            if conn.is_connected().await {
                return Ok(conn.clone());
            }
        }

        drop(connections);

        // Need to create new connection - return error for now
        Err(NodeExecutorError::ServiceNotConnected(service_id.clone()))
    }

    /// Process the response from node execution
    fn process_response(
        &self,
        response: JsonRpcResponse,
        node_id: &str,
    ) -> Result<NodeExecutionResult, NodeExecutorError> {
        if response.is_success() {
            let result = response.result.clone().unwrap_or(serde_json::json!({}));

            // Parse execution status from result
            let status_str = result.get("status")
                .and_then(|s| s.as_str())
                .unwrap_or("success");

            let status = match status_str {
                "success" => NodeExecutionStatus::Success,
                "failed" => NodeExecutionStatus::Failed,
                "skipped" => NodeExecutionStatus::Skipped,
                "pending" => NodeExecutionStatus::Pending,
                "retry" => NodeExecutionStatus::Retry,
                _ => NodeExecutionStatus::Success,
            };

            // Parse callbacks if present
            let callbacks: Vec<CallbackRecord> = result.get("callbacks")
                .and_then(|c| c.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|c| {
                            let callback_type = c.get("type").and_then(|t| t.as_str()).unwrap_or("");
                            let request_id = c.get("request_id").and_then(|r| r.as_str()).unwrap_or("");
                            let params = c.get("params").cloned().unwrap_or(serde_json::json!({}));
                            let result = c.get("result").cloned().unwrap_or(serde_json::json!({}));
                            let duration_ms = c.get("duration_ms").and_then(|d| d.as_u64()).unwrap_or(0);

                            Some(CallbackRecord {
                                callback_type: callback_type.to_string(),
                                request_id: request_id.to_string(),
                                params,
                                result,
                                duration_ms,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();

            // Extract metadata
            let metadata = result.get("metadata").cloned().unwrap_or(serde_json::json!({}));

            // Extract actual result data
            let result_data = result.get("data").cloned().unwrap_or(result);

            Ok(NodeExecutionResult {
                node_id: node_id.to_string(),
                result: result_data,
                status,
                metadata,
                duration_ms: 0, // Set by caller
                callbacks,
            })
        } else if response.is_error() {
            let error = response.error.clone().unwrap_or_else(|| {
                JsonRpcError::internal_error("Unknown error")
            });

            if error.code == ErrorCode::TIMEOUT_ERROR || error.code == ErrorCode::TRANSPORT_ERROR {
                Err(NodeExecutorError::Timeout {
                    node_id: node_id.to_string(),
                    timeout_ms: self.config.timeout_ms,
                })
            } else if error.code == ErrorCode::CALLBACK_ERROR {
                Err(NodeExecutorError::CallbackFailed {
                    node_id: node_id.to_string(),
                    callback_type: "unknown".to_string(),
                    message: error.message,
                })
            } else {
                Err(NodeExecutorError::ExecutionFailed {
                    node_id: node_id.to_string(),
                    message: error.message,
                    data: error.data,
                })
            }
        } else {
            Err(NodeExecutorError::InvalidResponse(
                "Response has neither result nor error".to_string()
            ))
        }
    }

    /// Register a transport connection for a service
    pub async fn register_connection(
        &self,
        service_id: ServiceId,
        _transport: StdioTransport,
    ) {
        let connection = Arc::new(NodeTransportConnection::new(service_id.clone()));
        *connection.connected.write().await = true;

        let mut connections = self.connections.write().await;
        connections.insert(service_id, connection);
    }

    /// Remove a transport connection
    pub async fn remove_connection(&self, service_id: &ServiceId) {
        let mut connections = self.connections.write().await;
        if let Some(conn) = connections.get(service_id) {
            *conn.connected.write().await = false;
        }
        connections.remove(service_id);
    }

    /// Check if a service is connected
    pub async fn is_connected(&self, service_id: &ServiceId) -> bool {
        let connections = self.connections.read().await;
        match connections.get(service_id) {
            Some(c) => c.is_connected().await,
            None => false,
        }
    }

    /// Handle a callback request from a node
    ///
    /// This is called when a node requests AI or tool execution.
    pub async fn handle_callback(
        &self,
        callback_type: &str,
        request_id: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, NodeExecutorError> {
        if !self.config.enable_callbacks {
            return Err(NodeExecutorError::CallbackFailed {
                node_id: "unknown".to_string(),
                callback_type: callback_type.to_string(),
                message: "Callbacks are disabled".to_string(),
            });
        }

        match callback_type {
            "ai" => self.handle_ai_callback(request_id, params).await,
            "tool" => self.handle_tool_callback(request_id, params).await,
            "context" => self.handle_context_callback(request_id, params).await,
            _ => Err(NodeExecutorError::CallbackFailed {
                node_id: "unknown".to_string(),
                callback_type: callback_type.to_string(),
                message: format!("Unknown callback type: {}", callback_type),
            }),
        }
    }

    /// Handle AI callback
    async fn handle_ai_callback(
        &self,
        _request_id: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, NodeExecutorError> {
        // In real implementation, this would call the AI provider
        // For now, return a placeholder
        let prompt = params.get("prompt").and_then(|p| p.as_str()).unwrap_or("");
        Ok(serde_json::json!({
            "response": format!("AI processing: {}", prompt),
            "model": "placeholder"
        }))
    }

    /// Handle tool callback
    async fn handle_tool_callback(
        &self,
        _request_id: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, NodeExecutorError> {
        // In real implementation, this would execute the tool
        let tool_name = params.get("tool_name").and_then(|t| t.as_str()).unwrap_or("");
        Ok(serde_json::json!({
            "result": format!("Tool {} executed", tool_name)
        }))
    }

    /// Handle context callback
    async fn handle_context_callback(
        &self,
        _request_id: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, NodeExecutorError> {
        // In real implementation, this would access workflow context
        let key = params.get("key").and_then(|k| k.as_str()).unwrap_or("");
        let operation = params.get("operation").and_then(|o| o.as_str()).unwrap_or("get");

        Ok(serde_json::json!({
            "key": key,
            "operation": operation,
            "value": null
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_execution_config_defaults() {
        let config = NodeExecutionConfig::default();
        assert_eq!(config.timeout_ms, 60_000);
        assert_eq!(config.max_retries, 3);
        assert!(config.enable_callbacks);
    }

    #[test]
    fn test_node_execution_status() {
        assert!(NodeExecutionStatus::Success.is_success());
        assert!(NodeExecutionStatus::Failed.is_failure());
        assert!(NodeExecutionStatus::Skipped.is_skipped());
        assert!(!NodeExecutionStatus::Pending.is_failure());
    }

    #[test]
    fn test_node_execution_config_builder() {
        let config = NodeExecutionConfig::new(5000)
            .max_retries(5)
            .no_callbacks()
            .callback_timeout(10000);

        assert_eq!(config.timeout_ms, 5000);
        assert_eq!(config.max_retries, 5);
        assert!(!config.enable_callbacks);
        assert_eq!(config.callback_timeout_ms, 10000);
    }

    #[test]
    fn test_callback_record() {
        let record = CallbackRecord {
            callback_type: "ai".to_string(),
            request_id: "cb-001".to_string(),
            params: serde_json::json!({"prompt": "test"}),
            result: serde_json::json!({"response": "ok"}),
            duration_ms: 100,
        };

        assert_eq!(record.callback_type, "ai");
        assert_eq!(record.duration_ms, 100);
    }
}