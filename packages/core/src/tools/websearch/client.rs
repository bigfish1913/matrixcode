//! HTTP client configuration for web search

use anyhow::{Context, Result};
use reqwest::Client;
use std::time::Duration;

/// Create HTTP client with optional proxy
pub fn create_client(proxy: Option<&str>, timeout_secs: u64) -> Result<Client> {
    let mut builder = Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .timeout(Duration::from_secs(timeout_secs))
        .connect_timeout(Duration::from_secs(10));

    if let Some(proxy_url) = proxy {
        let proxy = reqwest::Proxy::all(proxy_url)
            .with_context(|| format!("Failed to configure proxy: {}", proxy_url))?;
        builder = builder.proxy(proxy);
    }

    builder.build().with_context(|| "Failed to create HTTP client")
}

/// Load proxy from environment variables
pub fn load_proxy_from_env() -> Option<String> {
    std::env::var("HTTP_PROXY")
        .or_else(|_| std::env::var("HTTPS_PROXY"))
        .or_else(|_| std::env::var("ALL_PROXY"))
        .or_else(|_| std::env::var("http_proxy"))
        .or_else(|_| std::env::var("https_proxy"))
        .ok()
}