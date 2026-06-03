//! JSON-RPC 2.0 Error Codes and Types
//!
//! Defines standard and custom error codes per JSON-RPC 2.0 specification.

use serde::{Deserialize, Serialize};

/// JSON-RPC 2.0 standard error codes
///
/// Reference: https://www.jsonrpc.org/specification#error_object
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ErrorCode(pub i64);

impl ErrorCode {
    // ============== Standard JSON-RPC 2.0 Error Codes ==============

    /// Invalid JSON was received by the server.
    /// An error occurred on the server while parsing the JSON text.
    pub const PARSE_ERROR: Self = ErrorCode(-32700);

    /// The JSON sent is not a valid Request object.
    pub const INVALID_REQUEST: Self = ErrorCode(-32600);

    /// The method does not exist / is not available.
    pub const METHOD_NOT_FOUND: Self = ErrorCode(-32601);

    /// Invalid method parameter(s).
    pub const INVALID_PARAMS: Self = ErrorCode(-32602);

    /// Internal JSON-RPC error.
    pub const INTERNAL_ERROR: Self = ErrorCode(-32603);

    // ============== MatrixRPC Custom Error Codes ==============

    /// Server is shutting down
    pub const SERVER_SHUTDOWN: Self = ErrorCode(-32000);

    /// Transport error
    pub const TRANSPORT_ERROR: Self = ErrorCode(-32001);

    /// Timeout error
    pub const TIMEOUT_ERROR: Self = ErrorCode(-32002);

    /// Capability not supported
    pub const CAPABILITY_NOT_SUPPORTED: Self = ErrorCode(-32003);

    /// Resource not found
    pub const RESOURCE_NOT_FOUND: Self = ErrorCode(-32004);

    /// Resource already exists
    pub const RESOURCE_EXISTS: Self = ErrorCode(-32005);

    /// Permission denied
    pub const PERMISSION_DENIED: Self = ErrorCode(-32006);

    /// Invalid state for operation
    pub const INVALID_STATE: Self = ErrorCode(-32007);

    /// Callback registration failed
    pub const CALLBACK_ERROR: Self = ErrorCode(-32008);

    /// Get the error message for this code
    pub fn message(&self) -> &'static str {
        match self.0 {
            -32700 => "Parse error",
            -32600 => "Invalid request",
            -32601 => "Method not found",
            -32602 => "Invalid params",
            -32603 => "Internal error",
            -32000 => "Server shutdown",
            -32001 => "Transport error",
            -32002 => "Timeout",
            -32003 => "Capability not supported",
            -32004 => "Resource not found",
            -32005 => "Resource already exists",
            -32006 => "Permission denied",
            -32007 => "Invalid state",
            -32008 => "Callback error",
            _ => "Unknown error",
        }
    }

    /// Check if this is a standard JSON-RPC error code
    pub fn is_standard(&self) -> bool {
        (-32700..=-32603).contains(&self.0)
    }

    /// Check if this is a custom MatrixRPC error code
    pub fn is_custom(&self) -> bool {
        (-32099..=-32000).contains(&self.0)
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<i64> for ErrorCode {
    fn from(code: i64) -> Self {
        ErrorCode(code)
    }
}

impl From<ErrorCode> for i64 {
    fn from(code: ErrorCode) -> Self {
        code.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_error_codes() {
        assert_eq!(ErrorCode::PARSE_ERROR.0, -32700);
        assert_eq!(ErrorCode::INVALID_REQUEST.0, -32600);
        assert_eq!(ErrorCode::METHOD_NOT_FOUND.0, -32601);
        assert_eq!(ErrorCode::INVALID_PARAMS.0, -32602);
        assert_eq!(ErrorCode::INTERNAL_ERROR.0, -32603);
    }

    #[test]
    fn test_custom_error_codes() {
        assert_eq!(ErrorCode::SERVER_SHUTDOWN.0, -32000);
        assert_eq!(ErrorCode::TRANSPORT_ERROR.0, -32001);
        assert_eq!(ErrorCode::TIMEOUT_ERROR.0, -32002);
    }

    #[test]
    fn test_error_messages() {
        assert_eq!(ErrorCode::PARSE_ERROR.message(), "Parse error");
        assert_eq!(ErrorCode::METHOD_NOT_FOUND.message(), "Method not found");
        assert_eq!(ErrorCode::SERVER_SHUTDOWN.message(), "Server shutdown");
    }

    #[test]
    fn test_is_standard() {
        assert!(ErrorCode::PARSE_ERROR.is_standard());
        assert!(ErrorCode::INTERNAL_ERROR.is_standard());
        assert!(!ErrorCode::SERVER_SHUTDOWN.is_standard());
    }

    #[test]
    fn test_is_custom() {
        assert!(ErrorCode::SERVER_SHUTDOWN.is_custom());
        assert!(!ErrorCode::PARSE_ERROR.is_custom());
    }

    #[test]
    fn test_error_code_serde() {
        let code = ErrorCode::INVALID_PARAMS;
        let json = serde_json::to_string(&code).unwrap();
        assert_eq!(json, "-32602");

        let decoded: ErrorCode = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, code);
    }
}
