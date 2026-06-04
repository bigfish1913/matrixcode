//! TCP Transport for MatrixRPC
//!
//! Provides TCP-based transport for JSON-RPC communication with external services.
//! Uses binary frame format: [4 bytes length][JSON payload]
//!
//! # Ports
//!
//! - Registry Port (9527): Accepts external service registration
//! - Callback Port (9528): Accepts callback requests from external services

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::net::{TcpListener as TokioTcpListener, TcpStream as TokioTcpStream, tcp::{OwnedReadHalf, OwnedWriteHalf}};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tokio::time::timeout;

use crate::matrixrpc::protocol::JsonRpcMessage;
use super::{Transport, TransportConfig};

/// Binary frame format: 4-byte length prefix + JSON payload
/// More efficient than Content-Length format for TCP communication
const FRAME_HEADER_SIZE: usize = 4;

/// TCP Transport implementation
///
/// Supports both client and server modes:
/// - Client mode: Connect to external service
/// - Server mode: Accept connections from external services
pub struct TcpTransport {
    /// Read half of the TCP stream
    reader: Arc<Mutex<Option<OwnedReadHalf>>>,
    /// Write half of the TCP stream
    writer: Arc<Mutex<Option<OwnedWriteHalf>>>,
    /// Transport configuration
    config: TransportConfig,
    /// Remote address (for logging/debugging)
    remote_addr: Option<SocketAddr>,
    /// Connection state
    is_closed: bool,
}

impl TcpTransport {
    /// Create a new TCP transport by connecting to an address
    pub async fn connect(addr: &str) -> io::Result<Self> {
        Self::connect_with_config(addr, TransportConfig::default()).await
    }

    /// Create a new TCP transport with custom configuration
    pub async fn connect_with_config(addr: &str, config: TransportConfig) -> io::Result<Self> {
        let stream = TokioTcpStream::connect(addr).await?;
        let remote_addr = stream.peer_addr().ok();
        let (reader, writer) = stream.into_split();

        Ok(Self {
            reader: Arc::new(Mutex::new(Some(reader))),
            writer: Arc::new(Mutex::new(Some(writer))),
            config,
            remote_addr,
            is_closed: false,
        })
    }

    /// Create a transport from an existing TcpStream (server mode)
    pub fn from_stream(stream: TokioTcpStream, config: TransportConfig) -> Self {
        let remote_addr = stream.peer_addr().ok();
        let (reader, writer) = stream.into_split();

        Self {
            reader: Arc::new(Mutex::new(Some(reader))),
            writer: Arc::new(Mutex::new(Some(writer))),
            config,
            remote_addr,
            is_closed: false,
        }
    }

    /// Get the remote address
    pub fn remote_addr(&self) -> Option<SocketAddr> {
        self.remote_addr
    }

    /// Encode message with binary frame format
    fn encode_frame(message: &JsonRpcMessage) -> io::Result<Vec<u8>> {
        let json = message.to_json().map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("JSON encode error: {}", e),
            )
        })?;

        let json_bytes = json.into_bytes();
        let length = json_bytes.len() as u32;

        // Create frame: 4-byte length + JSON
        let mut frame = Vec::with_capacity(FRAME_HEADER_SIZE + json_bytes.len());
        frame.extend_from_slice(&length.to_be_bytes());
        frame.extend(json_bytes);

        Ok(frame)
    }

    /// Decode message from binary frame
    async fn decode_frame(
        reader: &mut OwnedReadHalf,
        max_size: usize,
    ) -> io::Result<Option<JsonRpcMessage>> {
        // Read 4-byte length header
        let mut header_buf = [0u8; FRAME_HEADER_SIZE];
        match reader.read_exact(&mut header_buf).await {
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e),
        }

        let length = u32::from_be_bytes(header_buf) as usize;

        // Validate size
        if length > max_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Frame size {} exceeds maximum {}", length, max_size),
            ));
        }

        if length == 0 {
            return Ok(None);
        }

        // Read payload
        let mut payload_buf = vec![0u8; length];
        reader.read_exact(&mut payload_buf).await?;

        // Parse JSON
        let json_str = String::from_utf8(payload_buf).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("UTF-8 decode error: {}", e),
            )
        })?;

        let message = JsonRpcMessage::from_json(&json_str).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("JSON parse error: {}", e),
            )
        })?;

        Ok(Some(message))
    }
}

#[async_trait]
impl Transport for TcpTransport {
    async fn send(&mut self, message: &JsonRpcMessage) -> io::Result<()> {
        if self.is_closed {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "Transport is closed",
            ));
        }

        let writer_guard = self.writer.lock().await;
        let writer = writer_guard.as_ref().ok_or_else(|| {
            io::Error::new(io::ErrorKind::BrokenPipe, "No stream available")
        })?;

        let frame = Self::encode_frame(message)?;

        // Write with timeout (need to clone writer for timeout usage)
        // Since OwnedWriteHalf can't be cloned, we use a different approach
        let result = timeout(
            Duration::from_millis(self.config.write_timeout_ms),
            async {
                // We need mutable access, so we need to lock mutably
                drop(writer_guard);
                let mut writer_guard = self.writer.lock().await;
                let writer = writer_guard.as_mut().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::BrokenPipe, "No stream available")
                })?;
                writer.write_all(&frame).await
            }
        )
        .await;

        match result {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Write timeout",
            )),
        }
    }

    async fn receive(&mut self) -> io::Result<Option<JsonRpcMessage>> {
        if self.is_closed {
            return Ok(None);
        }

        // Read with timeout
        let read_result = timeout(
            Duration::from_millis(self.config.read_timeout_ms),
            async {
                let mut reader_guard = self.reader.lock().await;
                let reader = reader_guard.as_mut().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::BrokenPipe, "No stream available")
                })?;
                Self::decode_frame(reader, self.config.max_message_size).await
            }
        )
        .await;

        match read_result {
            Ok(Ok(message)) => Ok(message),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Read timeout",
            )),
        }
    }

    async fn close(&mut self) -> io::Result<()> {
        if self.is_closed {
            return Ok(());
        }

        self.is_closed = true;

        // Drop the stream by taking it out
        let mut reader_guard = self.reader.lock().await;
        let mut writer_guard = self.writer.lock().await;
        reader_guard.take();
        writer_guard.take();

        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.is_closed
    }
}

/// TCP Listener for accepting incoming connections
///
/// Used by Extension Gateway to accept external service registrations
/// and callback requests.
pub struct TcpListener {
    /// Tokio TCP listener
    listener: TokioTcpListener,
    /// Local address
    local_addr: SocketAddr,
    /// Transport config for accepted connections
    config: TransportConfig,
}

impl TcpListener {
    /// Create a new TCP listener on the specified port
    pub async fn bind(port: u16) -> io::Result<Self> {
        Self::bind_with_config(port, TransportConfig::default()).await
    }

    /// Create a new TCP listener with custom configuration
    pub async fn bind_with_config(port: u16, config: TransportConfig) -> io::Result<Self> {
        let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
        let listener = TokioTcpListener::bind(addr).await?;
        let local_addr = listener.local_addr()?;

        Ok(Self {
            listener,
            local_addr,
            config,
        })
    }

    /// Get the local address
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Accept a new connection
    ///
    /// Returns a TcpTransport for the accepted connection.
    pub async fn accept(&self) -> io::Result<TcpTransport> {
        let (stream, _addr) = self.listener.accept().await?;
        Ok(TcpTransport::from_stream(stream, self.config.clone()))
    }

    /// Get port number
    pub fn port(&self) -> u16 {
        self.local_addr.port()
    }
}

/// Registry Port (9527) - Accepts external service registration
pub const REGISTRY_PORT: u16 = 9527;

/// Callback Port (9528) - Accepts callback requests
pub const CALLBACK_PORT: u16 = 9528;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_frame_simple() {
        use crate::matrixrpc::protocol::{JsonRpcRequest, JsonRpcId};

        let request = JsonRpcRequest::with_id("test.method", JsonRpcId::String("test-1".to_string()))
            .params(serde_json::json!({"param": "value"}));
        let message = JsonRpcMessage::Request(request);

        let frame = TcpTransport::encode_frame(&message).unwrap();

        // Check frame structure
        assert!(frame.len() > FRAME_HEADER_SIZE);

        // Extract length from header
        let length = u32::from_be_bytes([
            frame[0], frame[1], frame[2], frame[3],
        ]);
        assert!(length > 0);
        assert_eq!(frame.len(), FRAME_HEADER_SIZE + length as usize);
    }

    #[test]
    fn test_tcp_config() {
        let config = TransportConfig::new()
            .max_message_size(1024)
            .read_timeout(5000);

        assert_eq!(config.max_message_size, 1024);
        assert_eq!(config.read_timeout_ms, 5000);
    }

    #[test]
    fn test_frame_header_size() {
        assert_eq!(FRAME_HEADER_SIZE, 4);
    }

    #[test]
    fn test_port_constants() {
        assert_eq!(REGISTRY_PORT, 9527);
        assert_eq!(CALLBACK_PORT, 9528);
    }
}