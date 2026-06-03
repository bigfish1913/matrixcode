//! Router Module
//!
//! Provides routing logic for tool and node execution requests.
//! Routes JSON-RPC calls to the appropriate extension services.

mod tool_router;
mod node_router;

pub use tool_router::{ToolRouter, ToolRouterError, ToolRouteResult, ToolDefinition};
pub use node_router::{
    NodeRouter, NodeRouterError, NodeRouteResult, NodeDefinition, NodeContext,
    NodeType, NodeCapability,
};