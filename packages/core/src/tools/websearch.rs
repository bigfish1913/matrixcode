use anyhow::Result;
use async_trait::async_trait;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{Tool, ToolDefinition};

/// Client-side web search tool using DuckDuckGo HTML search.
/// This tool performs web searches without requiring any API key.
pub struct WebSearchTool;

#[async_trait]
impl Tool for WebSearchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "websearch".to_string(),
            description: "使用 DuckDuckGo 搜索网络信息。返回包含标题、URL 和摘要的搜索结果列表。用于查找互联网上的最新信息。".to_string(),
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
                    }
                },
                "required": ["query"]
            }),
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let query = params["query"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'query' parameter"))?;
        let max_results = params["max_results"].as_u64().unwrap_or(5).min(10) as usize;

        // Show spinner while searching - RAII guard ensures cleanup on error
        // let mut spinner = ToolSpinner::new(&format!("web-searching '{}'", query));

        let results = search_duckduckgo(query, max_results).await?;

        if results.is_empty() {
            // spinner.finish_success("0 results");
            return Ok("No results found.".to_string());
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

        // spinner.finish_success(&format!("{} results", results.len()));
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

/// Perform a web search using DuckDuckGo HTML interface.
async fn search_duckduckgo(query: &str, max_results: usize) -> Result<Vec<SearchResult>> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .build()?;

    let url = format!(
        "https://html.duckduckgo.com/html/?q={}",
        urlencoding_encode(query)
    );

    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        anyhow::bail!("Search request failed with status: {}", response.status());
    }

    let html = response.text().await?;
    let results = parse_ddg_html(&html, max_results);

    Ok(results)
}

/// Parse DuckDuckGo HTML search results.
fn parse_ddg_html(html: &str, max_results: usize) -> Vec<SearchResult> {
    let mut results = Vec::new();

    // DuckDuckGo HTML results are in <div class="result"> elements
    // Each result contains:
    // - <a class="result__a"> for the title and URL
    // - <a class="result__snippet"> for the snippet

    let _result_div_regex =
        Regex::new(r#"<div[^>]*class="[^"]*result[^"]*"[^>]*>(.*?)</div>\s*</div>"#).ok();
    let link_regex =
        Regex::new(r#"<a[^>]*class="[^"]*result__a[^"]*"[^>]*href="([^"]*)"[^>]*>(.*?)</a>"#).ok();
    let snippet_regex =
        Regex::new(r#"<a[^>]*class="[^"]*result__snippet[^"]*"[^>]*>(.*?)</a>"#).ok();

    // Alternative simpler parsing: look for result__a links
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

            // Skip ad results and empty URLs
            if url.is_empty() || title.is_empty() || url.contains("duckduckgo.com") {
                continue;
            }

            // Try to find snippet near this result
            let snippet = snippet_regex.as_ref().and_then(|snip_re| {
                snip_re
                    .captures_iter(html)
                    .find(|c| {
                        if let Some(m) = c.get(0) {
                            // Check if snippet is after current link position in HTML
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

    // If simple parsing didn't work well, try alternative approach
    if results.is_empty() {
        // Fallback: parse using a more lenient pattern
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

/// Clean DuckDuckGo redirect URLs to get the actual URL.
fn clean_url(url: &str) -> String {
    // DuckDuckGo uses redirect URLs like:
    // https://duckduckgo.com/l/?uddg=ENCODED_URL&rut=...
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

/// Decode URL encoding.
fn urlencoding_decode(s: &str) -> Result<String> {
    let decoded = urlencoding_decode_simple(s);
    Ok(decoded)
}

/// Simple URL decoding without external crate.
/// Correctly handles multi-byte UTF-8 sequences (e.g. %E4%B8%AD → 中).
fn urlencoding_decode_simple(s: &str) -> String {
    let mut bytes: Vec<u8> = Vec::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                bytes.push(byte);
            } else {
                // Invalid hex, push literal
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

/// Strip HTML tags from a string.
fn strip_html_tags(s: &str) -> String {
    // Remove HTML tags
    let re = Regex::new(r"<[^>]*>").unwrap();
    let without_tags = re.replace_all(s, "");

    // Decode common HTML entities
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
}
