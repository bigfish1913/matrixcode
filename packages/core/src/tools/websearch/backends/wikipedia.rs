//! Wikipedia API search backend

use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::Value;

use crate::tools::websearch::parser::{SearchResult, strip_html_tags};

fn parse_json(json: &Value, max_results: usize) -> Vec<SearchResult> {
    let mut results = Vec::new();

    if let Some(search_array) = json.get("query")
        .and_then(|q| q.get("search"))
        .and_then(|s| s.as_array())
    {
        for item in search_array.iter().take(max_results) {
            let title = item.get("title")
                .and_then(|t| t.as_str())
                .unwrap_or_default()
                .to_string();

            let snippet = item.get("snippet")
                .and_then(|s| s.as_str())
                .map(strip_html_tags);

            let page_id = item.get("pageid")
                .and_then(|p| p.as_u64())
                .unwrap_or(0);

            let url = if page_id > 0 {
                format!("https://en.wikipedia.org/?curid={}", page_id)
            } else {
                format!("https://en.wikipedia.org/wiki/{}", urlencoding::encode(&title))
            };

            if !title.is_empty() {
                results.push(SearchResult { title, url, snippet });
            }
        }
    }

    results
}

/// Search using Wikipedia API
pub async fn search(client: &Client, query: &str, max_results: usize) -> Result<Vec<SearchResult>> {
    let url = format!(
        "https://en.wikipedia.org/w/api.php?action=query&list=search&srsearch={}&format=json&srlimit={}",
        urlencoding::encode(query),
        max_results
    );

    let response = client
        .get(&url)
        .send()
        .await
        .with_context(|| "Wikipedia API request failed")?;

    if !response.status().is_success() {
        anyhow::bail!("Wikipedia API returned status: {}", response.status());
    }

    let json = response.json::<Value>().await
        .with_context(|| "Failed to parse Wikipedia API response")?;

    Ok(parse_json(&json, max_results))
}