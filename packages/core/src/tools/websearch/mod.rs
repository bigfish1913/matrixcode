//! Web Search Tool
//!
//! Performs web searches using multiple backends with proxy support and retry mechanism.

mod backends;
mod client;
mod parser;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;

use super::{Tool, ToolDefinition};
use client::{create_client, load_proxy_from_env};
use parser::SearchResult;

pub use parser::{SearchResultParser, clean_url};

/// Web search configuration
const DEFAULT_TIMEOUT_SECS: u64 = 30;
const MAX_TIMEOUT_SECS: u64 = 120;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchConfig {
    /// Proxy URL (e.g., "http://127.0.0.1:7890")
    pub proxy: Option<String>,
    /// Timeout in seconds
    pub timeout_secs: u64,
    /// Max retry attempts
    pub max_retries: u32,
    /// Enable fallback to alternative backends
    pub enable_fallback: bool,
}

impl Default for WebSearchConfig {
    fn default() -> Self {
        Self {
            proxy: None,
            timeout_secs: DEFAULT_TIMEOUT_SECS,
            max_retries: 3,
            enable_fallback: true,
        }
    }
}

/// Web search tool with proxy support and retry mechanism
pub struct WebSearchTool {
    config: WebSearchConfig,
}

impl Default for WebSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl WebSearchTool {
    pub fn new() -> Self {
        Self {
            config: WebSearchConfig::default(),
        }
    }

    pub fn with_config(config: WebSearchConfig) -> Self {
        Self { config }
    }

    /// Search with retry mechanism
    async fn search_with_retry(
        &self,
        query: &str,
        max_results: usize,
    ) -> Result<Vec<SearchResult>> {
        let client = create_client(self.config.proxy.as_deref(), self.config.timeout_secs)?;
        let mut last_error: Option<anyhow::Error> = None;

        for attempt in 0..self.config.max_retries {
            if attempt > 0 {
                let delay = Duration::from_secs(1 << (attempt - 1));
                tokio::time::sleep(delay).await;
                log::info!(
                    "WebSearch retry attempt {} after {}s delay",
                    attempt + 1,
                    delay.as_secs()
                );
            }

            match backends::search_duckduckgo(&client, query, max_results).await {
                Ok(results) if !results.is_empty() => {
                    log::info!("WebSearch succeeded on attempt {}", attempt + 1);
                    return Ok(results);
                }
                Ok(_) => {
                    log::warn!(
                        "WebSearch returned empty results on attempt {}",
                        attempt + 1
                    );
                    last_error = Some(anyhow::anyhow!("No search results found"));
                }
                Err(e) => {
                    log::warn!("WebSearch failed on attempt {}: {}", attempt + 1, e);
                    last_error = Some(e);
                }
            }
        }

        // Try fallback backends if enabled
        if self.config.enable_fallback {
            log::info!("Trying fallback search backends...");

            if let Ok(results) = backends::search_wikipedia(&client, query, max_results).await
                && !results.is_empty()
            {
                log::info!("Fallback search succeeded via Wikipedia");
                return Ok(results);
            }

            if let Ok(results) = backends::search_searxng(&client, query, max_results).await
                && !results.is_empty()
            {
                log::info!("Fallback search succeeded via SearXNG");
                return Ok(results);
            }
        }

        Err(last_error.unwrap_or_else(|| {
            anyhow::anyhow!("WebSearch failed after {} retries", self.config.max_retries)
        }))
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "websearch".to_string(),
            description: "使用 DuckDuckGo 搜索网络信息。返回包含标题、URL 和摘要的搜索结果列表。用于查找互联网上的最新信息。支持代理、自动重试和自定义超时。".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "搜索查询"
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "最大返回结果数（默认 5，最大 10）"
                    },
                    "use_proxy": {
                        "type": "boolean",
                        "description": "是否使用代理（默认自动检测环境变量 HTTP_PROXY）"
                    },
                    "timeout_secs": {
                        "type": "integer",
                        "description": format!("超时时间（秒，默认 {}，最大 {}）", DEFAULT_TIMEOUT_SECS, MAX_TIMEOUT_SECS)
                    }
                },
                "required": ["query"]
            }),
            ..Default::default()
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let query = params["query"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'query' parameter"))?;
        let max_results = params["max_results"].as_u64().unwrap_or(5).min(10) as usize;
        let use_proxy = params["use_proxy"].as_bool().unwrap_or(true);
        let timeout_secs = params["timeout_secs"]
            .as_u64()
            .unwrap_or(DEFAULT_TIMEOUT_SECS)
            .min(MAX_TIMEOUT_SECS);

        let mut config = self.config.clone();
        config.timeout_secs = timeout_secs;
        if use_proxy && config.proxy.is_none() {
            config.proxy = load_proxy_from_env();
            if config.proxy.is_some() {
                log::info!("WebSearch using proxy from environment: {:?}", config.proxy);
            }
        }

        let tool = Self::with_config(config);
        let results = tool.search_with_retry(query, max_results).await?;

        if results.is_empty() {
            return Ok("No results found. Suggestions:\n1. Check your network connection\n2. Try enabling proxy (set HTTP_PROXY env var)\n3. Try a different query".to_string());
        }

        let output = results
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let mut s = format!("{}. {}\n   {}", i + 1, r.title, r.url);
                if let Some(ref snippet) = r.snippet {
                    s.push_str(&format!("\n   {}", snippet));
                }
                s
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parser::{clean_url, strip_html_tags};

    #[test]
    fn test_strip_html_tags() {
        assert_eq!(strip_html_tags("<b>hello</b>"), "hello");
        assert_eq!(strip_html_tags("a &amp; b"), "a & b");
        assert_eq!(strip_html_tags("  <span>test</span>  "), "test");
    }

    #[test]
    fn test_clean_url() {
        let redirect_url = "https://duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com&rut=abc";
        assert_eq!(clean_url(redirect_url), "https://example.com");

        let normal_url = "https://example.com/page";
        assert_eq!(clean_url(normal_url), "https://example.com/page");
    }

    #[test]
    fn test_config_default() {
        let config = WebSearchConfig::default();
        assert_eq!(config.timeout_secs, 30);
        assert_eq!(config.max_retries, 3);
        assert!(config.enable_fallback);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use tokio;

    #[tokio::test]
    #[ignore]
    async fn test_real_websearch_full() {
        let tool = WebSearchTool::new();
        let params = json!({
            "query": "Rust programming",
            "max_results": 5
        });

        match tool.execute(params).await {
            Ok(result) => {
                println!("Full websearch result:\n{}", result);
                assert!(
                    !result.contains("No results found"),
                    "Should find results via Wikipedia fallback"
                );
            }
            Err(e) => {
                eprintln!("Error: {:?}", e);
                panic!("Websearch should succeed with Wikipedia fallback");
            }
        }
    }
}
