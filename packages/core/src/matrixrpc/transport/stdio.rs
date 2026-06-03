//! Stdio Transport for MatrixRPC
//!
//! Provides transport over stdin/stdout using Content-Length framed messages.
//! Used for parent-child process communication (e.g., LSP/MCP style).
//!
//! # Example
//!
//! ```no_run
//! use matrixcode_core::matrixrpc::transport::{StdioTransport, Transport};
//! use matrixcode_core::matrixrpc::protocol::JsonRpcRequest;
//!
//! #[tokio::main]
//! async fn main() -> std::io::Result<()> {
//!     let mut transport = StdioTransport::new();
//!
//!     // Send a request
//!     let request = JsonRpcRequest::new("initialize").into();
//!     transport.send(&request).await?;
//!
//!     // Receive a response
//!     if let Some(response) = transport.receive().await? {
//!         println!("Received: {:?}", response);
//!     }
//!
//!     Ok(())
//! }
//! ```

use std::io;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use super::{FrameCodec, Transport, TransportConfig};
use crate::matrixrpc::protocol::JsonRpcMessage;

/// Stdio transport for JSON-RPC communication
///
/// Supports two modes:
/// 1. Native mode: Uses process stdin/stdout directly
/// 2. Child mode: Communicates with a spawned child process
pub struct StdioTransport {
    /// Reader for incoming messages
    reader: Option<BufReader<Box<dyn AsyncRead + Send + Unpin>>>,
    /// Writer for outgoing messages
    writer: Option<Box<dyn AsyncWrite + Send + Unpin>>,
    /// Frame codec for encoding/decoding
    codec: FrameCodec,
    /// Configuration
    config: TransportConfig,
    /// Closed flag
    closed: bool,
    /// Optional child process (if spawned)
    child: Option<Child>,
    /// Read buffer for partial message assembly
    read_buffer: Vec<u8>,
}

impl StdioTransport {
    /// Create a new stdio transport using stdin/stdout
    ///
    /// This is used for server-side communication where the process
    /// reads from stdin and writes to stdout.
    pub fn new() -> Self {
        Self::with_config(TransportConfig::default())
    }

    /// Create a new stdio transport with custom configuration
    pub fn with_config(config: TransportConfig) -> Self {
        Self {
            reader: None,
            writer: None,
            codec: FrameCodec::with_max_size(config.max_message_size),
            config,
            closed: false,
            child: None,
            read_buffer: Vec::new(),
        }
    }

    /// Create a stdio transport from async read/write streams
    ///
    /// Wraps the reader in a BufReader for efficient reading.
    pub fn from_streams<R, W>(reader: R, writer: W, config: TransportConfig) -> Self
    where
        R: AsyncRead + Send + Unpin + 'static,
        W: AsyncWrite + Send + Unpin + 'static,
    {
        Self {
            reader: Some(BufReader::new(Box::new(reader))),
            writer: Some(Box::new(writer)),
            codec: FrameCodec::with_max_size(config.max_message_size),
            config,
            closed: false,
            child: None,
            read_buffer: Vec::new(),
        }
    }

    /// Spawn a child process and create a transport to communicate with it
    ///
    /// The child process should implement the JSON-RPC protocol over stdio.
    pub async fn spawn_child(command: &mut Command) -> io::Result<Self> {
        Self::spawn_child_with_config(command, TransportConfig::default()).await
    }

    /// Spawn a child process with custom configuration
    pub async fn spawn_child_with_config(
        command: &mut Command,
        config: TransportConfig,
    ) -> io::Result<Self> {
        let mut child = command
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "Failed to open stdin"))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "Failed to open stdout"))?;

        Ok(Self {
            reader: Some(BufReader::new(Box::new(stdout))),
            writer: Some(Box::new(stdin)),
            codec: FrameCodec::with_max_size(config.max_message_size),
            config,
            closed: false,
            child: Some(child),
            read_buffer: Vec::new(),
        })
    }

    /// Initialize with tokio stdin/stdout
    ///
    /// Must be called within a tokio runtime context.
    pub fn with_tokio_stdio() -> Self {
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        Self::from_streams(stdin, stdout, TransportConfig::default())
    }

    /// Get the child process if this transport was created via `spawn_child`
    pub fn child(&mut self) -> Option<&mut Child> {
        self.child.as_mut()
    }

    /// Check if there's a child process and if it's still running
    pub fn is_child_running(&mut self) -> bool {
        if let Some(child) = &mut self.child {
            child.try_wait().ok().flatten().is_none()
        } else {
            false
        }
    }

    /// Wait for the child process to exit and return its status
    pub async fn wait_child(&mut self) -> io::Result<Option<std::process::ExitStatus>> {
        if let Some(child) = &mut self.child {
            child.wait().await.map(Some)
        } else {
            Ok(None)
        }
    }
}

impl Default for StdioTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Transport for StdioTransport {
    async fn send(&mut self, message: &JsonRpcMessage) -> io::Result<()> {
        if self.closed {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "Transport is closed",
            ));
        }

        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "No writer available"))?;

        // Encode the message with Content-Length frame
        let frame = self.codec.encode(message)?;

        // Write with optional timeout
        if self.config.write_timeout_ms > 0 {
            let timeout_duration = std::time::Duration::from_millis(self.config.write_timeout_ms);
            tokio::time::timeout(timeout_duration, async {
                writer.write_all(&frame).await?;
                writer.flush().await
            })
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "Write timeout"))?
            .map_err(|e| io::Error::new(io::ErrorKind::BrokenPipe, e))?;
        } else {
            writer.write_all(&frame).await?;
            writer.flush().await?;
        }

        Ok(())
    }

    async fn receive(&mut self) -> io::Result<Option<JsonRpcMessage>> {
        if self.closed {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "Transport is closed",
            ));
        }

        let reader = self
            .reader
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "No reader available"))?;

        // Try to parse a complete message from existing buffer first
        if !self.read_buffer.is_empty() {
            if let (_, Some(message)) = self.codec.decode_from_buffer(&self.read_buffer)? {
                // Remove consumed data
                self.read_buffer.clear();
                return Ok(Some(message));
            }
        }

        // Read more data
        let mut temp_buf = vec![0u8; 8192];
        let bytes_read: usize = if self.config.read_timeout_ms > 0 {
            let timeout_duration = std::time::Duration::from_millis(self.config.read_timeout_ms);
            tokio::time::timeout(timeout_duration, reader.read(&mut temp_buf))
                .await
                .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "Read timeout"))??
        } else {
            reader.read(&mut temp_buf).await?
        };

        if bytes_read == 0 {
            // EOF
            return Ok(None);
        }

        // Append to read buffer
        self.read_buffer.extend_from_slice(&temp_buf[..bytes_read]);

        // Check max message size
        if self.read_buffer.len() > self.config.max_message_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Buffer size {} exceeds maximum {}",
                    self.read_buffer.len(),
                    self.config.max_message_size
                ),
            ));
        }

        // Try to parse
        let (remaining, message) = self.codec.decode_from_buffer(&self.read_buffer)?;

        // Update buffer with remaining data
        self.read_buffer = remaining.to_vec();

        Ok(message)
    }

    async fn close(&mut self) -> io::Result<()> {
        if self.closed {
            return Ok(());
        }

        // Flush and close writer
        if let Some(mut writer) = self.writer.take() {
            let _ = writer.shutdown().await;
        }

        // Close reader
        self.reader.take();

        // Kill child process if we own one
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
        }

        self.closed = true;
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.closed
    }
}

/// A thread-safe wrapper around StdioTransport for concurrent access
///
/// Uses Arc<Mutex<>> to allow sharing between multiple tasks.
pub type SharedStdioTransport = Arc<Mutex<StdioTransport>>;

/// Create a shared stdio transport
pub fn shared_stdio_transport() -> SharedStdioTransport {
    Arc::new(Mutex::new(StdioTransport::with_tokio_stdio()))
}

/// Create a shared stdio transport with custom config
pub fn shared_stdio_transport_with_config(config: TransportConfig) -> SharedStdioTransport {
    Arc::new(Mutex::new(StdioTransport::from_streams(
        tokio::io::stdin(),
        tokio::io::stdout(),
        config,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matrixrpc::protocol::{JsonRpcError, JsonRpcId, JsonRpcRequest, JsonRpcResponse};
    use serde_json::json;
    use tokio::io::{self, AsyncReadExt};

    #[tokio::test]
    async fn test_send_and_receive() {
        // Create a duplex stream - one end for transport, one for reading
        let (server_read, client_write) = io::duplex(1024);
        // Another duplex for the read side (not used in this test)
        let (client_read, _server_write) = io::duplex(1024);

        // Create transport from client side - pass ownership
        let mut transport = StdioTransport::from_streams(
            client_read,
            client_write,
            TransportConfig::default(),
        );

        // Client sends request
        let request = JsonRpcMessage::Request(JsonRpcRequest::new("test_method"));
        transport.send(&request).await.unwrap();

        // Read on server side - use a separate task
        let read_task = tokio::spawn(async move {
            let mut reader = server_read;
            let mut buf = vec![0u8; 1024];
            let n = reader.read(&mut buf).await.unwrap();
            let frame = String::from_utf8_lossy(&buf[..n]);
            assert!(frame.contains("Content-Length:"));
            assert!(frame.contains("\"method\":\"test_method\""));
        });

        read_task.await.unwrap();
    }

    #[tokio::test]
    async fn test_close_transport() {
        let (client_read, client_write) = io::duplex(1024);
        let mut transport =
            StdioTransport::from_streams(client_read, client_write, TransportConfig::default());

        assert!(!transport.is_closed());

        transport.close().await.unwrap();
        assert!(transport.is_closed());

        // Sending after close should fail
        let request = JsonRpcMessage::Request(JsonRpcRequest::new("test"));
        let result = transport.send(&request).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_transport_config() {
        let config = TransportConfig::new()
            .max_message_size(1024 * 1024)
            .read_timeout(5000)
            .write_timeout(10000);

        let transport = StdioTransport::with_config(config.clone());
        assert_eq!(transport.config.max_message_size, 1024 * 1024);
        assert_eq!(transport.config.read_timeout_ms, 5000);
        assert_eq!(transport.config.write_timeout_ms, 10000);
    }

    #[tokio::test]
    async fn test_encode_decode_roundtrip() {
        let (read, write) = io::duplex(4096);

        // Create transport to send
        let mut transport1 = StdioTransport::from_streams(
            tokio::io::empty(), // No input
            write,
            TransportConfig::default(),
        );

        // Send message
        let request = JsonRpcMessage::Request(
            JsonRpcRequest::with_id("test_method", 42).params(json!({"arg": "value"})),
        );
        transport1.send(&request).await.unwrap();

        // Read on the other side
        let mut transport2 = StdioTransport::from_streams(
            read,
            tokio::io::sink(), // Discard output
            TransportConfig::default(),
        );

        let received = transport2.receive().await.unwrap();
        assert!(received.is_some());
        let msg = received.unwrap();
        assert!(msg.is_request());
        assert_eq!(msg.as_request().unwrap().method, "test_method");
    }

    // ==================== Additional Edge Case Tests ====================

    #[tokio::test]
    async fn test_receive_on_closed_transport() {
        let (client_read, client_write) = io::duplex(1024);
        let mut transport =
            StdioTransport::from_streams(client_read, client_write, TransportConfig::default());

        transport.close().await.unwrap();

        // Receiving after close should fail
        let result = transport.receive().await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::BrokenPipe);
    }

    #[tokio::test]
    async fn test_send_without_writer() {
        let mut transport = StdioTransport::with_config(TransportConfig::default());
        // transport has no reader/writer set

        let request = JsonRpcMessage::Request(JsonRpcRequest::new("test"));
        let result = transport.send(&request).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::BrokenPipe);
    }

    #[tokio::test]
    async fn test_receive_without_reader() {
        let mut transport = StdioTransport::with_config(TransportConfig::default());
        // transport has no reader/writer set

        let result = transport.receive().await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::BrokenPipe);
    }

    #[tokio::test]
    async fn test_double_close() {
        let (client_read, client_write) = io::duplex(1024);
        let mut transport =
            StdioTransport::from_streams(client_read, client_write, TransportConfig::default());

        // Close twice should be idempotent
        transport.close().await.unwrap();
        assert!(transport.is_closed());
        transport.close().await.unwrap();
        assert!(transport.is_closed());
    }

    #[tokio::test]
    async fn test_send_and_receive_response() {
        // Create a pair of connected duplex streams
        let (_server_read, client_write) = io::duplex(4096);
        let (client_read, server_write) = io::duplex(4096);

        // Client transport - pass ownership of streams
        let mut client_transport = StdioTransport::from_streams(
            client_read,
            client_write,
            TransportConfig::default(),
        );

        // Server sends a response using the codec directly
        let response =
            JsonRpcMessage::Response(JsonRpcResponse::success(1, json!({"status": "ok"})));
        let frame = FrameCodec::new().encode(&response).unwrap();

        // Use a separate task to write from server side
        let write_task = tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            let mut writer = server_write;
            writer.write_all(&frame).await.unwrap();
            writer.flush().await.unwrap();
            writer
        });

        // Client receives the response
        let received = client_transport.receive().await.unwrap();
        assert!(received.is_some());
        let msg = received.unwrap();
        assert!(msg.is_response());
        assert!(msg.as_response().unwrap().is_success());

        // Wait for write task to complete
        write_task.await.unwrap();
    }

    #[tokio::test]
    async fn test_multiple_messages_codec_roundtrip() {
        // Test encoding/decoding multiple messages using the codec directly
        let codec = FrameCodec::new();
        let mut buffer = Vec::new();

        // Encode multiple messages
        for i in 0..5 {
            let request = JsonRpcMessage::Request(
                JsonRpcRequest::with_id("test", i).params(json!({"index": i})),
            );
            let frame = codec.encode(&request).unwrap();
            buffer.extend_from_slice(&frame);
        }

        // Decode all messages sequentially
        for i in 0..5 {
            let (remaining, message) = codec.decode_from_buffer(&buffer).unwrap();
            assert!(message.is_some());
            let msg = message.unwrap();
            assert!(msg.is_request());
            assert_eq!(msg.as_request().unwrap().id, Some(JsonRpcId::Number(i)));
            buffer = remaining.to_vec();
        }

        assert!(buffer.is_empty());
    }

    #[tokio::test]
    async fn test_receive_eof() {
        // Use an empty stream to simulate EOF
        let (read, write) = io::duplex(1024);

        let mut transport =
            StdioTransport::from_streams(read, write, TransportConfig::default());

        // Close the write side to simulate EOF
        drop(transport.writer.take());

        // Receiving from empty stream should return Ok(None) for EOF
        let result = transport.receive().await;
        // With empty buffer and EOF, should return Ok(None)
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_default_transport() {
        let transport = StdioTransport::default();
        assert!(!transport.is_closed());
        assert!(transport.child.is_none());
    }

    #[test]
    fn test_child_process_methods() {
        let mut transport = StdioTransport::new();
        assert!(transport.child().is_none());
        assert!(!transport.is_child_running());
    }

    #[tokio::test]
    async fn test_max_message_size_exceeded_on_receive() {
        // Send a message larger than the max size
        let large_params = "x".repeat(100);
        let request = JsonRpcMessage::Request(
            JsonRpcRequest::new("test").params(json!({"data": large_params})),
        );
        let frame = FrameCodec::new().encode(&request).unwrap();

        // Create a small max message size config
        let (read, write) = io::duplex(8192);

        // Write data in a separate task
        let frame_clone = frame.clone();
        let write_task = tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            let mut writer = write;
            writer.write_all(&frame_clone).await.unwrap();
            writer.flush().await.unwrap();
            writer
        });

        let mut small_transport = StdioTransport::from_streams(
            read,
            tokio::io::sink(),
            TransportConfig::new().max_message_size(10),
        );

        // Receive should fail with InvalidData
        let result = small_transport.receive().await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);

        write_task.await.unwrap();
    }

    #[test]
    fn test_send_notification_codec() {
        // Test notification encoding via codec (no async needed)
        let notification = JsonRpcMessage::Request(
            JsonRpcRequest::notification("log")
                .params(json!({"message": "hello"})),
        );

        let codec = FrameCodec::new();
        let frame = codec.encode(&notification).unwrap();
        let frame_str = String::from_utf8_lossy(&frame);

        assert!(frame_str.contains("Content-Length:"));
        assert!(frame_str.contains("\"method\":\"log\""));

        // Verify notification has no id in the JSON body
        let body_start = frame_str.find("\r\n\r\n").unwrap() + 4;
        let body = &frame_str[body_start..];
        let parsed: serde_json::Value = serde_json::from_str(body).unwrap();
        assert!(parsed.get("id").is_none());
    }

    #[test]
    fn test_send_error_response_codec() {
        // Test error response encoding via codec
        let error_response = JsonRpcMessage::Response(
            JsonRpcResponse::error(1, JsonRpcError::method_not_found("unknown")),
        );

        let codec = FrameCodec::new();
        let frame = codec.encode(&error_response).unwrap();
        let frame_str = String::from_utf8_lossy(&frame);

        assert!(frame_str.contains("\"error\""));
        assert!(frame_str.contains("Method 'unknown' not found"));
    }

    #[test]
    fn test_codec_roundtrip_with_string_id() {
        // Test with string ID
        let request = JsonRpcMessage::Request(
            JsonRpcRequest::with_id("test_method", "uuid-12345")
                .params(json!({"arg": "value"})),
        );

        let codec = FrameCodec::new();
        let frame = codec.encode(&request).unwrap();
        let (remaining, decoded) = codec.decode_from_buffer(&frame).unwrap();

        assert!(remaining.is_empty());
        assert!(decoded.is_some());
        let msg = decoded.unwrap();
        assert_eq!(msg.as_request().unwrap().id, Some(JsonRpcId::String("uuid-12345".to_string())));
    }
}
