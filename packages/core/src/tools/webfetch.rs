use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};
use std::time::Duration;

use super::{Tool, ToolDefinition};

const DEFAULT_TIMEOUT_SECS: u64 = 30;
const MAX_TIMEOUT_SECS: u64 = 120;

pub struct WebFetchTool;

#[async_trait]
impl Tool for WebFetchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "webfetch".to_string(),
            description: "从 URL 获取内容并返回为文本。支持自定义超时时间。".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "要获取的 URL"
                    },
                    "max_length": {
                        "type": "integer",
                        "description": "最大响应长度（字符数，默认 10000）"
                    },
                    "timeout_secs": {
                        "type": "integer",
                        "description": format!("超时时间（秒，默认 {}，最大 {}）", DEFAULT_TIMEOUT_SECS, MAX_TIMEOUT_SECS)
                    }
                },
                "required": ["url"]
            }),
            ..Default::default()
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let url = params["url"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'url'"))?;
        let max_length = params["max_length"].as_u64().unwrap_or(10000) as usize;
        let timeout_secs = params["timeout_secs"]
            .as_u64()
            .unwrap_or(DEFAULT_TIMEOUT_SECS)
            .min(MAX_TIMEOUT_SECS);

        // Create client with timeout
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .connect_timeout(Duration::from_secs(10))
            .build()?;

        let response = tokio::time::timeout(
            Duration::from_secs(timeout_secs + 5), // Extra buffer for response processing
            client.get(url).send()
        ).await
            .map_err(|_| anyhow::anyhow!("request timed out after {}s", timeout_secs))?
            .map_err(|e| anyhow::anyhow!("request failed: {}", e))?;

        let status = response.status();

        if !status.is_success() {
            anyhow::bail!("HTTP {} for {}", status, url);
        }

        let body = response.text().await?;

        let truncated = if body.len() > max_length {
            let end = body.floor_char_boundary(max_length);
            format!(
                "{}...\n\n(truncated, {} total bytes)",
                &body[..end],
                body.len()
            )
        } else {
            body
        };

        Ok(truncated)
    }
}
