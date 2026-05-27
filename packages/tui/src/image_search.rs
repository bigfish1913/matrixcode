//! Proxy Tool Executor Implementation
//!
//! 实现 ProxyToolExecutor trait，包含：
//! - exec(): 执行逻辑
//! - tool_definitions(): 工具定义（LLM 看到的）

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

use matrixcode_core::tools::toolproxy::{ProxyToolExecutor, ProxyToolDef};

use crate::image_utils;

/// 图片搜索执行器
///
/// 同时提供执行逻辑和工具定义
pub struct ImageSearchExecutor;

#[async_trait]
impl ProxyToolExecutor for ImageSearchExecutor {
    /// 执行图片搜索
    async fn exec(&self, tool_name: &str, input: Value) -> Result<String> {
        if tool_name == "image_search" {
            let query = input.get("query")
                .and_then(|q| q.as_str())
                .unwrap_or("");

            let max_results = input.get("max_results")
                .and_then(|m| m.as_u64())
                .unwrap_or(5) as u32;

            if query.is_empty() {
                return Ok(serde_json::json!({
                    "success": false,
                    "error": "query is required"
                }).to_string());
            }

            log::info!("Image search: query={}, max_results={}", query, max_results);

            let results = image_utils::search_all(query, max_results).await;

            match results {
                Ok(images) => {
                    Ok(serde_json::json!({
                        "success": true,
                        "query": query,
                        "total": images.len(),
                        "images": images
                    }).to_string())
                }
                Err(e) => {
                    Ok(serde_json::json!({
                        "success": false,
                        "error": e.to_string()
                    }).to_string())
                }
            }
        } else {
            Err(anyhow::anyhow!("Unknown proxy tool: {}", tool_name))
        }
    }

    /// 工具定义 - LLM 看到的描述
    fn tool_definitions() -> Vec<ProxyToolDef> {
        vec![
            ProxyToolDef::new(
                "image_search",
                "搜索网络图片资源。返回真实图片URL列表（Unsplash/Pexels/Pixabay）。适用于：找壁纸、素材、插图、风景照片等。",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "搜索关键词，建议使用英文获得更多结果"
                        },
                        "max_results": {
                            "type": "integer",
                            "description": "每个平台返回的最大结果数，默认5，最大10",
                            "default": 5
                        }
                    },
                    "required": ["query"]
                })
            )
            .with_priority(true)
            .with_timeout(30000)
        ]
    }
}

/// 创建执行器
pub fn create_image_search_executor() -> Arc<dyn ProxyToolExecutor> {
    Arc::new(ImageSearchExecutor)
}

/// 获取工具定义（直接从 trait 获取）
pub fn get_image_search_tool_defs() -> Vec<ProxyToolDef> {
    ImageSearchExecutor::tool_definitions()
}

// ============================================================================
// 别名函数（兼容 main.rs 调用）
// ============================================================================

/// 创建默认执行器（别名）
pub fn create_default_executor() -> Arc<dyn ProxyToolExecutor> {
    create_image_search_executor()
}

/// 获取默认代理工具定义（别名）
pub fn get_default_proxy_tools() -> Vec<ProxyToolDef> {
    get_image_search_tool_defs()
}