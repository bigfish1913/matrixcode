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

    /// 从参数中提取主题
    fn extract_topic(&self, params: &Value) -> Result<&str> {
        params.get("topic")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("缺少 topic 参数"))
    }

    /// 构建 AI prompt
    fn build_prompt(&self, params: &Value) -> String {
        let topic = params.get("topic")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let style = params.get("style")
            .and_then(|v| v.as_str())
            .unwrap_or("informative");

        let mut prompt = format!("主题: {}\n\n风格: {}\n\n", topic, style);

        if let Some(research) = params.get("research_data").and_then(|v| v.as_str()) {
            prompt.push_str(&format!("参考资料:\n{}\n\n", research));
        }

        self.append_image_instructions(&mut prompt, params);
        prompt.push_str("\n请生成一篇完整的图文文章，**必须包含图片**。");

        prompt
    }

    /// 添加图片插入指令
    fn append_image_instructions(&self, prompt: &mut String, params: &Value) {
        if let Some(images) = params.get("image_urls") {
            if let Some(arr) = images.as_array() {
                if !arr.is_empty() {
                    prompt.push_str("\n**重要：请在文章中插入以下图片**（使用 Markdown 图片格式 `![描述](URL)`）：\n");
                    for (idx, img) in arr.iter().enumerate() {
                        let (url, desc) = self.extract_image_info(img, idx);
                        prompt.push_str(&format!("{}. ![{}]({})\n", idx + 1, desc, url));
                    }
                    prompt.push_str("\n请将图片插入到文章的合适位置，使文章更加生动。\n");
                }
            }
        }
    }

    /// 从图片参数中提取 URL 和描述
    fn extract_image_info(&self, img: &Value, idx: usize) -> (String, String) {
        if let Some(url_str) = img.as_str() {
            (url_str.to_string(), format!("图片{}", idx + 1))
        } else if let Some(obj) = img.as_object() {
            let url = obj.get("url")
                .and_then(|u| u.as_str())
                .unwrap_or("")
                .to_string();
            let desc = obj.get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("配图")
                .to_string();
            (url, desc)
        } else {
            ("".to_string(), format!("图片{}", idx + 1))
        }
    }

    /// 构建图片画廊
    fn build_image_gallery(&self, params: &Value) -> String {
        if let Some(images) = params.get("image_urls") {
            if let Some(arr) = images.as_array() {
                if !arr.is_empty() {
                    let mut gallery = String::from("\n## 📷 配图\n\n");
                    for (idx, img) in arr.iter().enumerate().take(5) {
                        let (url, desc) = self.extract_image_info(img, idx);
                        if !url.is_empty() {
                            gallery.push_str(&format!("![{}]({})\n\n", desc, url));
                        }
                    }
                    return gallery;
                }
            }
        }
        String::new()
    }

    /// 调用 AI 生成内容
    async fn call_ai(&self, prompt: String) -> Result<String> {
        log::info!("ContentGenerationTool: starting for model {}", self.provider.model_name());

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
                log::info!("ContentGenerationTool: received successful response");
                Ok(self.extract_text_content(response))
            }
            Err(e) => {
                log::error!("ContentGenerationTool: AI call failed: {:?}", e);
                Err(e)
            }
        }
    }

    /// 从响应中提取文本内容
    fn extract_text_content(&self, response: crate::providers::ChatResponse) -> String {
        response.content.iter()
            .filter_map(|block| {
                match block {
                    crate::providers::ContentBlock::Text { text } => Some(text.clone()),
                    _ => None,
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 格式化最终响应
    fn format_response(&self, content: String, topic: &str, style: &str, params: &Value) -> String {
        let gallery = self.build_image_gallery(params);
        let final_content = if gallery.is_empty() {
            content
        } else {
            format!("{}\n\n{}", gallery, content)
        };

        json!({
            "content": final_content,
            "topic": topic,
            "style": style,
            "word_count": final_content.chars().count()
        }).to_string()
    }

    /// 格式化错误响应
    fn format_error_response(&self, topic: &str, style: &str, error: anyhow::Error) -> String {
        json!({
            "content": format!("主题《{}》的内容生成失败: {}", topic, error),
            "topic": topic,
            "style": style,
            "error": true
        }).to_string()
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
        let topic = self.extract_topic(&params)?;
        let style = params.get("style")
            .and_then(|v| v.as_str())
            .unwrap_or("informative");

        let prompt = self.build_prompt(&params);

        match self.call_ai(prompt).await {
            Ok(content) => Ok(self.format_response(content, topic, style, &params)),
            Err(e) => Ok(self.format_error_response(topic, style, e)),
        }
    }
}