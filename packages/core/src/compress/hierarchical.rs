//! Hierarchical summarization strategies for intelligent compression.
//!
//! This module provides multi-level summarization strategies that adapt
//! to message priority and context complexity.
//!
//! # 策略级别
//! - **Brief**: 极简摘要（20-30%保留），适合低优先级消息
//! - **Standard**: 标准摘要（40-50%保留），适合中等优先级
//! - **Detailed**: 详细摘要（60-70%保留），适合高优先级消息

use crate::providers::{Message, MessageContent, Role};
use crate::compress::priority::PriorityScore;
use crate::compress::hardcode_config::HardcodeConfig;
use std::collections::HashSet;

/// Summarization level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SummaryLevel {
    /// Brief summary: 20-30% retention
    /// 最小化保留，只提取核心意图
    Brief,
    
    /// Standard summary: 40-50% retention
    /// 平衡保留，包含关键细节
    Standard,
    
    /// Detailed summary: 60-70% retention
    /// 最大化保留，保持上下文连贯性
    Detailed,
}

impl SummaryLevel {
    /// Determine summary level based on priority score
    pub fn from_priority(priority: PriorityScore) -> Self {
        if priority.is_high() {
            SummaryLevel::Detailed
        } else if priority.is_medium() {
            SummaryLevel::Standard
        } else {
            SummaryLevel::Brief
        }
    }
    
    /// Get target retention ratio
    pub fn retention_ratio(&self) -> f32 {
        match self {
            SummaryLevel::Brief => 0.25,      // 25%
            SummaryLevel::Standard => 0.45,   // 45%
            SummaryLevel::Detailed => 0.65,   // 65%
        }
    }
    
    /// Get maximum tokens for this level
    pub fn max_tokens(&self) -> usize {
        match self {
            SummaryLevel::Brief => 100,
            SummaryLevel::Standard => 200,
            SummaryLevel::Detailed => 350,
        }
    }
}

/// Hierarchical summarizer configuration
#[derive(Debug, Clone)]
pub struct HierarchicalConfig {
    /// Enable progressive compression
    pub progressive: bool,
    /// Minimum messages to trigger summarization
    pub min_messages: usize,
    /// Maximum messages before forced summarization
    pub max_messages: usize,
}

impl Default for HierarchicalConfig {
    fn default() -> Self {
        Self {
            progressive: true,
            min_messages: 10,
            max_messages: 50,
        }
    }
}

/// Hierarchical summarizer
pub struct HierarchicalSummarizer {
    config: HierarchicalConfig,
    hardcode_config: HardcodeConfig,
}

impl Default for HierarchicalSummarizer {
    fn default() -> Self {
        Self::new(HierarchicalConfig::default())
    }
}

impl HierarchicalSummarizer {
    pub fn new(config: HierarchicalConfig) -> Self {
        Self {
            config,
            hardcode_config: HardcodeConfig::default(),
        }
    }
    
    /// Create with custom hardcode config
    pub fn with_hardcode_config(mut self, hardcode_config: HardcodeConfig) -> Self {
        self.hardcode_config = hardcode_config;
        self
    }
    
    /// Summarize a single message based on its priority
    pub fn summarize_message(&self, message: &Message, level: SummaryLevel) -> String {
        let content = match &message.content {
            MessageContent::Text(text) => text.clone(),
            MessageContent::Blocks(blocks) => {
                blocks.iter()
                    .filter_map(|b| match b {
                        crate::providers::ContentBlock::Text { text } => Some(text.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        };
        
        if content.is_empty() {
            return String::new();
        }
        
        // Apply summarization based on level
        match level {
            SummaryLevel::Brief => self.brief_summary(&content, &message.role),
            SummaryLevel::Standard => self.standard_summary(&content, &message.role),
            SummaryLevel::Detailed => self.detailed_summary(&content, &message.role),
        }
    }
    
    /// Brief summary: Extract core intent only
    fn brief_summary(&self, content: &str, role: &Role) -> String {
        let sentences: Vec<&str> = content
            .split(|c| c == '。' || c == '.' || c == '\n')
            .filter(|s| !s.trim().is_empty())
            .collect();
        
        if sentences.is_empty() {
            return truncate_to_chars(content, 50);
        }
        
        // Extract first sentence + key entities
        let first_sentence = sentences[0].trim();
        
        // Find key verbs/actions
        let key_actions = extract_key_actions(content);
        
        if key_actions.is_empty() {
            format!("[{}] {}", role_label(role), truncate_to_chars(first_sentence, 40))
        } else {
            format!("[{}] {} | {}", role_label(role), truncate_to_chars(first_sentence, 30), key_actions.join(", "))
        }
    }
    
    /// Standard summary: Keep key details
    fn standard_summary(&self, content: &str, role: &Role) -> String {
        let sentences: Vec<&str> = content
            .split(|c| c == '。' || c == '.' || c == '\n')
            .filter(|s| !s.trim().is_empty())
            .collect();
        
        if sentences.is_empty() {
            return truncate_to_chars(content, 100);
        }
        
        // Keep first 2-3 sentences + key information
        let mut summary_parts = Vec::new();
        
        // First sentence
        if let Some(first) = sentences.first() {
            summary_parts.push(first.trim().to_string());
        }
        
        // Middle key sentence (if exists)
        if sentences.len() > self.hardcode_config.brief_summary_sentence_count {
            if let Some(key_sentence) = find_key_sentence(&sentences[1..sentences.len()-1], &self.hardcode_config) {
                summary_parts.push(key_sentence);
            }
        }
        
        // Last sentence (conclusion)
        if sentences.len() > self.hardcode_config.min_messages_for_compression {
            if let Some(last) = sentences.last() {
                summary_parts.push(last.trim().to_string());
            }
        }
        
        // Extract key entities
        let entities = extract_entities(content, &self.hardcode_config);
        if !entities.is_empty() {
            summary_parts.push(format!("[{}]", entities.join(", ")));
        }
        
        format!("[{}] {}", role_label(role), summary_parts.join(" | "))
    }
    
    /// Detailed summary: Preserve context
    fn detailed_summary(&self, content: &str, role: &Role) -> String {
        let sentences: Vec<&str> = content
            .split(|c| c == '。' || c == '.' || c == '\n')
            .filter(|s| !s.trim().is_empty())
            .collect();
        
        if sentences.is_empty() {
            return truncate_to_chars(content, 200);
        }
        
        // Keep most sentences but compress them
        let compressed_sentences: Vec<String> = sentences
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if i == 0 || i == sentences.len() - 1 {
                    // Keep first and last intact
                    s.trim().to_string()
                } else {
                    // Compress middle sentences
                    compress_sentence(s.trim(), &self.hardcode_config)
                }
            })
            .collect();
        
        // Add role marker and structure
        let mut result = format!("[{}] ", role_label(role));
        result.push_str(&compressed_sentences.join(" → "));
        
        // Preserve code blocks if present
        if content.contains("```") {
            let code_blocks = extract_code_blocks(content);
            if !code_blocks.is_empty() {
                result.push_str("\n[代码: ");
                result.push_str(&code_blocks.len().to_string());
                result.push_str(" 个代码块]");
            }
        }
        
        result
    }
    
    /// Determine optimal summarization level for a batch of messages
    pub fn determine_batch_level(&self, messages: &[Message], priorities: &[PriorityScore]) -> SummaryLevel {
        if messages.is_empty() || priorities.is_empty() {
            return SummaryLevel::Standard;
        }
        
        // Calculate weighted average priority
        let priority_scores: Vec<f32> = priorities
            .iter()
            .map(|p| p.value())
            .collect();
        
        let avg_score: f32 = priority_scores.iter().sum::<f32>() / priority_scores.len() as f32;
        
        // Also consider message count
        let count_factor = if messages.len() > self.hardcode_config.large_conversation_threshold {
            0.8  // More aggressive compression
        } else if messages.len() > self.hardcode_config.medium_conversation_threshold {
            0.9
        } else {
            1.0
        };
        
        let adjusted_score = avg_score * count_factor;
        
        if adjusted_score >= 0.75 {
            SummaryLevel::Detailed
        } else if adjusted_score >= 0.45 {
            SummaryLevel::Standard
        } else {
            SummaryLevel::Brief
        }
    }
    
    /// Progressive summarization: Start from one end, progressively compress
    pub fn progressive_summarize(&self, messages: &[Message], priorities: &[PriorityScore]) -> Vec<String> {
        if messages.is_empty() {
            return Vec::new();
        }
        
        let mut summaries = Vec::with_capacity(messages.len());
        let total = messages.len();
        
        for (i, (msg, priority)) in messages.iter().zip(priorities.iter()).enumerate() {
            // Progressive strategy: older messages get more compression
            let base_level = SummaryLevel::from_priority(*priority);
            
            let level = if self.config.progressive {
                let age_factor = (total - i) as f32 / total as f32;
                
                // Older messages (smaller i) get higher compression
                if age_factor > 0.7 {
                    // Recent messages: use base level
                    base_level
                } else if age_factor > 0.4 {
                    // Middle messages: compress one level
                    compress_level(base_level)
                } else {
                    // Old messages: compress two levels
                    compress_level(compress_level(base_level))
                }
            } else {
                base_level
            };
            
            summaries.push(self.summarize_message(msg, level));
        }
        
        summaries
    }
}

// === Helper Functions ===

/// Get role label for display
fn role_label(role: &Role) -> &'static str {
    match role {
        Role::User => "U",
        Role::Assistant => "A",
        Role::System => "S",
        Role::Tool => "T",
    }
}

/// Truncate string to max characters
fn truncate_to_chars(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        s.chars().take(max_chars).collect::<String>() + "..."
    }
}

/// Extract key actions from content
fn extract_key_actions(content: &str) -> Vec<String> {
    let action_keywords = [
        "创建", "删除", "修改", "更新", "查询", "搜索", "分析", "优化",
        "create", "delete", "update", "query", "search", "analyze", "optimize",
        "fix", "add", "remove", "refactor", "test"
    ];
    
    let mut actions = Vec::new();
    let lower = content.to_lowercase();
    
    for keyword in &action_keywords {
        if lower.contains(keyword) {
            actions.push(keyword.to_string());
            if actions.len() >= 3 {
                break;
            }
        }
    }
    
    actions
}

/// Extract entities (nouns, names) from content
fn extract_entities(content: &str, config: &HardcodeConfig) -> Vec<String> {
    // Simple entity extraction: capitalized words and quoted strings
    let mut entities = Vec::new();
    
    // Find quoted strings
    let in_quotes: Vec<&str> = content
        .split('"')
        .enumerate()
        .filter(|(i, _)| i % 2 == 1)
        .map(|(_, s)| s)
        .take(3)
        .collect();
    
    for q in in_quotes {
        if config.is_valid_question_length(q.len()) {
            entities.push(format!("\"{}\"", truncate_to_chars(q, config.max_question_extract_length)));
        }
    }
    
    entities
}

/// Find the most important sentence in a set
fn find_key_sentence(sentences: &[&str], config: &HardcodeConfig) -> Option<String> {
    // Simple heuristic: longest sentence with key terms
    let key_terms = ["error", "问题", "result", "结果", "success", "成功", "fail", "失败"];
    
    sentences
        .iter()
        .filter(|s| s.len() > config.min_sentence_length)
        .max_by(|a, b| {
            let a_score = key_terms.iter().filter(|t| a.contains(*t)).count();
            let b_score = key_terms.iter().filter(|t| b.contains(*t)).count();
            a_score.cmp(&b_score)
        })
        .map(|s| s.to_string())
}

/// Compress a single sentence
fn compress_sentence(sentence: &str, config: &HardcodeConfig) -> String {
    // Remove filler words
    let fillers = ["的", "了", "然后", "接着", "因此", "所以", "that", "the", "then", "therefore"];
    
    let mut compressed = sentence.to_string();
    for filler in &fillers {
        if compressed.len() > config.max_compressed_output_length {
            compressed = compressed.replace(filler, "");
        }
    }
    
    truncate_to_chars(&compressed, config.short_summary_word_count * 5)
}

/// Extract code blocks
fn extract_code_blocks(content: &str) -> Vec<&str> {
    let mut blocks = Vec::new();
    let mut in_block = false;
    
    for line in content.lines() {
        if line.contains("```") {
            in_block = !in_block;
        } else if in_block {
            blocks.push(line);
        }
    }
    
    blocks
}

/// Compress summarization level by one degree
fn compress_level(level: SummaryLevel) -> SummaryLevel {
    match level {
        SummaryLevel::Detailed => SummaryLevel::Standard,
        SummaryLevel::Standard => SummaryLevel::Brief,
        SummaryLevel::Brief => SummaryLevel::Brief,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_summary_level_from_priority() {
        assert_eq!(SummaryLevel::from_priority(PriorityScore::new(0.9)), SummaryLevel::Detailed);
        assert_eq!(SummaryLevel::from_priority(PriorityScore::new(0.75)), SummaryLevel::Detailed);
        assert_eq!(SummaryLevel::from_priority(PriorityScore::new(0.5)), SummaryLevel::Standard);
        assert_eq!(SummaryLevel::from_priority(PriorityScore::new(0.3)), SummaryLevel::Brief);
    }
    
    #[test]
    fn test_retention_ratio() {
        assert!((SummaryLevel::Brief.retention_ratio() - 0.25).abs() < 0.01);
        assert!((SummaryLevel::Standard.retention_ratio() - 0.45).abs() < 0.01);
        assert!((SummaryLevel::Detailed.retention_ratio() - 0.65).abs() < 0.01);
    }
    
    #[test]
    fn test_brief_summary() {
        let summarizer = HierarchicalSummarizer::default();
        let msg = Message {
            role: Role::User,
            content: MessageContent::Text("我需要创建一个新的API接口来处理用户认证。请帮我实现这个功能。".to_string()),
        };
        
        let summary = summarizer.summarize_message(&msg, SummaryLevel::Brief);
        assert!(summary.contains("[U]"));
        assert!(summary.len() < 100);
    }
    
    #[test]
    fn test_standard_summary() {
        let summarizer = HierarchicalSummarizer::default();
        let msg = Message {
            role: Role::Assistant,
            content: MessageContent::Text("好的，我来创建API接口。首先需要设计数据结构。然后实现认证逻辑。最后添加测试用例。".to_string()),
        };
        
        let summary = summarizer.summarize_message(&msg, SummaryLevel::Standard);
        assert!(summary.contains("[A]"));
        assert!(summary.len() < 200);
    }
    
    #[test]
    fn test_detailed_summary() {
        let summarizer = HierarchicalSummarizer::default();
        let msg = Message {
            role: Role::Assistant,
            content: MessageContent::Text("这是一个详细的实现方案。首先，我们需要考虑性能问题。其次，安全性也���重要。最后，要确保代码可维护性。".to_string()),
        };
        
        let summary = summarizer.summarize_message(&msg, SummaryLevel::Detailed);
        assert!(summary.contains("[A]"));
        assert!(summary.contains("→"));  // Should have arrows
    }
    
    #[test]
    fn test_progressive_summarize() {
        let summarizer = HierarchicalSummarizer::default();
        let messages = vec![
            Message {
                role: Role::User,
                content: MessageContent::Text("第一条消息".to_string()),
            },
            Message {
                role: Role::Assistant,
                content: MessageContent::Text("第二条消息".to_string()),
            },
            Message {
                role: Role::User,
                content: MessageContent::Text("第三条消息".to_string()),
            },
        ];
        let priorities = vec![
            PriorityScore::new(0.3),
            PriorityScore::new(0.5),
            PriorityScore::new(0.8),
        ];
        
        let summaries = summarizer.progressive_summarize(&messages, &priorities);
        assert_eq!(summaries.len(), 3);
    }
}