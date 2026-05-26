//! Keywords configuration for memory detection.
//! Loads from ~/.matrix/keywords.json or uses embedded defaults.

use std::collections::{HashMap, HashSet};
use std::env::var_os;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::entry::MemoryCategory;

fn home_dir() -> Option<PathBuf> {
    var_os("HOME").or_else(|| var_os("USERPROFILE")).map(PathBuf::from)
}

// ============================================================================
// Embedded Defaults (simplified)
// ============================================================================

pub fn get_default_keywords() -> KeywordsConfig {
    KeywordsConfig {
        version: 1,
        patterns: default_patterns(),
        stop_words: default_stop_words(),
        contradiction_signals: default_contradiction_signals(),
    }
}

fn default_patterns() -> HashMap<String, Vec<String>> {
    let data: [(&str, &[&str]); 6] = [
        ("decision", &["决定", "选择", "采用", "定下", "decided", "chose"]),
        ("preference", &["偏好", "习惯", "喜欢", "首选", "prefer", "like"]),
        ("solution", &["解决", "修复", "搞定", "改成", "fixed", "solved"]),
        ("finding", &["发现", "原来", "原因", "定位", "found", "reason"]),
        ("technical", &["技术栈", "框架", "用的", "基于", "stack", "using"]),
        ("structure", &["入口", "主文件", "目录", "位于", "entry", "main"]),
    ];
    data.into_iter()
        .map(|(k, v)| (k.to_string(), v.iter().map(|s| s.to_string()).collect()))
        .collect()
}

fn default_stop_words() -> Vec<String> {
    // Common Chinese + English stop words (simplified)
    vec![
        // Chinese
        "的", "了", "是", "在", "我", "有", "和", "就", "不", "都", "一", "也", "很", "到", "要", "去",
        "你", "会", "着", "没有", "看", "好", "这", "那", "什么", "怎么", "请", "能", "可以", "需要",
        // English
        "the", "a", "an", "is", "are", "was", "were", "be", "have", "has", "do", "will", "would",
        "could", "should", "can", "to", "of", "in", "for", "on", "with", "at", "by", "from", "and",
        "but", "or", "not", "if", "then", "this", "that", "it", "i", "me", "my", "we", "you", "he",
        "she", "they", "please", "help", "need", "want", "let", "use",
    ].into_iter().map(|s| s.to_string()).collect()
}

fn default_contradiction_signals() -> Vec<String> {
    vec!["改用", "换成", "改为", "不再", "弃用", "switched", "changed", "replaced", "deprecated"]
        .into_iter().map(|s| s.to_string()).collect()
}

// ============================================================================
// Configuration Structure
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeywordsConfig {
    pub version: u32,
    pub patterns: HashMap<String, Vec<String>>,
    pub stop_words: Vec<String>,
    pub contradiction_signals: Vec<String>,
}

impl KeywordsConfig {
    pub fn load() -> Self {
        // Try ~/.matrix/keywords.json
        if let Some(home) = home_dir() {
            let path = home.join(".matrix").join("keywords.json");
            if path.exists()
                && let Ok(content) = std::fs::read_to_string(&path)
                    && let Ok(config) = serde_json::from_str::<Self>(&content) {
                        return config;
                    }
        }
        get_default_keywords()
    }

    pub fn get_patterns(&self, category: MemoryCategory) -> &[String] {
        let key = match category {
            MemoryCategory::Decision | MemoryCategory::KeyDecision => "decision",
            MemoryCategory::Preference | MemoryCategory::UserIntentPattern => "preference",
            MemoryCategory::Solution | MemoryCategory::FailedApproach | MemoryCategory::TaskPattern => "solution",
            MemoryCategory::Finding => "finding",
            MemoryCategory::Technical => "technical",
            MemoryCategory::Structure => "structure",
        };
        self.patterns.get(key).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn get_stop_words_set(&self) -> HashSet<&str> {
        self.stop_words.iter().map(|s| s.as_str()).collect()
    }

    pub fn get_aliases() -> Vec<(&'static str, &'static str)> {
        // Static aliases (most common mappings)
        vec![
            ("数据库", "database"), ("db", "database"),
            ("前端", "frontend"), ("ui", "frontend"),
            ("后端", "backend"), ("服务", "service"),
            ("接口", "api"), ("配置", "config"),
            ("测试", "test"), ("缓存", "cache"),
            ("认证", "auth"), ("登录", "login"),
        ]
    }
}

impl Default for KeywordsConfig {
    fn default() -> Self {
        Self::load()
    }
}  