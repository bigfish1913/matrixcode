//! Proxy Tool
//!
//! 代理工具 - 外部系统注入的工具，由调用方自行执行

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Tool, ToolDefinition};

/// 代理工具元数据 - 调用方自定义信息
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProxyMetadata {
    /// 工具类型标识（用于调用方识别）
    pub tool_type: String,
    
    /// 调用方 endpoint（可选）
    pub endpoint: Option<String>,
    
    /// 超时时间（毫秒）
    pub timeout_ms: u64,
    
    /// 自定义元数据（JSON 格式，调用方可扩展）
    pub custom: Option<Value>,
}

/// 代理工具 - 不真正执行，返回特殊标记让 Agent 知道需要透传
#[derive(Debug)]
pub struct ProxyTool {
    definition: ToolDefinition,
    metadata: ProxyMetadata,
}

impl ProxyTool {
    /// 创建新的代理工具
    pub fn new(definition: ToolDefinition, metadata: ProxyMetadata) -> Self {
        Self {
            definition,
            metadata,
        }
    }
    
    /// 获取元数据
    pub fn metadata(&self) -> &ProxyMetadata {
        &self.metadata
    }
    
    /// 是否是代理工具（用于 Agent 判断）
    pub fn is_proxy() -> bool {
        true
    }
}

#[async_trait]
impl Tool for ProxyTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }
    
    /// execute() 不真正执行，返回特殊标记
    /// Agent 会检测到这个标记，改为调用 handle_proxy_tool()
    async fn execute(&self, _params: Value) -> Result<String> {
        // 这个方法不应该被直接调用
        // Agent 会通过 is_proxy() 判断并走特殊流程
        Err(anyhow::anyhow!(
            "ProxyTool should not be executed directly. \
             Agent will detect proxy tools and use handle_proxy_tool() instead."
        ))
    }
}

/// 扩展 Tool trait，添加代理判断
pub trait ToolExt: Tool {
    /// 判断是否是代理工具
    fn is_proxy(&self) -> bool {
        false // 默认不是代理工具
    }
}

/// 代理工具请求 - 发送给调用方
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyToolRequest {
    /// 请求 ID（用于匹配响应）
    pub request_id: String,
    
    /// 工具名称
    pub tool_name: String,
    
    /// 工具输入参数
    pub tool_input: Value,
    
    /// 元数据
    pub metadata: ProxyMetadata,
}

/// 代理工具响应 - 调用方返回结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyToolResponse {
    /// 请求 ID（匹配 request_id）
    pub request_id: String,
    
    /// 执行结果
    pub result: String,
    
    /// 是否错误
    pub is_error: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_proxy_tool_creation() {
        let definition = ToolDefinition {
            name: "custom_search".to_string(),
            description: "自定义搜索工具".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"}
                },
                "required": ["query"]
            }),
            ..Default::default()
        };
        
        let metadata = ProxyMetadata {
            tool_type: "search".to_string(),
            endpoint: Some("http://my-service/api".to_string()),
            timeout_ms: 30000,
            custom: None,
        };
        
        let proxy_tool = ProxyTool::new(definition.clone(), metadata);
        
        assert_eq!(proxy_tool.definition().name, "custom_search");
        assert_eq!(proxy_tool.metadata().tool_type, "search");
    }
    
    #[test]
    fn test_proxy_tool_execute_should_fail() {
        let definition = ToolDefinition {
            name: "test_tool".to_string(),
            description: "Test".to_string(),
            parameters: json!({}),
            ..Default::default()
        };
        
        let metadata = ProxyMetadata {
            tool_type: "test".to_string(),
            endpoint: None,
            timeout_ms: 1000,
            custom: None,
        };
        
        let proxy_tool = ProxyTool::new(definition, metadata);
        
        // execute() 应该返回错误，提示不应该直接调用
        // 注意：execute 是 async，需要在 async block 中调用
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(proxy_tool.execute(json!({"test": "value"})));
        assert!(result.is_err());
    }
}