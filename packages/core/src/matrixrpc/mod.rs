//! MatrixRPC - Dynamic Extension Protocol
//!
//! A JSON-RPC 2.0 based protocol for dynamic extension communication.
//! Supports both Stdio and TCP transport modes.
//!
//! # Architecture
//!
//! ```text
//! +----------------+     +----------------+     +----------------+
//! |   Extension    | <--|   Transport    |<-->|   Host/App     |
//! |   (Plugin)     | -->|   (Stdio/TCP)  | -->|   (Core)       |
//! +----------------+     +----------------+     +----------------+
//! ```
//!
//! # Components
//!
//! - `protocol`: JSON-RPC 2.0 types (Request, Response, Error, etc.)
//! - `transport`: Communication layer (Stdio, TCP)
//! - `config`: Configuration loading (matrixrpc.toml)
//! - `service`: Extension service data models
//! - `registry`: Service registration and discovery
//! - `lifecycle`: Connection and lifecycle management
//! - `router`: Tool and node routing logic
//! - `executor`: Tool and node execution with transport
//! - `callback`: AI, tool, and context callback handling
//! - `gateway`: Unified entry point coordinating all components
//!
//! # Example
//!
//! ```no_run
//! use matrixcode_core::matrixrpc::{ExtensionGateway, ServiceRegistrationRequest};
//! use matrixcode_core::matrixrpc::transport::{StdioTransport, Transport};
//! use matrixcode_core::matrixrpc::protocol::JsonRpcRequest;
//! use serde_json::json;
//!
//! #[tokio::main]
//! async fn main() -> std::io::Result<()> {
//!     // Create extension gateway
//!     let gateway = ExtensionGateway::new();
//!
//!     // Register a service
//!     let request = ServiceRegistrationRequest::new("my-plugin", "1.0.0");
//!     let service_id = gateway.register_service(request).await.unwrap();
//!
//!     // Create a stdio transport
//!     let mut transport = StdioTransport::new();
//!
//!     // Send a request
//!     let request = JsonRpcRequest::new("initialize")
//!         .params(json!({"version": "1.0"}));
//!     transport.send(&request.into()).await?;
//!
//!     // Receive response
//!     if let Some(response) = transport.receive().await? {
//!         println!("Response: {:?}", response);
//!     }
//!
//!     Ok(())
//! }
//! ```

pub mod callback;
pub mod config;
pub mod executor;
pub mod gateway;
pub mod lifecycle;
pub mod protocol;
pub mod registry;
pub mod router;
pub mod service;
pub mod transport;

// Re-export commonly used types
pub use config::{
    ConfigError, GlobalConfig, MatrixRpcConfig, ServiceDefinition, ServiceTransportType,
    CONFIG_FILE_NAME,
};
pub use executor::{
    ExecutionConfig, NodeExecutionConfig, NodeExecutionResult, NodeExecutionStatus, NodeExecutor,
    NodeExecutorError, RetryStrategy, ToolExecutor, ToolExecutorError,
};
pub use gateway::{
    ExtensionGateway, GatewayConfig, GatewayError, GatewayEvent, GatewayStats,
    ServiceRegistrationRequest,
};
pub use lifecycle::{LifecycleConfig, LifecycleError, LifecycleEvent, LifecycleManager};
pub use protocol::{
    ErrorCode, JsonRpcError, JsonRpcId, JsonRpcMessage, JsonRpcRequest, JsonRpcResponse,
};
pub use registry::{
    RegistryBuilder, RegistryError, RegistryService, RegistryStats, ServiceFilter,
};
pub use router::{
    NodeCapability, NodeContext, NodeDefinition, NodeRouter, NodeRouterError, NodeRouteResult,
    NodeType, ToolDefinition, ToolRouter, ToolRouterError, ToolRouteResult,
};
pub use service::{
    Capability, ExtensionService, RegistrationInfo, ServiceId, ServiceStatus, TransportConfig,
    TransportType,
};
pub use transport::{
    FrameCodec, StdioTransport, Transport, TransportConfig as TransportSettings,
};
pub use callback::{
    CallbackConfig, CallbackError, CallbackHandler, CallbackResult, CallbackType,
    AiCallbackHandler, AiCallbackRequest, AiCallbackResult,
    ToolCallbackHandler, ToolCallbackRequest, ToolCallbackResult,
    ContextCallbackHandler, ContextCallbackRequest, ContextCallbackResult, ContextOperation,
    SecurityValidator, SecurityError, TokenInfo, ValidationResult,
};
