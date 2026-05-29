//! 浏览器打开工具
//!
//! 使用系统默认浏览器打开指定的 URL

use async_trait::async_trait;
use serde_json::{Value, json};

use crate::tools::Tool;

/// 浏览器打开工具
pub struct BrowserOpenTool;

#[async_trait]
impl Tool for BrowserOpenTool {
    fn definition(&self) -> crate::tools::ToolDefinition {
        crate::tools::ToolDefinition {
            name: "browser_open".to_string(),
            description: "使用系统默认浏览器打开指定的 URL。适用于预览网页、文档或在线资源。"
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "要打开的 URL"
                    }
                },
                "required": ["url"]
            }),
            ..Default::default()
        }
    }

    async fn execute(&self, params: Value) -> anyhow::Result<String> {
        let url = params["url"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'url' parameter"))?;

        // 验证 URL 格式
        let url = url.trim();
        if url.is_empty() {
            return Err(anyhow::anyhow!("URL 不能为空"));
        }

        // 确保 URL 以 http:// 或 https:// 开头
        let url = if url.starts_with("http://") || url.starts_with("https://") {
            url.to_string()
        } else {
            format!("https://{}", url)
        };

        // 使用系统默认浏览器打开 URL
        #[cfg(target_os = "windows")]
        let result = {
            std::process::Command::new("cmd")
                .args(["/C", "start", &url])
                .spawn()
                .map(|_| ())
                .map_err(|e| anyhow::anyhow!("无法打开浏览器: {}", e))
        };

        #[cfg(target_os = "macos")]
        let result = {
            std::process::Command::new("open")
                .arg(&url)
                .spawn()
                .map(|_| ())
                .map_err(|e| anyhow::anyhow!("无法打开浏览器: {}", e))
        };

        #[cfg(target_os = "linux")]
        let result = {
            std::process::Command::new("xdg-open")
                .arg(&url)
                .spawn()
                .map(|_| ())
                .map_err(|e| anyhow::anyhow!("无法打开浏览器: {}", e))
        };

        result.map(|_| format!("已打开浏览器: {}", url))
    }
}
