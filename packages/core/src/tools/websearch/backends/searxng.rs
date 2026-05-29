//! SearXNG search backend

use anyhow::Result;
use reqwest::Client;
use serde_json::Value;

use crate::tools::websearch::parser::SearchResult;

fn parse_json(json: &Value, max_results: usize) -> Vec<SearchResult> {
    let mut results = Vec::new();

    if let Some(results_array) = json.get("results").and_then(|r| r.as_array()) {
        for item in results_array.iter().take(max_results) {
            let title = item
                .get("title")
                .and_then(|t| t.as_str())
                .unwrap_or_default()
                .to_string();

            let url = item
                .get("url")
                .and_then(|u| u.as_str())
                .unwrap_or_default()
                .to_string();

            let snippet = item
                .get("content")
                .and_then(|c| c.as_str())
                .map(|s| s.to_string());

            if !title.is_empty() && !url.is_empty() {
                results.push(SearchResult {
                    title,
                    url,
                    snippet,
                });
            }
        }
    }

    results
}

/// Public SearXNG instances
const SEARXNG_INSTANCES: &[&str] = &[
    "https://searx.be",
    "https://search.bus-hit.me",
    "https://searx.fmac.xyz",
];

/// Search using SearXNG public instances
pub async fn search(client: &Client, query: &str, max_results: usize) -> Result<Vec<SearchResult>> {
    for instance in SEARXNG_INSTANCES {
        let url = format!(
            "{}{}search?q={}&format=json",
            instance,
            if instance.ends_with('/') { "" } else { "/" },
            urlencoding::encode(query)
        );

        let response = client.get(&url).send().await;

        if let Ok(resp) = response
            && resp.status().is_success()
            && let Ok(json) = resp.json::<Value>().await
        {
            let results = parse_json(&json, max_results);
            if !results.is_empty() {
                return Ok(results);
            }
        }
    }

    Err(anyhow::anyhow!("All SearXNG instances failed"))
}
