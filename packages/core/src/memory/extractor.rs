//! Memory extraction: AI-based and rule-based detection.

use crate::truncate::truncate_chars;
use anyhow::Result;
use serde::Deserialize;

use super::config::*;
use super::entry::{MemoryCategory, MemoryEntry};
use super::manager::AutoMemory;

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

    /// Create a minimal extractor (for background tasks, uses simplified prompt).
    /// This is more efficient for non-blocking background extraction.
    pub fn new_minimal(model: String) -> Self {
        // Create a minimal provider that uses the global config
        // This is for background tasks, so we use a simplified approach
        Self {
            provider: crate::create_minimal_provider(&model),
            model,
        }
    }
}

const MEMORY_EXTRACT_SYSTEM_PROMPT: &str = r#"你是记忆提取助手。从对话中提取值得长期记忆的关键信息。

# 记忆类型

<types>
<type>
    <name>decision</name>
    <description>项目或技术选型的决定</description>
    <when_to_save>用户明确做出技术决策时</when_to_save>
    <body_structure>先写决策内容，然后 **Why:** 决策原因，**Context:** 适用场景</body_structure>
</type>
<type>
    <name>preference</name>
    <description>用户习惯或偏好</description>
    <when_to_save>用户表达"我喜欢/习惯/偏好"时</when_to_save>
    <body_structure>先写偏好内容，然后 **Why:** 偏好原因（如有）</body_structure>
</type>
<type>
    <name>solution</name>
    <description>解决问题的具体方法</description>
    <when_to_save>问题成功解决且方法可复用时</when_to_save>
    <body_structure>先写解决方案，然后 **Problem:** 解决的问题，**Key:** 关键步骤</body_structure>
</type>
<type>
    <name>finding</name>
    <description>重要发现或信息</description>
    <when_to_save>发现非显而易见的信息时</when_to_save>
</type>
<type>
    <name>technical</name>
    <description>技术栈或框架信息</description>
    <when_to_save>确认项目使用的技术时</when_to_save>
</type>
<type>
    <name>structure</name>
    <description>项目结构信息</description>
    <when_to_save>发现关键入口或核心文件时</when_to_save>
</type>
</types>

# 不要保存什么到记忆中

- 代码路径、文件名、目录结构 — 可从项目实时获取
- Git 历史、最近更改 — git log/blame 是权威来源
- 临时状态：进行中的任务、当前对话上下文
- 已在 CLAUDE.md/MATRIX.md 中记录的内容
- 错误信息和调试细节 — 问题解决后无需保留

这些排除规则即使当用户要求保存时也适用。
如果他们要求保存临时信息，问："有什么 surprising 或 non-obvious 的部分？"

# 输出格式

严格 JSON：
{
  "memories": [
    {
      "category": "decision",
      "content": "采用 PostgreSQL 作为主数据库。**Why:** 性能要求和团队经验",
      "importance": 85,
      "keywords": ["PostgreSQL", "数据库", "database"],
      "tags": ["backend", "storage"]
    }
  ]
}

关键词提取：3-5 个核心关键词（技术名词、项目名、关键概念）
标签提取：1-3 个分类标签（backend、frontend、config、auth 等）

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

/// Detect memories from text using hard-coded patterns.
pub fn detect_memories_fallback(text: &str, session_id: Option<&str>, project_path: Option<&str>) -> Vec<MemoryEntry> {
    let mut entries = Vec::new();
    let text_lower = text.to_lowercase();

    // Hard-coded patterns for each category
    let patterns = [
        (MemoryCategory::Decision, ["决定", "选择", "采用", "定下", "decided", "chose"]),
        (MemoryCategory::Preference, ["偏好", "习惯", "喜欢", "首选", "prefer", "like"]),
        (MemoryCategory::Solution, ["解决", "修复", "搞定", "改成", "fixed", "solved"]),
        (MemoryCategory::Finding, ["发现", "原来", "原因", "定位", "found", "reason"]),
        (MemoryCategory::Technical, ["技术栈", "框架", "用的", "基于", "stack", "using"]),
        (MemoryCategory::Structure, ["入口", "主文件", "目录", "位于", "entry", "main"]),
    ];

    for (category, keywords) in patterns {
        for keyword in keywords {
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
