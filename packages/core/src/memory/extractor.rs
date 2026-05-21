//! Memory extraction: AI-based and rule-based detection.

use anyhow::Result;
use serde::Deserialize;

use super::config::*;
use super::types::{AutoMemory, MemoryCategory, MemoryEntry};

// ============================================================================
// Memory Extractor Trait
// ============================================================================

/// Trait for memory extraction implementations.
#[async_trait::async_trait]
pub trait MemoryExtractor: Send + Sync {
    /// Extract memories from conversation text using AI.
    async fn extract(&self, text: &str, session_id: Option<&str>) -> Result<Vec<MemoryEntry>>;

    /// Get the model name used for extraction.
    fn model_name(&self) -> &str;
}

/// AI-based memory extractor using a fast/cheap model.
pub struct AiMemoryExtractor {
    provider: Box<dyn crate::providers::Provider>,
    model: String,
}

impl AiMemoryExtractor {
    /// Create a new AI memory extractor.
    pub fn new(provider: Box<dyn crate::providers::Provider>, model: String) -> Self {
        Self { provider, model }
    }
}

const MEMORY_EXTRACT_SYSTEM_PROMPT: &str = r#"你是一个记忆提取助手。你的任务是从对话中识别并提取值得长期记忆的关键信息。

记忆类型：
1. Decision（决策）: 项目或技术选型的决定
2. Preference（偏好）: 用户习惯或偏好
3. Solution（解决方案）: 解决问题的具体方法
4. Finding（发现）: 重要发现或信息
5. Technical（技术）: 技术栈或框架信息
6. Structure（结构）: 项目结构信息

输出格式（严格 JSON）：
{"memories": [{"category": "decision", "content": "...", "importance": 90}]}
"#;

#[async_trait::async_trait]
impl MemoryExtractor for AiMemoryExtractor {
    async fn extract(&self, text: &str, session_id: Option<&str>) -> Result<Vec<MemoryEntry>> {
        use crate::providers::{ChatRequest, Message, MessageContent, Role};

        let truncated = if text.len() > 4000 {
            &text[..4000]
        } else {
            text
        };

        let request = ChatRequest {
            messages: vec![Message {
                role: Role::User,
                content: MessageContent::Text(format!(
                    "请从以下对话中提取值得记忆的关键信息：\n\n{}",
                    truncated
                )),
            }],
            tools: vec![],
            system: Some(MEMORY_EXTRACT_SYSTEM_PROMPT.to_string()),
            think: false,
            max_tokens: 512,
            server_tools: vec![],
            enable_caching: false,
        };

        let response = self.provider.chat(request).await?;

        let response_text = response
            .content
            .iter()
            .filter_map(|b| {
                if let crate::providers::ContentBlock::Text { text } = b {
                    Some(text.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("");

        parse_memory_response(&response_text, session_id)
    }

    fn model_name(&self) -> &str {
        &self.model
    }
}

fn parse_memory_response(json_text: &str, session_id: Option<&str>) -> Result<Vec<MemoryEntry>> {
    let cleaned = json_text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    #[derive(Deserialize)]
    struct MemoryResponse {
        memories: Vec<MemoryItem>,
    }

    #[derive(Deserialize)]
    struct MemoryItem {
        category: String,
        content: String,
        #[serde(default)]
        importance: f64,
    }

    let parsed: MemoryResponse = serde_json::from_str(cleaned)?;

    let entries = parsed
        .memories
        .into_iter()
        .filter_map(|item| {
            let category = match item.category.to_lowercase().as_str() {
                "decision" => MemoryCategory::Decision,
                "preference" => MemoryCategory::Preference,
                "solution" => MemoryCategory::Solution,
                "finding" => MemoryCategory::Finding,
                "technical" => MemoryCategory::Technical,
                "structure" => MemoryCategory::Structure,
                _ => return None,
            };

            if item.content.len() < MIN_MEMORY_CONTENT_LENGTH {
                return None;
            }

            let mut entry =
                MemoryEntry::new(category, item.content, session_id.map(|s| s.to_string()));
            if item.importance > 0.0 {
                entry.importance = item.importance.clamp(0.0, 100.0);
            }

            Some(entry)
        })
        .collect();

    Ok(deduplicate_entries(entries))
}

fn deduplicate_entries(entries: Vec<MemoryEntry>) -> Vec<MemoryEntry> {
    let mut seen: Vec<String> = Vec::new();
    entries
        .into_iter()
        .filter(|e| {
            let content_lower = e.content.to_lowercase();
            if seen.iter().any(|s| {
                AutoMemory::calculate_similarity(s, &content_lower) >= SIMILARITY_THRESHOLD
            }) {
                false
            } else {
                seen.push(content_lower);
                true
            }
        })
        .take(MAX_DETECTED_ENTRIES)
        .collect()
}

// ============================================================================
// Rule-based Detection
// ============================================================================

/// Detect memories from text using rule-based patterns.
pub fn detect_memories_fallback(text: &str, session_id: Option<&str>) -> Vec<MemoryEntry> {
    let mut entries = Vec::new();
    let text_lower = text.to_lowercase();

    let patterns: Vec<(MemoryCategory, Vec<&str>)> = vec![
        (
            MemoryCategory::Decision,
            vec![
                "最终决定",
                "决定采用",
                "我们决定",
                "选择使用",
                "采用方案",
                "定下来",
                "就定这个",
                "敲定",
                "拍板",
                "we decided",
                "final decision",
            ],
        ),
        (
            MemoryCategory::Preference,
            vec![
                "我喜欢",
                "我偏好",
                "我习惯",
                "最常用",
                "一直用",
                "推荐",
                "建议使用",
                "首选",
                "i like",
                "i prefer",
            ],
        ),
        (
            MemoryCategory::Solution,
            vec![
                "通过修改",
                "解决方案是",
                "搞定",
                "解决了",
                "修复成功",
                "改成",
                "优化了",
                "fixed by",
                "solved by",
            ],
        ),
        (
            MemoryCategory::Finding,
            vec![
                "发现",
                "注意到",
                "原来",
                "找到问题",
                "定位到",
                "排查发现",
                "原因是",
                "found that",
                "discovered",
            ],
        ),
        (
            MemoryCategory::Technical,
            vec![
                "技术栈是",
                "框架使用",
                "用的是",
                "基于",
                "tech stack",
                "using framework",
                "built with",
            ],
        ),
        (
            MemoryCategory::Structure,
            vec![
                "入口文件是",
                "主文件位于",
                "项目结构是",
                "入口是",
                "目录是",
                "entry point",
                "main file",
            ],
        ),
    ];

    for (category, keywords) in patterns {
        for keyword in keywords {
            if text_lower.contains(keyword) {
                let content = extract_memory_content(text, keyword);
                if !content.is_empty() && content.len() >= MIN_MEMORY_CONTENT_LENGTH {
                    entries.push(MemoryEntry::new(
                        category,
                        content,
                        session_id.map(|s| s.to_string()),
                    ));
                }
            }
        }
    }

    deduplicate_entries(entries)
}

/// Detect memories from text (wrapper for fallback).
pub fn detect_memories_from_text(text: &str, session_id: Option<&str>) -> Vec<MemoryEntry> {
    detect_memories_fallback(text, session_id)
}

/// Smart detection: rule-based + AI fallback.
pub async fn detect_memories_smart(
    text: &str,
    session_id: Option<&str>,
    extractor: Option<&AiMemoryExtractor>,
) -> Vec<MemoryEntry> {
    // First try rule-based
    let rule_entries = detect_memories_fallback(text, session_id);

    // Check if we need AI fallback
    let mode = AiDetectionMode::from_env();
    if mode.should_use_ai_for_text(text.len())
        && extractor.is_some()
        && let Some(ex) = extractor
        && let Ok(ai_entries) = ex.extract(text, session_id).await
    {
        // Combine and deduplicate
        let combined = rule_entries.into_iter().chain(ai_entries).collect();
        return deduplicate_entries(combined);
    }

    rule_entries
}

fn extract_memory_content(text: &str, keyword: &str) -> String {
    let text_lower = text.to_lowercase();
    let keyword_lower = keyword.to_lowercase();

    let pos = match text_lower.find(&keyword_lower) {
        Some(p) => p,
        None => return String::new(),
    };

    // Find sentence containing the keyword
    let start = text[..pos]
        .rfind(['.', '。', '\n'])
        .map(|i| i + 1)
        .unwrap_or(0);

    let end = text[pos..]
        .find(['.', '。', '\n'])
        .map(|i| pos + i + 1)
        .unwrap_or(text.len());

    let sentence = text[start..end].trim();

    if sentence.len() > MAX_MEMORY_CONTENT_LENGTH {
        sentence[..MAX_MEMORY_CONTENT_LENGTH].to_string()
    } else {
        sentence.to_string()
    }
}

/// Infer category from content.
pub fn infer_category_from_content(content: &str) -> MemoryCategory {
    let lower = content.to_lowercase();

    if lower.contains("决定")
        || lower.contains("选择")
        || lower.contains("采用")
        || lower.contains("decided")
    {
        return MemoryCategory::Decision;
    }
    if lower.contains("喜欢")
        || lower.contains("偏好")
        || lower.contains("习惯")
        || lower.contains("prefer")
    {
        return MemoryCategory::Preference;
    }
    if lower.contains("解决")
        || lower.contains("修复")
        || lower.contains("搞定")
        || lower.contains("fixed")
    {
        return MemoryCategory::Solution;
    }
    if lower.contains("发现")
        || lower.contains("原因")
        || lower.contains("原来")
        || lower.contains("found")
    {
        return MemoryCategory::Finding;
    }
    if lower.contains("技术")
        || lower.contains("框架")
        || lower.contains("库")
        || lower.contains("tech")
    {
        return MemoryCategory::Technical;
    }
    if lower.contains("文件")
        || lower.contains("目录")
        || lower.contains("入口")
        || lower.contains("file")
    {
        return MemoryCategory::Structure;
    }

    MemoryCategory::Finding // Default
}
