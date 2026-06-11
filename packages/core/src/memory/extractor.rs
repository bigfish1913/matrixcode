//! Memory extraction: AI-based and rule-based detection.

use std::collections::HashSet;
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
    <description>项目结构信息（重要！）</description>
    <when_to_save>发现关键模块位置、核心文件路径、代码组织方式时</when_to_save>
    <body_structure>先写结构描述，然后 **Location:** 具体路径，**Purpose:** 模块职责</body_structure>
    <example>"上下文压缩模块位于 packages/core/src/compress/。**Location:** packages/core/src/compress/compressor.rs 是核心入口，**Purpose:** 负责上下文 token 优化"</example>
</type>
</types>

# 不要保存什么到记忆中

- Git 历史、最近更改 — git log/blame 是权威来源
- 临时状态：进行中的任务、当前对话上下文
- 错误信息和调试细节 — 问题解决后无需保留
- 临时文件路径、临时变量名

# 重要：应该保存的结构信息

项目结构信息（structure 类型）应该保存，包括：
- 关键模块的位置（如 "compress 模块在 packages/core/src/compress/"）
- 核心文件的功能（如 "agent/streaming.rs 负责流式响应处理"）
- 常见问题的定位路径（如 "上下文大小判断在 compressor.rs 的 estimate_tokens 函数"）
- 代码组织模式（如 "providers 模块实现了 Provider trait"）

这些信息能大幅减少未来会话的探索时间！

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
            // Add AI-extracted keywords and tags with filtering
            let valid_keywords: Vec<String> = item.keywords
                .iter()
                .filter(|k| k.len() >= 2 && !is_noise_word(k))
                .cloned()
                .collect();
            
            let valid_tags: Vec<String> = item.tags
                .iter()
                .filter(|t| t.len() >= 2 && !is_noise_word(t))
                .cloned()
                .collect();
            
            entry.tags.extend(valid_keywords);
            entry.tags.extend(valid_tags);
            entry.tags.dedup();
            
            // Limit tag count to avoid overwhelming
            if entry.tags.len() > 10 {
                entry.tags.truncate(10);
            }

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
        // AI failed - try rule-based fallback for critical memories only
        log::warn!("AI memory extraction failed, trying rule-based fallback for critical memories");
        
        // Extract only the most critical memory types (avoid noise)
        let critical_memories = detect_critical_memories(text, session_id, project_path);
        
        crate::debug::debug_log().memory_ai_detection(
            "rule-fallback",
            critical_memories.len(),
            text_len,
            false,
        );
        
        return ExtractionResult {
            memories: critical_memories,
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

/// Detect critical memories using rule-based patterns (fallback when AI fails).
/// Only extracts high-value memory types: structure, technical, decision.
fn detect_critical_memories(
    text: &str,
    session_id: Option<&str>,
    project_path: Option<&str>,
) -> Vec<MemoryEntry> {
    // Critical patterns with high importance
    let critical_patterns = [
        // Structure information (highest priority)
        (MemoryCategory::Structure, ["位于", "入口", "模块", "packages/", "src/"], 85.0),
        // Technical information (high priority)
        (MemoryCategory::Technical, ["技术栈", "框架", "基于", "使用", ""], 80.0),
        // Decision information (important)
        (MemoryCategory::Decision, ["决定", "选择", "采用", "", ""], 75.0),
    ];
    
    let mut entries = Vec::new();
    let text_lower = text.to_lowercase();
    
    for (category, keywords, importance) in critical_patterns {
        // Check if any keyword matches
        for keyword in keywords {
            if text_lower.contains(&keyword.to_lowercase()) {
                let content = extract_memory_content(text, keyword);
                
                if !content.is_empty() && content.len() >= MIN_MEMORY_CONTENT_LENGTH {
                    let mut entry = MemoryEntry::new(
                        category,
                        content,
                        session_id.map(|s| s.to_string()),
                        project_path.map(|p| p.to_string()),
                    );
                    // Set importance for critical memories
                    entry.importance = importance;
                    entries.push(entry);
                    break; // Only one entry per category
                }
            }
        }
    }
    
    // Deduplicate and limit
    deduplicate_entries(entries)
}

fn extract_memory_content(text: &str, keyword: &str) -> String {
    let text_lower = text.to_lowercase();
    let keyword_lower = keyword.to_lowercase();

    let pos = match text_lower.find(&keyword_lower) {
        Some(p) => p,
        None => return String::new(),
    };

    // Improved sentence boundary detection
    let start = find_sentence_start(text, pos);
    let end = find_sentence_end(text, pos);

    let sentence = text[start..end].trim();

    // Clean and format content
    let cleaned = clean_memory_content(sentence);

    if cleaned.len() > MAX_MEMORY_CONTENT_LENGTH {
        // Intelligent truncation: preserve key information
        truncate_intelligently(&cleaned, MAX_MEMORY_CONTENT_LENGTH)
    } else {
        cleaned
    }
}

/// Find sentence start position (improved boundary detection).
fn find_sentence_start(text: &str, pos: usize) -> usize {
    // Collect chars once at function start for O(n) instead of O(n^2)
    let chars: Vec<char> = text.chars().collect();
    let mut start = pos;
    while start > 0 {
        let ch = chars[start - 1];
        // Sentence boundary markers
        if ch == '.' || ch == '。' || ch == '\n' || ch == '!' || ch == '?' || ch == '！' || ch == '？' {
            return start;
        }
        // Avoid splitting code blocks (check for ```)
        if start >= 3 {
            let slice: String = chars[start - 3..start].iter().collect();
            if slice == "```" {
                return start - 3;
            }
        }
        start -= 1;
    }
    0
}

/// Find sentence end position (improved boundary detection).
fn find_sentence_end(text: &str, pos: usize) -> usize {
    // Look forward for sentence boundary markers
    let chars: Vec<char> = text.chars().collect();
    let mut end = pos;
    while end < chars.len() {
        let ch = chars[end];
        // Sentence boundary markers
        if ch == '.' || ch == '。' || ch == '\n' || ch == '!' || ch == '?' || ch == '！' || ch == '？' {
            return end + 1;
        }
        // Avoid splitting code blocks (check for ```)
        if end + 3 <= chars.len() {
            let slice: String = chars[end..end + 3].iter().collect();
            if slice == "```" {
                return end + 3;
            }
        }
        end += 1;
    }
    text.len()
}

/// Clean memory content: remove Markdown markers, normalize format.
fn clean_memory_content(content: &str) -> String {
    // Remove Markdown bold/italic markers
    let cleaned = content
        .replace("**Why:**", "原因:")
        .replace("**Context:**", "场景:")
        .replace("**Location:**", "位置:")
        .replace("**Purpose:**", "功能:")
        .replace("**Problem:**", "问题:")
        .replace("**Key:**", "关键:")
        .replace("**", "")
        .replace("`", "")
        .replace("#", "");
    
    // Normalize whitespace
    let cleaned: String = cleaned
        .split_whitespace()
        .fold(String::new(), |mut acc, s| {
            if !acc.is_empty() {
                acc.push(' ');
            }
            acc.push_str(s);
            acc
        });
    
    cleaned.trim().to_string()
}

/// Intelligent truncation: preserve key information (paths, names).
fn truncate_intelligently(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        return text.to_string();
    }

    // Try to preserve important parts:
    // 1. Keep "Location:" information if present
    // 2. Keep "Purpose:" information if present
    // 3. Keep technical names/paths

    let parts: Vec<&str> = text.split_whitespace().collect();
    let mut result_vec = Vec::new();
    let mut result_set = HashSet::new(); // O(1) lookup for dedup
    let mut current_len = 0;

    // Priority keywords to keep (small array, direct iteration is fine)
    const PRIORITY_KEYWORDS: [&str; 10] = [
        "位置:", "Location:", "功能:", "Purpose:",
        "packages/", "src/", ".rs", ".ts", ".js", ".py"
    ];

    // First pass: collect priority parts
    for &part in &parts {
        if PRIORITY_KEYWORDS.iter().any(|k| part.contains(k))
            && current_len + part.len() < max_len
            && !result_set.contains(part) {
                result_vec.push(part);
                result_set.insert(part);
                current_len += part.len() + 1;
            }
    }

    // Second pass: add other parts if space available
    if result_vec.is_empty() || current_len < max_len / 2 {
        for &part in &parts {
            if !result_set.contains(part) && current_len + part.len() < max_len {
                result_vec.push(part);
                result_set.insert(part);
                current_len += part.len() + 1;
            }
        }
    }

    if result_vec.is_empty() {
        // Simple truncation as last resort
        text.chars().take(max_len).collect()
    } else {
        result_vec.join(" ")
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

## 1. 长期记忆 (memories) - 最重要！
- decision: 技术决策（如"决定使用 PostgreSQL"、"采用 React 架构"）
- preference: 用户偏好（如"我喜欢简洁的代码风格"、"习惯用 VS Code"）
- solution: 解决方案（如"通过添加缓存解决了性能问题"）
- finding: 重要发现（如"发现内存泄漏的原因"）
- technical: 技术栈（如"项目使用 Rust + Tokio"）
- structure: **项目结构信息（优先保存！）**（如"compress 模块在 packages/core/src/compress/"、"上下文判断逻辑在 compressor.rs:518"）

## 结构信息的重要性

项目结构信息（structure 类型）能大幅减少未来会话的探索时间，必须保存：
- 关键模块位置："Agent 循环在 packages/core/src/agent/run.rs"
- 核心文件功能："streaming.rs 负责 API 流式响应处理"
- 问题定位路径："上下文大小判断在 estimate_tokens 函数（compressor.rs:518-561）"
- 代码组织模式："providers 模块实现了 Provider trait"

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
      "category": "structure",
      "content": "上下文压缩模块位于 packages/core/src/compress/。**Location:** compressor.rs:518-561 是 estimate_tokens 函数，**Purpose:** 计算上下文 token 数量",
      "importance": 80,
      "keywords": ["compress", "estimate_tokens", "context"],
      "tags": ["core", "context-management"]
    },
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
1. structure 类型的记忆优先级最高，发现就保存
2. 只提取明确出现的信息，不要推测
3. 如果某类信息没有，返回空数组/对象
4. importance 范围：memories 0-100，focus_points 0.0-1.0
5. confidence 范围：0.0-1.0，常见模式置信度较低
6. 关键词提取 3-5 个核心关键词
7. 只返回 JSON，不要其他解释"#;

/// Unified extraction prompt with focus selection.
/// This prompt includes existing focuses and asks AI to select or create focus.
const UNIFIED_EXTRACTION_WITH_FOCUS_PROMPT: &str = r#"你是信息提取和焦点决策助手。从对话中一次性完成以下任务：

## 1. 焦点决策 (focus_decision) - 最重要！

你会收到当前已有的焦点列表。请判断：

### 选择现有焦点
如果最新对话与某个现有焦点匹配：
- selected_focus_id: 该焦点的 ID
- need_new_focus: false
- confidence: 匹配置信度 (0.0-1.0)

### 创建新焦点
如果没有任何现有焦点匹配：
- selected_focus_id: null
- need_new_focus: true
- new_focus_topic: 新焦点主题
- new_core_question: 核心问题
- confidence: 创建置信度

### 判断话题切换
- is_topic_switch: 是否从某焦点切换到另一焦点
- previous_focus_id: 切换前的焦点 ID（如果有）

### 焦点类型 (focus_type)
- problem_solving: 修复 bug、解决错误
- task_execution: 实现功能、完成任务
- knowledge_exploration: 学习、研究、探索
- decision_making: 技术选型、架构设计
- code_optimization: 性能优化、重构
- general: 一般对话

## 2. 长期记忆 (memories)
- decision: 技术决策
- preference: 用户偏好
- solution: 解决方案
- finding: 重要发现
- technical: 技术栈
- structure: 项目结构

## 3. 焦点关键词 (focus_keywords)
- transition: 话题转换词
- question: 提问词
- task: 任务词
- tech: 技术词

## 输出格式（严格 JSON）

```json
{
  "focus_decision": {
    "selected_focus_id": "focus-1",
    "need_new_focus": false,
    "new_focus_topic": null,
    "new_core_question": null,
    "confidence": 0.85,
    "focus_type": "code_optimization",
    "is_topic_switch": true,
    "previous_focus_id": "focus-2",
    "focus_keywords": ["API", "latency", "performance"],
    "related_entities": ["api.rs", "handle_request()"],
    "reasoning": "用户从数据库切换到 API 性能话题"
  },
  "memories": [...],
  "focus_keywords": {
    "transition": ["换个话题"],
    "question": ["怎么"],
    "task": ["优化"],
    "tech": ["api", "性能"]
  }
}
```

## 规则
1. focus_decision 是最重要的输出，必须仔细判断
2. 现有焦点列表会随对话文本一起提供
3. 如果现有焦点都不匹配，必须标记 need_new_focus=true
4. confidence 反映你对决策的确信程度
5. 只返回 JSON，不要其他解释"#;

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

    /// Extract all information WITH focus selection in a single AI call.
    ///
    /// This method receives existing focuses and asks AI to select the best match
    /// or create a new focus if none matches. This ensures focus continuity.
    ///
    /// # Arguments
    /// * `text` - Conversation text to analyze
    /// * `existing_foci` - Current focus points from FocusManager (id, topic, keywords)
    /// * `session_id` - Optional session ID
    /// * `project_path` - Optional project path
    ///
    /// # Returns
    /// UnifiedExtractionResult with focus_decision field populated
    pub async fn extract_unified_with_foci(
        &self,
        text: &str,
        existing_foci: &[(&str, &str, &[String])], // (id, topic, keywords)
        session_id: Option<&str>,
        project_path: Option<&str>,
    ) -> Result<UnifiedExtractionResult> {
        use crate::providers::{ChatRequest, Message, MessageContent, Role};

        // Safely truncate to ~4000 chars respecting UTF-8 boundaries
        let truncated = truncate_chars(text, 4000);

        // Format existing focuses for AI
        let foci_text = if existing_foci.is_empty() {
            "（当前没有现有焦点）".to_string()
        } else {
            let mut foci_list = Vec::new();
            for (id, topic, keywords) in existing_foci {
                foci_list.push(format!(
                    "- ID: {}\n  主题: {}\n  关键词: {}",
                    id,
                    topic,
                    keywords.join(", ")
                ));
            }
            format!("现有焦点列表：\n{}", foci_list.join("\n"))
        };

        let user_prompt = format!(
            "{}\n\n最新对话：\n{}\n\n请判断最新对话与现有焦点的匹配关系，并做出焦点决策。",
            foci_text,
            truncated
        );

        let request = ChatRequest {
            messages: vec![Message {
                role: Role::User,
                content: MessageContent::Text(user_prompt),
            }],
            tools: vec![],
            system: Some(UNIFIED_EXTRACTION_WITH_FOCUS_PROMPT.to_string()),
            think: false,
            max_tokens: 1024,
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

        parse_unified_response_with_focus(&response_text, session_id, project_path)
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
        focus_decision: None, // Not populated in basic extraction
    })
}

/// Parse unified extraction response with focus decision from AI.
fn parse_unified_response_with_focus(
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
    struct UnifiedResponseWithFocus {
        #[serde(default)]
        focus_decision: Option<FocusDecisionItem>,
        #[serde(default)]
        memories: Vec<MemoryItem>,
        #[serde(default)]
        focus_keywords: FocusKeywordsItem,
    }

    #[derive(Deserialize)]
    struct FocusDecisionItem {
        #[serde(default)]
        selected_focus_id: Option<String>,
        #[serde(default)]
        need_new_focus: bool,
        #[serde(default)]
        new_focus_topic: Option<String>,
        #[serde(default)]
        new_core_question: Option<String>,
        #[serde(default)]
        confidence: f32,
        #[serde(default)]
        focus_type: String,
        #[serde(default)]
        is_topic_switch: bool,
        #[serde(default)]
        previous_focus_id: Option<String>,
        #[serde(default)]
        focus_keywords: Vec<String>,
        #[serde(default)]
        related_entities: Vec<String>,
        #[serde(default)]
        reasoning: String,
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

    let parsed: UnifiedResponseWithFocus = serde_json::from_str(cleaned)?;

    // Parse focus decision
    let focus_decision = parsed.focus_decision.map(|item| {
        use super::unified_extraction::{FocusDecision, FocusType};

        let focus_type = match item.focus_type.to_lowercase().as_str() {
            "problem_solving" => FocusType::ProblemSolving,
            "task_execution" => FocusType::TaskExecution,
            "knowledge_exploration" => FocusType::KnowledgeExploration,
            "decision_making" => FocusType::DecisionMaking,
            "code_optimization" => FocusType::CodeOptimization,
            _ => FocusType::General,
        };

        FocusDecision {
            selected_focus_id: item.selected_focus_id,
            need_new_focus: item.need_new_focus,
            new_focus_topic: item.new_focus_topic,
            new_core_question: item.new_core_question,
            confidence: item.confidence.clamp(0.0, 1.0),
            focus_type,
            is_topic_switch: item.is_topic_switch,
            previous_focus_id: item.previous_focus_id,
            focus_keywords: item.focus_keywords,
            related_entities: item.related_entities,
            reasoning: item.reasoning,
        }
    });

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

    // Parse focus keywords
    let focus_keywords = ExtractedKeywords {
        transition: parsed.focus_keywords.transition,
        question: parsed.focus_keywords.question,
        task: parsed.focus_keywords.task,
        tech: parsed.focus_keywords.tech,
    };

    Ok(UnifiedExtractionResult {
        memories: deduplicate_entries(entries),
        focus_points: Vec::new(), // Not used in focus selection mode
        conversation_patterns: Vec::new(), // Not used in focus selection mode
        focus_keywords,
        focus_decision,
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

/// Check if a word is a noise word (should be filtered from tags).
fn is_noise_word(word: &str) -> bool {
    let noise_words = [
        // English noise words
        "the", "a", "an", "is", "are", "was", "were", "be", "been", "being",
        "have", "has", "had", "do", "does", "did", "will", "would", "could",
        "should", "may", "might", "must", "shall", "can", "need", "dare",
        "ought", "used", "to", "of", "in", "for", "on", "with", "at", "by",
        "from", "as", "into", "through", "during", "before", "after",
        "above", "below", "between", "under", "again", "further", "then",
        "once", "here", "there", "when", "where", "why", "how", "all", "each",
        "few", "more", "most", "other", "some", "such", "no", "nor", "not",
        "only", "own", "same", "so", "than", "too", "very", "just", "and",
        "but", "if", "or", "because", "until", "while", "although", "though",
        // Chinese noise words
        "的", "了", "在", "是", "我", "有", "和", "就", "不", "人", "都",
        "一", "个", "也", "很", "要", "这", "那", "他", "她", "它", "们",
        "为", "与", "以", "及", "或", "但", "如", "而", "因", "所", "能",
        "会", "可", "把", "被", "让", "给", "从", "到", "对", "向", "比",
        "等", "时", "地", "得", "着", "过", "来", "去", "上", "下", "里",
        "中", "外", "前", "后", "左", "右", "好", "多", "少", "大", "小",
        "高", "低", "长", "短", "快", "慢", "新", "旧", "早", "晚", "真",
        "假", "全", "每", "各", "哪", "什么", "怎么", "怎样", "如何",
        "为什么", "因为", "所以", "如果", "虽然", "但是", "然后", "接着",
        "最后", "开始", "结束", "一直", "总是", "有时", "常常", "经常",
    ];
    
    noise_words.contains(&word.to_lowercase().as_str())
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
