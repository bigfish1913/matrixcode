//! Frame Codec for JSON-RPC messages
//!
//! Implements Content-Length based framing (LSP/MCP style):
//! ```
//! Content-Length: <length>\r\n
//! \r\n
//! <json-payload>
//! ```
//!
//! This is a widely adopted format used by:
//! - Language Server Protocol (LSP)
//! - Model Context Protocol (MCP)
//! - Debug Adapter Protocol (DAP)

use std::io;

use crate::matrixrpc::protocol::JsonRpcMessage;

/// Frame codec for encoding/decoding JSON-RPC messages
///
/// Uses Content-Length header framing similar to LSP/MCP.
#[derive(Debug, Default)]
pub struct FrameCodec {
    /// Maximum allowed message size
    max_message_size: usize,
}

impl FrameCodec {
    /// Create a new frame codec with default settings
    pub fn new() -> Self {
        Self {
            max_message_size: 16 * 1024 * 1024, // 16MB default
        }
    }

    /// Create a frame codec with a custom max message size
    pub fn with_max_size(max_message_size: usize) -> Self {
        Self { max_message_size }
    }

    /// Encode a message into a framed byte vector
    ///
    /// Format: `Content-Length: <length>\r\n\r\n<json>`
    pub fn encode(&self, message: &JsonRpcMessage) -> io::Result<Vec<u8>> {
        let json = message.to_json().map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("JSON encode error: {}", e),
            )
        })?;

        let json_bytes = json.into_bytes();
        if json_bytes.len() > self.max_message_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Message size {} exceeds maximum {}",
                    json_bytes.len(),
                    self.max_message_size
                ),
            ));
        }

        let header = format!("Content-Length: {}\r\n\r\n", json_bytes.len());
        let mut frame = header.into_bytes();
        frame.extend(json_bytes);

        Ok(frame)
    }

    /// Encode a message and write to a writer
    pub fn encode_to_writer<W: std::io::Write>(
        &self,
        writer: &mut W,
        message: &JsonRpcMessage,
    ) -> io::Result<()> {
        let frame = self.encode(message)?;
        writer.write_all(&frame)?;
        writer.flush()?;
        Ok(())
    }

    /// Decode a message from a complete buffer
    ///
    /// Returns the remaining unconsumed bytes and the parsed message.
    /// Returns Ok((remaining, None)) if more data is needed.
    pub fn decode_from_buffer<'a>(
        &self,
        buffer: &'a [u8],
    ) -> io::Result<(&'a [u8], Option<JsonRpcMessage>)> {
        // Find end of headers
        let header_end = match find_header_end(buffer) {
            Some(pos) => pos,
            None => return Ok((buffer, None)), // Need more data
        };

        // Parse headers
        let header_str = std::str::from_utf8(&buffer[..header_end]).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid UTF-8 in headers: {}", e),
            )
        })?;

        let content_length = parse_content_length(header_str)?;

        // Check if we have enough data
        let body_start = header_end + 4; // Skip \r\n\r\n
        if buffer.len() < body_start + content_length {
            return Ok((buffer, None)); // Need more data
        }

        // Extract and parse the body
        let body = &buffer[body_start..body_start + content_length];
        let json_str = std::str::from_utf8(body).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid UTF-8 in body: {}", e),
            )
        })?;

        let message = JsonRpcMessage::from_json(json_str).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("JSON decode error: {}", e),
            )
        })?;

        // Return remaining buffer
        let remaining = &buffer[body_start + content_length..];
        Ok((remaining, Some(message)))
    }

    /// Get the maximum message size
    pub fn max_message_size(&self) -> usize {
        self.max_message_size
    }
}

/// Find the end of HTTP-like headers (double CRLF)
fn find_header_end(buffer: &[u8]) -> Option<usize> {
    let pattern = b"\r\n\r\n";
    if buffer.len() < 4 {
        return None;
    }

    for i in 0..=buffer.len() - 4 {
        if &buffer[i..i + 4] == pattern {
            return Some(i);
        }
    }
    None
}

/// Parse Content-Length from header string
fn parse_content_length(headers: &str) -> io::Result<usize> {
    for line in headers.lines() {
        let line = line.trim();
        if let Some((key, value)) = line.split_once(':') {
            if key.trim().eq_ignore_ascii_case("Content-Length") {
                let length: usize = value.trim().parse().map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Invalid Content-Length: {}", e),
                    )
                })?;
                return Ok(length);
            }
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "Missing Content-Length header",
    ))
}

/// Encode a single message to bytes (convenience function)
pub fn encode_message(message: &JsonRpcMessage) -> io::Result<Vec<u8>> {
    FrameCodec::new().encode(message)
}

/// Decode a single message from bytes (convenience function)
///
/// Expects the buffer to contain exactly one framed message (with headers)
pub fn decode_message_from_buffer(buffer: &[u8]) -> io::Result<(Vec<u8>, JsonRpcMessage)> {
    let codec = FrameCodec::new();
    let (remaining, message) = codec.decode_from_buffer(buffer)?;
    match message {
        Some(msg) => Ok((remaining.to_vec(), msg)),
        None => Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "Incomplete message",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_encode_message() {
        let request = JsonRpcMessage::Request(
            crate::matrixrpc::protocol::JsonRpcRequest::new("test_method")
                .params(json!({"key": "value"})),
        );

        let codec = FrameCodec::new();
        let frame = codec.encode(&request).unwrap();

        let frame_str = String::from_utf8_lossy(&frame);
        assert!(frame_str.starts_with("Content-Length:"));
        assert!(frame_str.contains("\r\n\r\n"));
        assert!(frame_str.contains("\"method\":\"test_method\""));
    }

    #[test]
    fn test_decode_message() {
        let json = r#"{"jsonrpc":"2.0","method":"test","id":1}"#;
        let frame = format!("Content-Length: {}\r\n\r\n{}", json.len(), json);

        let codec = FrameCodec::new();
        let (remaining, message) = codec.decode_from_buffer(frame.as_bytes()).unwrap();

        assert!(message.is_some());
        let msg = message.unwrap();
        assert!(msg.is_request());
        assert!(remaining.is_empty());
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let request = JsonRpcMessage::Request(
            crate::matrixrpc::protocol::JsonRpcRequest::with_id("test_method", 42)
                .params(json!({"arg": "value"})),
        );

        let codec = FrameCodec::new();
        let frame = codec.encode(&request).unwrap();

        let (_, decoded) = codec.decode_from_buffer(&frame).unwrap();
        let decoded = decoded.unwrap();

        assert_eq!(
            decoded.as_request().unwrap().method,
            request.as_request().unwrap().method
        );
    }

    #[test]
    fn test_max_message_size() {
        let codec = FrameCodec::with_max_size(10);
        let request =
            JsonRpcMessage::Request(crate::matrixrpc::protocol::JsonRpcRequest::new("test"));

        let result = codec.encode(&request);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err().kind(),
            io::ErrorKind::InvalidData
        ));
    }

    #[test]
    fn test_incomplete_message() {
        let json = r#"{"jsonrpc":"2.0","method":"test","id":1}"#;
        let partial_frame = format!("Content-Length: {}\r\n\r\n", json.len()); // Missing body

        let codec = FrameCodec::new();
        let result = codec.decode_from_buffer(partial_frame.as_bytes()).unwrap();

        assert!(result.1.is_none());
    }

    #[test]
    fn test_multiple_messages_in_buffer() {
        let json1 = r#"{"jsonrpc":"2.0","method":"test1","id":1}"#;
        let json2 = r#"{"jsonrpc":"2.0","method":"test2","id":2}"#;

        let codec = FrameCodec::new();
        let mut buffer = Vec::new();
        buffer.extend(
            codec
                .encode(&JsonRpcMessage::Request(
                    crate::matrixrpc::protocol::JsonRpcRequest::from_json(json1).unwrap(),
                ))
                .unwrap(),
        );
        buffer.extend(
            codec
                .encode(&JsonRpcMessage::Request(
                    crate::matrixrpc::protocol::JsonRpcRequest::from_json(json2).unwrap(),
                ))
                .unwrap(),
        );

        // Decode first message
        let (remaining1, msg1) = codec.decode_from_buffer(&buffer).unwrap();
        let msg1 = msg1.unwrap();
        assert_eq!(msg1.as_request().unwrap().method, "test1");

        // Decode second message
        let (_, msg2) = codec.decode_from_buffer(remaining1).unwrap();
        let msg2 = msg2.unwrap();
        assert_eq!(msg2.as_request().unwrap().method, "test2");
    }

    #[test]
    fn test_convenience_functions() {
        let request = JsonRpcMessage::Request(crate::matrixrpc::protocol::JsonRpcRequest::new(
            "test_method",
        ));

        let encoded = encode_message(&request).unwrap();
        let (_, decoded) = decode_message_from_buffer(&encoded).unwrap();

        assert!(decoded.is_request());
    }

    // ==================== Additional Edge Case Tests ====================

    #[test]
    fn test_decode_missing_content_length() {
        let json = r#"{"jsonrpc":"2.0","method":"test","id":1}"#;
        let frame = format!("Content-Type: application/json\r\n\r\n{}", json);

        let codec = FrameCodec::new();
        let result = codec.decode_from_buffer(frame.as_bytes());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("Missing Content-Length"));
    }

    #[test]
    fn test_decode_malformed_content_length() {
        let json = r#"{"jsonrpc":"2.0","method":"test","id":1}"#;
        let frame = format!("Content-Length: abc\r\n\r\n{}", json);

        let codec = FrameCodec::new();
        let result = codec.decode_from_buffer(frame.as_bytes());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("Invalid Content-Length"));
    }

    #[test]
    fn test_decode_negative_content_length() {
        let json = r#"{"jsonrpc":"2.0","method":"test","id":1}"#;
        let frame = format!("Content-Length: -1\r\n\r\n{}", json);

        let codec = FrameCodec::new();
        let result = codec.decode_from_buffer(frame.as_bytes());
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_case_insensitive_header() {
        let json = r#"{"jsonrpc":"2.0","method":"test","id":1}"#;
        // Test with different case variations
        for header in [
            "content-length",
            "CONTENT-LENGTH",
            "Content-length",
            "CONTENT-length",
        ] {
            let frame = format!("{}: {}\r\n\r\n{}", header, json.len(), json);
            let codec = FrameCodec::new();
            let (_, message) = codec.decode_from_buffer(frame.as_bytes()).unwrap();
            assert!(
                message.is_some(),
                "Failed to parse with header: {}",
                header
            );
        }
    }

    #[test]
    fn test_decode_with_extra_headers() {
        let json = r#"{"jsonrpc":"2.0","method":"test","id":1}"#;
        let frame = format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            json.len(),
            json
        );

        let codec = FrameCodec::new();
        let (_, message) = codec.decode_from_buffer(frame.as_bytes()).unwrap();
        assert!(message.is_some());
    }

    #[test]
    fn test_decode_zero_content_length() {
        let frame = "Content-Length: 0\r\n\r\n";

        let codec = FrameCodec::new();
        // Zero-length body should fail JSON parse
        let result = codec.decode_from_buffer(frame.as_bytes());
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_invalid_utf8_in_header() {
        // Invalid UTF8 in the header part (before \r\n\r\n)
        // The header parsing will fail when trying to convert to UTF8
        let invalid_bytes = b"Content-Length: \xFF\xFE\r\n\r\n{}";

        let codec = FrameCodec::new();
        let result = codec.decode_from_buffer(invalid_bytes);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("Invalid UTF-8"));
    }

    #[test]
    fn test_decode_invalid_json_body() {
        let invalid_json = r#"{"jsonrpc":"2.0","method":}"#; // Malformed JSON
        let frame = format!("Content-Length: {}\r\n\r\n{}", invalid_json.len(), invalid_json);

        let codec = FrameCodec::new();
        let result = codec.decode_from_buffer(frame.as_bytes());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("JSON decode error"));
    }

    #[test]
    fn test_decode_empty_buffer() {
        let codec = FrameCodec::new();
        let (remaining, message) = codec.decode_from_buffer(b"").unwrap();
        assert!(message.is_none());
        assert!(remaining.is_empty());
    }

    #[test]
    fn test_decode_partial_header() {
        let partial = b"Content-Length: 10";

        let codec = FrameCodec::new();
        let (remaining, message) = codec.decode_from_buffer(partial).unwrap();
        assert!(message.is_none());
        assert_eq!(remaining, partial);
    }

    #[test]
    fn test_decode_partial_body() {
        let json = r#"{"jsonrpc":"2.0","method":"test","id":1}"#;
        // Header says 100 bytes but only provides partial body
        let partial = format!("Content-Length: 100\r\n\r\n{}", json);

        let codec = FrameCodec::new();
        let (remaining, message) = codec
            .decode_from_buffer(partial.as_bytes())
            .unwrap();
        assert!(message.is_none());
        assert!(!remaining.is_empty());
    }

    #[test]
    fn test_encode_response_message() {
        let response = JsonRpcMessage::Response(
            crate::matrixrpc::protocol::JsonRpcResponse::success(1, json!({"result": "ok"})),
        );

        let codec = FrameCodec::new();
        let frame = codec.encode(&response).unwrap();
        let frame_str = String::from_utf8_lossy(&frame);

        assert!(frame_str.contains("\"result\":"));
        assert!(frame_str.contains("\"ok\""));
    }

    #[test]
    fn test_encode_error_response() {
        let error = JsonRpcMessage::Response(
            crate::matrixrpc::protocol::JsonRpcResponse::error(
                1,
                crate::matrixrpc::protocol::JsonRpcError::method_not_found("unknown"),
            ),
        );

        let codec = FrameCodec::new();
        let frame = codec.encode(&error).unwrap();
        let frame_str = String::from_utf8_lossy(&frame);

        assert!(frame_str.contains("\"error\""));
        assert!(frame_str.contains("Method 'unknown' not found"));
    }

    #[test]
    fn test_encode_batch_message() {
        let batch = JsonRpcMessage::Batch(vec![
            JsonRpcMessage::Request(
                crate::matrixrpc::protocol::JsonRpcRequest::new("method1"),
            ),
            JsonRpcMessage::Request(
                crate::matrixrpc::protocol::JsonRpcRequest::new("method2"),
            ),
        ]);

        let codec = FrameCodec::new();
        let frame = codec.encode(&batch).unwrap();
        let frame_str = String::from_utf8_lossy(&frame);

        assert!(frame_str.starts_with('[') || frame_str.contains("["));
        assert!(frame_str.contains("method1"));
        assert!(frame_str.contains("method2"));
    }

    #[test]
    fn test_encode_notification() {
        let notification = JsonRpcMessage::Request(
            crate::matrixrpc::protocol::JsonRpcRequest::notification("notify_event")
                .params(json!({"event": "test"})),
        );

        let codec = FrameCodec::new();
        let frame = codec.encode(&notification).unwrap();
        let frame_str = String::from_utf8_lossy(&frame);

        // Notification should have no id
        let body_start = frame_str.find("\r\n\r\n").unwrap() + 4;
        let body = &frame_str[body_start..];
        let parsed: serde_json::Value = serde_json::from_str(body).unwrap();
        assert!(parsed.get("id").is_none());
        assert_eq!(parsed["method"], "notify_event");
    }

    #[test]
    fn test_decode_with_trailing_data() {
        let json = r#"{"jsonrpc":"2.0","method":"test","id":1}"#;
        let frame = format!("Content-Length: {}\r\n\r\n{}extra_data", json.len(), json);

        let codec = FrameCodec::new();
        let (remaining, message) = codec.decode_from_buffer(frame.as_bytes()).unwrap();

        assert!(message.is_some());
        assert_eq!(remaining, b"extra_data");
    }

    #[test]
    fn test_decode_message_from_buffer_incomplete() {
        let partial = b"Content-Length: 100\r\n\r\n{}";
        let result = decode_message_from_buffer(partial);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn test_content_length_whitespace() {
        let json = r#"{"jsonrpc":"2.0","method":"test","id":1}"#;
        // Content-Length with extra whitespace
        let frame = format!("Content-Length:  {}  \r\n\r\n{}", json.len(), json);

        let codec = FrameCodec::new();
        let (_, message) = codec.decode_from_buffer(frame.as_bytes()).unwrap();
        assert!(message.is_some());
    }

    #[test]
    fn test_large_message_within_limit() {
        // Create a large message within the default 16MB limit
        let large_params = "x".repeat(1024 * 1024); // 1MB of data
        let request = JsonRpcMessage::Request(
            crate::matrixrpc::protocol::JsonRpcRequest::new("test").params(json!({"data": large_params})),
        );

        let codec = FrameCodec::new();
        let result = codec.encode(&request);
        assert!(result.is_ok());
    }
}
