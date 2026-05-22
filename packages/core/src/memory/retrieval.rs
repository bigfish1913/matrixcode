//! Retrieval helpers: TF-IDF search, semantic aliases, keyword extraction.

use std::collections::{HashMap, HashSet};

use super::config::*;
use super::keywords_config::KeywordsConfig;
use super::types::{AutoMemory, MemoryEntry};

// ============================================================================
// Keyword Extraction (uses KeywordsConfig)
// ============================================================================

/// Extract meaningful keywords from conversation context.
/// Uses KeywordsConfig for stop words and tech keywords.
/// Improved: avoids meaningless character fragments.
pub fn extract_context_keywords(context: &str) -> Vec<String> {
    let config = KeywordsConfig::load();
    let stop_words = config.get_stop_words_set();
    let tech_patterns = config.get_tech_keywords_set();

    let lower = context.to_lowercase();
    let mut keywords: HashSet<String> = HashSet::new();

    // 1. Extract English words (must be meaningful - at least 3 chars)
    for word in lower.split_whitespace() {
        let cleaned = word
            .trim_matches(|c: char| !c.is_alphanumeric())
            .to_string();
        // Only accept words at least 3 chars to avoid fragments like "ok", "go"
        if cleaned.len() >= 3 && !stop_words.contains(cleaned.as_str()) {
            keywords.insert(cleaned.clone());
        }
        // Always accept known tech patterns even if short
        if tech_patterns.contains(cleaned.as_str()) {
            keywords.insert(cleaned);
        }
    }

    // 2. Extract meaningful Chinese phrases using known patterns from config
    // Instead of sliding window (which produces nonsense fragments),
    // use the predefined patterns for decision/preference/solution/finding
    for category_patterns in config.patterns.values() {
        for pattern in category_patterns {
            if lower.contains(&pattern.to_lowercase()) {
                keywords.insert(pattern.clone());
            }
        }
    }

    // 3. Extract known tech keywords from config
    for kw in &config.tech_keywords {
        if lower.contains(&kw.to_lowercase()) && !stop_words.contains(kw.as_str()) {
            keywords.insert(kw.clone());
        }
    }

    // 4. Extract specific tech patterns (camelCase, snake_case, file paths)
    let tech_regexes = [
        r"[a-zA-Z_][a-zA-Z0-9_]*\.[a-zA-Z]{1,4}",       // file extensions like .rs, .ts
        r"[a-zA-Z_][a-zA-Z0-9_]*\.[a-zA-Z_][a-zA-Z0-9_]*", // module.function
        r"[A-Z][a-z]+[A-Z][a-zA-Z]*",                   // CamelCase
        r"[a-z][a-z0-9]*_[a-z][a-z0-9_]*",              // snake_case
        r"[0-9]+[kKmMgGtT][bB]?",                       // sizes like 4KB, 2MB
        r"[a-zA-Z]+-[a-zA-Z]+",                         // hyphenated like react-dom
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

    // Sort by length (longer = more specific) and limit to 10
    let mut result: Vec<String> = keywords.into_iter().collect();
    result.sort_by_key(|b| std::cmp::Reverse(b.len()));
    result.truncate(10);

    result
}

/// Calculate word-based similarity between two strings (Jaccard coefficient).
pub fn calculate_similarity(a: &str, b: &str) -> f64 {
    AutoMemory::calculate_similarity(a, b)
}

// ============================================================================
// Semantic Aliases (uses KeywordsConfig)
// ============================================================================

/// Get semantic aliases from KeywordsConfig.
pub fn get_semantic_aliases() -> Vec<(&'static str, &'static str)> {
    // Note: This returns static references for compatibility
    // For dynamic config, use KeywordsConfig::load().get_aliases()
    SEMANTIC_ALIASES_DEFAULT.to_vec()
}

/// Default semantic aliases (embedded for fallback).
pub const SEMANTIC_ALIASES_DEFAULT: &[(&str, &str)] = &[
    // Database related
    ("数据库", "database"),
    ("db", "database"),
    ("postgresql", "postgres"),
    ("mysql", "mysql"),
    ("mongodb", "mongo"),
    ("redis", "redis"),
    ("sqlite", "sqlite"),
    ("sql", "database"),
    // Frontend related
    ("前端", "frontend"),
    ("ui", "frontend"),
    ("界面", "frontend"),
    ("页面", "page"),
    ("组件", "component"),
    ("react", "react"),
    ("vue", "vue"),
    ("angular", "angular"),
    // Backend related
    ("后端", "backend"),
    ("api", "api"),
    ("接口", "api"),
    ("服务", "service"),
    ("server", "backend"),
    ("服务器", "backend"),
    // Framework/Language
    ("rust", "rust"),
    ("python", "python"),
    ("javascript", "js"),
    ("typescript", "ts"),
    ("java", "java"),
    ("go", "golang"),
    ("golang", "go"),
    ("c++", "cpp"),
    ("cpp", "c++"),
    ("nodejs", "node"),
    ("node", "nodejs"),
    // Tools
    ("编辑器", "editor"),
    ("ide", "editor"),
    ("vim", "vim"),
    ("vscode", "vscode"),
    ("emacs", "emacs"),
    // Config
    ("配置", "config"),
    ("设置", "config"),
    ("config", "config"),
    ("setting", "config"),
    // Structure
    ("目录", "directory"),
    ("文件", "file"),
    ("文件夹", "directory"),
    ("路径", "path"),
    ("模块", "module"),
    ("包", "package"),
    // Testing
    ("测试", "test"),
    ("test", "test"),
    ("单元测试", "unittest"),
    ("unittest", "test"),
    // Cache
    ("缓存", "cache"),
    ("cache", "cache"),
    // Auth
    ("认证", "auth"),
    ("登录", "login"),
    ("auth", "auth"),
    ("登录", "auth"),
    // Performance
    ("性能", "performance"),
    ("优化", "optimize"),
    ("速度", "speed"),
    ("慢", "slow"),
    // Common verbs
    ("创建", "create"),
    ("删除", "delete"),
    ("修改", "modify"),
    ("添加", "add"),
    ("更新", "update"),
    ("查询", "query"),
];

/// Expand keywords with semantic aliases from KeywordsConfig.
pub fn expand_semantic_keywords(keywords: &[String]) -> Vec<String> {
    let config = KeywordsConfig::load();
    let mut expanded: Vec<String> = keywords.to_vec();

    for keyword in keywords {
        let kw_lower = keyword.to_lowercase();
        for (alias, target) in config.get_aliases() {
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
/// Uses KeywordsConfig for contradiction signals.
pub fn has_contradiction_signal(old: &str, new: &str) -> bool {
    let config = KeywordsConfig::load();

    // Check contradiction signals from config
    for signal in &config.contradiction_signals {
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

        // Add multiple documents so IDF calculation works properly
        // (IDF = ln(N/df) where N is total docs, df is docs containing word)
        memory.add(super::super::types::MemoryEntry::new(
            super::super::types::MemoryCategory::Decision,
            "使用 PostgreSQL 作为数据库".to_string(),
            None,
        ));
        memory.add(super::super::types::MemoryEntry::new(
            super::super::types::MemoryCategory::Decision,
            "前端使用 React 框架开发".to_string(),
            None,
        ));
        memory.add(super::super::types::MemoryEntry::new(
            super::super::types::MemoryCategory::Decision,
            "后端采用 Rust 编写".to_string(),
            None,
        ));

        tfidf.index(&memory);
        let results = tfidf.search("数据库", Some(5));
        assert!(!results.is_empty());

        // The PostgreSQL document should be the top result
        assert!(results[0].0.contains("PostgreSQL"));
    }
}
