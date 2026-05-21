//! Retrieval helpers: TF-IDF search, semantic aliases, keyword extraction.

use std::collections::{HashMap, HashSet};

use super::config::*;
use super::types::{AutoMemory, MemoryEntry};

// ============================================================================
// Keyword Extraction
// ============================================================================

/// Extract meaningful keywords from conversation context.
/// Filters out common stop words and short tokens.
pub fn extract_context_keywords(context: &str) -> Vec<String> {
    // Common stop words (Chinese + English)
    let stop_words: HashSet<&str> = [
        // Chinese stop words
        "的", "了", "是", "在", "我", "有", "和", "就", "不", "人", "都", "一", "一个",
        "上", "也", "很", "到", "说", "要", "去", "你", "会", "着", "没有", "看", "好",
        "自己", "这", "他", "她", "它", "们", "那", "些", "什么", "怎么", "如何", "请",
        "能", "可以", "需要", "应该", "可能", "因为", "所以", "但是", "然后", "还是",
        "已经", "正在", "将要", "曾经", "一下", "一点", "一些", "所有", "每个", "任何",
        // English stop words
        "the", "a", "an", "is", "are", "was", "were", "be", "been", "being",
        "have", "has", "had", "do", "does", "did", "will", "would", "could",
        "should", "may", "might", "can", "shall", "to", "of", "in", "for",
        "on", "with", "at", "by", "from", "as", "into", "through", "during",
        "before", "after", "above", "below", "between", "and", "but", "or",
        "not", "no", "so", "if", "then", "than", "too", "very", "just",
        "this", "that", "these", "those", "it", "its", "i", "me", "my",
        "we", "our", "you", "your", "he", "his", "she", "her", "they", "their",
        "please", "help", "need", "want", "make", "get", "let", "use",
    ].iter().copied().collect();

    // Technical/meaningful patterns
    let tech_patterns: HashSet<&str> = [
        "api", "cli", "gui", "tui", "web", "http", "json", "xml", "sql", "db",
        "git", "npm", "cargo", "rust", "js", "ts", "py", "go", "java", "cpp",
        "cpu", "gpu", "io", "fs", "os", "ui", "ux", "ai", "ml", "dl",
        "rs", "js", "ts", "py", "go", "java", "c", "h", "cpp", "hpp",
        "json", "yaml", "yml", "toml", "md", "txt", "html", "css", "scss",
        "bug", "fix", "add", "new", "old", "use", "run", "build", "test",
        "code", "data", "file", "dir", "path", "name", "type", "value",
    ].iter().copied().collect();

    let lower = context.to_lowercase();
    let mut keywords: HashSet<String> = HashSet::new();

    // 1. Extract English words
    for word in lower.split_whitespace() {
        let cleaned = word.trim_matches(|c: char| !c.is_alphanumeric()).to_string();
        if cleaned.len() >= 2 && !stop_words.contains(cleaned.as_str()) {
            keywords.insert(cleaned.clone());
        }
        if tech_patterns.contains(cleaned.as_str()) {
            keywords.insert(cleaned);
        }
    }

    // 2. Extract Chinese words/phrases
    let chinese_chars: Vec<char> = lower
        .chars()
        .filter(|c| *c >= '\u{4E00}' && *c <= '\u{9FFF}')
        .collect();

    for window_size in 2..=4 {
        if chinese_chars.len() >= window_size {
            for window in chinese_chars.windows(window_size) {
                let phrase: String = window.iter().collect();
                let has_stop = stop_words.iter().any(|sw| phrase.contains(sw));
                if !has_stop && phrase.len() >= window_size {
                    keywords.insert(phrase);
                }
            }
        }
    }

    // 3. Extract specific patterns
    let patterns = [
        r"[a-zA-Z_][a-zA-Z0-9_]*\.[a-zA-Z]{1,4}",
        r"[a-zA-Z_][a-zA-Z0-9_]*\.[a-zA-Z_][a-zA-Z0-9_]*",
        r"[A-Z][a-z]+[A-Z][a-zA-Z]*",
        r"[a-z][a-z0-9]*_[a-z][a-z0-9_]*",
        r"[0-9]+[kKmMgGtT][bB]?",
    ];

    for pattern in patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            for cap in re.find_iter(&lower) {
                keywords.insert(cap.as_str().to_string());
            }
        }
    }

    let mut result: Vec<String> = keywords.into_iter().collect();
    result.sort_by_key(|b| std::cmp::Reverse(b.len()));
    result.truncate(15);

    result
}

/// Calculate word-based similarity between two strings (Jaccard coefficient).
pub fn calculate_similarity(a: &str, b: &str) -> f64 {
    AutoMemory::calculate_similarity(a, b)
}

// ============================================================================
// Semantic Aliases
// ============================================================================

/// Semantic alias mappings for better keyword matching.
pub const SEMANTIC_ALIASES: &[(&str, &str)] = &[
    // Database related
    ("数据库", "database"), ("db", "database"),
    ("postgresql", "postgres"), ("mysql", "mysql"),
    ("mongodb", "mongo"), ("redis", "redis"),
    ("sqlite", "sqlite"), ("sql", "database"),
    // Frontend related
    ("前端", "frontend"), ("ui", "frontend"),
    ("界面", "frontend"), ("页面", "page"),
    ("组件", "component"), ("react", "react"),
    ("vue", "vue"), ("angular", "angular"),
    // Backend related
    ("后端", "backend"), ("api", "api"),
    ("接口", "api"), ("服务", "service"),
    ("server", "backend"), ("服务器", "backend"),
    // Framework/Language
    ("rust", "rust"), ("python", "python"),
    ("javascript", "js"), ("typescript", "ts"),
    ("java", "java"), ("go", "golang"),
    ("golang", "go"), ("c++", "cpp"),
    ("cpp", "c++"), ("nodejs", "node"),
    ("node", "nodejs"),
    // Tools
    ("编辑器", "editor"), ("ide", "editor"),
    ("vim", "vim"), ("vscode", "vscode"),
    ("emacs", "emacs"),
    // Config
    ("配置", "config"), ("设置", "config"),
    ("config", "config"), ("setting", "config"),
    // Structure
    ("目录", "directory"), ("文件", "file"),
    ("文件夹", "directory"), ("路径", "path"),
    ("模块", "module"), ("包", "package"),
    // Testing
    ("测试", "test"), ("test", "test"),
    ("单元测试", "unittest"), ("unittest", "test"),
    // Cache
    ("缓存", "cache"), ("cache", "cache"),
    // Auth
    ("认证", "auth"), ("登录", "login"),
    ("auth", "auth"), ("登录", "auth"),
    // Performance
    ("性能", "performance"), ("优化", "optimize"),
    ("速度", "speed"), ("慢", "slow"),
    // Common verbs
    ("创建", "create"), ("删除", "delete"),
    ("修改", "modify"), ("添加", "add"),
    ("更新", "update"), ("查询", "query"),
];

/// Expand keywords with semantic aliases.
pub fn expand_semantic_keywords(keywords: &[String]) -> Vec<String> {
    let mut expanded: Vec<String> = keywords.to_vec();

    for keyword in keywords {
        let kw_lower = keyword.to_lowercase();
        for (alias, target) in SEMANTIC_ALIASES {
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
// Relevance & Contradiction Detection
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

    let tag_matches = entry.tags
        .iter()
        .filter(|tag| {
            let tag_lower = tag.to_lowercase();
            expanded_keywords.iter().any(|kw| {
                tag_lower.contains(&kw.to_lowercase()) ||
                kw.to_lowercase().contains(&tag_lower)
            })
        })
        .count();

    let tag_score = if tag_matches > 0 { 0.2 + (tag_matches as f64 * 0.05).min(0.1) } else { 0.0 };

    (keyword_score + tag_score).min(1.0)
}

/// Detect if two memory contents have contradiction signals.
pub fn has_contradiction_signal(old: &str, new: &str) -> bool {
    let change_signals = [
        "改用", "换成", "替换", "改为", "切换到", "迁移到",
        "不再使用", "弃用", "放弃", "取消",
        "switched to", "replaced", "migrated to", "changed to",
        "no longer", "deprecated", "abandoned",
    ];

    for signal in &change_signals {
        if new.contains(signal) {
            return true;
        }
    }

    let action_verbs = [
        "决定使用", "选择使用", "采用", "使用",
        "decided to use", "chose", "using", "adopted",
    ];

    for verb in &action_verbs {
        if old.contains(verb) && new.contains(verb) {
            return true;
        }
    }

    let pref_verbs = ["偏好", "喜欢", "prefer", "like"];
    for verb in &pref_verbs {
        if old.contains(verb) && new.contains(verb) {
            return true;
        }
    }

    false
}

// ============================================================================
// AI Keyword Extraction (Hybrid)
// ============================================================================

/// Extract keywords using hybrid approach (rule-based + AI fallback).
pub async fn extract_keywords_hybrid(
    context: &str,
    fast_provider: Option<&dyn crate::providers::Provider>,
) -> Vec<String> {
    // First try rule-based extraction
    let rule_keywords = extract_context_keywords(context);

    // Check if we need AI fallback
    let mode = AiKeywordMode::from_env();
    if mode.should_use_ai(rule_keywords.len()) && fast_provider.is_some() {
        // Use AI for keyword extraction
        if let Some(provider) = fast_provider {
            let ai_keywords = extract_keywords_with_ai(context, provider).await;
            if !ai_keywords.is_empty() {
                return ai_keywords;
            }
        }
    }

    rule_keywords
}

/// Extract keywords using AI provider.
async fn extract_keywords_with_ai(
    context: &str,
    provider: &dyn crate::providers::Provider,
) -> Vec<String> {
    use crate::providers::{ChatRequest, Message, MessageContent, Role};

    let truncated = if context.len() > 2000 {
        &context[..2000]
    } else {
        context
    };

    let prompt = format!(
        "从以下对话内容中提取关键词（用于记忆检索），最多返回10个关键词，以逗号分隔：\n\n{}",
        truncated
    );

    let request = ChatRequest {
        messages: vec![Message {
            role: Role::User,
            content: MessageContent::Text(prompt),
        }],
        tools: vec![],
        system: Some("你是一个关键词提取助手，返回关键词列表，不要其他解释。".to_string()),
        think: false,
        max_tokens: 100,
        server_tools: vec![],
        enable_caching: false,
    };

    let response = match provider.chat(request).await {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let text = response.content
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

    text.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| s.len() >= 2)
        .collect()
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
    fn compute_tfidf_similarity(&self, query_freq: &HashMap<String, f32>, doc_freq: &HashMap<String, f32>) -> f32 {
        let mut similarity = 0.0;

        for (word, tf_query) in query_freq {
            if let Some(tf_doc) = doc_freq.get(word)
                && let Some(idf) = self.idf_cache.get(word) {
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

#[cfg(test)]
mod tests {
    use super::*;

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

        memory.add(super::super::types::MemoryEntry::new(
            super::super::types::MemoryCategory::Decision,
            "使用 PostgreSQL 作为数据库".to_string(),
            None,
        ));

        tfidf.index(&memory);
        let results = tfidf.search("数据库", Some(5));
        assert!(!results.is_empty());
    }
}