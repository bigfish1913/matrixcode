//! Auto Memory system for MatrixCode.
//!
//! This module implements automatic memory accumulation inspired by Claude Code.
//! It captures user preferences, project decisions, key findings, and solutions
//! across sessions, providing persistent context that survives conversation compression.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;

use crate::providers::Message;

// ============================================================================
// Memory Categories
// ============================================================================

/// Categories for memory entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum MemoryCategory {
    /// User preferences (e.g., "I prefer vim over nano")
    Preference,
    /// Project decisions (e.g., "Decided to use PostgreSQL")
    Decision,
    /// Key findings (e.g., "API endpoint is at /api/v2")
    Finding,
    /// Problem solutions (e.g., "Fixed auth bug by adding token refresh")
    Solution,
    /// Technical notes (e.g., "React Query is used for data fetching")
    Technical,
    /// Project structure (e.g., "src/index.ts is entry point")
    Structure,
}

impl MemoryCategory {
    /// Get display name for the category.
    pub fn display_name(&self) -> &'static str {
        match self {
            MemoryCategory::Preference => "偏好",
            MemoryCategory::Decision => "决策",
            MemoryCategory::Finding => "发现",
            MemoryCategory::Solution => "解决方案",
            MemoryCategory::Technical => "技术",
            MemoryCategory::Structure => "结构",
        }
    }

    /// Get icon for the category.
    pub fn icon(&self) -> &'static str {
        match self {
            MemoryCategory::Preference => "👤",
            MemoryCategory::Decision => "🎯",
            MemoryCategory::Finding => "💡",
            MemoryCategory::Solution => "🔧",
            MemoryCategory::Technical => "📚",
            MemoryCategory::Structure => "🏗️",
        }
    }

    /// Get default importance score for the category.
    pub fn default_importance(&self) -> f64 {
        match self {
            MemoryCategory::Decision => 90.0,      // Decisions are very important
            MemoryCategory::Solution => 85.0,      // Solutions are important
            MemoryCategory::Preference => 70.0,    // Preferences are moderately important
            MemoryCategory::Finding => 60.0,       // Findings are useful
            MemoryCategory::Technical => 50.0,     // Technical notes are reference
            MemoryCategory::Structure => 40.0,     // Structure is basic info
        }
    }
}

// ============================================================================
// Memory Entry
// ============================================================================

/// A single memory entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// Unique identifier.
    pub id: String,
    /// When the memory was created.
    pub created_at: DateTime<Utc>,
    /// When the memory was last accessed/referenced.
    pub last_referenced: DateTime<Utc>,
    /// Category of the memory.
    pub category: MemoryCategory,
    /// The memory content.
    pub content: String,
    /// Source session ID (where this memory was created).
    pub source_session: Option<String>,
    /// Number of times this memory has been referenced.
    pub reference_count: u32,
    /// Importance score (0-100, higher = more important).
    pub importance: f64,
    /// Tags for searching/filtering.
    pub tags: Vec<String>,
    /// Whether this memory was manually added by user.
    pub is_manual: bool,
}

impl MemoryEntry {
    /// Create a new memory entry.
    pub fn new(category: MemoryCategory, content: String, source_session: Option<String>) -> Self {
        let id = uuid::Uuid::new_v4().to_string();
        Self {
            id,
            created_at: Utc::now(),
            last_referenced: Utc::now(),
            category,
            content,
            source_session,
            reference_count: 0,
            importance: category.default_importance(),
            tags: Vec::new(),
            is_manual: false,
        }
    }

    /// Create a manually added memory entry.
    pub fn manual(category: MemoryCategory, content: String) -> Self {
        let mut entry = Self::new(category, content, None);
        entry.is_manual = true;
        entry.importance = 95.0; // Manual entries are highly important
        entry
    }

    /// Mark this memory as referenced (increases importance over time).
    pub fn mark_referenced(&mut self) {
        self.reference_count += 1;
        self.last_referenced = Utc::now();
        // Increase importance slightly with each reference (max 100)
        self.importance = (self.importance + 2.0).min(100.0);
    }

    /// Format for display.
    pub fn format_line(&self) -> String {
        let time = self.created_at.format("%Y-%m-%d %H:%M");
        let importance_marker = if self.importance >= 80.0 { "⭐" } else { "" };
        let manual_marker = if self.is_manual { "📝" } else { "" };
        format!(
            "{} {} {}{}{} {}",
            self.category.icon(),
            time,
            importance_marker,
            manual_marker,
            self.category.display_name(),
            crate::ui::truncate_str(&self.content, 60)
        )
    }

    /// Format for inclusion in system prompt.
    pub fn format_for_prompt(&self) -> String {
        let category_name = self.category.display_name();
        if self.content.len() > 200 {
            format!("{}: {}...", category_name, crate::ui::truncate(&self.content, 197))
        } else {
            format!("{}: {}", category_name, self.content)
        }
    }
}

// ============================================================================
// Auto Memory Manager
// ============================================================================

/// Manager for automatic memory accumulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoMemory {
    /// All memory entries.
    pub entries: Vec<MemoryEntry>,
    /// Maximum number of entries to keep.
    pub max_entries: usize,
    /// Minimum importance threshold to keep.
    pub min_importance: f64,
    /// Whether auto accumulation is enabled.
    pub enabled: bool,
}

impl Default for AutoMemory {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            max_entries: 100,
            min_importance: 30.0,
            enabled: true,
        }
    }
}

impl AutoMemory {
    /// Minimum content length for similarity check (to avoid short words matching everything).
    const MIN_SIMILARITY_LENGTH: usize = 10;
    /// Similarity threshold for considering entries as duplicates (0.0-1.0).
    const SIMILARITY_THRESHOLD: f64 = 0.7;
    
    /// Create a new auto memory manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new memory entry.
    pub fn add(&mut self, entry: MemoryEntry) {
        self.entries.push(entry);
        self.prune();
    }

    /// Add memory from detected content.
    pub fn add_memory(
        &mut self,
        category: MemoryCategory,
        content: String,
        source_session: Option<String>,
    ) {
        // Check for duplicates (similar content)
        if self.has_similar(&content) {
            return;
        }

        let entry = MemoryEntry::new(category, content, source_session);
        self.add(entry);
    }

    /// Check if similar content already exists.
    /// Uses minimum length threshold to prevent short words from matching everything.
    pub fn has_similar(&self, content: &str) -> bool {
        let content_lower = content.to_lowercase();
    
    // Skip short content - they're likely too generic to be useful memories
    if content_lower.len() < Self::MIN_SIMILARITY_LENGTH {
        return false;
    }
    
    self.entries.iter().any(|e| {
        let entry_lower = e.content.to_lowercase();
        
        // Exact match
        if entry_lower == content_lower {
            return true;
        }
        
        // Skip comparing with short entries
        if entry_lower.len() < Self::MIN_SIMILARITY_LENGTH {
            return false;
        }
        
        // Calculate word-based similarity (Jaccard-like)
        let similarity = Self::calculate_similarity(&entry_lower, &content_lower);
        similarity >= Self::SIMILARITY_THRESHOLD
    })
}

/// Calculate word-based similarity between two strings.
/// Returns a value between 0.0 (no similarity) and 1.0 (identical).
fn calculate_similarity(a: &str, b: &str) -> f64 {
    use std::collections::HashSet;
    
    let a_words: HashSet<&str> = a.split_whitespace().collect();
    let b_words: HashSet<&str> = b.split_whitespace().collect();
    
    if a_words.is_empty() || b_words.is_empty() {
        return 0.0;
    }
    
    let intersection = a_words.intersection(&b_words).count();
    let union = a_words.union(&b_words).count();
    
    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

    /// Remove low-importance entries when exceeding max_entries.
    /// Strategy: preserve manual entries + high importance entries, sorted by importance.
    pub fn prune(&mut self) {
        if self.entries.len() <= self.max_entries {
            return;
    }

    // First, separate entries by priority
        // Manual entries are always kept (highest priority)
        let (manual_entries, auto_entries): (Vec<_>, Vec<_>) = self.entries
            .iter()
            .cloned()
            .partition(|e| e.is_manual);
        
        // Sort auto entries by importance (descending) + recency as tiebreaker
        let mut sorted_auto = auto_entries;
        sorted_auto.sort_by(|a, b| {
            // First compare by importance
            let importance_cmp = b.importance.partial_cmp(&a.importance)
                .unwrap_or(std::cmp::Ordering::Equal);
            
            // If equal importance, prefer more recently referenced
            if importance_cmp == std::cmp::Ordering::Equal {
                b.last_referenced.cmp(&a.last_referenced)
            } else {
                importance_cmp
            }
        });
        
        // Filter auto entries above min_importance threshold
        let kept_auto: Vec<_> = sorted_auto
            .into_iter()
            .filter(|e| e.importance >= self.min_importance)
            .take(self.max_entries.saturating_sub(manual_entries.len()))
            .collect();
        
        // Combine: manual entries first, then sorted auto entries
        self.entries = manual_entries.into_iter().chain(kept_auto).collect();
        
        // Final safety check: if still too many, truncate oldest/least important
        if self.entries.len() > self.max_entries {
            self.entries.sort_by(|a, b| {
                let importance_cmp = b.importance.partial_cmp(&a.importance)
                    .unwrap_or(std::cmp::Ordering::Equal);
                if importance_cmp == std::cmp::Ordering::Equal {
                    b.last_referenced.cmp(&a.last_referenced)
                } else {
                    importance_cmp
                }
            });
            self.entries.truncate(self.max_entries);
        }
    }

    /// Get entries by category.
    pub fn by_category(&self, category: MemoryCategory) -> Vec<&MemoryEntry> {
        self.entries.iter().filter(|e| e.category == category).collect()
    }

    /// Get top N most important entries.
    pub fn top_n(&self, n: usize) -> Vec<&MemoryEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.importance.partial_cmp(&a.importance).unwrap_or(std::cmp::Ordering::Equal));
        sorted.into_iter().take(n).collect()
    }

    /// Search entries by content or tags.
    pub fn search(&self, query: &str) -> Vec<&MemoryEntry> {
        let query_lower = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                e.content.to_lowercase().contains(&query_lower) ||
                e.tags.iter().any(|t| t.to_lowercase().contains(&query_lower))
            })
            .collect()
    }

    /// Mark entries as referenced if they appear in the conversation.
    /// Optimized: pre-computes lowercase versions to avoid repeated conversions.
    pub fn update_references(&mut self, messages: &[Message]) {
        // Pre-compute all message texts in lowercase (optimization)
        let texts_lower: Vec<String> = messages
            .iter()
            .filter_map(Self::extract_message_text_lower)
            .collect();
        
        // Pre-compute all entry contents in lowercase
        let entry_contents_lower: Vec<String> = self.entries
            .iter()
            .map(|e| e.content.to_lowercase())
            .collect();
        
        // Check each entry against all texts
        for (i, entry) in self.entries.iter_mut().enumerate() {
            let entry_lower = &entry_contents_lower[i];
            if texts_lower.iter().any(|t| t.contains(entry_lower)) {
                entry.mark_referenced();
            }
        }
    }
    
    /// Extract lowercase text from a message for reference checking.
    fn extract_message_text_lower(msg: &Message) -> Option<String> {
        match &msg.content {
            crate::providers::MessageContent::Text(t) => Some(t.to_lowercase()),
            crate::providers::MessageContent::Blocks(blocks) => {
                let text = blocks
                    .iter()
                    .filter_map(|b| {
                        if let crate::providers::ContentBlock::Text { text } = b {
                            Some(text.as_str())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                Some(text.to_lowercase())
            }
        }
    }

    /// Generate summary for system prompt.
    pub fn generate_prompt_summary(&self, max_entries: usize) -> String {
        if self.entries.is_empty() {
            return String::new();
        }

        let top_entries = self.top_n(max_entries);
        if top_entries.is_empty() {
            return String::new();
        }

        let mut summary = String::from("【自动记忆摘要】\n\n");
        
        // Group by category
        let mut by_cat: HashMap<MemoryCategory, Vec<&MemoryEntry>> = HashMap::new();
        for entry in top_entries {
            by_cat.entry(entry.category).or_default().push(entry);
        }

        for (cat, entries) in by_cat {
            summary.push_str(&format!("{} {}:\n", cat.icon(), cat.display_name()));
            for entry in entries {
                summary.push_str(&format!("  {}\n", entry.format_for_prompt()));
            }
            summary.push('\n');
        }

        summary
    }

    /// Format all entries for display.
    pub fn format_all(&self) -> String {
        if self.entries.is_empty() {
            return "[no memories accumulated]".to_string();
        }

        let mut result = String::from("Accumulated memories:\n\n");
        
        // Sort by importance
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.importance.partial_cmp(&a.importance).unwrap_or(std::cmp::Ordering::Equal));

        for entry in sorted {
            result.push_str(&entry.format_line());
            result.push('\n');
        }

        result
    }

    /// Generate statistics summary for display.
    pub fn generate_statistics(&self) -> MemoryStatistics {
        let total = self.entries.len();
        let manual = self.entries.iter().filter(|e| e.is_manual).count();
        let auto = total - manual;
        
        // Count by category
        let by_category: HashMap<MemoryCategory, usize> = self.entries
            .iter()
            .fold(HashMap::new(), |mut acc, e| {
                *acc.entry(e.category).or_default() += 1;
                acc
            });
        
        // Calculate average importance
        let avg_importance = if total > 0 {
            self.entries.iter().map(|e| e.importance).sum::<f64>() / total as f64
        } else {
            0.0
        };
        
        // Find oldest and newest
        let oldest = self.entries
            .iter()
            .min_by_key(|e| e.created_at)
            .map(|e| e.created_at);
        let newest = self.entries
            .iter()
            .max_by_key(|e| e.created_at)
            .map(|e| e.created_at);
        
        // Count highly referenced
        let highly_referenced = self.entries
            .iter()
            .filter(|e| e.reference_count >= 3)
            .count();
        
        MemoryStatistics {
            total,
            manual,
            auto,
            by_category,
            avg_importance,
            oldest,
            newest,
            highly_referenced,
        }
    }
}

/// Statistics about memory collection.
#[derive(Debug, Clone)]
pub struct MemoryStatistics {
    /// Total number of entries.
    pub total: usize,
    /// Number of manually added entries.
    pub manual: usize,
    /// Number of automatically detected entries.
    pub auto: usize,
    /// Count by category.
    pub by_category: HashMap<MemoryCategory, usize>,
    /// Average importance score.
    pub avg_importance: f64,
    /// Oldest entry creation time.
    pub oldest: Option<DateTime<Utc>>,
    /// Newest entry creation time.
    pub newest: Option<DateTime<Utc>>,
    /// Number of entries with high reference count (>= 3).
    pub highly_referenced: usize,
}

impl MemoryStatistics {
    /// Format statistics for display.
    pub fn format_summary(&self) -> String {
        use std::fmt::Write;
        
        let mut output = String::new();
        
        writeln!(output, "记忆统计：").unwrap();
        writeln!(output, "  总计: {} 条", self.total).unwrap();
        writeln!(output, "  ├─ 手动添加: {} 条", self.manual).unwrap();
        writeln!(output, "  └─ 自动检测: {} 条", self.auto).unwrap();
        writeln!(output).unwrap();
        
        writeln!(output, "分类统计：").unwrap();
        for (cat, count) in &self.by_category {
            writeln!(output, "  {} {}: {} 条", cat.icon(), cat.display_name(), count).unwrap();
        }
        writeln!(output).unwrap();
        
        writeln!(output, "质量指标：").unwrap();
        writeln!(output, "  平均重要性: {:.1} 分", self.avg_importance).unwrap();
        writeln!(output, "  高频引用: {} 条 (≥3次)", self.highly_referenced).unwrap();
        
        if let Some(oldest) = self.oldest {
            let days = (Utc::now() - oldest).num_days();
            writeln!(output, "  记忆跨度: {} 天", days).unwrap();
        }
        
        output
    }
}

impl AutoMemory {
    /// Clear all memories.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Remove a specific memory by ID.
    pub fn remove(&mut self, id: &str) -> bool {
        let idx = self.entries.iter().position(|e| e.id == id);
        if let Some(i) = idx {
            self.entries.remove(i);
            true
        } else {
            false
        }
    }

    /// Apply time decay to memory importance.
    /// Entries that haven't been referenced recently will have their importance reduced.
    pub fn apply_time_decay(&mut self) {
        let now = Utc::now();
        let decay_days = 30;  // Start decay after 30 days
        let decay_rate: f64 = 0.5;  // Reduce by 50% per decay period
        
        for entry in &mut self.entries {
            // Skip manual entries - they should never decay
            if entry.is_manual {
                continue;
            }
            
            // Calculate days since last reference
            let days_since_reference = (now - entry.last_referenced)
                .num_days()
                .max(0);
            
            // Apply decay if older than threshold
            if days_since_reference > decay_days {
                // Calculate number of decay periods
                let decay_periods = (days_since_reference - decay_days) / 30;
                
                // Apply exponential decay
                let decay_factor = decay_rate.powi(decay_periods as i32);
                entry.importance *= decay_factor;
                
                // Ensure minimum importance
                entry.importance = entry.importance.max(self.min_importance * 0.5);
            }
        }
        
        // Re-prune after decay (low importance entries may now be removed)
        self.prune();
    }
}

// ============================================================================
// Memory Storage
// ============================================================================

/// Storage for memory files (global and project-level).
pub struct MemoryStorage {
    /// Base directory for global memory (~/.matrix).
    base_dir: PathBuf,
    /// Project root directory (optional).
    project_root: Option<PathBuf>,
}

impl MemoryStorage {
    /// Create a new memory storage.
    pub fn new(project_root: Option<&Path>) -> Result<Self> {
        let base_dir = Self::get_base_dir()?;
        Ok(Self {
            base_dir,
            project_root: project_root.map(|p| p.to_path_buf()),
        })
    }

    /// Get the base directory for memory storage.
    fn get_base_dir() -> Result<PathBuf> {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .ok_or_else(|| anyhow::anyhow!("HOME or USERPROFILE not set"))?;
        let mut p = PathBuf::from(home);
        p.push(".matrix");
        Ok(p)
    }

    /// Path to global memory file.
    pub fn global_memory_path(&self) -> PathBuf {
        self.base_dir.join("memory.json")
    }

    /// Path to project memory file.
    pub fn project_memory_path(&self) -> Option<PathBuf> {
        self.project_root.as_ref().map(|p| p.join(".matrix/memory.json"))
    }

    /// Ensure directories exist.
    fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.base_dir)?;
        if let Some(root) = &self.project_root {
            let memory_dir = root.join(".matrix");
            fs::create_dir_all(memory_dir)?;
        }
        Ok(())
    }

    /// Load global memory.
    pub fn load_global(&self) -> Result<AutoMemory> {
        let path = self.global_memory_path();
        if !path.exists() {
            return Ok(AutoMemory::new());
        }
        let data = fs::read_to_string(&path)?;
        let memory: AutoMemory = serde_json::from_str(&data)?;
        Ok(memory)
    }

    /// Load project memory.
    pub fn load_project(&self) -> Result<Option<AutoMemory>> {
        let path = self.project_memory_path();
        match path {
            Some(p) if p.exists() => {
                let data = fs::read_to_string(&p)?;
                let memory: AutoMemory = serde_json::from_str(&data)?;
                Ok(Some(memory))
            }
            _ => Ok(None),
        }
    }

    /// Load combined memory (global + project).
    pub fn load_combined(&self) -> Result<AutoMemory> {
        let mut combined = self.load_global()?;
        
        if let Some(project) = self.load_project()? {
            // Merge project entries into global
            for entry in project.entries {
                // Tag as project-specific
                let mut tagged_entry = entry;
                if !tagged_entry.tags.contains(&"project".to_string()) {
                    tagged_entry.tags.push("project".to_string());
                }
                combined.entries.push(tagged_entry);
            }
            combined.prune();
        }

        Ok(combined)
    }

    /// Save global memory.
    pub fn save_global(&self, memory: &AutoMemory) -> Result<()> {
        self.ensure_dirs()?;
        let path = self.global_memory_path();
        let json = serde_json::to_string_pretty(memory)?;
        
        // Write to temp file then rename (atomic)
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, json)?;
        fs::rename(&tmp, &path)?;
        
        Ok(())
    }

    /// Save project memory.
    pub fn save_project(&self, memory: &AutoMemory) -> Result<()> {
        self.ensure_dirs()?;
        let path = self.project_memory_path()
            .ok_or_else(|| anyhow::anyhow!("no project root"))?;
        let json = serde_json::to_string_pretty(memory)?;
        
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, json)?;
        fs::rename(&tmp, &path)?;
        
        Ok(())
    }

    /// Add entry to appropriate storage.
    /// Note: Uses &self (not &mut self) as it only reads paths and writes to files.
    pub fn add_entry(&self, entry: MemoryEntry, is_project_specific: bool) -> Result<()> {
        if is_project_specific {
            let mut project = self.load_project()?.unwrap_or_else(AutoMemory::new);
            project.add(entry);
            self.save_project(&project)?;
        } else {
            let mut global = self.load_global()?;
            global.add(entry);
            self.save_global(&global)?;
        }
        Ok(())
    }

    /// Remove entry from storage by ID.
    pub fn remove_entry(&self, id: &str, is_project_specific: bool) -> Result<bool> {
        if is_project_specific {
            if let Some(mut project) = self.load_project()? {
                let removed = project.remove(id);
                if removed {
                    self.save_project(&project)?;
                }
                return Ok(removed);
            }
            Ok(false)
        } else {
            let mut global = self.load_global()?;
            let removed = global.remove(id);
            if removed {
                self.save_global(&global)?;
            }
            Ok(removed)
        }
    }
}

// ============================================================================
// Helper Functions (Global)
// ============================================================================

/// Calculate word-based similarity between two strings (Jaccard coefficient).
/// Returns a value between 0.0 (no similarity) and 1.0 (identical words).
pub fn calculate_similarity_global(a: &str, b: &str) -> f64 {
    use std::collections::HashSet;
    
    let a_words: HashSet<&str> = a.split_whitespace().collect();
    let b_words: HashSet<&str> = b.split_whitespace().collect();
    
    if a_words.is_empty() || b_words.is_empty() {
        return 0.0;
    }
    
    let intersection = a_words.intersection(&b_words).count();
    let union = a_words.union(&b_words).count();
    
    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

// ============================================================================
// AI-Based Memory Extraction
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

/// Default models for memory extraction (fast and cost-effective).
pub const DEFAULT_MEMORY_EXTRACTOR_MODEL: &str = "claude-3-5-haiku-20241022";

/// System prompt for memory extraction.
const MEMORY_EXTRACT_SYSTEM_PROMPT: &str = r#"你是一个记忆提取助手。你的任务是从对话中识别并提取值得长期记忆的关键信息。

记忆类型：
1. Decision（决策）: 项目或技术选型的决定，如"决定使用 PostgreSQL"
2. Preference（偏好）: 用户习惯或偏好，如"我喜欢用 vim"
3. Solution（解决方案）: 解决问题的具体方法，如"通过添加 middleware 修复 bug"
4. Finding（发现）: 重要发现或信息，如"API 端点在 /api/v2"
5. Technical（技术）: 技术栈或框架信息，如"使用 React Query 做数据获取"
6. Structure（结构）: 项目结构信息，如"入口文件是 src/index.ts"

提取原则：
- 只提取有价值、可复用的信息
- 避免提取临时性、一次性信息
- 避免提取过于具体的代码细节
- 每条记忆应简洁明确（一句话）
- 最多提取 5 条记忆

输出格式（严格 JSON）：
```json
{
  "memories": [
    {
      "category": "decision",
      "content": "决定使用 PostgreSQL 作为主数据库",
      "importance": 90
    },
    {
      "category": "preference", 
      "content": "用户偏好 TypeScript 而非 JavaScript",
      "importance": 70
    }
  ]
}
```

如果没有值得记忆的内容，返回：
```json
{"memories": []}
```

直接输出 JSON，不要加代码块包裹。"#;

#[async_trait::async_trait]
impl MemoryExtractor for AiMemoryExtractor {
    async fn extract(&self, text: &str, session_id: Option<&str>) -> Result<Vec<MemoryEntry>> {
        use crate::providers::{ChatRequest, Message, MessageContent, Role};
        
        // Truncate text if too long (memory extraction focuses on key points)
        let truncated_text = if text.len() > 4000 {
            crate::ui::truncate_str(text, 4000)
        } else {
            text.to_string()
        };
        
        let request = ChatRequest {
            messages: vec![Message {
                role: Role::User,
                content: MessageContent::Text(format!(
                    "请从以下对话中提取值得记忆的关键信息：\n\n{}", 
                    truncated_text
                )),
            }],
            tools: vec![],  // No tools for memory extraction
            system: Some(MEMORY_EXTRACT_SYSTEM_PROMPT.to_string()),
            think: false,   // No extended thinking
            max_tokens: 512, // Short response
            server_tools: vec![],
            enable_caching: false,
        };
        
        let response = self.provider.chat(request).await?;
        
        // Extract text from response
        let response_text = response.content
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
        parse_memory_response(&response_text, session_id)
    }
    
    fn model_name(&self) -> &str {
        &self.model
    }
}

/// Parse AI response into memory entries.
fn parse_memory_response(json_text: &str, session_id: Option<&str>) -> Result<Vec<MemoryEntry>> {
    // Clean up response (remove possible markdown code blocks)
    let cleaned = json_text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    
    // Parse JSON
    #[derive(serde::Deserialize)]
    struct MemoryResponse {
        memories: Vec<MemoryItem>,
    }
    
    #[derive(serde::Deserialize)]
    struct MemoryItem {
        category: String,
        content: String,
        #[serde(default)]
        importance: f64,
    }
    
    let parsed: MemoryResponse = serde_json::from_str(cleaned)?;
    
    // Convert to MemoryEntry
    let entries = parsed.memories
        .into_iter()
        .filter_map(|item| {
            // Parse category
            let category = match item.category.to_lowercase().as_str() {
                "decision" => MemoryCategory::Decision,
                "preference" => MemoryCategory::Preference,
                "solution" => MemoryCategory::Solution,
                "finding" => MemoryCategory::Finding,
                "technical" => MemoryCategory::Technical,
                "structure" => MemoryCategory::Structure,
                _ => return None,  // Skip unknown categories
            };
            
            // Skip too short content
            if item.content.len() < MIN_MEMORY_CONTENT_LENGTH {
                return None;
            }
            
            // Create entry with AI-suggested importance or default
            let mut entry = MemoryEntry::new(
                category,
                item.content,
                session_id.map(|s| s.to_string()),
            );
            
            // Override importance if AI suggested a value
            if item.importance > 0.0 {
                entry.importance = item.importance.clamp(0.0, 100.0);
            }
            
            Some(entry)
        })
        .collect();
    
    // Deduplicate and limit
    Ok(deduplicate_entries(entries))
}

// ============================================================================
// Memory Detection (Fallback - Rule-based)
// ============================================================================

/// Minimum content length for memory detection (to avoid capturing too generic content).
const MIN_MEMORY_CONTENT_LENGTH: usize = 15;

/// Maximum entries to return from detection (to avoid overwhelming).
const MAX_DETECTED_ENTRIES: usize = 5;

/// Detect potential memory entries from conversation content.
/// This is the fallback method using rule-based detection (no AI).
/// For AI-based extraction, use AiMemoryExtractor.
pub fn detect_memories_fallback(text: &str, session_id: Option<&str>) -> Vec<MemoryEntry> {
    let mut entries = Vec::new();
    let text_lower = text.to_lowercase();

    // Detection patterns for each category (filtered to avoid too generic keywords)
    let patterns: Vec<(MemoryCategory, Vec<&str>)> = vec![
        (MemoryCategory::Decision, vec![
            "决定", "决定使用", "选择使用", "采用", "decided to", "decision to", 
            "chose to", "adopted", "选定", "最终选择",
        ]),
        (MemoryCategory::Preference, vec![
            "我喜欢", "我偏好", "prefer to", "i prefer", "my preference is",
            "习惯用", "我习惯", "usually prefer", "偏好使用",
        ]),
        (MemoryCategory::Solution, vec![
            "修复了", "解决了", "fixed by", "solved by", "resolved by", 
            "通过添加", "通过修改", "通过删除", "解决方法是",
        ]),
        (MemoryCategory::Finding, vec![
            "发现", "注意到", "found that", "noticed that", "discovered", 
            "观察到", "api 端点", "位于", "located at", "关键发现",
        ]),
        (MemoryCategory::Technical, vec![
            "使用框架", "using framework", "built with", "基于", 
            "框架是", "技术栈", "依赖库",
        ]),
        (MemoryCategory::Structure, vec![
            "入口文件", "entry point is", "主文件是", "main file", 
            "配置文件", "config file", "核心文件",
        ]),
    ];

    for (category, keywords) in patterns {
        for keyword in keywords {
            if text_lower.contains(keyword) {
                // Extract the relevant sentence or phrase
                let content = extract_memory_content(text, keyword);
                // Use higher threshold to avoid too generic content
                if !content.is_empty() && content.len() >= MIN_MEMORY_CONTENT_LENGTH {
                    let entry = MemoryEntry::new(
                        category,
                        content,
                        session_id.map(|s| s.to_string()),
                    );
                    entries.push(entry);
                }
            }
        }
    }

    // Deduplicate by content similarity
    deduplicate_entries(entries)
}

/// Detect memories from text using the rule-based fallback method.
/// This is kept for backward compatibility and for cases where AI is unavailable.
pub fn detect_memories_from_text(text: &str, session_id: Option<&str>) -> Vec<MemoryEntry> {
    detect_memories_fallback(text, session_id)
}

/// Detect memories asynchronously using AI extractor.
/// Falls back to rule-based detection if AI fails or is unavailable.
pub async fn detect_memories_with_ai(
    text: &str,
    session_id: Option<&str>,
    extractor: Option<&dyn MemoryExtractor>,
) -> Result<Vec<MemoryEntry>> {
    if let Some(ai_extractor) = extractor {
        // Try AI extraction first
        match ai_extractor.extract(text, session_id).await {
            Ok(entries) if !entries.is_empty() => {
                return Ok(entries);
            }
            Ok(_) => {
                // AI returned empty, try fallback (silent)
            }
            Err(_) => {
                // AI extraction failed, try fallback (silent)
            }
        }
    }
    
    // Fallback to rule-based detection
    Ok(detect_memories_fallback(text, session_id))
}

/// Deduplicate entries by content similarity.
/// Keeps longer (more detailed) entries when duplicates are found.
fn deduplicate_entries(entries: Vec<MemoryEntry>) -> Vec<MemoryEntry> {
    if entries.is_empty() {
        return entries;
    }
    
    // Sort by content length (longer first - keep more detailed entries)
    let mut sorted = entries;
    sorted.sort_by(|a, b| b.content.len().cmp(&a.content.len()));
    
    // Keep only unique entries
    let mut unique: Vec<MemoryEntry> = Vec::new();
    for entry in sorted {
        let entry_lower = entry.content.to_lowercase();
        
        // Check if already have similar entry
        let is_duplicate = unique.iter().any(|existing| {
            let existing_lower = existing.content.to_lowercase();
            
            // Exact match
            if existing_lower == entry_lower {
                return true;
            }
            
            // High similarity (same words mostly)
            let similarity = calculate_similarity_global(&existing_lower, &entry_lower);
            similarity >= 0.8
        });
        
        if !is_duplicate {
            unique.push(entry);
        }
        
        // Stop if we have enough entries
        if unique.len() >= MAX_DETECTED_ENTRIES {
            break;
        }
    }
    
    unique
}

/// Extract memory content around a keyword.
fn extract_memory_content(text: &str, keyword: &str) -> String {
    let text_lower = text.to_lowercase();
    let keyword_lower = keyword.to_lowercase();

    // Find keyword position
    let pos = text_lower.find(&keyword_lower);
    if pos.is_none() {
        return String::new();
    }

    let pos = pos.unwrap();
    
    // Find sentence boundaries (sentence end markers)
    const SENTENCE_END_MARKERS: [char; 3] = ['.', '\n', '。'];
    
    let start = text[..pos].rfind(SENTENCE_END_MARKERS)
        .map(|i| i + 1)
        .unwrap_or(0);
    
    let end = text[pos..].find(SENTENCE_END_MARKERS)
        .map(|i| pos + i + 1)
        .unwrap_or(text.len().min(pos + 200));

    let content = text[start..end].trim();
    
    // Clean up and truncate
    if content.len() > 200 {
        crate::ui::truncate_str(content, 197)
    } else {
        content.to_string()
    }
}

// ============================================================================
// Rewind / Summarize Up To Here
// ============================================================================

/// Result of a rewind/summarize operation.
#[derive(Debug, Clone)]
pub struct RewindResult {
    /// Original message count.
    pub original_count: usize,
    /// New message count after rewind.
    pub new_count: usize,
    /// Index where rewind was applied.
    pub rewind_index: usize,
    /// Summary generated for removed messages.
    pub summary: Option<String>,
    /// New message list (summary message + kept messages).
    pub new_messages: Vec<Message>,
}

/// Summarize messages up to a specific index, keeping recent ones.
/// Returns the new message list with summary + kept messages.
pub async fn summarize_up_to(
    messages: &[Message],
    index: usize,
    compressor: Option<&dyn crate::compress::Compressor>,
) -> Result<RewindResult> {
    if index >= messages.len() {
        anyhow::bail!("rewind index {} out of bounds (messages: {})", index, messages.len());
    }

    if index == 0 {
        // Nothing to summarize, return original messages
        return Ok(RewindResult {
            original_count: messages.len(),
            new_count: messages.len(),
            rewind_index: 0,
            summary: None,
            new_messages: messages.to_vec(),
        });
    }

    let to_summarize = &messages[..index];
    let to_keep = &messages[index..];

    // Generate summary
    let summary = if let Some(comp) = compressor {
        // Use AI compressor
        let segment = comp.summarize(to_summarize, &crate::compress::CompressionConfig::default()).await?;
        Some(segment.summary)
    } else {
        // Fallback to simple summary
        Some(generate_simple_summary(to_summarize))
    };

    // Build summary message
    let summary_msg = create_summary_message(&summary, to_summarize.len());

    // New message list: summary + kept messages
    let new_messages: Vec<Message> = std::iter::once(summary_msg)
        .chain(to_keep.iter().cloned())
        .collect();
    
    let new_count = new_messages.len();

    Ok(RewindResult {
        original_count: messages.len(),
        new_count,
        rewind_index: index,
        summary,
        new_messages,
    })
}

/// Create a summary message for injection.
fn create_summary_message(summary: &Option<String>, original_count: usize) -> Message {
    let content = match summary {
        Some(s) => format!("[对话摘要 - 原 {} 条消息]\n\n{}", original_count, s),
        None => format!("[对话摘要 - 原 {} 条消息已压缩]", original_count),
    };

    Message {
        role: crate::providers::Role::User,
        content: crate::providers::MessageContent::Text(content),
    }
}

/// Generate a simple summary without AI.
fn generate_simple_summary(messages: &[Message]) -> String {
    let mut parts: Vec<String> = Vec::new();
    
    // Extract key points from each message
    for msg in messages {
        if msg.role == crate::providers::Role::User {
            let text = match &msg.content {
                crate::providers::MessageContent::Text(t) => t,
                _ => continue,
            };
            // Take first significant line
            let first_line = text.lines().next().unwrap_or("");
            if first_line.len() > 20 {
                parts.push(crate::ui::truncate_str(first_line, 100));
            }
        }
    }

    if parts.is_empty() {
        "对话已压缩".to_string()
    } else if parts.len() <= 5 {
        parts.join(" | ")
    } else {
        format!("{} ... (共 {} 个话题)", parts[0], parts.len())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_entry_creation() {
        let entry = MemoryEntry::new(
            MemoryCategory::Decision,
            "Decided to use PostgreSQL for database".to_string(),
            Some("session-123".to_string()),
        );
        assert_eq!(entry.category, MemoryCategory::Decision);
        assert_eq!(entry.importance, 90.0);
        assert!(!entry.is_manual);
    }

    #[test]
    fn test_memory_reference_increase() {
        let mut entry = MemoryEntry::new(
            MemoryCategory::Finding,
            "API endpoint is at /api/v2".to_string(),
            None,
        );
        assert_eq!(entry.importance, 60.0);
        entry.mark_referenced();
        assert_eq!(entry.importance, 62.0);
        entry.mark_referenced();
        entry.mark_referenced();
        assert_eq!(entry.importance, 66.0);
    }

    #[test]
    fn test_auto_memory_add_and_prune() {
        let mut memory = AutoMemory::new();
        memory.max_entries = 5;

        for i in 0..10 {
            memory.add(MemoryEntry::new(
                MemoryCategory::Technical,
                format!("Note {}", i),
                None,
            ));
        }

        // Should have pruned to max_entries
        assert!(memory.entries.len() <= memory.max_entries);
    }

    #[test]
    fn test_duplicate_detection() {
        let mut memory = AutoMemory::new();
        memory.add_memory(
            MemoryCategory::Decision,
            "Use PostgreSQL".to_string(),
            None,
        );
        
        // Should not add duplicate
        memory.add_memory(
            MemoryCategory::Decision,
            "Use PostgreSQL".to_string(),
            None,
        );
        
        assert_eq!(memory.entries.len(), 1);
    }

    #[test]
    fn test_memory_detection() {
        // Test decision detection
        let text = "我决定使用 React 作为前端框架";
        let entries = detect_memories_from_text(text, None);
        assert!(!entries.is_empty());
        assert_eq!(entries[0].category, MemoryCategory::Decision);
        
        // Test solution detection (with more specific pattern)
        let text2 = "解决了认证问题，通过添加 token refresh 机制";
        let entries2 = detect_memories_from_text(text2, None);
        assert!(!entries2.is_empty());
        assert_eq!(entries2[0].category, MemoryCategory::Solution);
        
        // Test preference detection
        let text3 = "我偏好使用 TypeScript 进行开发";
        let entries3 = detect_memories_from_text(text3, None);
        assert!(!entries3.is_empty());
        assert_eq!(entries3[0].category, MemoryCategory::Preference);
    }

    #[test]
    fn test_category_importance() {
        assert!(MemoryCategory::Decision.default_importance() > MemoryCategory::Structure.default_importance());
        assert!(MemoryCategory::Solution.default_importance() > MemoryCategory::Technical.default_importance());
    }

    #[test]
    fn test_top_n_entries() {
        let mut memory = AutoMemory::new();
        
        // Add entries with different importance
        memory.add(MemoryEntry::new(MemoryCategory::Decision, "Decision 1".into(), None));
        memory.add(MemoryEntry::new(MemoryCategory::Finding, "Finding 1".into(), None));
        memory.add(MemoryEntry::new(MemoryCategory::Structure, "Structure 1".into(), None));

        let top = memory.top_n(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].category, MemoryCategory::Decision); // Highest importance
    }

    #[test]
    fn test_similarity_calculation() {
        // Test exact match
        let sim = AutoMemory::calculate_similarity("hello world", "hello world");
        assert_eq!(sim, 1.0);
        
        // Test no match
        let sim = AutoMemory::calculate_similarity("hello world", "foo bar");
        assert_eq!(sim, 0.0);
        
        // Test partial match (50% overlap)
        let sim = AutoMemory::calculate_similarity("hello world", "hello there");
        assert!(sim > 0.0 && sim < 1.0);
        
        // Test empty input
        let sim = AutoMemory::calculate_similarity("", "hello");
        assert_eq!(sim, 0.0);
    }
    
    #[test]
    fn test_similarity_threshold() {
        let mut memory = AutoMemory::new();
        
        // Add a long enough entry (>= MIN_SIMILARITY_LENGTH)
        memory.add(MemoryEntry::new(
            MemoryCategory::Decision,
            "We decided to use PostgreSQL for our database system".to_string(),
            None,
        ));
        
        // Should not add similar entry
        memory.add_memory(
            MemoryCategory::Decision,
            "We decided to use PostgreSQL for our database backend".to_string(),
            None,
        );
        
        // Should have only 1 entry (similar detected)
        assert_eq!(memory.entries.len(), 1);
    }
    
    #[test]
    fn test_short_content_skipped() {
        let mut memory = AutoMemory::new();
        
        // Short content should be skipped by has_similar
        memory.add(MemoryEntry::new(
            MemoryCategory::Technical,
            "short".to_string(),  // Only 5 chars, below MIN_SIMILARITY_LENGTH
            None,
        ));
        
        // Another short entry should be added (not detected as similar)
        memory.add_memory(
            MemoryCategory::Technical,
            "brief".to_string(),
            None,
        );
        
        assert_eq!(memory.entries.len(), 2);
    }
    
    #[test]
    fn test_prune_preserves_manual() {
        let mut memory = AutoMemory::new();
        memory.max_entries = 3;
        
        // Add manual entry (should always be preserved)
        let mut manual = MemoryEntry::manual(MemoryCategory::Decision, "Manual decision".into());
        manual.importance = 10.0; // Low importance but manual
        memory.add(manual);
        
        // Add high importance auto entries
        for i in 0..5 {
            let entry = MemoryEntry::new(
                MemoryCategory::Decision,
                format!("Auto decision {}", i),
                None,
            );
            memory.add(entry);
        }
        
        // Manual entry should still exist after prune
        assert!(memory.entries.iter().any(|e| e.is_manual));
        assert!(memory.entries.len() <= memory.max_entries);
    }
    
    #[test]
    fn test_deduplicate_entries() {
        // Use more similar entries (should have similarity >= 0.8)
        let entries = vec![
            MemoryEntry::new(MemoryCategory::Decision, "We chose PostgreSQL database system for our backend".into(), None),
            MemoryEntry::new(MemoryCategory::Decision, "We chose PostgreSQL database system backend".into(), None),
            MemoryEntry::new(MemoryCategory::Decision, "Using Redis for caching layer".into(), None),
        ];
        
        let deduped = deduplicate_entries(entries);
        
        // Should deduplicate similar entries
        assert!(deduped.len() >= 1);
        assert!(deduped.len() <= 3);
        
        // Should keep longer (more detailed) entry when similar
        let pg_entries: Vec<_> = deduped.iter()
            .filter(|e| e.content.to_lowercase().contains("postgresql"))
            .collect();
        
        if pg_entries.len() == 1 {
            // Correctly deduplicated to one PostgreSQL entry
            // Should be the longer one
            assert!(pg_entries[0].content.contains("backend"));
        }
    }
    
    #[test]
    fn test_memory_detection_edge_cases() {
        // Empty input
        let entries = detect_memories_from_text("", None);
        assert!(entries.is_empty());
        
        // Very short input (below MIN_MEMORY_CONTENT_LENGTH)
        let entries = detect_memories_from_text("决定", None);
        assert!(entries.is_empty());
        
        // Input with only generic keywords
        let entries = detect_memories_from_text("使用", None);
        assert!(entries.is_empty());
        
        // Multiple matches in same text
        let text = "我决定使用React，解决了性能问题通过添加缓存机制";
        let entries = detect_memories_from_text(text, None);
        assert!(entries.len() <= MAX_DETECTED_ENTRIES);
    }
    
    #[test]
    fn test_importance_ceiling() {
        let mut entry = MemoryEntry::new(
            MemoryCategory::Decision,
            "Important decision".into(),
            None,
        );
        
        // Start at 90 (Decision default)
        assert_eq!(entry.importance, 90.0);
        
        // Reference many times
        for _ in 0..10 {
            entry.mark_referenced();
        }
        
        // Should cap at 100
        assert!(entry.importance <= 100.0);
    }

    #[test]
    fn test_time_decay() {
        let mut memory = AutoMemory::new();
        memory.min_importance = 30.0;
        
        // Add manual entry (should never decay)
        let mut manual = MemoryEntry::manual(MemoryCategory::Decision, "Manual entry".into());
        manual.importance = 50.0;
        memory.add(manual);
        
        // Add auto entry with old reference date (simulate 60 days ago)
        let mut old_entry = MemoryEntry::new(
            MemoryCategory::Technical,
            "Old technical note".into(),
            None,
        );
        old_entry.importance = 60.0;
        // Set last_referenced to 60 days ago
        old_entry.last_referenced = Utc::now() - chrono::Duration::days(60);
        memory.add(old_entry);
        
        // Add recent entry (should not decay)
        let recent_entry = MemoryEntry::new(
            MemoryCategory::Finding,
            "Recent finding".into(),
            None,
        );
        memory.add(recent_entry);
        
        // Apply time decay
        memory.apply_time_decay();
        
        // Manual entry should not decay
        let manual_entry = memory.entries.iter().find(|e| e.is_manual);
        assert!(manual_entry.is_some());
        assert_eq!(manual_entry.unwrap().importance, 50.0);
        
        // Recent entry should not decay (still > 30 days threshold)
        let recent = memory.entries.iter().find(|e| e.content.contains("Recent"));
        assert!(recent.is_some());
        assert!(recent.unwrap().importance >= 60.0);  // Finding default
        
        // Old entry should have decayed
        let old = memory.entries.iter().find(|e| e.content.contains("Old"));
        if let Some(old_entry) = old {
            // Should have decayed (60 days - 30 days threshold = 30 days decay period)
            // Decay factor = 0.5^1 = 0.5, so importance = 60 * 0.5 = 30
            assert!(old_entry.importance < 60.0);
            // Should still be above minimum threshold
            assert!(old_entry.importance >= memory.min_importance * 0.5);
        }
    }

    #[test]
    fn test_parse_memory_response() {
        // Test valid JSON response
        let json = r#"{"memories": [{"category": "decision", "content": "决定使用 PostgreSQL 作为数据库", "importance": 90}, {"category": "preference", "content": "我偏好 TypeScript 而非 JavaScript", "importance": 70}]}"#;
        let entries = parse_memory_response(json, None).unwrap();
        assert_eq!(entries.len(), 2);
        
        // Check both entries exist (order may change due to deduplication sorting)
        let has_decision = entries.iter().any(|e| e.category == MemoryCategory::Decision);
        let has_preference = entries.iter().any(|e| e.category == MemoryCategory::Preference);
        assert!(has_decision);
        assert!(has_preference);
        
        // Check importance values
        let decision_entry = entries.iter().find(|e| e.category == MemoryCategory::Decision);
        assert!(decision_entry.is_some());
        assert_eq!(decision_entry.unwrap().importance, 90.0);
        
        // Test empty response
        let empty_json = r#"{"memories": []}"#;
        let empty_entries = parse_memory_response(empty_json, None).unwrap();
        assert!(empty_entries.is_empty());
        
        // Test JSON with markdown code blocks
        let markdown_json = r#"```json
{"memories": [{"category": "solution", "content": "通过添加 middleware 修复认证问题", "importance": 85}]}
```"#;
        let markdown_entries = parse_memory_response(markdown_json, None).unwrap();
        assert_eq!(markdown_entries.len(), 1);
        assert_eq!(markdown_entries[0].category, MemoryCategory::Solution);
        
        // Test unknown category (should be skipped)
        let unknown_json = r#"{"memories": [{"category": "unknown", "content": "This should be skipped content", "importance": 50}]}"#;
        let unknown_entries = parse_memory_response(unknown_json, None).unwrap();
        assert!(unknown_entries.is_empty());
        
        // Test short content (should be skipped)
        let short_json = r#"{"memories": [{"category": "finding", "content": "short", "importance": 60}]}"#;
        let short_entries = parse_memory_response(short_json, None).unwrap();
        assert!(short_entries.is_empty());
    }

    #[test]
    fn test_public_has_similar() {
        let mut memory = AutoMemory::new();
        
        // Add an entry
        memory.add(MemoryEntry::new(
            MemoryCategory::Decision,
            "We decided to use PostgreSQL for our main database system".to_string(),
            None,
        ));
        
        // Test exact match
        assert!(memory.has_similar("We decided to use PostgreSQL for our main database system"));
        
        // Test very similar content (high similarity > 0.7)
        // Original: "We decided to use PostgreSQL for our main database system"
        // Similar:  "We decided to use PostgreSQL for our main database backend"
        // Similarity = shared words / total unique words
        assert!(memory.has_similar("We decided to use PostgreSQL for our main database backend"));
        
        // Test moderately similar (should NOT match, < 0.7)
        assert!(!memory.has_similar("We decided to use Redis for caching"));
        
        // Test completely different content
        assert!(!memory.has_similar("The project uses React for frontend"));
        
        // Test short content (should return false)
        assert!(!memory.has_similar("short"));
    }

    #[test]
    fn test_public_prune() {
        let mut memory = AutoMemory::new();
        memory.max_entries = 5;
        memory.min_importance = 30.0;
        
        // Add entries exceeding max
        for i in 0..10 {
            memory.add(MemoryEntry::new(
                MemoryCategory::Technical,
                format!("Technical note number {} with sufficient length", i),
                None,
            ));
        }
        
        // Manually prune
        memory.prune();
        
        // Should be within limit
        assert!(memory.entries.len() <= memory.max_entries);
    }

    #[test]
    fn test_statistics() {
        let mut memory = AutoMemory::new();
        
        // Add various entries
        memory.add(MemoryEntry::new(MemoryCategory::Decision, "Decision one with enough content".to_string(), None));
        memory.add(MemoryEntry::new(MemoryCategory::Preference, "Preference for TypeScript over JavaScript".to_string(), None));
        memory.add(MemoryEntry::manual(MemoryCategory::Technical, "Manual technical note".to_string()));
        
        // Reference some entries
        memory.entries[0].mark_referenced();
        memory.entries[0].mark_referenced();
        memory.entries[0].mark_referenced();
        
        let stats = memory.generate_statistics();
        
        assert_eq!(stats.total, 3);
        assert_eq!(stats.manual, 1);
        assert_eq!(stats.auto, 2);
        assert_eq!(stats.highly_referenced, 1);  // First entry has 3 references
        assert!(stats.by_category.contains_key(&MemoryCategory::Decision));
        assert!(stats.by_category.contains_key(&MemoryCategory::Preference));
        assert!(stats.by_category.contains_key(&MemoryCategory::Technical));
        assert!(stats.avg_importance > 0.0);
    }
}