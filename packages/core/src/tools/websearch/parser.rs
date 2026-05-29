//! Search result types and parser trait

use serde::{Deserialize, Serialize};

/// A single search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: Option<String>,
}

/// Trait for parsing search results from different backends
pub trait SearchResultParser {
    /// Parse results from raw response text
    fn parse(&self, content: &str, max_results: usize) -> Vec<SearchResult>;
}

/// Helper to clean DuckDuckGo redirect URLs
pub fn clean_url(url: &str) -> String {
    if url.contains("duckduckgo.com/l/")
        && let Some(query) = url.split("uddg=").nth(1)
        && let Some(encoded) = query.split('&').next()
    {
        return urlencoding::decode(encoded)
            .unwrap_or_default()
            .into_owned();
    }
    url.to_string()
}

/// Strip HTML tags and decode entities
pub fn strip_html_tags(s: &str) -> String {
    let re = regex::Regex::new(r"<[^>]*>").unwrap();
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
