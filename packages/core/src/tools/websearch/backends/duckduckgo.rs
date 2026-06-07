//! DuckDuckGo search backend

use anyhow::{Context, Result};
use reqwest::Client;

use crate::tools::websearch::parser::{clean_url, SearchResult, SearchResultParser, strip_html_tags};

/// DuckDuckGo HTML parser
pub struct DuckDuckGoHtmlParser;

impl SearchResultParser for DuckDuckGoHtmlParser {
    fn parse(&self, html: &str, max_results: usize) -> Vec<SearchResult> {
        let mut results = Vec::new();
        let link_regex = regex::Regex::new(
            r#"<a[^>]*class="[^"]*result__a[^"]*"[^>]*href="([^"]*)"[^>]*>(.*?)</a>"#
        ).ok();
        let snippet_regex = regex::Regex::new(
            r#"<a[^>]*class="[^"]*result__snippet[^"]*"[^>]*>(.*?)</a>"#
        ).ok();

        if let Some(ref link_re) = link_regex {
            for cap in link_re.captures_iter(html) {
                if results.len() >= max_results {
                    break;
                }

                let url = cap.get(1)
                    .map(|m| clean_url(m.as_str()))
                    .unwrap_or_default();
                let title = cap.get(2)
                    .map(|m| strip_html_tags(m.as_str()))
                    .unwrap_or_default();

                if url.is_empty() || title.is_empty() || url.contains("duckduckgo.com") {
                    continue;
                }

                let snippet = snippet_regex.as_ref().and_then(|snip_re| {
                    snip_re.captures_iter(html)
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

                results.push(SearchResult { title, url, snippet });
            }
        }

        // Fallback parsing if no results
        if results.is_empty() {
            fallback_parse(html, max_results, &mut results);
        }

        results
    }
}

/// DuckDuckGo Lite HTML parser (simpler format)
pub struct DuckDuckGoLiteParser;

impl SearchResultParser for DuckDuckGoLiteParser {
    fn parse(&self, html: &str, max_results: usize) -> Vec<SearchResult> {
        let mut results = Vec::new();
        let link_re = regex::Regex::new(
            r#"<a[^>]*rel="nofollow"[^>]*href="([^"]*)"[^>]*>([^<]+)</a>"#
        ).ok();

        if let Some(re) = link_re {
            for cap in re.captures_iter(html) {
                if results.len() >= max_results {
                    break;
                }

                let url = clean_url(cap.get(1).map(|m| m.as_str()).unwrap_or_default());
                let title = strip_html_tags(cap.get(2).map(|m| m.as_str()).unwrap_or_default());

                if url.is_empty() || title.is_empty() || url.contains("duckduckgo.com") {
                    continue;
                }

                results.push(SearchResult { title, url, snippet: None });
            }
        }

        results
    }
}

fn fallback_parse(html: &str, max_results: usize, results: &mut Vec<SearchResult>) {
    let alt_link_re = regex::Regex::new(
        r#"<a[^>]*class="[^"]*result[^"]*"[^>]*href="([^"]*)"[^>]*>([^<]*)</a>"#
    ).ok();

    if let Some(re) = alt_link_re {
        for cap in re.captures_iter(html) {
            if results.len() >= max_results {
                break;
            }

            let url = clean_url(cap.get(1).map(|m| m.as_str()).unwrap_or_default());
            let title = cap.get(2)
                .map(|m| strip_html_tags(m.as_str()))
                .unwrap_or_default();

            if url.is_empty() || title.is_empty() || url.contains("duckduckgo.com") {
                continue;
            }

            results.push(SearchResult { title, url, snippet: None });
        }
    }
}

/// Search using DuckDuckGo
pub async fn search(client: &Client, query: &str, max_results: usize) -> Result<Vec<SearchResult>> {
    // Try Lite first (less likely to be blocked)
    if let Ok(results) = search_lite(client, query, max_results).await
        && !results.is_empty() {
            return Ok(results);
        }

    // Fallback to HTML interface
    let url = format!(
        "https://html.duckduckgo.com/html/?q={}",
        urlencoding::encode(query)
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

    Ok(DuckDuckGoHtmlParser.parse(&html, max_results))
}

/// Search using DuckDuckGo Lite
pub async fn search_lite(client: &Client, query: &str, max_results: usize) -> Result<Vec<SearchResult>> {
    let url = format!(
        "https://lite.duckduckgo.com/lite/?q={}",
        urlencoding::encode(query)
    );

    let response = client
        .get(&url)
        .send()
        .await
        .with_context(|| "DuckDuckGo Lite request failed")?;

    if !response.status().is_success() {
        anyhow::bail!("DuckDuckGo Lite returned status: {}", response.status());
    }

    let html = response.text().await
        .with_context(|| "Failed to read DuckDuckGo Lite response")?;

    Ok(DuckDuckGoLiteParser.parse(&html, max_results))
}