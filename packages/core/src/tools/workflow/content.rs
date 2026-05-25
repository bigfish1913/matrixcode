//! Content Generation Tool
//!
//! AI 内容生成工具，用于工作流的最终输出节点

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};

use crate::tools::{Tool, ToolDefinition};
use crate::providers::Provider;
use std::sync::Arc;

/// AI 内容生成工具
pub struct ContentGenerationTool {
    provider: Arc<dyn Provider>,
}

impl ContentGenerationTool {
    pub fn new(provider: Arc<dyn Provider>) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl Tool for ContentGenerationTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "content_generation".to_string(),
            description: "使用 AI 生成内容（文章等）。需要 Provider 支持。".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "topic": {
                        "type": "string",
                        "description": "主题"
                    },
                    "research_data": {
                        "type": "string",
                        "description": "研究资料（可选）"
                    },
                    "image_urls": {
                        "type": "array",
                        "description": "图片 URL 列表（可选）",
                        "items": {"type": "string"}
                    },
                    "style": {
                        "type": "string",
                        "description": "写作风格（informative/casual/professional）",
                        "default": "informative"
                    }
                },
                "required": ["topic"]
            }),
            ..Default::default()
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let topic = params.get("topic")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("缺少 topic 参数"))?;

        let style = params.get("style")
            .and_then(|v| v.as_str())
            .unwrap_or("informative");

        // 构建 prompt
        let mut prompt = format!("主题: {}\n\n风格: {}\n\n", topic, style);

        if let Some(research) = params.get("research_data").and_then(|v| v.as_str()) {
            prompt.push_str(&format!("参考资料:\n{}\n\n", research));
        }

        if let Some(images) = params.get("image_urls") {
            // Handle both array of strings and array of objects
            if let Some(arr) = images.as_array() {
                if !arr.is_empty() {
                    prompt.push_str("\n**重要：请在文章中插入以下图片**（使用 Markdown 图片格式 `![描述](URL)`）：\n");
                    for (idx, img) in arr.iter().enumerate() {
                        // Check if it's a string URL or an object with url field
                        if let Some(url_str) = img.as_str() {
                            prompt.push_str(&format!("{}. ![图片{}]({})\n", idx + 1, idx + 1, url_str));
                        } else if let Some(obj) = img.as_object() {
                            let url = obj.get("url").and_then(|u| u.as_str()).unwrap_or("");
                            let desc = obj.get("description").and_then(|d| d.as_str()).unwrap_or("配图");
                            prompt.push_str(&format!("{}. ![{}({})\n", idx + 1, desc, url));
                        }
                    }
                    prompt.push_str("\n请将图片插入到文章的合适位置，使文章更加生动。\n");
                }
            }
        }

        prompt.push_str("\n请生成一篇完整的图文文章，**必须包含图片**。");

        // 调用 AI Provider
        let request = crate::providers::ChatRequest {
            messages: vec![crate::providers::Message {
                role: crate::providers::Role::User,
                content: crate::providers::MessageContent::Text(prompt),
            }],
            system: Some("你是一个专业的内容创作者。".to_string()),
            tools: vec![],
            think: false,
            max_tokens: 4096,
            server_tools: vec![],
            enable_caching: false,
        };

        match self.provider.chat(request).await {
            Ok(response) => {
                // 提取文本内容
                let content = response.content.iter()
                    .filter_map(|block| {
                        match block {
                            crate::providers::ContentBlock::Text { text } => Some(text.clone()),
                            _ => None,
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                // 构建图片画廊（自动添加到文章开头）
                let image_gallery = if let Some(images) = params.get("image_urls") {
                    if let Some(arr) = images.as_array() {
                        if !arr.is_empty() {
                            let mut gallery = String::from("\n## 📷 配图\n\n");
                            for (idx, img) in arr.iter().enumerate().take(5) { // 最多显示5张
                                if let Some(obj) = img.as_object() {
                                    let url = obj.get("url").and_then(|u| u.as_str()).unwrap_or("");
                                    let desc = obj.get("description").and_then(|d| d.as_str()).unwrap_or("配图");
                                    gallery.push_str(&format!("![{}({})\n\n", desc, url));
                                } else if let Some(url_str) = img.as_str() {
                                    gallery.push_str(&format!("![图片{}]({})\n\n", idx + 1, url_str));
                                }
                            }
                            gallery
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };

                // 将图片画廊添加到内容中
                let final_content = if image_gallery.is_empty() {
                    content
                } else {
                    format!("{}\n\n{}", image_gallery, content)
                };

                Ok(json!({
                    "content": final_content,
                    "topic": topic,
                    "style": style,
                    "word_count": final_content.chars().count()
                }).to_string())
            }
            Err(e) => {
                // 如果 AI 失败，返回占位内容
                Ok(json!({
                    "content": format!("主题《{}》的内容生成失败: {}", topic, e),
                    "topic": topic,
                    "style": style,
                    "error": true
                }).to_string())
            }
        }
    }
}