//! JSON-RPC 2.0 Protocol Types
//!
//! Implements the core types for JSON-RPC 2.0 protocol.
//! Reference: https://www.jsonrpc.org/specification

use serde::{Deserialize, Serialize};
use std::fmt;

use super::ErrorCode;

/// JSON-RPC version string
pub const JSONRPC_VERSION: &str = "2.0";

/// JSON-RPC Request ID
///
/// The id member MUST be a String, Number, or Null value.
/// Null is allowed but discouraged for good practice.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcId {
    /// Numeric ID (commonly used)
    Number(i64),
    /// String ID (useful for UUIDs)
    String(String),
    /// Null ID (discouraged but allowed)
    Null,
}

impl fmt::Display for JsonRpcId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JsonRpcId::Number(n) => write!(f, "{}", n),
            JsonRpcId::String(s) => write!(f, "{}", s),
            JsonRpcId::Null => write!(f, "null"),
        }
    }
}

impl From<i64> for JsonRpcId {
    fn from(n: i64) -> Self {
        JsonRpcId::Number(n)
    }
}

impl From<String> for JsonRpcId {
    fn from(s: String) -> Self {
        JsonRpcId::String(s)
    }
}

impl From<&str> for JsonRpcId {
    fn from(s: &str) -> Self {
        JsonRpcId::String(s.to_string())
    }
}

impl Default for JsonRpcId {
    fn default() -> Self {
        JsonRpcId::Number(1)
    }
}

impl JsonRpcId {
    /// Generate a new unique string ID using UUID
    pub fn generate() -> Self {
        JsonRpcId::String(uuid::Uuid::new_v4().to_string())
    }
}

/// JSON-RPC 2.0 Request Object
///
/// A RPC call is represented by sending a Request object to a Server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    /// JSON-RPC version - MUST be exactly "2.0"
    pub jsonrpc: String,

    /// Method name to invoke
    pub method: String,

    /// Parameters for the method (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,

    /// Request ID (must be included for non-notification requests)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<JsonRpcId>,
}

impl JsonRpcRequest {
    /// Create a new request with a method name
    pub fn new(method: impl Into<String>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: method.into(),
            params: None,
            id: Some(JsonRpcId::default()),
        }
    }

    /// Create a new request with ID
    pub fn with_id(method: impl Into<String>, id: impl Into<JsonRpcId>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: method.into(),
            params: None,
            id: Some(id.into()),
        }
    }

    /// Add parameters to the request
    pub fn params(mut self, params: impl Into<serde_json::Value>) -> Self {
        self.params = Some(params.into());
        self
    }

    /// Create a notification (no response expected)
    pub fn notification(method: impl Into<String>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: method.into(),
            params: None,
            id: None,
        }
    }

    /// Create a notification with parameters
    pub fn notification_with_params(
        method: impl Into<String>,
        params: impl Into<serde_json::Value>,
    ) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: method.into(),
            params: Some(params.into()),
            id: None,
        }
    }

    /// Check if this is a notification (no response expected)
    pub fn is_notification(&self) -> bool {
        self.id.is_none()
    }

    /// Serialize to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Serialize to JSON bytes
    pub fn to_json_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Deserialize from JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// JSON-RPC 2.0 Error Object
///
/// When a RPC call encounters an error, the Response object MUST contain
/// an error member with this structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// Error code indicating the type of error
    pub code: ErrorCode,

    /// Short description of the error
    pub message: String,

    /// Additional information about the error (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcError {
    /// Create a new error with code and message
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    /// Create a new error with additional data
    pub fn with_data(
        code: ErrorCode,
        message: impl Into<String>,
        data: impl Into<serde_json::Value>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            data: Some(data.into()),
        }
    }

    /// Create a parse error (-32700)
    pub fn parse_error(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::PARSE_ERROR, message)
    }

    /// Create an invalid request error (-32600)
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::INVALID_REQUEST, message)
    }

    /// Create a method not found error (-32601)
    pub fn method_not_found(method: impl Into<String>) -> Self {
        Self::new(
            ErrorCode::METHOD_NOT_FOUND,
            format!("Method '{}' not found", method.into()),
        )
    }

    /// Create an invalid params error (-32602)
    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::INVALID_PARAMS, message)
    }

    /// Create an internal error (-32603)
    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::INTERNAL_ERROR, message)
    }

    /// Create a server shutdown error (-32000)
    pub fn server_shutdown() -> Self {
        Self::new(
            ErrorCode::SERVER_SHUTDOWN,
            ErrorCode::SERVER_SHUTDOWN.message(),
        )
    }

    /// Create a timeout error (-32002)
    pub fn timeout() -> Self {
        Self::new(ErrorCode::TIMEOUT_ERROR, ErrorCode::TIMEOUT_ERROR.message())
    }

    /// Add data to the error
    pub fn data(mut self, data: impl Into<serde_json::Value>) -> Self {
        self.data = Some(data.into());
        self
    }

    /// Serialize to JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

impl std::fmt::Display for JsonRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JsonRpcError({}: {})", self.code, self.message)
    }
}

impl std::error::Error for JsonRpcError {}

/// JSON-RPC 2.0 Response Object
///
/// When a RPC call is made, the Server replies with a Response object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    /// JSON-RPC version - MUST be exactly "2.0"
    pub jsonrpc: String,

    /// Result of the method call (present on success)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,

    /// Error object (present on failure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,

    /// Request ID (must match the request)
    pub id: Option<JsonRpcId>,
}

impl JsonRpcResponse {
    /// Create a successful response
    pub fn success(id: impl Into<JsonRpcId>, result: impl Into<serde_json::Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            result: Some(result.into()),
            error: None,
            id: Some(id.into()),
        }
    }

    /// Create an error response
    pub fn error(id: impl Into<JsonRpcId>, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            result: None,
            error: Some(error),
            id: Some(id.into()),
        }
    }

    /// Create a response for a notification (always None)
    pub fn notification_ack() -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            result: None,
            error: None,
            id: None,
        }
    }

    /// Check if this is a successful response
    pub fn is_success(&self) -> bool {
        self.error.is_none() && self.result.is_some()
    }

    /// Check if this is an error response
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }

    /// Get the result if successful
    pub fn get_result<T: for<'de> Deserialize<'de>>(&self) -> Result<Option<T>, serde_json::Error> {
        match &self.result {
            Some(value) => Ok(Some(serde_json::from_value(value.clone())?)),
            None => Ok(None),
        }
    }

    /// Serialize to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Serialize to JSON bytes
    pub fn to_json_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Deserialize from JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Represents either a single request/response or a batch
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcMessage {
    /// Single request
    Request(JsonRpcRequest),
    /// Single response
    Response(JsonRpcResponse),
    /// Batch of requests/responses
    Batch(Vec<JsonRpcMessage>),
}

impl JsonRpcMessage {
    /// Check if this is a request
    pub fn is_request(&self) -> bool {
        matches!(self, JsonRpcMessage::Request(_))
    }

    /// Check if this is a response
    pub fn is_response(&self) -> bool {
        matches!(self, JsonRpcMessage::Response(_))
    }

    /// Check if this is a batch
    pub fn is_batch(&self) -> bool {
        matches!(self, JsonRpcMessage::Batch(_))
    }

    /// Get as request
    pub fn as_request(&self) -> Option<&JsonRpcRequest> {
        match self {
            JsonRpcMessage::Request(req) => Some(req),
            _ => None,
        }
    }

    /// Get as response
    pub fn as_response(&self) -> Option<&JsonRpcResponse> {
        match self {
            JsonRpcMessage::Response(res) => Some(res),
            _ => None,
        }
    }

    /// Parse from JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        // Try to parse as batch first, then as single message
        if json.trim_start().starts_with('[') {
            let batch: Vec<JsonRpcMessage> = serde_json::from_str(json)?;
            Ok(JsonRpcMessage::Batch(batch))
        } else {
            // Try request first, then response
            match serde_json::from_str::<JsonRpcRequest>(json) {
                Ok(req) => Ok(JsonRpcMessage::Request(req)),
                Err(_) => {
                    let res = serde_json::from_str::<JsonRpcResponse>(json)?;
                    Ok(JsonRpcMessage::Response(res))
                }
            }
        }
    }

    /// Serialize to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_json_rpc_id_display() {
        assert_eq!(JsonRpcId::Number(42).to_string(), "42");
        assert_eq!(JsonRpcId::String("abc".to_string()).to_string(), "abc");
        assert_eq!(JsonRpcId::Null.to_string(), "null");
    }

    #[test]
    fn test_json_rpc_request_creation() {
        let req = JsonRpcRequest::new("test_method");
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.method, "test_method");
        assert!(req.params.is_none());
        assert!(req.id.is_some());
    }

    #[test]
    fn test_json_rpc_request_with_params() {
        let req = JsonRpcRequest::new("test_method").params(json!({"key": "value"}));
        assert_eq!(req.params, Some(json!({"key": "value"})));
    }

    #[test]
    fn test_json_rpc_notification() {
        let req = JsonRpcRequest::notification("notify_event");
        assert!(req.is_notification());
        assert!(req.id.is_none());
    }

    #[test]
    fn test_json_rpc_request_serialization() {
        let req = JsonRpcRequest::with_id("test", 1).params(json!({"arg": "value"}));
        let json = req.to_json().unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"method\":\"test\""));
        assert!(json.contains("\"id\":1"));
    }

    #[test]
    fn test_json_rpc_request_deserialization() {
        let json = r#"{"jsonrpc":"2.0","method":"test","params":{"key":"value"},"id":1}"#;
        let req: JsonRpcRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.method, "test");
        assert_eq!(req.id, Some(JsonRpcId::Number(1)));
    }

    #[test]
    fn test_json_rpc_error_creation() {
        let err = JsonRpcError::method_not_found("unknown_method");
        assert_eq!(err.code, ErrorCode::METHOD_NOT_FOUND);
        assert!(err.message.contains("unknown_method"));
        assert!(err.data.is_none());
    }

    #[test]
    fn test_json_rpc_error_with_data() {
        let err =
            JsonRpcError::invalid_params("Missing required field").data(json!({"field": "name"}));
        assert!(err.data.is_some());
    }

    #[test]
    fn test_json_rpc_response_success() {
        let res = JsonRpcResponse::success(1, json!({"status": "ok"}));
        assert!(res.is_success());
        assert!(!res.is_error());
        assert_eq!(res.id, Some(JsonRpcId::Number(1)));
    }

    #[test]
    fn test_json_rpc_response_error() {
        let res = JsonRpcResponse::error(1, JsonRpcError::internal_error("Something went wrong"));
        assert!(res.is_error());
        assert!(!res.is_success());
    }

    #[test]
    fn test_json_rpc_response_get_result() {
        let res = JsonRpcResponse::success(1, json!({"name": "test"}));
        let result: Option<serde_json::Value> = res.get_result().unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap()["name"], "test");
    }

    #[test]
    fn test_json_rpc_message_parse_request() {
        let json = r#"{"jsonrpc":"2.0","method":"test","id":1}"#;
        let msg = JsonRpcMessage::from_json(json).unwrap();
        assert!(msg.is_request());
    }

    #[test]
    fn test_json_rpc_message_parse_response() {
        let json = r#"{"jsonrpc":"2.0","result":{"status":"ok"},"id":1}"#;
        let msg = JsonRpcMessage::from_json(json).unwrap();
        assert!(msg.is_response());
    }

    #[test]
    fn test_json_rpc_message_parse_batch() {
        let json = r#"[{"jsonrpc":"2.0","method":"test1","id":1},{"jsonrpc":"2.0","method":"test2","id":2}]"#;
        let msg = JsonRpcMessage::from_json(json).unwrap();
        assert!(msg.is_batch());
    }
}
