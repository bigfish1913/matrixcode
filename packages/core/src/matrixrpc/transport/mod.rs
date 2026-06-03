//! Transport Layer for MatrixRPC
//!
//! Provides transport abstraction for JSON-RPC communication.
//! Supports Stdio and TCP transports.

mod codec;
mod stdio;
mod tcp;

use std::io;

use async_trait::async_trait;

pub use codec::FrameCodec;
pub use stdio::StdioTransport;
pub use tcp::{TcpTransport, TcpListener, REGISTRY_PORT, CALLBACK_PORT};

use crate::matrixrpc::protocol::JsonRpcMessage;

/// Transport trait for JSON-RPC communication
///
/// Implementations provide bidirectional message transport with
/// proper framing and error handling.
///
/// Note: Only `Send` is required since async transports typically
/// operate in single-threaded async contexts.
#[async_trait]
pub trait Transport: Send {
    /// Send a message through the transport
    async fn send(&mut self, message: &JsonRpcMessage) -> io::Result<()>;

    /// Receive a message from the transport
    async fn receive(&mut self) -> io::Result<Option<JsonRpcMessage>>;

    /// Close the transport
    async fn close(&mut self) -> io::Result<()>;

    /// Check if the transport is closed
    fn is_closed(&self) -> bool;
}

/// Builder for configuring transport options
#[derive(Debug, Clone)]
pub struct TransportConfig {
    /// Maximum message size in bytes (default: 16MB)
    pub max_message_size: usize,
    /// Read timeout in milliseconds (default: 30000)
    pub read_timeout_ms: u64,
    /// Write timeout in milliseconds (default: 30000)
    pub write_timeout_ms: u64,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            max_message_size: 16 * 1024 * 1024, // 16MB
            read_timeout_ms: 30_000,            // 30 seconds
            write_timeout_ms: 30_000,           // 30 seconds
        }
    }
}

impl TransportConfig {
    /// Create a new transport config with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum message size
    pub fn max_message_size(mut self, size: usize) -> Self {
        self.max_message_size = size;
        self
    }

    /// Set read timeout in milliseconds
    pub fn read_timeout(mut self, ms: u64) -> Self {
        self.read_timeout_ms = ms;
        self
    }

    /// Set write timeout in milliseconds
    pub fn write_timeout(mut self, ms: u64) -> Self {
        self.write_timeout_ms = ms;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_config_defaults() {
        let config = TransportConfig::default();
        assert_eq!(config.max_message_size, 16 * 1024 * 1024);
        assert_eq!(config.read_timeout_ms, 30_000);
        assert_eq!(config.write_timeout_ms, 30_000);
    }

    #[test]
    fn test_transport_config_builder() {
        let config = TransportConfig::new()
            .max_message_size(1024)
            .read_timeout(5000)
            .write_timeout(10000);
        assert_eq!(config.max_message_size, 1024);
        assert_eq!(config.read_timeout_ms, 5000);
        assert_eq!(config.write_timeout_ms, 10000);
    }
}
