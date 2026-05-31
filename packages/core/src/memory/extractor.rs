//! Memory extraction: AI-based and rule-based detection.

use crate::truncate::truncate_chars;
use anyhow::Result;
use serde::Deserialize;

use super::config::*;
use super::entry::{MemoryCategory, MemoryEntry};
use super::manager::AutoMemory;
use super::conversation_pattern::{ConversationPattern, PatternType, PatternSource};
use super::unified_extraction::{UnifiedExtractionResult, ExtractedKeywords};
use crate::compress::FocusPoint;

// ============================================================================
// Memory Extractor Trait
// ============================================================================

/// Trait for memory extraction implementations.
#[async_trait::async_trait]
pub trait MemoryExtractor: Send + Sync {
    /// Extract memories and focus points from conversation text using AI.
    async fn extract(
        &self,
        text: &str,
        session_id: Option<&str>,
        project_path: Option<&str>,
    ) -> Result<ExtractionResult>;

    /// Get the model name used for extraction.
    fn model_name(&self) -> &str;
}

/// Result of memory extraction (memories + focus points + conversation patterns).
#[derive(Debug, Clone)]
pub struct ExtractionResult {
    pub memories: Vec<MemoryEntry>,
    pub focus_points: Vec<FocusPoint>,
    /// Extracted conversation patterns (reference and code patterns).
    pub conversation_patterns: Vec<ConversationPattern>,
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

# 对话模式提取

当对话文本较长时（超过500字符），还要提取对话中使用的模式：

1. **引用模式 (reference)**：用户如何引用之前的内容
   - 示例："正如前面所说"、"接着刚才的话题"、"as mentioned"、"previously"

2. **代码模式 (code)**：对话中涉及的代码风格关键词
   - 示例：语言关键词（fn, function, class）、代码块标记（```）

模式提取规则：
- 只提取明确出现的模式，不要推测
- confidence 范围 0.0-1.0，越常见越低（常见模式置信度低）
- 只在文本 > 500 字符时提取模式

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
  ],
  "focus_points": [],
  "conversation_patterns": [
    {
      "pattern_type": "reference",
      "pattern": "正如我所说",
      "confidence": 0.8
    },
    {
      "pattern_type": "code",
      "pattern": "fn ",
      "confidence": 0.6
    }
  ]
}

关键词提取：3-5 个核心关键词（技术名词、项目名、关键概念）
标签提取：1-3 个分类标签（backend、frontend、config、auth 等）

只返回 JSON，不要其他解释。"#;

#[async_trait::async_trait]
impl MemoryExtractor for AiMemoryExtractor {
    async fn extract(
        &self,
        text: &str,
        session_id: Option<&str>,
        project_path: Option<&str>,
    ) -> Result<ExtractionResult> {
        use crate::providers::{ChatRequest, Message, MessageContent, Role};

        // Safely truncate to ~4000 chars respecting UTF-8 boundaries
        let truncated = truncate_chars(text, 4000);

        let request = ChatRequest {
            messages: vec![Message {
                role: Role::User,
                content: MessageContent::Text(format!(
                    "请从以下对话中提取值得记忆的关键信息和当前聚焦点：\n\n{}",
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

fn parse_memory_response(
    json_text: &str,
    session_id: Option<&str>,
    project_path: Option<&str>,
) -> Result<ExtractionResult> {
    let cleaned = json_text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    #[derive(Deserialize)]
    struct MemoryResponse {
        memories: Vec<MemoryItem>,
        #[serde(default)]
        focus_points: Vec<FocusPointItem>,
        #[serde(default)]
        conversation_patterns: Vec<ConversationPatternItem>,
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

    #[derive(Deserialize)]
    struct FocusPointItem {
        topic: String,
        #[serde(default)]
        keywords: Vec<String>,
        #[serde(default)]
        entities: Vec<String>,
        #[serde(default)]
        core_question: Option<String>,
        #[serde(default = "default_importance")]
        importance: f32,
        #[serde(default = "default_is_current")]
        is_current: bool,
    }

    #[derive(Deserialize)]
    struct ConversationPatternItem {
        pattern_type: String,
        pattern: String,
        #[serde(default)]
        confidence: f32,
    }

    fn default_importance() -> f32 { 0.7 }
    fn default_is_current() -> bool { true }

    let parsed: MemoryResponse = serde_json::from_str(cleaned)?;

    // Parse memories
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

            let mut entry = MemoryEntry::new(
                category,
                item.content,
                session_id.map(|s| s.to_string()),
                project_path.map(|p| p.to_string()),
            );
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

    // Parse focus points
    use chrono::Utc;
    use crate::compress::FocusStatus;

    let focus_points = parsed
        .focus_points
        .into_iter()
        .map(|item| {
            let mut focus = FocusPoint::new(
                format!("focus-{}", Utc::now().timestamp()),
                item.topic,
                item.keywords,
                item.entities,
                item.core_question,
                0,
            );
            focus.importance = item.importance.clamp(0.0, 1.0);
            if !item.is_current {
                focus.status = FocusStatus::Suspended;
            }
            focus
        })
        .collect();

    // Parse conversation patterns
    let conversation_patterns = parsed
        .conversation_patterns
        .into_iter()
        .filter_map(|item| {
            // Parse pattern type
            let pattern_type = match item.pattern_type.to_lowercase().as_str() {
                "reference" => PatternType::Reference,
                "code" => PatternType::Code,
                _ => return None, // Skip unknown pattern types
            };

            // Skip empty patterns
            if item.pattern.trim().is_empty() {
                return None;
            }

            // Create pattern with UserConversation source
            let mut pattern = ConversationPattern::new(
                pattern_type,
                item.pattern,
                PatternSource::UserConversation {
                    example: String::new(), // Will be filled when pattern is used
                },
            );

            // Set confidence (default to 0.5 if not specified or out of range)
            pattern.confidence = if item.confidence > 0.0 {
                item.confidence.clamp(0.0, 1.0)
            } else {
                0.5
            };

            Some(pattern)
        })
        .collect();

    Ok(ExtractionResult {
        memories: deduplicate_entries(entries),
        focus_points,
        conversation_patterns,
    })
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
pub fn detect_memories_fallback(
    text: &str,
    session_id: Option<&str>,
    project_path: Option<&str>,
) -> Vec<MemoryEntry> {
    let mut entries = Vec::new();
    let text_lower = text.to_lowercase();

    // Hard-coded patterns for each category
    let patterns = [
        (
            MemoryCategory::Decision,
            ["决定", "选择", "采用", "定下", "decided", "chose"],
        ),
        (
            MemoryCategory::Preference,
            ["偏好", "习惯", "喜欢", "首选", "prefer", "like"],
        ),
        (
            MemoryCategory::Solution,
            ["解决", "修复", "搞定", "改成", "fixed", "solved"],
        ),
        (
            MemoryCategory::Finding,
            ["发现", "原来", "原因", "定位", "found", "reason"],
        ),
        (
            MemoryCategory::Technical,
            ["技术栈", "框架", "用的", "基于", "stack", "using"],
        ),
        (
            MemoryCategory::Structure,
            ["入口", "主文件", "目录", "位于", "entry", "main"],
        ),
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
pub fn detect_memories_from_text(
    text: &str,
    session_id: Option<&str>,
    project_path: Option<&str>,
) -> Vec<MemoryEntry> {
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
) -> ExtractionResult {
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
        if let Ok(result) = ex.extract(text, session_id, project_path).await {
            // AI succeeded - use AI results entirely (skip hardcoded rules)
            // Debug log: AI result
            crate::debug::debug_log().memory_ai_detection(
                ex.model_name(),
                result.memories.len(),
                text_len,
                true,
            );
            return result;
        }
        // AI failed - log and skip rule-based fallback (per user request)
        log::warn!("AI memory extraction failed, skipping detection for this turn");
        return ExtractionResult {
            memories: vec![],
            focus_points: vec![],
            conversation_patterns: vec![],
        };
    }

    // For short texts (< 200 chars), skip detection entirely (per user request)
    // No rule-based fallback
    ExtractionResult {
        memories: vec![],
        focus_points: vec![],
        conversation_patterns: vec![],
    }
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

// ============================================================================
// Unified Extraction (One AI Call for All Information)
// ============================================================================

/// Unified extraction system prompt for extracting all information in one call.
const UNIFIED_EXTRACTION_PROMPT: &str = r#"你是信息提取助手。从对话中一次性提取以下信息：

## 1. 长期记忆 (memories)
- decision: 技术决策（如"决定使用 PostgreSQL"、"采用 React 架构"）
- preference: 用户偏好（如"我喜欢简洁的代码风格"、"习惯用 VS Code"）
- solution: 解决方案（如"通过添加缓存解决了性能问题"）
- finding: 重要发现（如"发现内存泄漏的原因"）
- technical: 技术栈（如"项目使用 Rust + Tokio"）
- structure: 项目结构（如"主入口是 src/main.rs"）

## 2. 当前焦点 (focus_points)
- topic: 当前讨论的主题
- keywords: 相关关键词
- entities: 涉及的文件/函数/类名
- core_question: 核心问题（可选）

## 3. 对话模式 (conversation_patterns)
- reference: 引用模式（如"正如前面所说"、"as mentioned"、"previously"）
- code: 代码模式（如"fn ", "function", "```", "class "）

## 4. 焦点关键词 (focus_keywords)
- transition: 话题转换词（如"换个话题", "switching", "however", "等等"）
- question: 提问词（如"怎么", "how", "为什么", "why", "请问"）
- task: 任务词（如"帮我", "implement", "创建", "create", "修复"）
- tech: 技术词（如"rust", "数据库", "api", "性能", "优化"）

## 输出格式（严格 JSON）

```json
{
  "memories": [
    {
      "category": "decision",
      "content": "采用 PostgreSQL 作为主数据库。**Why:** 性能要求",
      "importance": 85,
      "keywords": ["PostgreSQL", "数据库"],
      "tags": ["backend", "storage"]
    }
  ],
  "focus_points": [
    {
      "topic": "API 设计优化",
      "keywords": ["API", "REST", "性能"],
      "entities": ["api.rs", "handler"],
      "core_question": "如何优化 API 响应时间？",
      "importance": 0.8,
      "is_current": true
    }
  ],
  "conversation_patterns": [
    {
      "pattern_type": "reference",
      "pattern": "正如我所说",
      "confidence": 0.8
    },
    {
      "pattern_type": "code",
      "pattern": "fn ",
      "confidence": 0.6
    }
  ],
  "focus_keywords": {
    "transition": ["换个话题", "switching"],
    "question": ["怎么", "how"],
    "task": ["帮我", "implement"],
    "tech": ["rust", "性能"]
  }
}
```

## 规则
1. 只提取明确出现的信息，不要推测
2. 如果某类信息没有，返回空数组/对象
3. importance 范围：memories 0-100，focus_points 0.0-1.0
4. confidence 范围：0.0-1.0，常见模式置信度较低
5. 关键词提取 3-5 个核心关键词
6. 只返回 JSON，不要其他解释"#;

/// Unified extractor that extracts all information in a single AI call.
///
/// This replaces the separate AiMemoryExtractor and FocusExtractor,
/// reducing API calls and providing consistent extraction.
pub struct UnifiedExtractor {
    provider: Box<dyn crate::providers::Provider>,
    model: String,
}

impl UnifiedExtractor {
    /// Create a new unified extractor.
    pub fn new(provider: Box<dyn crate::providers::Provider>, model: String) -> Self {
        Self { provider, model }
    }

    /// Create a minimal unified extractor for background tasks.
    pub fn new_minimal(model: String) -> Self {
        Self {
            provider: crate::create_minimal_provider(&model),
            model,
        }
    }

    /// Extract all information from conversation text in a single AI call.
    pub async fn extract_unified(
        &self,
        text: &str,
        session_id: Option<&str>,
        project_path: Option<&str>,
    ) -> Result<UnifiedExtractionResult> {
        use crate::providers::{ChatRequest, Message, MessageContent, Role};

        // Safely truncate to ~4000 chars respecting UTF-8 boundaries
        let truncated = truncate_chars(text, 4000);

        let request = ChatRequest {
            messages: vec![Message {
                role: Role::User,
                content: MessageContent::Text(format!(
                    "请从以下对话中提取所有信息：\n\n{}",
                    truncated
                )),
            }],
            tools: vec![],
            system: Some(UNIFIED_EXTRACTION_PROMPT.to_string()),
            think: false,
            max_tokens: 1024, // Larger token limit for unified extraction
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

        parse_unified_response(&response_text, session_id, project_path)
    }

    /// Get the model name used for extraction.
    pub fn model_name(&self) -> &str {
        &self.model
    }
}

/// Parse unified extraction response from AI.
fn parse_unified_response(
    json_text: &str,
    session_id: Option<&str>,
    project_path: Option<&str>,
) -> Result<UnifiedExtractionResult> {
    let cleaned = json_text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    #[derive(Deserialize)]
    struct UnifiedResponse {
        #[serde(default)]
        memories: Vec<MemoryItem>,
        #[serde(default)]
        focus_points: Vec<FocusPointItem>,
        #[serde(default)]
        conversation_patterns: Vec<ConversationPatternItem>,
        #[serde(default)]
        focus_keywords: FocusKeywordsItem,
    }

    #[derive(Deserialize, Default)]
    struct FocusKeywordsItem {
        #[serde(default)]
        transition: Vec<String>,
        #[serde(default)]
        question: Vec<String>,
        #[serde(default)]
        task: Vec<String>,
        #[serde(default)]
        tech: Vec<String>,
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

    #[derive(Deserialize)]
    struct FocusPointItem {
        topic: String,
        #[serde(default)]
        keywords: Vec<String>,
        #[serde(default)]
        entities: Vec<String>,
        #[serde(default)]
        core_question: Option<String>,
        #[serde(default = "default_importance")]
        importance: f32,
        #[serde(default = "default_is_current")]
        is_current: bool,
    }

    #[derive(Deserialize)]
    struct ConversationPatternItem {
        pattern_type: String,
        pattern: String,
        #[serde(default)]
        confidence: f32,
    }

    fn default_importance() -> f32 { 0.7 }
    fn default_is_current() -> bool { true }

    let parsed: UnifiedResponse = serde_json::from_str(cleaned)?;

    // Parse memories (reuse existing logic)
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

            let mut entry = MemoryEntry::new(
                category,
                item.content,
                session_id.map(|s| s.to_string()),
                project_path.map(|p| p.to_string()),
            );
            if item.importance > 0.0 {
                entry.importance = item.importance.clamp(0.0, 100.0);
            }
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

    // Parse focus points (reuse existing logic)
    use chrono::Utc;
    use crate::compress::FocusStatus;

    let focus_points = parsed
        .focus_points
        .into_iter()
        .map(|item| {
            let mut focus = FocusPoint::new(
                format!("focus-{}", Utc::now().timestamp()),
                item.topic,
                item.keywords,
                item.entities,
                item.core_question,
                0,
            );
            focus.importance = item.importance.clamp(0.0, 1.0);
            if !item.is_current {
                focus.status = FocusStatus::Suspended;
            }
            focus
        })
        .collect();

    // Parse conversation patterns (reuse existing logic)
    let conversation_patterns = parsed
        .conversation_patterns
        .into_iter()
        .filter_map(|item| {
            let pattern_type = match item.pattern_type.to_lowercase().as_str() {
                "reference" => PatternType::Reference,
                "code" => PatternType::Code,
                _ => return None,
            };

            if item.pattern.trim().is_empty() {
                return None;
            }

            let mut pattern = ConversationPattern::new(
                pattern_type,
                item.pattern,
                PatternSource::UserConversation {
                    example: String::new(),
                },
            );

            pattern.confidence = if item.confidence > 0.0 {
                item.confidence.clamp(0.0, 1.0)
            } else {
                0.5
            };

            Some(pattern)
        })
        .collect();

    // Parse focus keywords
    let focus_keywords = ExtractedKeywords {
        transition: parsed.focus_keywords.transition,
        question: parsed.focus_keywords.question,
        task: parsed.focus_keywords.task,
        tech: parsed.focus_keywords.tech,
    };

    Ok(UnifiedExtractionResult {
        memories: deduplicate_entries(entries),
        focus_points,
        conversation_patterns,
        focus_keywords,
    })
}

/// Smart unified extraction: AI-first with graceful fallback.
///
/// Uses UnifiedExtractor for single API call extraction.
pub async fn detect_unified_smart(
    text: &str,
    session_id: Option<&str>,
    project_path: Option<&str>,
    extractor: Option<&UnifiedExtractor>,
) -> UnifiedExtractionResult {
    let mode = AiDetectionMode::from_env();
    let text_len = text.len();

    // Only use AI for text > 200 chars
    let should_try_ai = mode != AiDetectionMode::Never && extractor.is_some() && text_len > 200;

    if should_try_ai && let Some(ex) = extractor {
        if let Ok(result) = ex.extract_unified(text, session_id, project_path).await {
            return result;
        }
        // AI failed - skip detection for this turn
        log::warn!("Unified extraction failed, skipping detection for this turn");
    }

    // Return empty result for short texts or failed AI
    UnifiedExtractionResult::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Conversation Pattern Parsing Tests
    // =========================================================================

    #[test]
    fn test_parse_memory_response_with_patterns() {
        let json = r#"{
            "memories": [],
            "focus_points": [],
            "conversation_patterns": [
                {
                    "pattern_type": "reference",
                    "pattern": "正如我所说",
                    "confidence": 0.8
                },
                {
                    "pattern_type": "code",
                    "pattern": "fn ",
                    "confidence": 0.6
                }
            ]
        }"#;

        let result = parse_memory_response(json, None, None).unwrap();
        assert_eq!(result.memories.len(), 0);
        assert_eq!(result.focus_points.len(), 0);
        assert_eq!(result.conversation_patterns.len(), 2);

        // Check first pattern (reference)
        let ref_pattern = &result.conversation_patterns[0];
        assert_eq!(ref_pattern.pattern_type, PatternType::Reference);
        assert_eq!(ref_pattern.pattern, "正如我所说");
        assert_eq!(ref_pattern.confidence, 0.8);
        assert!(ref_pattern.is_active);

        // Check second pattern (code)
        let code_pattern = &result.conversation_patterns[1];
        assert_eq!(code_pattern.pattern_type, PatternType::Code);
        assert_eq!(code_pattern.pattern, "fn ");
        assert_eq!(code_pattern.confidence, 0.6);
    }

    #[test]
    fn test_parse_memory_response_patterns_default_confidence() {
        let json = r#"{
            "memories": [],
            "focus_points": [],
            "conversation_patterns": [
                {
                    "pattern_type": "reference",
                    "pattern": "as mentioned"
                }
            ]
        }"#;

        let result = parse_memory_response(json, None, None).unwrap();
        assert_eq!(result.conversation_patterns.len(), 1);

        // Default confidence should be 0.5
        let pattern = &result.conversation_patterns[0];
        assert_eq!(pattern.confidence, 0.5);
    }

    #[test]
    fn test_parse_memory_response_patterns_empty() {
        let json = r#"{
            "memories": [],
            "focus_points": []
        }"#;

        let result = parse_memory_response(json, None, None).unwrap();
        assert_eq!(result.conversation_patterns.len(), 0);
    }

    #[test]
    fn test_parse_memory_response_patterns_invalid_type() {
        let json = r#"{
            "memories": [],
            "focus_points": [],
            "conversation_patterns": [
                {
                    "pattern_type": "invalid_type",
                    "pattern": "test",
                    "confidence": 0.5
                },
                {
                    "pattern_type": "reference",
                    "pattern": "valid pattern",
                    "confidence": 0.7
                }
            ]
        }"#;

        let result = parse_memory_response(json, None, None).unwrap();
        // Invalid pattern type should be skipped
        assert_eq!(result.conversation_patterns.len(), 1);
        assert_eq!(result.conversation_patterns[0].pattern, "valid pattern");
    }

    #[test]
    fn test_parse_memory_response_patterns_empty_string() {
        let json = r#"{
            "memories": [],
            "focus_points": [],
            "conversation_patterns": [
                {
                    "pattern_type": "reference",
                    "pattern": "",
                    "confidence": 0.5
                },
                {
                    "pattern_type": "code",
                    "pattern": "   ",
                    "confidence": 0.5
                },
                {
                    "pattern_type": "reference",
                    "pattern": "valid",
                    "confidence": 0.8
                }
            ]
        }"#;

        let result = parse_memory_response(json, None, None).unwrap();
        // Empty patterns should be skipped
        assert_eq!(result.conversation_patterns.len(), 1);
        assert_eq!(result.conversation_patterns[0].pattern, "valid");
    }

    #[test]
    fn test_parse_memory_response_patterns_confidence_clamped() {
        let json = r#"{
            "memories": [],
            "focus_points": [],
            "conversation_patterns": [
                {
                    "pattern_type": "reference",
                    "pattern": "test1",
                    "confidence": 1.5
                },
                {
                    "pattern_type": "code",
                    "pattern": "test2",
                    "confidence": -0.3
                }
            ]
        }"#;

        let result = parse_memory_response(json, None, None).unwrap();
        assert_eq!(result.conversation_patterns.len(), 2);

        // Confidence should be clamped to [0.0, 1.0]
        assert_eq!(result.conversation_patterns[0].confidence, 1.0);
        // Negative confidence should use default 0.5 (since <= 0.0 triggers default)
        assert_eq!(result.conversation_patterns[1].confidence, 0.5);
    }

    #[test]
    fn test_parse_memory_response_patterns_source() {
        let json = r#"{
            "memories": [],
            "focus_points": [],
            "conversation_patterns": [
                {
                    "pattern_type": "reference",
                    "pattern": "PR #123",
                    "confidence": 0.9
                }
            ]
        }"#;

        let result = parse_memory_response(json, None, None).unwrap();
        let pattern = &result.conversation_patterns[0];

        // Source should be UserConversation
        match &pattern.source {
            PatternSource::UserConversation { example } => {
                assert_eq!(example, "");
            }
            _ => panic!("Expected UserConversation source"),
        }
    }

    #[test]
    fn test_parse_memory_response_backward_compatible() {
        // Old format without conversation_patterns should still work
        let json = r#"{
            "memories": [
                {
                    "category": "decision",
                    "content": "使用 Rust 作为主要语言",
                    "importance": 80,
                    "keywords": ["Rust"],
                    "tags": ["backend"]
                }
            ],
            "focus_points": [
                {
                    "topic": "API设计",
                    "keywords": ["API", "REST"],
                    "importance": 0.8
                }
            ]
        }"#;

        let result = parse_memory_response(json, None, None).unwrap();
        assert_eq!(result.memories.len(), 1);
        assert_eq!(result.focus_points.len(), 1);
        assert_eq!(result.conversation_patterns.len(), 0);

        // Verify memory content
        assert_eq!(result.memories[0].category, MemoryCategory::Decision);
        assert!(result.memories[0].content.contains("Rust"));
    }

    #[test]
    fn test_parse_memory_response_with_code_block_markers() {
        // JSON wrapped in code block markers should still parse
        let json = r#"```json
{
    "memories": [],
    "focus_points": [],
    "conversation_patterns": [
        {
            "pattern_type": "code",
            "pattern": "```",
            "confidence": 0.7
        }
    ]
}
```"#;

        let result = parse_memory_response(json, None, None).unwrap();
        assert_eq!(result.conversation_patterns.len(), 1);
        assert_eq!(result.conversation_patterns[0].pattern, "```");
    }

    // =========================================================================
    // ExtractionResult Tests
    // =========================================================================

    #[test]
    fn test_extraction_result_has_patterns_field() {
        let result = ExtractionResult {
            memories: vec![],
            focus_points: vec![],
            conversation_patterns: vec![
                ConversationPattern::new(
                    PatternType::Reference,
                    "test pattern",
                    PatternSource::Manual,
                ),
            ],
        };

        assert_eq!(result.conversation_patterns.len(), 1);
    }

    #[test]
    fn test_extraction_result_clone() {
        let result = ExtractionResult {
            memories: vec![],
            focus_points: vec![],
            conversation_patterns: vec![
                ConversationPattern::new(
                    PatternType::Code,
                    "fn test()",
                    PatternSource::Manual,
                ),
            ],
        };

        let cloned = result.clone();
        assert_eq!(cloned.conversation_patterns.len(), 1);
        assert_eq!(cloned.conversation_patterns[0].pattern, "fn test()");
    }

    #[test]
    fn test_extraction_result_empty_patterns() {
        // Test ExtractionResult with empty patterns
        let result = ExtractionResult {
            memories: vec![],
            focus_points: vec![],
            conversation_patterns: vec![],
        };

        assert!(result.conversation_patterns.is_empty());
        assert!(result.memories.is_empty());
        assert!(result.focus_points.is_empty());
    }

    // =========================================================================
    // AI Prompt Validation Tests
    // =========================================================================

    #[test]
    fn test_memory_extract_prompt_contains_patterns_guidance() {
        // Verify the prompt includes conversation pattern extraction guidance
        assert!(
            MEMORY_EXTRACT_SYSTEM_PROMPT.contains("对话模式提取"),
            "Prompt should contain pattern extraction guidance"
        );
        assert!(
            MEMORY_EXTRACT_SYSTEM_PROMPT.contains("reference"),
            "Prompt should mention reference pattern type"
        );
        assert!(
            MEMORY_EXTRACT_SYSTEM_PROMPT.contains("code"),
            "Prompt should mention code pattern type"
        );
    }

    #[test]
    fn test_memory_extract_prompt_contains_trigger_condition() {
        // Verify the prompt mentions >500 chars trigger condition
        assert!(
            MEMORY_EXTRACT_SYSTEM_PROMPT.contains("500"),
            "Prompt should mention 500 chars trigger condition"
        );
        assert!(
            MEMORY_EXTRACT_SYSTEM_PROMPT.contains("> 500") || MEMORY_EXTRACT_SYSTEM_PROMPT.contains("超过500"),
            "Prompt should specify > 500 chars condition"
        );
    }

    #[test]
    fn test_memory_extract_prompt_contains_output_format() {
        // Verify the prompt shows correct JSON output format with patterns
        assert!(
            MEMORY_EXTRACT_SYSTEM_PROMPT.contains("conversation_patterns"),
            "Prompt should show conversation_patterns in output format"
        );
        assert!(
            MEMORY_EXTRACT_SYSTEM_PROMPT.contains("pattern_type"),
            "Prompt should show pattern_type field"
        );
        assert!(
            MEMORY_EXTRACT_SYSTEM_PROMPT.contains("confidence"),
            "Prompt should show confidence field"
        );
    }

    // =========================================================================
    // Integration Tests - Combined Extraction
    // =========================================================================

    #[test]
    fn test_parse_memory_response_full_integration() {
        // Test complete extraction with memories, focus_points, and patterns together
        let json = r#"{
            "memories": [
                {
                    "category": "decision",
                    "content": "使用 Rust 作为主要语言。**Why:** 性能要求",
                    "importance": 85,
                    "keywords": ["Rust"],
                    "tags": ["backend"]
                }
            ],
            "focus_points": [
                {
                    "topic": "API设计",
                    "keywords": ["API", "REST"],
                    "entities": ["User", "Order"],
                    "importance": 0.8
                }
            ],
            "conversation_patterns": [
                {
                    "pattern_type": "reference",
                    "pattern": "正如我所说",
                    "confidence": 0.9
                },
                {
                    "pattern_type": "code",
                    "pattern": "fn ",
                    "confidence": 0.7
                }
            ]
        }"#;

        let result = parse_memory_response(json, Some("session-123"), Some("/project/path")).unwrap();

        // Verify all three components
        assert_eq!(result.memories.len(), 1);
        assert_eq!(result.focus_points.len(), 1);
        assert_eq!(result.conversation_patterns.len(), 2);

        // Check memory
        assert_eq!(result.memories[0].category, MemoryCategory::Decision);
        assert!(result.memories[0].content.contains("Rust"));

        // Check focus point
        assert_eq!(result.focus_points[0].topic, "API设计");

        // Check patterns
        assert_eq!(result.conversation_patterns[0].pattern_type, PatternType::Reference);
        assert_eq!(result.conversation_patterns[1].pattern_type, PatternType::Code);
    }

    #[test]
    fn test_parse_memory_response_mixed_valid_invalid_patterns() {
        // Test with mix of valid and invalid patterns
        let json = r#"{
            "memories": [],
            "focus_points": [],
            "conversation_patterns": [
                {
                    "pattern_type": "reference",
                    "pattern": "valid pattern 1",
                    "confidence": 0.8
                },
                {
                    "pattern_type": "unknown_type",
                    "pattern": "should be skipped",
                    "confidence": 0.5
                },
                {
                    "pattern_type": "code",
                    "pattern": "fn valid",
                    "confidence": 0.6
                },
                {
                    "pattern_type": "reference",
                    "pattern": "",
                    "confidence": 0.9
                }
            ]
        }"#;

        let result = parse_memory_response(json, None, None).unwrap();

        // Should only have 2 valid patterns
        assert_eq!(result.conversation_patterns.len(), 2);
        assert_eq!(result.conversation_patterns[0].pattern, "valid pattern 1");
        assert_eq!(result.conversation_patterns[1].pattern, "fn valid");
    }

    #[test]
    fn test_parse_memory_response_patterns_with_session_and_project() {
        // Verify session_id and project_path are passed through correctly
        // (patterns don't use them, but the function accepts them)
        let json = r#"{
            "memories": [
                {
                    "category": "technical",
                    "content": "Using PostgreSQL database",
                    "importance": 70,
                    "keywords": ["PostgreSQL"],
                    "tags": ["database"]
                }
            ],
            "focus_points": [],
            "conversation_patterns": [
                {
                    "pattern_type": "reference",
                    "pattern": "as mentioned",
                    "confidence": 0.7
                }
            ]
        }"#;

        let result = parse_memory_response(json, Some("test-session"), Some("/test/project")).unwrap();

        // Memory should have source_session and project_path
        assert_eq!(result.memories[0].source_session, Some("test-session".to_string()));
        assert_eq!(result.memories[0].project_path, Some("/test/project".to_string()));

        // Pattern should be parsed correctly
        assert_eq!(result.conversation_patterns.len(), 1);
    }

    #[test]
    fn test_parse_memory_response_all_pattern_types() {
        // Test both supported pattern types
        let json = r#"{
            "memories": [],
            "focus_points": [],
            "conversation_patterns": [
                {
                    "pattern_type": "reference",
                    "pattern": "previously discussed",
                    "confidence": 0.8
                },
                {
                    "pattern_type": "Reference",
                    "pattern": "case insensitive",
                    "confidence": 0.7
                },
                {
                    "pattern_type": "CODE",
                    "pattern": "function ",
                    "confidence": 0.6
                },
                {
                    "pattern_type": "code",
                    "pattern": "class ",
                    "confidence": 0.5
                }
            ]
        }"#;

        let result = parse_memory_response(json, None, None).unwrap();

        // All should be parsed (case-insensitive pattern_type)
        assert_eq!(result.conversation_patterns.len(), 4);

        // Check types
        assert_eq!(result.conversation_patterns[0].pattern_type, PatternType::Reference);
        assert_eq!(result.conversation_patterns[1].pattern_type, PatternType::Reference);
        assert_eq!(result.conversation_patterns[2].pattern_type, PatternType::Code);
        assert_eq!(result.conversation_patterns[3].pattern_type, PatternType::Code);
    }

    #[test]
    fn test_extraction_result_debug_trait() {
        // Test that ExtractionResult implements Debug
        let result = ExtractionResult {
            memories: vec![],
            focus_points: vec![],
            conversation_patterns: vec![
                ConversationPattern::new(
                    PatternType::Reference,
                    "test",
                    PatternSource::Manual,
                ),
            ],
        };

        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("ExtractionResult"));
        assert!(debug_str.contains("conversation_patterns"));
    }

    // =========================================================================
    // Unified Extraction Tests
    // =========================================================================

    #[test]
    fn test_parse_unified_response_full() {
        let json = r#"{
            "memories": [
                {
                    "category": "decision",
                    "content": "使用 Rust 作为主要语言",
                    "importance": 85,
                    "keywords": ["Rust"],
                    "tags": ["backend"]
                }
            ],
            "focus_points": [
                {
                    "topic": "API设计",
                    "keywords": ["API", "REST"],
                    "entities": ["User", "Order"],
                    "core_question": "如何优化 API？",
                    "importance": 0.8,
                    "is_current": true
                }
            ],
            "conversation_patterns": [
                {
                    "pattern_type": "reference",
                    "pattern": "正如我所说",
                    "confidence": 0.8
                }
            ],
            "focus_keywords": {
                "transition": ["换个话题"],
                "question": ["怎么"],
                "task": ["帮我"],
                "tech": ["rust"]
            }
        }"#;

        let result = parse_unified_response(json, Some("session-123"), Some("/project")).unwrap();

        // Verify all components
        assert_eq!(result.memories.len(), 1);
        assert_eq!(result.memories[0].category, MemoryCategory::Decision);
        assert!(result.memories[0].content.contains("Rust"));

        assert_eq!(result.focus_points.len(), 1);
        assert_eq!(result.focus_points[0].topic, "API设计");

        assert_eq!(result.conversation_patterns.len(), 1);
        assert_eq!(result.conversation_patterns[0].pattern_type, PatternType::Reference);

        assert!(!result.focus_keywords.is_empty());
        assert_eq!(result.focus_keywords.transition.len(), 1);
        assert_eq!(result.focus_keywords.question.len(), 1);
        assert_eq!(result.focus_keywords.task.len(), 1);
        assert_eq!(result.focus_keywords.tech.len(), 1);
    }

    #[test]
    fn test_parse_unified_response_empty() {
        let json = r#"{
            "memories": [],
            "focus_points": [],
            "conversation_patterns": [],
            "focus_keywords": {
                "transition": [],
                "question": [],
                "task": [],
                "tech": []
            }
        }"#;

        let result = parse_unified_response(json, None, None).unwrap();

        assert!(result.memories.is_empty());
        assert!(result.focus_points.is_empty());
        assert!(result.conversation_patterns.is_empty());
        assert!(result.focus_keywords.is_empty());
    }

    #[test]
    fn test_parse_unified_response_partial() {
        // Test with only memories (no focus_keywords)
        let json = r#"{
            "memories": [
                {
                    "category": "technical",
                    "content": "使用 PostgreSQL 作为主数据库存储",
                    "importance": 70
                }
            ]
        }"#;

        let result = parse_unified_response(json, None, None).unwrap();

        assert_eq!(result.memories.len(), 1);
        assert!(result.focus_points.is_empty());
        assert!(result.conversation_patterns.is_empty());
        assert!(result.focus_keywords.is_empty());
    }

    #[test]
    fn test_parse_unified_response_with_code_block() {
        let json = r#"```json
{
    "memories": [],
    "focus_points": [],
    "conversation_patterns": [],
    "focus_keywords": {
        "transition": ["switching"],
        "question": [],
        "task": [],
        "tech": []
    }
}
```"#;

        let result = parse_unified_response(json, None, None).unwrap();

        assert_eq!(result.focus_keywords.transition.len(), 1);
        assert_eq!(result.focus_keywords.transition[0], "switching");
    }

    #[test]
    fn test_unified_extraction_result_default() {
        let result = UnifiedExtractionResult::default();
        assert!(result.memories.is_empty());
        assert!(result.focus_points.is_empty());
        assert!(result.conversation_patterns.is_empty());
        assert!(result.focus_keywords.is_empty());
    }

    #[test]
    fn test_unified_extraction_prompt_contains_all_sections() {
        // Verify the unified prompt contains all extraction sections
        assert!(UNIFIED_EXTRACTION_PROMPT.contains("长期记忆"));
        assert!(UNIFIED_EXTRACTION_PROMPT.contains("当前焦点"));
        assert!(UNIFIED_EXTRACTION_PROMPT.contains("对话模式"));
        assert!(UNIFIED_EXTRACTION_PROMPT.contains("焦点关键词"));
    }

    #[test]
    fn test_unified_extraction_prompt_contains_keyword_categories() {
        assert!(UNIFIED_EXTRACTION_PROMPT.contains("transition"));
        assert!(UNIFIED_EXTRACTION_PROMPT.contains("question"));
        assert!(UNIFIED_EXTRACTION_PROMPT.contains("task"));
        assert!(UNIFIED_EXTRACTION_PROMPT.contains("tech"));
    }

    #[test]
    fn test_parse_unified_response_keywords_merged() {
        let json = r#"{
            "memories": [],
            "focus_points": [],
            "conversation_patterns": [],
            "focus_keywords": {
                "transition": ["换个话题", "switching", "however"],
                "question": ["怎么", "how", "为什么"],
                "task": ["帮我", "implement", "创建"],
                "tech": ["rust", "数据库", "api"]
            }
        }"#;

        let result = parse_unified_response(json, None, None).unwrap();

        assert_eq!(result.focus_keywords.transition.len(), 3);
        assert_eq!(result.focus_keywords.question.len(), 3);
        assert_eq!(result.focus_keywords.task.len(), 3);
        assert_eq!(result.focus_keywords.tech.len(), 3);
        assert_eq!(result.focus_keywords.total_count(), 12);
    }
}
