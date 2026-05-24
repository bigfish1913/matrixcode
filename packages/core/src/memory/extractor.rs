//! Memory extraction: AI-based and rule-based detection.

use crate::truncate::truncate_chars;
use anyhow::Result;
use serde::Deserialize;

use super::config::*;
use super::keywords_config::KeywordsConfig;
use super::types::{AutoMemory, MemoryCategory, MemoryEntry};

// ============================================================================
// Memory Extractor Trait
// ============================================================================

/// Trait for memory extraction implementations.
#[async_trait::async_trait]
pub trait MemoryExtractor: Send + Sync {
    /// Extract memories from conversation text using AI.
    async fn extract(&self, text: &str, session_id: Option<&str>, project_path: Option<&str>) -> Result<Vec<MemoryEntry>>;

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

const MEMORY_EXTRACT_SYSTEM_PROMPT: &str = r#"你是一个记忆提取助手。从对话中提取值得长期记忆的关键信息。

记忆类型：
- decision: 项目或技术选型的决定
- preference: 用户习惯或偏好
- solution: 解决问题的具体方法
- finding: 重要发现或信息
- technical: 技术栈或框架信息
- structure: 项目结构信息

输出格式（严格 JSON）：
{
  "memories": [
    {
      "category": "decision",
      "content": "采用 PostgreSQL 作为主数据库",
      "importance": 85,
      "keywords": ["PostgreSQL", "数据库", "database"],
      "tags": ["backend", "storage"]
    }
  ]
}

关键词提取要求：
- 提取 3-5 个核心关键词（技术名词、项目名、关键概念）
- 中英文关键词都提取
- 用于后续记忆检索匹配

标签提取要求：
- 提取 1-3 个分类标签（如 backend、frontend、config、auth 等）
- 用于记忆分类筛选

只返回 JSON，不要其他解释。"#;

#[async_trait::async_trait]
impl MemoryExtractor for AiMemoryExtractor {
    async fn extract(&self, text: &str, session_id: Option<&str>, project_path: Option<&str>) -> Result<Vec<MemoryEntry>> {
        use crate::providers::{ChatRequest, Message, MessageContent, Role};

        // Safely truncate to ~4000 chars respecting UTF-8 boundaries
        let truncated = truncate_chars(text, 4000);

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

        parse_memory_response(&response_text, session_id, project_path)
    }

    fn model_name(&self) -> &str {
        &self.model
    }
}

fn parse_memory_response(json_text: &str, session_id: Option<&str>, project_path: Option<&str>) -> Result<Vec<MemoryEntry>> {
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
        #[serde(default)]
        keywords: Vec<String>,
        #[serde(default)]
        tags: Vec<String>,
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
                MemoryEntry::new(category, item.content, session_id.map(|s| s.to_string()), project_path.map(|p| p.to_string()));
            if item.importance > 0.0 {
                entry.importance = item.importance.clamp(0.0, 100.0);
            }
            // Add AI-extracted keywords and tags
            if !item.keywords.is_empty() {
                entry.tags.extend(item.keywords);
            }
            if !item.tags.is_empty() {
                entry.tags.extend(item.tags);
            }
            entry.tags.dedup();

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
// Rule-based Detection (uses KeywordsConfig)
// ============================================================================

/// Detect memories from text using configurable patterns.
pub fn detect_memories_fallback(text: &str, session_id: Option<&str>, project_path: Option<&str>) -> Vec<MemoryEntry> {
    let config = KeywordsConfig::load();
    let mut entries = Vec::new();
    let text_lower = text.to_lowercase();

    let categories = [
        (MemoryCategory::Decision, "decision"),
        (MemoryCategory::Preference, "preference"),
        (MemoryCategory::Solution, "solution"),
        (MemoryCategory::Finding, "finding"),
        (MemoryCategory::Technical, "technical"),
        (MemoryCategory::Structure, "structure"),
    ];

    for (category, key) in categories {
        let patterns = config
            .patterns
            .get(key)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        for keyword in patterns {
            if text_lower.contains(&keyword.to_lowercase()) {
                let content = extract_memory_content(text, keyword);
                if !content.is_empty() && content.len() >= MIN_MEMORY_CONTENT_LENGTH {
                    entries.push(MemoryEntry::new(
                        category,
                        content,
                        session_id.map(|s| s.to_string()),
                        project_path.map(|p| p.to_string()),
                    ));
                }
            }
        }
    }

    deduplicate_entries(entries)
}

/// Detect memories from text (wrapper for fallback).
pub fn detect_memories_from_text(text: &str, session_id: Option<&str>, project_path: Option<&str>) -> Vec<MemoryEntry> {
    detect_memories_fallback(text, session_id, project_path)
}

/// Smart detection: AI-first with rule-based fallback.
///
/// Priority order:
/// 1. AI extraction (if text > 200 chars and extractor available)
/// 2. Rule-based fallback (if AI fails or text too short)
pub async fn detect_memories_smart(
    text: &str,
    session_id: Option<&str>,
    project_path: Option<&str>,
    extractor: Option<&AiMemoryExtractor>,
) -> Vec<MemoryEntry> {
    let mode = AiDetectionMode::from_env();
    let text_len = text.len();

    // Determine if we should try AI first
    // Only use AI for text > 200 chars (avoid API overhead for short texts)
    let should_try_ai = mode != AiDetectionMode::Never && extractor.is_some() && text_len > 200;

    // Debug log: show method and model
    let model_name = extractor.map(|e| e.model_name()).unwrap_or("none");
    crate::debug::debug_log().memory_ai_detection(
        model_name,
        0, // Will update after detection
        text_len,
        should_try_ai,
    );

    if should_try_ai && let Some(ex) = extractor {
        if let Ok(ai_entries) = ex.extract(text, session_id, project_path).await {
            // AI succeeded - use AI results entirely (skip hardcoded rules)
            // Debug log: AI result
            crate::debug::debug_log().memory_ai_detection(
                ex.model_name(),
                ai_entries.len(),
                text_len,
                true,
            );
            return deduplicate_entries(ai_entries);
        }
        // AI failed - log and skip rule-based fallback (per user request)
        log::warn!("AI memory extraction failed, skipping detection for this turn");
        return Vec::new();
    }

    // For short texts (< 200 chars), skip detection entirely (per user request)
    // No rule-based fallback
    Vec::new()
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
