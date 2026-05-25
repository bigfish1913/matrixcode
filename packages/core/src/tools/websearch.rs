//! Web Search Tool
//!
//! Performs web searches using multiple backends with proxy support and retry mechanism.

use anyhow::{Context, Result};
use async_trait::async_trait;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;

use super::{Tool, ToolDefinition};

/// Web search configuration
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
            timeout_secs: 30,
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
    /// Create new websearch tool with default config
    pub fn new() -> Self {
        Self {
            config: WebSearchConfig::default(),
        }
    }

    /// Create websearch tool with custom config
    pub fn with_config(config: WebSearchConfig) -> Self {
        Self { config }
    }

    /// Try to load proxy from environment or config
    pub fn load_proxy_from_env() -> Option<String> {
        // Try common proxy environment variables
        std::env::var("HTTP_PROXY")
            .or_else(|_| std::env::var("HTTPS_PROXY"))
            .or_else(|_| std::env::var("ALL_PROXY"))
            .or_else(|_| std::env::var("http_proxy"))
            .or_else(|_| std::env::var("https_proxy"))
            .ok()
    }

    /// Create HTTP client with optional proxy
    fn create_client(&self) -> Result<reqwest::Client> {
        let mut builder = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .timeout(Duration::from_secs(self.config.timeout_secs))
            .connect_timeout(Duration::from_secs(10));

        // Add proxy if configured
        if let Some(ref proxy_url) = self.config.proxy {
            let proxy = reqwest::Proxy::all(proxy_url)
                .with_context(|| format!("Failed to configure proxy: {}", proxy_url))?;
            builder = builder.proxy(proxy);
        }

        builder.build()
            .with_context(|| "Failed to create HTTP client")
    }

    /// Search with retry mechanism
    async fn search_with_retry(&self, query: &str, max_results: usize) -> Result<Vec<SearchResult>> {
        let mut last_error: Option<anyhow::Error> = None;

        for attempt in 0..self.config.max_retries {
            // Exponential backoff: 1s, 2s, 4s...
            if attempt > 0 {
                let delay = Duration::from_secs(1 << (attempt - 1));
                tokio::time::sleep(delay).await;
                log::info!("WebSearch retry attempt {} after {}s delay", attempt + 1, delay.as_secs());
            }

            // Try primary backend (DuckDuckGo)
            match self.search_duckduckgo(query, max_results).await {
                Ok(results) if !results.is_empty() => {
                    log::info!("WebSearch succeeded on attempt {}", attempt + 1);
                    return Ok(results);
                }
                Ok(_) => {
                    // Empty results, try fallback
                    log::warn!("WebSearch returned empty results on attempt {}", attempt + 1);
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

            // Try SearXNG instances
            if let Ok(results) = self.search_searxng(query, max_results).await {
                if !results.is_empty() {
                    log::info!("Fallback search succeeded via SearXNG");
                    return Ok(results);
                }
            }
        }

        // Return last error
        Err(last_error
            .unwrap_or_else(|| anyhow::anyhow!("WebSearch failed after {} retries", self.config.max_retries)))
    }

    /// Search using DuckDuckGo HTML interface
    async fn search_duckduckgo(&self, query: &str, max_results: usize) -> Result<Vec<SearchResult>> {
        let client = self.create_client()?;

        let url = format!(
            "https://html.duckduckgo.com/html/?q={}",
            urlencoding_encode(query)
        );

        let response = client
            .get(&url)
            .send()
            .await
            .with_context(|| "DuckDuckGo request failed")?;

        if !response.status().is_success() {
            anyhow::bail!("DuckDuckGo returned status: {}", response.status());
        }

        let html = response.text().await
            .with_context(|| "Failed to read DuckDuckGo response")?;

        Ok(parse_ddg_html(&html, max_results))
    }

    /// Search using SearXNG public instances (fallback)
    async fn search_searxng(&self, query: &str, max_results: usize) -> Result<Vec<SearchResult>> {
        let client = self.create_client()?;

        // List of public SearXNG instances
        let instances = [
            "https://searx.be",
            "https://search.bus-hit.me",
            "https://searx.fmac.xyz",
        ];

        for instance in &instances {
            let url = format!(
                "{}{}search?q={}&format=json",
                instance,
                if instance.ends_with('/') { "" } else { "/" },
                urlencoding_encode(query)
            );

            let response = client
                .get(&url)
                .send()
                .await;

            if let Ok(resp) = response {
                if resp.status().is_success() {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        return Ok(parse_searxng_json(&json, max_results));
                    }
                }
            }
        }

        Err(anyhow::anyhow!("All SearXNG instances failed"))
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "websearch".to_string(),
            description: "使用 DuckDuckGo 搜索网络信息。返回包含标题、URL 和摘要的搜索结果列表。用于查找互联网上的最新信息。支持代理和自动重试。".to_string(),
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

        // Auto-detect proxy from environment if enabled
        let mut config = self.config.clone();
        if use_proxy && config.proxy.is_none() {
            config.proxy = Self::load_proxy_from_env();
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

/// A single search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SearchResult {
    title: String,
    url: String,
    snippet: Option<String>,
}

/// Parse DuckDuckGo HTML search results.
fn parse_ddg_html(html: &str, max_results: usize) -> Vec<SearchResult> {
    let mut results = Vec::new();

    let link_regex =
        Regex::new(r#"<a[^>]*class="[^"]*result__a[^"]*"[^>]*href="([^"]*)"[^>]*>(.*?)</a>"#).ok();
    let snippet_regex =
        Regex::new(r#"<a[^>]*class="[^"]*result__snippet[^"]*"[^>]*>(.*?)</a>"#).ok();

    if let Some(ref link_re) = link_regex {
        for cap in link_re.captures_iter(html) {
            if results.len() >= max_results {
                break;
            }

            let url = cap
                .get(1)
                .map(|m| clean_url(m.as_str()))
                .unwrap_or_default();
            let title = cap
                .get(2)
                .map(|m| strip_html_tags(m.as_str()))
                .unwrap_or_default();

            if url.is_empty() || title.is_empty() || url.contains("duckduckgo.com") {
                continue;
            }

            let snippet = snippet_regex.as_ref().and_then(|snip_re| {
                snip_re
                    .captures_iter(html)
                    .find(|c| {
                        if let Some(m) = c.get(0) {
                            let link_pos = cap.get(0).unwrap().start();
                            let snip_pos = m.start();
                            snip_pos > link_pos && snip_pos < link_pos + 1000
                        } else {
                            false
                        }
                    })
                    .and_then(|c| c.get(1).map(|m| strip_html_tags(m.as_str())))
            });

            results.push(SearchResult {
                title,
                url,
                snippet,
            });
        }
    }

    // Fallback parsing
    if results.is_empty() {
        let alt_link_re =
            Regex::new(r#"<a[^>]*class="[^"]*result[^"]*"[^>]*href="([^"]*)"[^>]*>([^<]*)</a>"#)
                .ok();
        if let Some(re) = alt_link_re {
            for cap in re.captures_iter(html) {
                if results.len() >= max_results {
                    break;
                }

                let url = clean_url(cap.get(1).map(|m| m.as_str()).unwrap_or_default());
                let title = cap
                    .get(2)
                    .map(|m| strip_html_tags(m.as_str()))
                    .unwrap_or_default();

                if url.is_empty() || title.is_empty() || url.contains("duckduckgo.com") {
                    continue;
                }

                results.push(SearchResult {
                    title,
                    url,
                    snippet: None,
                });
            }
        }
    }

    results
}

/// Parse SearXNG JSON response
fn parse_searxng_json(json: &Value, max_results: usize) -> Vec<SearchResult> {
    let mut results = Vec::new();

    if let Some(results_array) = json.get("results").and_then(|r| r.as_array()) {
        for item in results_array.iter().take(max_results) {
            let title = item.get("title")
                .and_then(|t| t.as_str())
                .unwrap_or_default()
                .to_string();

            let url = item.get("url")
                .and_then(|u| u.as_str())
                .unwrap_or_default()
                .to_string();

            let snippet = item.get("content")
                .and_then(|c| c.as_str())
                .map(|s| s.to_string());

            if !title.is_empty() && !url.is_empty() {
                results.push(SearchResult { title, url, snippet });
            }
        }
    }

    results
}

/// Clean DuckDuckGo redirect URLs
fn clean_url(url: &str) -> String {
    if url.contains("duckduckgo.com/l/")
        && let Some(query) = url.split("uddg=").nth(1)
        && let Some(encoded) = query.split('&').next()
        && let Ok(decoded) = urlencoding_decode(encoded)
    {
        return decoded;
    }
    url.to_string()
}

fn urlencoding_encode(s: &str) -> String {
    let mut result = String::new();
    for c in s.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => result.push(c),
            ' ' => result.push('+'),
            _ => {
                for byte in c.to_string().as_bytes() {
                    result.push_str(&format!("%{:02X}", byte));
                }
            }
        }
    }
    result
}

fn urlencoding_decode(s: &str) -> Result<String> {
    Ok(urlencoding_decode_simple(s))
}

fn urlencoding_decode_simple(s: &str) -> String {
    let mut bytes: Vec<u8> = Vec::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                bytes.push(byte);
            } else {
                bytes.push(b'%');
                bytes.extend_from_slice(hex.as_bytes());
            }
        } else if c == '+' {
            bytes.push(b' ');
        } else {
            let mut buf = [0u8; 4];
            let encoded = c.encode_utf8(&mut buf);
            bytes.extend_from_slice(encoded.as_bytes());
        }
    }

    String::from_utf8(bytes).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

fn strip_html_tags(s: &str) -> String {
    let re = Regex::new(r"<[^>]*>").unwrap();
    let without_tags = re.replace_all(s, "");

    without_tags
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_html_tags() {
        assert_eq!(strip_html_tags("<b>hello</b>"), "hello");
        assert_eq!(strip_html_tags("a &amp; b"), "a & b");
        assert_eq!(strip_html_tags("  <span>test</span>  "), "test");
    }

    #[test]
    fn test_urlencoding_decode() {
        assert_eq!(urlencoding_decode_simple("hello%20world"), "hello world");
        assert_eq!(urlencoding_decode_simple("a+b"), "a b");
        assert_eq!(urlencoding_decode_simple("%3Ctest%3E"), "<test>");
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

    #[test]
    fn test_parse_searxng_json() {
        let json = serde_json::json!({
            "results": [
                {"title": "Test Result", "url": "https://example.com", "content": "Some content"}
            ]
        });
        let results = parse_searxng_json(&json, 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Test Result");
    }
}
