//! Tool Executor Implementation
//!
//! Executes tool calls via JSON-RPC transport with timeout and retry support.
//! Handles request/response correlation and error handling.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;

use crate::matrixrpc::{
    ErrorCode, JsonRpcError, JsonRpcId, JsonRpcResponse,
    ServiceId, ServiceStatus, RegistryService,
};
use crate::matrixrpc::transport::{StdioTransport, TransportConfig as TransportSettings};
use crate::matrixrpc::router::{ToolRouter, ToolRouteResult, ToolRouterError};

/// Error type for tool execution operations
#[derive(Debug, thiserror::Error)]
pub enum ToolExecutorError {
    /// Transport error during execution
    #[error("Transport error: {0}")]
    TransportError(String),

    /// Execution timeout
    #[error("Tool '{tool}' execution timed out after {timeout_ms}ms")]
    Timeout { tool: String, timeout_ms: u64 },

    /// Retry exhausted
    #[error("Tool '{tool}' execution failed after {attempts} attempts")]
    RetryExhausted { tool: String, attempts: u32, last_error: String },

    /// Service not connected
    #[error("Service '{0}' is not connected")]
    ServiceNotConnected(ServiceId),

    /// Invalid response
    #[error("Invalid response from service: {0}")]
    InvalidResponse(String),

    /// Tool execution failed
    #[error("Tool '{tool}' execution failed: {message}")]
    ExecutionFailed { tool: String, message: String, data: Option<serde_json::Value> },

    /// Routing error
    #[error("Routing error: {0}")]
    RoutingError(#[from] ToolRouterError),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Retry strategy configuration
#[derive(Debug, Clone)]
pub struct RetryStrategy {
    /// Maximum retry attempts
    pub max_attempts: u32,
    /// Base retry interval in milliseconds
    pub base_interval_ms: u64,
    /// Backoff strategy
    pub backoff: BackoffStrategy,
}

impl Default for RetryStrategy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_interval_ms: 1000,
            backoff: BackoffStrategy::Exponential,
        }
    }
}

impl RetryStrategy {
    /// Create a new retry strategy
    pub fn new(max_attempts: u32, base_interval_ms: u64) -> Self {
        Self {
            max_attempts,
            base_interval_ms,
            backoff: BackoffStrategy::Exponential,
        }
    }

    /// No retries
    pub fn none() -> Self {
        Self {
            max_attempts: 1,
            base_interval_ms: 0,
            backoff: BackoffStrategy::Fixed,
        }
    }

    /// Fixed interval retries
    pub fn fixed(max_attempts: u32, interval_ms: u64) -> Self {
        Self {
            max_attempts,
            base_interval_ms: interval_ms,
            backoff: BackoffStrategy::Fixed,
        }
    }

    /// Linear backoff retries
    pub fn linear(max_attempts: u32, base_interval_ms: u64) -> Self {
        Self {
            max_attempts,
            base_interval_ms,
            backoff: BackoffStrategy::Linear,
        }
    }

    /// Get the delay for a given attempt
    pub fn get_delay_ms(&self, attempt: u32) -> u64 {
        if attempt >= self.max_attempts {
            return 0;
        }

        match self.backoff {
            BackoffStrategy::Fixed => self.base_interval_ms,
            BackoffStrategy::Linear => self.base_interval_ms * (attempt + 1) as u64,
            BackoffStrategy::Exponential => {
                self.base_interval_ms * 2u64.pow(attempt)
            }
        }
    }
}

/// Backoff strategy for retries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackoffStrategy {
    /// Fixed interval between retries
    Fixed,
    /// Linear increasing interval
    Linear,
    /// Exponential increasing interval
    Exponential,
}

/// Execution configuration
#[derive(Debug, Clone)]
pub struct ExecutionConfig {
    /// Timeout for individual execution (milliseconds)
    pub timeout_ms: u64,
    /// Retry strategy
    pub retry: RetryStrategy,
    /// Transport settings
    pub transport: TransportSettings,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 30_000,
            retry: RetryStrategy::default(),
            transport: TransportSettings::default(),
        }
    }
}

impl ExecutionConfig {
    /// Create a new execution config
    pub fn new(timeout_ms: u64) -> Self {
        Self {
            timeout_ms,
            ..Default::default()
        }
    }

    /// Set retry strategy
    pub fn retry(mut self, retry: RetryStrategy) -> Self {
        self.retry = retry;
        self
    }

    /// Set transport settings
    pub fn transport(mut self, transport: TransportSettings) -> Self {
        self.transport = transport;
        self
    }

    /// No retries, single attempt
    pub fn no_retry(timeout_ms: u64) -> Self {
        Self {
            timeout_ms,
            retry: RetryStrategy::none(),
            transport: TransportSettings::default(),
        }
    }
}

/// Transport connection wrapper
#[derive(Debug)]
#[allow(dead_code)]
struct TransportConnection {
    /// Service ID for this connection
    service_id: ServiceId,
    /// Connection status
    connected: RwLock<bool>,
}

impl TransportConnection {
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

/// Tool Executor
///
/// Executes tool calls through the transport layer with support for
/// timeout, retry, and proper error handling.
#[derive(Debug)]
pub struct ToolExecutor {
    /// Tool router for finding services
    router: Arc<ToolRouter>,
    /// Registry service for status checks
    registry: Arc<RegistryService>,
    /// Execution configuration
    config: ExecutionConfig,
    /// Active transport connections (service_id -> connection)
    connections: RwLock<HashMap<ServiceId, Arc<TransportConnection>>>,
}

use std::collections::HashMap;

impl ToolExecutor {
    /// Create a new tool executor
    pub fn new(router: Arc<ToolRouter>, registry: Arc<RegistryService>) -> Self {
        Self {
            router,
            registry,
            config: ExecutionConfig::default(),
            connections: RwLock::new(HashMap::new()),
        }
    }

    /// Create with custom configuration
    pub fn with_config(
        router: Arc<ToolRouter>,
        registry: Arc<RegistryService>,
        config: ExecutionConfig,
    ) -> Self {
        Self {
            router,
            registry,
            config,
            connections: RwLock::new(HashMap::new()),
        }
    }

    /// Execute a tool call
    ///
    /// Routes the call to the appropriate service and executes it
    /// with timeout and retry support.
    pub async fn execute(
        &self,
        tool_name: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, ToolExecutorError> {
        let request_id = JsonRpcId::generate();

        // Route the call
        let route_result = self.router
            .route(tool_name, params.clone(), request_id.clone())
            .await?;

        // Get timeout for this tool
        let tool_timeout = self.router.get_timeout(tool_name).await;

        // Execute with retry
        self.execute_with_retry(route_result, tool_timeout).await
    }

    /// Execute a tool call with a specific request ID
    pub async fn execute_with_id(
        &self,
        tool_name: &str,
        params: serde_json::Value,
        request_id: JsonRpcId,
    ) -> Result<serde_json::Value, ToolExecutorError> {
        // Route the call
        let route_result = self.router
            .route(tool_name, params.clone(), request_id.clone())
            .await?;

        // Get timeout for this tool
        let tool_timeout = self.router.get_timeout(tool_name).await;

        // Execute with retry
        self.execute_with_retry(route_result, tool_timeout).await
    }

    /// Execute with retry strategy
    async fn execute_with_retry(
        &self,
        route_result: ToolRouteResult,
        timeout_ms: u64,
    ) -> Result<serde_json::Value, ToolExecutorError> {
        let mut attempts = 0;
        let mut last_error: Option<String> = None;

        while attempts < self.config.retry.max_attempts {
            attempts += 1;

            // Get delay before this attempt (skip delay for first attempt)
            if attempts > 1 {
                let delay_ms = self.config.retry.get_delay_ms(attempts - 1);
                if delay_ms > 0 {
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                }
            }

            // Check service status
            let service = self.registry.get(&route_result.service_id).await;
            match service {
                Some(s) if s.status == ServiceStatus::Running => {
                    // Service is running, proceed
                }
                Some(s) => {
                    last_error = Some(format!("Service status: {:?}", s.status));
                    continue; // Retry
                }
                None => {
                    return Err(ToolExecutorError::ServiceNotConnected(route_result.service_id.clone()));
                }
            }

            // Execute the call
            let result = self.execute_single(&route_result, timeout_ms).await;

            match result {
                Ok(value) => return Ok(value),
                Err(ToolExecutorError::Timeout { .. }) => {
                    last_error = Some("Timeout".to_string());
                    // Retry on timeout
                }
                Err(ToolExecutorError::TransportError(_)) => {
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
        Err(ToolExecutorError::RetryExhausted {
            tool: route_result.tool_name.clone(),
            attempts,
            last_error: last_error.unwrap_or_else(|| "Unknown error".to_string()),
        })
    }

    /// Execute a single attempt
    async fn execute_single(
        &self,
        route_result: &ToolRouteResult,
        __timeout_ms: u64,
    ) -> Result<serde_json::Value, ToolExecutorError> {
        // Create the request
        let __request = self.router.create_tool_request(route_result.clone());

        // Get or create connection for this service
        let _connection = self.get_connection(&route_result.service_id).await?;

        // In real implementation, send request via transport
        // For now, return a placeholder since actual connection requires service config
        Err(ToolExecutorError::ServiceNotConnected(route_result.service_id.clone()))
    }

    /// Get or create a transport connection for a service
    async fn get_connection(
        &self,
        service_id: &ServiceId,
    ) -> Result<Arc<TransportConnection>, ToolExecutorError> {
        let connections = self.connections.read().await;

        if let Some(conn) = connections.get(service_id) {
            if conn.is_connected().await {
                return Ok(conn.clone());
            }
        }

        drop(connections);

        // Need to create new connection
        // In real implementation, this would spawn a child process or connect to TCP
        // For now, we return an error since actual connection requires service config
        Err(ToolExecutorError::ServiceNotConnected(service_id.clone()))
    }

    /// Process the response from tool execution
#[allow(dead_code)]
    fn process_response(
        &self,
        response: JsonRpcResponse,
        tool_name: &str,
    ) -> Result<serde_json::Value, ToolExecutorError> {
        if response.is_success() {
            // Return the result
            response.result.clone().ok_or_else(|| {
                ToolExecutorError::InvalidResponse("No result in success response".to_string())
            })
        } else if response.is_error() {
            // Extract error details
            let error = response.error.clone().unwrap_or_else(|| {
                JsonRpcError::internal_error("Unknown error")
            });

            // Check if it's a retriable error
            if error.code == ErrorCode::TIMEOUT_ERROR || error.code == ErrorCode::TRANSPORT_ERROR {
                Err(ToolExecutorError::Timeout {
                    tool: tool_name.to_string(),
                    timeout_ms: self.config.timeout_ms,
                })
            } else {
                Err(ToolExecutorError::ExecutionFailed {
                    tool: tool_name.to_string(),
                    message: error.message,
                    data: error.data,
                })
            }
        } else {
            Err(ToolExecutorError::InvalidResponse(
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
        let connection = Arc::new(TransportConnection::new(service_id.clone()));
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_config_defaults() {
        let config = ExecutionConfig::default();
        assert_eq!(config.timeout_ms, 30_000);
        assert_eq!(config.retry.max_attempts, 3);
    }

    #[test]
    fn test_retry_strategy_delays() {
        let fixed = RetryStrategy::fixed(3, 1000);
        assert_eq!(fixed.get_delay_ms(0), 1000);
        assert_eq!(fixed.get_delay_ms(1), 1000);

        let linear = RetryStrategy::linear(3, 1000);
        assert_eq!(linear.get_delay_ms(0), 1000);
        assert_eq!(linear.get_delay_ms(1), 2000);

        let exponential = RetryStrategy::new(3, 1000);
        assert_eq!(exponential.get_delay_ms(0), 1000);
        assert_eq!(exponential.get_delay_ms(1), 2000);
        assert_eq!(exponential.get_delay_ms(2), 4000);
    }

    #[test]
    fn test_retry_strategy_none() {
        let none = RetryStrategy::none();
        assert_eq!(none.max_attempts, 1);
        assert_eq!(none.get_delay_ms(0), 0);
    }

    #[test]
    fn test_execution_config_no_retry() {
        let config = ExecutionConfig::no_retry(5000);
        assert_eq!(config.timeout_ms, 5000);
        assert_eq!(config.retry.max_attempts, 1);
    }
}