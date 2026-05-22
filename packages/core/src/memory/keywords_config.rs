//! Keywords configuration for memory detection.
//! Loads from ~/.matrix/keywords.json or uses embedded defaults.

use std::collections::{HashMap, HashSet};
use std::env::var_os;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::types::MemoryCategory;

// ============================================================================
// Helper Functions
// ============================================================================

/// Get the home directory.
fn home_dir() -> Option<PathBuf> {
    var_os("HOME")
        .or_else(|| var_os("USERPROFILE"))
        .map(PathBuf::from)
}

// ============================================================================
// Embedded Default Keywords (fallback when no config file)
// ============================================================================

/// Get embedded default keywords configuration.
pub fn get_default_keywords() -> KeywordsConfig {
    KeywordsConfig {
        version: 1,
        patterns: default_patterns(),
        stop_words: default_stop_words(),
        semantic_aliases: default_aliases(),
        contradiction_signals: default_contradiction_signals(),
        tech_keywords: default_tech_keywords(),
    }
}

fn default_patterns() -> HashMap<String, Vec<String>> {
    HashMap::from([
        (
            "decision".to_string(),
            vec![
                "最终决定".to_string(),
                "决定采用".to_string(),
                "我们决定".to_string(),
                "选择使用".to_string(),
                "采用方案".to_string(),
                "定下来".to_string(),
                "就定这个".to_string(),
                "敲定".to_string(),
                "拍板".to_string(),
                "we decided".to_string(),
                "final decision".to_string(),
                "decided to".to_string(),
                "chose to".to_string(),
            ],
        ),
        (
            "preference".to_string(),
            vec![
                "我喜欢".to_string(),
                "我偏好".to_string(),
                "我习惯".to_string(),
                "最常用".to_string(),
                "一直用".to_string(),
                "推荐".to_string(),
                "建议使用".to_string(),
                "首选".to_string(),
                "prefer".to_string(),
                "i like".to_string(),
                "i prefer".to_string(),
                "my favorite".to_string(),
            ],
        ),
        (
            "solution".to_string(),
            vec![
                "通过修改".to_string(),
                "解决方案是".to_string(),
                "搞定".to_string(),
                "解决了".to_string(),
                "修复成功".to_string(),
                "改成".to_string(),
                "优化了".to_string(),
                "fixed by".to_string(),
                "solved by".to_string(),
                "resolved".to_string(),
            ],
        ),
        (
            "finding".to_string(),
            vec![
                "发现".to_string(),
                "注意到".to_string(),
                "原来".to_string(),
                "找到问题".to_string(),
                "定位到".to_string(),
                "排查发现".to_string(),
                "原因是".to_string(),
                "found that".to_string(),
                "discovered".to_string(),
                "the reason is".to_string(),
            ],
        ),
        (
            "technical".to_string(),
            vec![
                "技术栈是".to_string(),
                "框架使用".to_string(),
                "用的是".to_string(),
                "基于".to_string(),
                "tech stack".to_string(),
                "using framework".to_string(),
                "built with".to_string(),
                "powered by".to_string(),
            ],
        ),
        (
            "structure".to_string(),
            vec![
                "入口文件是".to_string(),
                "主文件位于".to_string(),
                "项目结构是".to_string(),
                "入口是".to_string(),
                "目录是".to_string(),
                "entry point".to_string(),
                "main file".to_string(),
                "located at".to_string(),
            ],
        ),
    ])
}

fn default_stop_words() -> StopWordsConfig {
    StopWordsConfig {
        chinese: vec![
            "的", "了", "是", "在", "我", "有", "和", "就", "不", "人", "都", "一", "一个", "上",
            "也", "很", "到", "说", "要", "去", "你", "会", "着", "没有", "看", "好", "自己", "这",
            "他", "她", "它", "们", "那", "些", "什么", "怎么", "如何", "请", "能", "可以", "需要",
            "应该", "可能", "因为", "所以", "但是", "然后", "还是", "已经", "正在", "将要", "曾经",
            "一下", "一点", "一些", "所有", "每个", "任何",
        ]
        .into_iter()
        .map(|s| s.to_string())
        .collect(),
        english: vec![
            "the", "a", "an", "is", "are", "was", "were", "be", "been", "being", "have", "has",
            "had", "do", "does", "did", "will", "would", "could", "should", "may", "might", "can",
            "shall", "to", "of", "in", "for", "on", "with", "at", "by", "from", "as", "into",
            "through", "during", "before", "after", "above", "below", "between", "and", "but",
            "or", "not", "no", "so", "if", "then", "than", "too", "very", "just", "this", "that",
            "these", "those", "it", "its", "i", "me", "my", "we", "our", "you", "your", "he",
            "his", "she", "her", "they", "their", "please", "help", "need", "want", "make", "get",
            "let", "use",
        ]
        .into_iter()
        .map(|s| s.to_string())
        .collect(),
    }
}

fn default_aliases() -> Vec<[String; 2]> {
    vec![
        ["数据库".to_string(), "database".to_string()],
        ["db".to_string(), "database".to_string()],
        ["前端".to_string(), "frontend".to_string()],
        ["ui".to_string(), "frontend".to_string()],
        ["界面".to_string(), "frontend".to_string()],
        ["后端".to_string(), "backend".to_string()],
        ["api".to_string(), "api".to_string()],
        ["接口".to_string(), "api".to_string()],
        ["服务".to_string(), "service".to_string()],
        ["服务器".to_string(), "server".to_string()],
        ["配置".to_string(), "config".to_string()],
        ["设置".to_string(), "setting".to_string()],
        ["目录".to_string(), "directory".to_string()],
        ["文件".to_string(), "file".to_string()],
        ["路径".to_string(), "path".to_string()],
        ["测试".to_string(), "test".to_string()],
        ["缓存".to_string(), "cache".to_string()],
        ["认证".to_string(), "auth".to_string()],
        ["登录".to_string(), "login".to_string()],
        ["性能".to_string(), "performance".to_string()],
        ["优化".to_string(), "optimize".to_string()],
        ["创建".to_string(), "create".to_string()],
        ["删除".to_string(), "delete".to_string()],
        ["修改".to_string(), "modify".to_string()],
        ["添加".to_string(), "add".to_string()],
        ["更新".to_string(), "update".to_string()],
        ["查询".to_string(), "query".to_string()],
    ]
}

fn default_contradiction_signals() -> Vec<String> {
    vec![
        "改用".to_string(),
        "换成".to_string(),
        "替换".to_string(),
        "改为".to_string(),
        "切换到".to_string(),
        "迁移到".to_string(),
        "不再使用".to_string(),
        "弃用".to_string(),
        "放弃".to_string(),
        "取消".to_string(),
        "switched to".to_string(),
        "replaced".to_string(),
        "migrated to".to_string(),
        "changed to".to_string(),
        "no longer".to_string(),
        "deprecated".to_string(),
        "abandoned".to_string(),
    ]
}

fn default_tech_keywords() -> Vec<String> {
    vec![
        "api".to_string(),
        "cli".to_string(),
        "gui".to_string(),
        "tui".to_string(),
        "web".to_string(),
        "http".to_string(),
        "json".to_string(),
        "xml".to_string(),
        "sql".to_string(),
        "db".to_string(),
        "git".to_string(),
        "npm".to_string(),
        "cargo".to_string(),
        "rust".to_string(),
        "js".to_string(),
        "ts".to_string(),
        "py".to_string(),
        "go".to_string(),
        "java".to_string(),
        "cpp".to_string(),
        "cpu".to_string(),
        "gpu".to_string(),
        "io".to_string(),
        "fs".to_string(),
        "os".to_string(),
        "ux".to_string(),
        "ai".to_string(),
        "ml".to_string(),
        "dl".to_string(),
        "yaml".to_string(),
        "yml".to_string(),
        "toml".to_string(),
        "md".to_string(),
        "txt".to_string(),
        "html".to_string(),
        "css".to_string(),
        "scss".to_string(),
        "bug".to_string(),
        "fix".to_string(),
        "code".to_string(),
        "data".to_string(),
    ]
}

// ============================================================================
// Configuration Structures
// ============================================================================

/// Keywords configuration for memory detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeywordsConfig {
    /// Configuration version.
    pub version: u32,
    /// Category detection patterns.
    pub patterns: HashMap<String, Vec<String>>,
    /// Stop words for keyword filtering.
    pub stop_words: StopWordsConfig,
    /// Semantic alias mappings.
    pub semantic_aliases: Vec<[String; 2]>,
    /// Contradiction/change signal words.
    pub contradiction_signals: Vec<String>,
    /// Technical keywords for extraction.
    pub tech_keywords: Vec<String>,
}

/// Stop words configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopWordsConfig {
    /// Chinese stop words.
    pub chinese: Vec<String>,
    /// English stop words.
    pub english: Vec<String>,
}

impl KeywordsConfig {
    /// Load configuration from file or use defaults.
    pub fn load() -> Self {
        // Try user config directory first
        if let Some(home) = home_dir() {
            let config_path = home.join(".matrix").join("keywords.json");
            if config_path.exists()
                && let Ok(content) = std::fs::read_to_string(&config_path)
            {
                if let Ok(config) = serde_json::from_str::<Self>(&content) {
                    log::info!("Loaded keywords config from {}", config_path.display());
                    return config;
                } else {
                    log::warn!(
                        "Failed to parse keywords config from {}, using defaults",
                        config_path.display()
                    );
                }
            }
        }

        // Try project config directory
        if let Ok(cwd) = std::env::current_dir() {
            let project_config = cwd.join(".matrix").join("keywords.json");
            if project_config.exists()
                && let Ok(content) = std::fs::read_to_string(&project_config)
                && let Ok(config) = serde_json::from_str::<Self>(&content)
            {
                log::info!("Loaded keywords config from {}", project_config.display());
                return config;
            }
        }

        // Use embedded defaults
        get_default_keywords()
    }

    /// Get patterns for a specific category.
    pub fn get_patterns(&self, category: MemoryCategory) -> &[String] {
        let key = match category {
            MemoryCategory::Decision => "decision",
            MemoryCategory::Preference => "preference",
            MemoryCategory::Solution => "solution",
            MemoryCategory::Finding => "finding",
            MemoryCategory::Technical => "technical",
            MemoryCategory::Structure => "structure",
            // New categories - use same patterns as parent categories
            MemoryCategory::KeyDecision => "decision",
            MemoryCategory::FailedApproach => "solution",
            MemoryCategory::UserIntentPattern => "preference",
            MemoryCategory::TaskPattern => "solution",
        };
        self.patterns.get(key).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get all stop words as a set.
    pub fn get_stop_words_set(&self) -> HashSet<&str> {
        self.stop_words
            .chinese
            .iter()
            .chain(self.stop_words.english.iter())
            .map(|s| s.as_str())
            .collect()
    }

    /// Get tech keywords as a set.
    pub fn get_tech_keywords_set(&self) -> HashSet<&str> {
        self.tech_keywords.iter().map(|s| s.as_str()).collect()
    }

    /// Get semantic aliases as tuples.
    pub fn get_aliases(&self) -> Vec<(&str, &str)> {
        self.semantic_aliases
            .iter()
            .map(|pair| (pair[0].as_str(), pair[1].as_str()))
            .collect()
    }

    /// Save default config to user directory (for customization).
    pub fn save_default_to_user_dir() -> anyhow::Result<PathBuf> {
        if let Some(home) = home_dir() {
            let config_dir = home.join(".matrix");
            std::fs::create_dir_all(&config_dir)?;
            let config_path = config_dir.join("keywords.json");
            let default = get_default_keywords();
            let content = serde_json::to_string_pretty(&default)?;
            std::fs::write(&config_path, content)?;
            log::info!("Saved default keywords config to {}", config_path.display());
            return Ok(config_path);
        }
        anyhow::bail!("Could not determine home directory")
    }

    /// Add new tech keywords (from AI extraction).
    /// Returns true if any new keywords were added.
    pub fn add_keywords(&mut self, new_keywords: &[String]) -> bool {
        let mut added = false;
        for kw in new_keywords {
            let kw_lower = kw.to_lowercase();
            if !self
                .tech_keywords
                .iter()
                .any(|t| t.to_lowercase() == kw_lower)
            {
                self.tech_keywords.push(kw.clone());
                added = true;
                log::debug!("Added new keyword: {}", kw);
            }
        }
        added
    }

    /// Add new semantic alias (from AI extraction).
    /// Returns true if the alias was added.
    pub fn add_alias(&mut self, alias: &str, target: &str) -> bool {
        // Check if alias already exists
        for pair in &self.semantic_aliases {
            if pair[0] == alias || pair[1] == target {
                return false;
            }
        }
        self.semantic_aliases
            .push([alias.to_string(), target.to_string()]);
        log::debug!("Added new alias: {} -> {}", alias, target);
        true
    }

    /// Add new pattern for a category (from AI extraction).
    /// Returns true if the pattern was added.
    pub fn add_pattern(&mut self, category: &str, pattern: &str) -> bool {
        let patterns = self.patterns.get_mut(category);
        if let Some(list) = patterns {
            let pattern_lower = pattern.to_lowercase();
            if !list.iter().any(|p| p.to_lowercase() == pattern_lower) {
                list.push(pattern.to_string());
                log::debug!("Added new pattern for {}: {}", category, pattern);
                return true;
            }
        }
        false
    }

    /// Save config to user directory (~/.matrix/keywords.json).
    pub fn save(&self) -> anyhow::Result<PathBuf> {
        if let Some(home) = home_dir() {
            let config_dir = home.join(".matrix");
            std::fs::create_dir_all(&config_dir)?;
            let config_path = config_dir.join("keywords.json");
            let content = serde_json::to_string_pretty(self)?;
            std::fs::write(&config_path, content)?;
            log::info!("Saved keywords config to {}", config_path.display());
            return Ok(config_path);
        }
        anyhow::bail!("Could not determine home directory")
    }

    /// Load, update with new keywords, and save.
    /// Used by AI to automatically expand the keyword library.
    pub fn update_and_save(
        new_keywords: &[String],
        new_aliases: Option<&[(String, String)]>,
    ) -> anyhow::Result<PathBuf> {
        let mut config = Self::load();
        let mut changed = config.add_keywords(new_keywords);

        if let Some(aliases) = new_aliases {
            for (alias, target) in aliases {
                changed |= config.add_alias(alias, target);
            }
        }

        if changed {
            config.save()
        } else {
            // Return path even if not saved
            home_dir()
                .map(|h| h.join(".matrix").join("keywords.json"))
                .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))
        }
    }
}

impl Default for KeywordsConfig {
    fn default() -> Self {
        Self::load()
    }
}
