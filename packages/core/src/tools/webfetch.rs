use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};

use super::{Tool, ToolDefinition};

pub struct WebFetchTool;

#[async_trait]
impl Tool for WebFetchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "webfetch".to_string(),
            description: "从 URL 获取内容并返回为文本".to_string(),
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
                    }
                },
                "required": ["url"]
            }),
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let url = params["url"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'url'"))?;
        let max_length = params["max_length"].as_u64().unwrap_or(10000) as usize;

        // Show spinner while fetching - RAII guard ensures cleanup on error
        // let mut spinner = ToolSpinner::new(&format!("fetching {}", url));

        let response = reqwest::get(url).await?;
        let status = response.status();

        if !status.is_success() {
            // spinner.finish_error(&format!("HTTP {}", status));
            anyhow::bail!("HTTP {} for {}", status, url);
        }

        let body = response.text().await?;

        let truncated = if body.len() > max_length {
            // 找到不超过 max_length 的最后一个有效字符边界
            let end = body.floor_char_boundary(max_length);
            format!(
                "{}...\n\n(truncated, {} total bytes)",
                &body[..end],
                body.len()
            )
        } else {
            body
        };

        // spinner.finish_success(&format!("{} bytes", bytes));
        Ok(truncated)
    }
}
