//! Retrieval helpers: TF-IDF search, keyword extraction.

use std::collections::{HashMap, HashSet};

use super::entry::MemoryEntry;
use super::manager::AutoMemory;

// ============================================================================
// Hard-coded Stop Words (simplified)
// ============================================================================

/// Common Chinese + English stop words (minimal set).
fn get_stop_words() -> HashSet<&'static str> {
    HashSet::from([
        // Chinese
        "的", "了", "是", "在", "我", "有", "和", "就", "不", "都", "一", "也", "很", "到", "要",
        "去", "你", "会", "着", "没有", "看", "好", "这", "那", "什么", "怎么", "请", "能", "可以",
        "需要", // English
        "the", "a", "an", "is", "are", "was", "were", "be", "have", "has", "do", "will", "would",
        "could", "should", "can", "to", "of", "in", "for", "on", "with", "at", "by", "from", "and",
        "but", "or", "not", "if", "then", "this", "that", "it", "i", "me", "my", "we", "you", "he",
        "she", "they", "please", "help", "need", "want", "let", "use",
    ])
}

// ============================================================================
// Keyword Extraction (simplified)
// ============================================================================

/// Extract meaningful keywords from conversation context.
pub fn extract_context_keywords(context: &str) -> Vec<String> {
    let stop_words = get_stop_words();
    let lower = context.to_lowercase();
    let mut keywords: HashSet<String> = HashSet::new();

    // 1. Extract English words (must be meaningful - at least 3 chars)
    for word in lower.split_whitespace() {
        let cleaned = word
            .trim_matches(|c: char| !c.is_alphanumeric())
            .to_string();
        if cleaned.len() >= 3 && !stop_words.contains(cleaned.as_str()) {
            keywords.insert(cleaned);
        }
    }

    // 2. Extract tech patterns (camelCase, snake_case, file paths)
    let tech_regexes = [
        r"[a-zA-Z_][a-zA-Z0-9_]*\.[a-zA-Z]{1,4}", // file extensions
        r"[A-Z][a-z]+[A-Z][a-zA-Z]*",             // CamelCase
        r"[a-z][a-z0-9]*_[a-z][a-z0-9_]*",        // snake_case
        r"[0-9]+[kKmMgGtT][bB]?",                 // sizes like 4KB
    ];

    for pattern in tech_regexes {
        if let Ok(re) = regex::Regex::new(pattern) {
            for cap in re.find_iter(&lower) {
                let match_str = cap.as_str();
                if !stop_words.contains(match_str) {
                    keywords.insert(match_str.to_string());
                }
            }
        }
    }

    // Sort by length and limit
    let mut result: Vec<String> = keywords.into_iter().collect();
    result.sort_by_key(|b| std::cmp::Reverse(b.len()));
    result.truncate(10);
    result
}

// ============================================================================
// Skip Simple Messages (Greeting/Short)
// ============================================================================

/// Greeting patterns to skip keyword extraction.
const GREETING_PATTERNS: &[&str] = &[
    "你好",
    "您好",
    "hi",
    "hello",
    "hey",
    "嗨",
    "早上好",
    "下午好",
    "晚上好",
    "good morning",
    "good afternoon",
    "good evening",
    "请问",
    "帮忙",
    "帮我",
    "帮我看",
    "看看",
    "help",
    "请",
    "开始",
    "start",
    "准备好了",
    "ready",
];

/// Check if message is simple (greeting/short) and should skip AI keyword extraction.
/// Returns true if should skip.
pub fn should_skip_simple_message(msg: &str) -> bool {
    let trimmed = msg.trim();

    // Skip if too short (< 15 chars)
    if trimmed.len() < 15 {
        return true;
    }

    // Skip greeting patterns
    let lower = trimmed.to_lowercase();
    for pattern in GREETING_PATTERNS {
        if lower.starts_with(pattern) || lower == *pattern {
            return true;
        }
    }

    false
}

/// Calculate word-based similarity between two strings (Jaccard coefficient).
pub fn calculate_similarity(a: &str, b: &str) -> f64 {
    AutoMemory::calculate_similarity(a, b)
}

// ============================================================================
// Semantic Aliases (uses KeywordsConfig)
// ============================================================================

/// Get semantic aliases (hard-coded).
pub fn get_semantic_aliases() -> Vec<(&'static str, &'static str)> {
    vec![
        // Technical terms
        ("rust", "Rust"),
        ("typescript", "TypeScript"),
        ("javascript", "JavaScript"),
        ("python", "Python"),
        ("react", "React"),
        ("vue", "Vue"),
        ("angular", "Angular"),
        ("数据库", "database"),
        ("db", "database"),
        // Actions
        ("修复", "fix"),
        ("解决", "solve"),
        ("优化", "optimize"),
        ("重构", "refactor"),
        ("更新", "update"),
        ("删除", "delete"),
        // Preferences
        ("喜欢", "prefer"),
        ("偏好", "prefer"),
        ("首选", "prefer"),
        // Structures
        ("入口", "entry"),
        ("主文件", "main"),
        ("目录", "directory"),
    ]
}

/// Expand keywords with semantic aliases.
pub fn expand_semantic_keywords(keywords: &[String]) -> Vec<String> {
    let aliases = get_semantic_aliases();
    let mut expanded: Vec<String> = keywords.to_vec();

    for keyword in keywords {
        let kw_lower = keyword.to_lowercase();
        for &(alias, target) in &aliases {
            if kw_lower.contains(alias) {
                expanded.push(target.to_string());
            }
            if kw_lower.contains(target) {
                expanded.push(alias.to_string());
            }
        }
    }

    expanded.sort();
    expanded.dedup();
    expanded
}

// ============================================================================
// Relevance & Contradiction Detection (uses KeywordsConfig)
// ============================================================================

/// Compute relevance score of a memory entry to context keywords.
/// Returns 0.0-1.0 where 1.0 means highly relevant.
pub fn compute_relevance(entry: &MemoryEntry, context_keywords: &[String]) -> f64 {
    if context_keywords.is_empty() {
        return 0.0;
    }

    let expanded_keywords = expand_semantic_keywords(context_keywords);
    let content_lower = entry.content.to_lowercase();

    let matches = expanded_keywords
        .iter()
        .filter(|kw| content_lower.contains(&kw.to_lowercase()))
        .count();

    let keyword_score = matches as f64 / expanded_keywords.len().max(context_keywords.len()) as f64;

    let tag_matches = entry
        .tags
        .iter()
        .filter(|tag| {
            let tag_lower = tag.to_lowercase();
            expanded_keywords.iter().any(|kw| {
                tag_lower.contains(&kw.to_lowercase()) || kw.to_lowercase().contains(&tag_lower)
            })
        })
        .count();

    let tag_score = if tag_matches > 0 {
        0.2 + (tag_matches as f64 * 0.05).min(0.1)
    } else {
        0.0
    };

    (keyword_score + tag_score).min(1.0)
}

/// Detect if two memory contents have contradiction signals.
/// Uses hard-coded contradiction signals.
pub fn has_contradiction_signal(old: &str, new: &str) -> bool {
    // Hard-coded contradiction signals
    let contradiction_signals = [
        "不再",
        "改为",
        "换成",
        "放弃",
        "no longer",
        "instead of",
        "changed to",
        "switched to",
    ];

    // Check contradiction signals
    for signal in &contradiction_signals {
        if new.contains(signal) {
            return true;
        }
    }

    // Check action verbs that indicate change
    let action_verbs = [
        "决定使用",
        "选择使用",
        "采用",
        "使用",
        "decided to use",
        "chose",
        "using",
        "adopted",
    ];

    for verb in &action_verbs {
        if old.contains(verb) && new.contains(verb) {
            return true;
        }
    }

    // Check preference verbs
    let pref_verbs = ["偏好", "喜欢", "prefer", "like"];
    for verb in &pref_verbs {
        if old.contains(verb) && new.contains(verb) {
            return true;
        }
    }

    false
}

// ============================================================================
// TF-IDF Search
// ============================================================================

/// Semantic search using TF-IDF algorithm.
///
/// TF-IDF (Term Frequency-Inverse Document Frequency) is a
/// semantic search method without needing an AI model.
pub struct TfIdfSearch {
    /// Word frequency in each document.
    doc_word_freq: HashMap<String, HashMap<String, f32>>,
    /// Total documents.
    total_docs: usize,
    /// IDF cache.
    idf_cache: HashMap<String, f32>,
}

impl TfIdfSearch {
    /// Create a new TF-IDF search instance.
    pub fn new() -> Self {
        Self {
            doc_word_freq: HashMap::new(),
            total_docs: 0,
            idf_cache: HashMap::new(),
        }
    }

    /// Index all memories for TF-IDF search.
    pub fn index(&mut self, memory: &AutoMemory) {
        self.clear();
        self.total_docs = memory.entries.len();

        for entry in &memory.entries {
            let words = self.tokenize(&entry.content);
            let word_freq = self.compute_word_freq(&words);
            self.doc_word_freq.insert(entry.content.clone(), word_freq);
        }

        self.compute_idf();
    }

    /// Tokenize text into words.
    fn tokenize(&self, text: &str) -> Vec<String> {
        let lower = text.to_lowercase();
        let mut tokens = Vec::new();

        for word in lower.split_whitespace() {
            let trimmed = word.trim_matches(|c: char| !c.is_alphanumeric());
            if trimmed.len() > 1 {
                tokens.push(trimmed.to_string());
            }

            let chars: Vec<char> = trimmed.chars().collect();
            let has_cjk = chars.iter().any(|c| Self::is_cjk(*c));

            if has_cjk {
                for c in &chars {
                    if Self::is_cjk(*c) {
                        tokens.push(c.to_string());
                    }
                }
                for window in chars.windows(2) {
                    if Self::is_cjk(window[0]) || Self::is_cjk(window[1]) {
                        tokens.push(window.iter().collect::<String>());
                    }
                }
            }
        }

        tokens
    }

    /// Check if a character is CJK.
    fn is_cjk(c: char) -> bool {
        matches!(c,
            '\u{4E00}'..='\u{9FFF}' |
            '\u{3400}'..='\u{4DBF}' |
            '\u{F900}'..='\u{FAFF}' |
            '\u{3000}'..='\u{303F}' |
            '\u{3040}'..='\u{309F}' |
            '\u{30A0}'..='\u{30FF}'
        )
    }

    /// Compute word frequency in a document.
    fn compute_word_freq(&self, words: &[String]) -> HashMap<String, f32> {
        let total = words.len() as f32;
        let mut freq = HashMap::new();

        for word in words {
            *freq.entry(word.clone()).or_insert(0.0) += 1.0;
        }

        for (_, count) in freq.iter_mut() {
            *count /= total;
        }

        freq
    }

    /// Compute IDF for all words.
    fn compute_idf(&mut self) {
        let mut word_doc_count: HashMap<String, usize> = HashMap::new();

        for word_freq in &self.doc_word_freq {
            for word in word_freq.1.keys() {
                *word_doc_count.entry(word.clone()).or_insert(0) += 1;
            }
        }

        for (word, count) in word_doc_count {
            let idf = (self.total_docs as f32 / count as f32).ln();
            self.idf_cache.insert(word, idf);
        }
    }

    /// Search using TF-IDF similarity.
    pub fn search(&self, query: &str, limit: Option<usize>) -> Vec<(String, f32)> {
        let query_words = self.tokenize(query);
        let query_freq = self.compute_word_freq(&query_words);

        let mut results: Vec<(String, f32)> = Vec::new();

        for (doc, doc_freq) in &self.doc_word_freq {
            let similarity = self.compute_tfidf_similarity(&query_freq, doc_freq);

            if similarity > 0.0 {
                results.push((doc.clone(), similarity));
            }
        }

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        if let Some(max) = limit {
            results.into_iter().take(max).collect()
        } else {
            results
        }
    }

    /// Search with multiple keywords.
    pub fn search_multi(&self, keywords: &[&str], limit: Option<usize>) -> Vec<(String, f64)> {
        let mut doc_scores: HashMap<String, f64> = HashMap::new();

        for keyword in keywords {
            let results = self.search(keyword, None);
            for (doc, score) in results {
                *doc_scores.entry(doc).or_insert(0.0) += score as f64;
            }
        }

        let num_keywords = keywords.len().max(1);
        for (_, score) in doc_scores.iter_mut() {
            *score /= num_keywords as f64;
        }

        let mut results: Vec<(String, f64)> = doc_scores.into_iter().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        if let Some(max) = limit {
            results.into_iter().take(max).collect()
        } else {
            results
        }
    }

    /// Compute TF-IDF similarity.
    fn compute_tfidf_similarity(
        &self,
        query_freq: &HashMap<String, f32>,
        doc_freq: &HashMap<String, f32>,
    ) -> f32 {
        let mut similarity = 0.0;

        for (word, tf_query) in query_freq {
            if let Some(tf_doc) = doc_freq.get(word)
                && let Some(idf) = self.idf_cache.get(word)
            {
                similarity += tf_query * idf * tf_doc * idf;
            }
        }

        similarity
    }

    /// Clear all indices.
    pub fn clear(&mut self) {
        self.doc_word_freq.clear();
        self.idf_cache.clear();
        self.total_docs = 0;
    }
}

impl Default for TfIdfSearch {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// AI Memory Selection (Claude Code style)
// ============================================================================

/// System prompt for AI memory selection.
const SELECT_MEMORIES_SYSTEM_PROMPT: &str = r#"你正在选择对处理用户查询有用的记忆。你会收到用户的查询和可用记忆文件列表（包含描述）。

返回最有用的记忆索引列表（最多5个），以 JSON 数组格式返回。
- 只选择你确定会有帮助的记忆
- 如果不确定某个记忆是否有用，不要选择它
- 如果没有明显有用的记忆，可以返回空数组 []
- 优先选择与当前问题直接相关的记忆

返回格式示例：{"selected": [0, 2, 5]}
"#;

/// Select relevant memories using AI (Claude Code style).
///
/// Takes user query and memory manifest (descriptions), uses AI to select
/// the most relevant ones (up to 5).
pub async fn ai_select_memories(
    query: &str,
    memory_manifest: &str,
    provider: &dyn crate::providers::Provider,
) -> Vec<usize> {
    use crate::providers::{ChatRequest, Message, MessageContent, Role};

    // Truncate query if too long
    let truncated_query = if query.len() > 1000 {
        &query[..1000]
    } else {
        query
    };

    let user_prompt = format!(
        "查询: {}\n\n可用记忆列表:\n{}\n\n请选择最有用的记忆索引（最多5个）：",
        truncated_query, memory_manifest
    );

    let request = ChatRequest {
        messages: vec![Message {
            role: Role::User,
            content: MessageContent::Text(user_prompt),
        }],
        tools: vec![],
        system: Some(SELECT_MEMORIES_SYSTEM_PROMPT.to_string()),
        think: false,
        max_tokens: 100,
        server_tools: vec![],
        enable_caching: false,
    };

    let response = match provider.chat(request).await {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    // Extract text from response
    let text = response
        .content
        .iter()
        .filter_map(|block| {
            if let crate::providers::ContentBlock::Text { text } = block {
                Some(text.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("");

    // Parse JSON response
    parse_selected_indices(&text)
}

/// Parse selected indices from AI response.
fn parse_selected_indices(text: &str) -> Vec<usize> {
    // Try to parse as JSON
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(text) {
        if let Some(selected) = json.get("selected").and_then(|s| s.as_array()) {
            return selected
                .iter()
                .filter_map(|v| v.as_u64().map(|n| n as usize))
                .collect();
        }
        // Also try direct array format
        if let Some(arr) = json.as_array() {
            return arr
                .iter()
                .filter_map(|v| v.as_u64().map(|n| n as usize))
                .collect();
        }
    }

    // Fallback: try to extract numbers from text
    let mut indices = Vec::new();
    for part in text.split(',') {
        let trimmed = part.trim();
        if let Ok(n) = trimmed.parse::<usize>() {
            indices.push(n);
        }
    }
    indices
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::MemoryCategory;

    #[test]
    fn test_extract_keywords() {
        let keywords = extract_context_keywords("使用 PostgreSQL 数据库配置");
        assert!(!keywords.is_empty());
    }

    #[test]
    fn test_semantic_aliases() {
        let keywords = vec!["数据库".to_string()];
        let expanded = expand_semantic_keywords(&keywords);
        assert!(expanded.contains(&"database".to_string()));
    }

    #[test]
    fn test_tfidf_search() {
        let mut tfidf = TfIdfSearch::new();
        let mut memory = AutoMemory::new();

        // Add multiple documents so IDF calculation works properly
        // (IDF = ln(N/df) where N is total docs, df is docs containing word)
        memory.add(MemoryEntry::new(
            MemoryCategory::Decision,
            "使用 PostgreSQL 作为数据库".to_string(),
            None,
            None,
        ));
        memory.add(MemoryEntry::new(
            MemoryCategory::Decision,
            "前端使用 React 框架开发".to_string(),
            None,
            None,
        ));
        memory.add(MemoryEntry::new(
            MemoryCategory::Decision,
            "后端采用 Rust 编写".to_string(),
            None,
            None,
        ));

        tfidf.index(&memory);
        let results = tfidf.search("数据库", Some(5));
        assert!(!results.is_empty());

        // The PostgreSQL document should be the top result
        assert!(results[0].0.contains("PostgreSQL"));
    }
}
