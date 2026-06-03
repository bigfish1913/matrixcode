//! JSON-RPC 2.0 Protocol Types
//!
//! This module implements the JSON-RPC 2.0 specification types.
//! Reference: https://www.jsonrpc.org/specification

mod error;
mod types;

pub use error::*;
pub use types::*;
