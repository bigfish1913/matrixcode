//! Proxy Tool Executor Trait
//!
//! 简化的代理工具接口 - 外部只需实现一个 trait
//!
//! ## 设计原则
//!
//! 实现者必须同时提供：
//! 1. `exec()` - 执行逻辑
//! 2. `tool_definitions()` - 工具定义（LLM 看到的）
//!
//! 这样每个 Executor 都知道自己的工具定义，紧密绑定。

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::ToolDefinition;

/// 代理工具定义（发送给 LLM）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyToolDef {
    pub definition: ToolDefinition,
    pub timeout_ms: u64,
}

impl ProxyToolDef {
    pub fn new(name: impl Into<String>, description: impl Into<String>, parameters: Value) -> Self {
        Self {
            definition: ToolDefinition {
                name: name.into(),
                description: description.into(),
                parameters,
                is_priority: false,
            },
            timeout_ms: 30000,
        }
    }

    pub fn with_priority(mut self, is_priority: bool) -> Self {
        self.definition.is_priority = is_priority;
        self
    }

    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }
}

/// 代理工具执行器 trait
///
/// **必须实现两个方法：**
/// - `exec()` - 执行逻辑
/// - `tool_definitions()` - 工具定义列表
///
/// # Example
///
/// ```rust
/// struct ImageSearchExecutor;
///
/// #[async_trait]
/// impl ProxyToolExecutor for ImageSearchExecutor {
///     async fn exec(&self, tool_name: &str, input: Value) -> Result<String> {
///         // 执行逻辑...
///     }
///
///     fn tool_definitions() -> Vec<ProxyToolDef> {
///         vec![
///             ProxyToolDef::new("image_search", "搜索图片", json!({...}))
///                 .with_priority(true)
///         ]
///     }
/// }
///
/// // 使用
/// let executor = Arc::new(ImageSearchExecutor);
/// let tool_defs = ImageSearchExecutor::tool_definitions();
/// builder.proxy_executor(executor, tool_defs)
/// ```
#[async_trait]
pub trait ProxyToolExecutor: Send + Sync {
    /// 执行代理工具（必需）
    async fn exec(&self, tool_name: &str, input: Value) -> Result<String>;

    /// 返回工具定义列表（必需 - LLM 看到的）
    fn tool_definitions() -> Vec<ProxyToolDef> where Self: Sized;
}

// ============================================================================
// Legacy types for backward compatibility
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProxyMetadata {
    pub tool_type: String,
    pub endpoint: Option<String>,
    pub timeout_ms: u64,
    pub custom: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyToolRequest {
    pub request_id: String,
    pub tool_name: String,
    pub tool_input: Value,
    pub metadata: ProxyMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyToolResponse {
    pub request_id: String,
    pub result: String,
    pub is_error: bool,
}

#[derive(Debug)]
pub struct ProxyTool {
    definition: ToolDefinition,
    metadata: ProxyMetadata,
}

impl ProxyTool {
    pub fn new(definition: ToolDefinition, metadata: ProxyMetadata) -> Self {
        Self { definition, metadata }
    }

    pub fn metadata(&self) -> &ProxyMetadata {
        &self.metadata
    }

    pub fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proxy_tool_def_creation() {
        let def = ProxyToolDef::new("test", "测试工具", serde_json::json!({}))
            .with_priority(true)
            .with_timeout(60000);

        assert_eq!(def.definition.name, "test");
        assert!(def.definition.is_priority);
        assert_eq!(def.timeout_ms, 60000);
    }
}