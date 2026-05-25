//! Proxy Tool Handler for CLI
//!
//! 处理 Agent 的代理工具请求，在 CLI 层执行工具逻辑

use anyhow::Result;
use matrixcode_core::{
    event::{EventData, EventType},
    tools::{ProxyMetadata, ProxyToolResponse},
    AgentEvent,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::mpsc;

/// 代理工具处理器
pub struct ProxyToolHandler {
    /// 响应发送 channel
    response_tx: mpsc::Sender<ProxyToolResponse>,
}

impl ProxyToolHandler {
    pub fn new(response_tx: mpsc::Sender<ProxyToolResponse>) -> Self {
        Self { response_tx }
    }
    
    /// 处理代理工具请求事件
    pub async fn handle_event(&self, event: &AgentEvent) -> Result<()> {
        if event.event_type != EventType::ProxyToolRequest {
            return Ok(());
        }
        
        if let Some(EventData::ProxyToolRequest {
            request_id,
            tool_name,
            tool_input,
            metadata,
        }) = &event.data {
            log::info!(
                "ProxyTool request: {} (request_id={})",
                tool_name,
                request_id
            );
            
            // 执行工具
            let result = self.execute_tool(tool_name, tool_input.clone(), metadata);
            
            // 发送响应
            let response = ProxyToolResponse {
                request_id: request_id.clone(),
                result: result.content,
                is_error: result.is_error,
            };
            
            self.response_tx.send(response).await?;
            log::info!("ProxyTool response sent for request_id={}", request_id);
        }
        
        Ok(())
    }
    
    /// 执行工具逻辑
    fn execute_tool(&self, tool_name: &str, input: Value, metadata: &ProxyMetadata) -> ToolResult {
        match tool_name {
            "image_search" => self.execute_image_search(input, metadata),
            _ => ToolResult::error(format!("Unknown proxy tool: {}", tool_name)),
        }
    }
    
    /// 执行图片搜索
    fn execute_image_search(&self, input: Value, _metadata: &ProxyMetadata) -> ToolResult {
        let query = input
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        
        let max_results = input
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(3)
            .min(10) as usize;
        
        log::info!("Image search: query='{}', max_results={}", query, max_results);
        
        // TODO: 实现真实图片搜索（使用 DuckDuckGo Images API）
        // 当前返回模拟数据用于测试
        let urls: Vec<String> = (1..=max_results)
            .map(|i| format!("https://example.com/image{}.jpg?query={}", i, query))
            .collect();
        
        let result = json!({
            "query": query,
            "urls": urls,
            "count": urls.len(),
            "source": "DuckDuckGo Images",
            "note": "图片搜索代理工具测试成功"
        });
        
        ToolResult::success(result.to_string())
    }
}

/// 工具执行结果
struct ToolResult {
    content: String,
    is_error: bool,
}

impl ToolResult {
    fn success(content: String) -> Self {
        Self { content, is_error: false }
    }
    
    fn error(content: String) -> Self {
        Self { content, is_error: true }
    }
}

/// 创建 image_search 代理工具定义
pub fn create_image_search_proxy_tool() -> matrixcode_core::tools::ProxyTool {
    use matrixcode_core::tools::{ProxyTool, ToolDefinition};
    
    ProxyTool::new(
        ToolDefinition {
            name: "image_search".to_string(),
            description: "搜索图片资源。返回图片 URL 列表。由 UI 层执行。".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "搜索关键词"
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "最大结果数（默认 3，最大 10）",
                        "default": 3
                    }
                },
                "required": ["query"]
            }),
        },
        ProxyMetadata {
            tool_type: "image_search".to_string(),
            endpoint: None,  // 本地执行，无需 endpoint
            timeout_ms: 5000,
            custom: Some(json!({
                "executor": "cli_layer",
                "api": "duckduckgo_images"
            })),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_create_image_search_proxy_tool() {
        let tool = create_image_search_proxy_tool();
        assert_eq!(tool.definition().name, "image_search");
        
        let metadata = tool.metadata();
        assert_eq!(metadata.tool_type, "image_search");
        assert_eq!(metadata.timeout_ms, 5000);
    }
    
    #[test]
    fn test_execute_image_search() {
        let (tx, _rx) = mpsc::channel(10);
        let handler = ProxyToolHandler::new(tx);
        
        let input = json!({
            "query": "rust logo",
            "max_results": 5
        });
        
        let metadata = ProxyMetadata {
            tool_type: "image_search".to_string(),
            endpoint: None,
            timeout_ms: 5000,
            custom: None,
        };
        
        let result = handler.execute_image_search(input, &metadata);
        assert!(!result.is_error);
        
        let parsed: Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(parsed["query"], "rust logo");
        assert_eq!(parsed["count"], 5);
    }
}