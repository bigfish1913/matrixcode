use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

use super::{Tool, ToolDefinition};

pub struct WebFetchTool;

#[async_trait]
impl Tool for WebFetchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "webfetch".to_string(),
            description: "Fetch content from a URL and return it as text".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The URL to fetch"
                    },
                    "max_length": {
                        "type": "integer",
                        "description": "Maximum response length in characters (default 10000)"
                    }
                },
                "required": ["url"]
            }),
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let url = params["url"].as_str().ok_or_else(|| anyhow::anyhow!("missing 'url'"))?;
        let max_length = params["max_length"].as_u64().unwrap_or(10000) as usize;

        // Show spinner while fetching
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::with_template("{spinner:.cyan} {msg}")
                .unwrap_or_else(|_| ProgressStyle::default_spinner())
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
        );
        spinner.set_message(format!("fetching {}", url));
        spinner.enable_steady_tick(Duration::from_millis(80));
        spinner.tick(); // force an immediate draw so fast operations still show the spinner

        let response = reqwest::get(url).await?;
        let status = response.status();

        if !status.is_success() {
            spinner.finish_with_message(format!("✗ HTTP {}", status));
            anyhow::bail!("HTTP {} for {}", status, url);
        }

        let body = response.text().await?;
        let bytes = body.len();

        let truncated = if body.len() > max_length {
            // 找到不超过 max_length 的最后一个有效字符边界
            let end = body.floor_char_boundary(max_length);
            format!("{}...\n\n(truncated, {} total bytes)", &body[..end], body.len())
        } else {
            body
        };

        spinner.finish_with_message(format!("✓ {} bytes", bytes));
        Ok(truncated)
    }
}