//! Feedback learning and behavior inference.

use std::collections::HashMap;

use super::config::MIN_MEMORY_CONTENT_LENGTH;
use super::extractor::infer_category_from_content;
use super::retrieval::extract_context_keywords;
use super::types::{AutoMemory, MemoryCategory, MemoryEntry};

// ============================================================================
// Feedback Detection
// ============================================================================

/// Action to take when user feedback is detected.
#[derive(Debug, Clone, PartialEq)]
pub enum FeedbackAction {
    Correct,
    Delete,
    Add,
    NegativePreference,
}

/// Result of feedback detection.
#[derive(Debug, Clone)]
pub struct FeedbackResult {
    pub action: FeedbackAction,
    pub category: Option<MemoryCategory>,
    pub new_content: Option<String>,
    pub search_keywords: Vec<String>,
    pub original_text: String,
}

/// Detect user feedback patterns.
pub fn detect_feedback_patterns(text: &str) -> Vec<FeedbackResult> {
    let mut results = Vec::new();
    let text_lower = text.to_lowercase();

    let correction_patterns = [
        "不对，应该是", "错了，实际上", "不是，是", "应该是",
        "no, it should be", "wrong, actually", "should be",
    ];

    let delete_patterns = [
        "不要那个", "不需要那个", "删掉那个", "不再用",
        "don't need that", "no longer need", "remove that",
    ];

    let add_patterns = [
        "记一下", "记住", "记录一下", "要记住",
        "remember this", "note this", "keep this",
    ];

    let negative_patterns = [
        "不喜欢", "不偏好", "讨厌", "不想用",
        "i don't like", "i dislike", "i hate",
    ];

    for pattern in correction_patterns {
        if text_lower.contains(pattern) {
            let content = extract_feedback_content(text, pattern);
            if content.len() >= MIN_MEMORY_CONTENT_LENGTH {
                results.push(FeedbackResult {
                    action: FeedbackAction::Correct,
                    category: Some(infer_category_from_content(&content)),
                    new_content: Some(content.clone()),
                    search_keywords: extract_context_keywords(&content),
                    original_text: text.to_string(),
                });
            }
        }
    }

    for pattern in delete_patterns {
        if text_lower.contains(pattern) {
            let content = extract_feedback_content(text, pattern);
            results.push(FeedbackResult {
                action: FeedbackAction::Delete,
                category: None,
                new_content: None,
                search_keywords: if content.is_empty() {
                    vec![pattern.to_string()]
                } else {
                    extract_context_keywords(&content)
                },
                original_text: text.to_string(),
            });
        }
    }

    for pattern in add_patterns {
        if text_lower.contains(pattern) {
            let content = extract_feedback_content(text, pattern);
            if content.len() >= MIN_MEMORY_CONTENT_LENGTH {
                results.push(FeedbackResult {
                    action: FeedbackAction::Add,
                    category: Some(infer_category_from_content(&content)),
                    new_content: Some(content),
                    search_keywords: vec![],
                    original_text: text.to_string(),
                });
            }
        }
    }

    for pattern in negative_patterns {
        if text_lower.contains(pattern) {
            let content = extract_feedback_content(text, pattern);
            if content.len() >= MIN_MEMORY_CONTENT_LENGTH {
                results.push(FeedbackResult {
                    action: FeedbackAction::NegativePreference,
                    category: Some(MemoryCategory::Preference),
                    new_content: Some(format!("不喜欢: {}", content)),
                    search_keywords: extract_context_keywords(&content),
                    original_text: text.to_string(),
                });
            }
        }
    }

    results
}

fn extract_feedback_content(text: &str, pattern: &str) -> String {
    let pos = match text.to_lowercase().find(&pattern.to_lowercase()) {
        Some(p) => p,
        None => return String::new(),
    };

    let start = pos + pattern.len();
    if start >= text.len() {
        return String::new();
    }

    let remaining = &text[start..];
    let end = remaining.find(|c: char| c == '.' || c == '。' || c == '\n')
        .map(|i| i)
        .unwrap_or(remaining.len().min(100));

    remaining[..end].trim().to_string()
}

/// Apply feedback to memory.
pub fn apply_feedback_to_memory(memory: &mut AutoMemory, feedback: &FeedbackResult) -> usize {
    let mut changes = 0;

    match feedback.action {
        FeedbackAction::Correct => {
            if let Some(ref content) = feedback.new_content {
                // Find matching entries and update
                for entry in &mut memory.entries {
                    if feedback.search_keywords.iter().any(|k| entry.content.to_lowercase().contains(&k.to_lowercase())) {
                        entry.content = content.clone();
                        entry.importance = entry.importance.max(80.0);
                        changes += 1;
                    }
                }
                if changes == 0 {
                    // No matching entry, add new
                    let category = feedback.category.unwrap_or(MemoryCategory::Finding);
                    memory.add_memory(category, content.clone(), None);
                    changes += 1;
                }
            }
        }
        FeedbackAction::Delete => {
            let ids_to_delete: Vec<String> = memory.entries
                .iter()
                .filter(|e| feedback.search_keywords.iter().any(|k| e.content.to_lowercase().contains(&k.to_lowercase())))
                .take(3)
                .map(|e| e.id.clone())
                .collect();

            for id in ids_to_delete {
                if memory.remove(&id) {
                    changes += 1;
                }
            }
        }
        FeedbackAction::Add => {
            if let Some(ref content) = feedback.new_content {
                let category = feedback.category.unwrap_or(MemoryCategory::Finding);
                let entry = MemoryEntry::manual(category, content.clone());
                memory.add(entry);
                changes += 1;
            }
        }
        FeedbackAction::NegativePreference => {
            if let Some(ref content) = feedback.new_content {
                let mut entry = MemoryEntry::manual(MemoryCategory::Preference, content.clone());
                entry.tags.push("negative".to_string());
                memory.add(entry);
                changes += 1;
            }
        }
    }

    changes
}

// ============================================================================
// Behavior Inference
// ============================================================================

/// Configuration for behavior inference.
#[derive(Clone)]
pub struct BehaviorInferenceConfig {
    pub min_occurrences: usize,
    pub min_confidence: f64,
    pub max_inferences: usize,
}

impl Default for BehaviorInferenceConfig {
    fn default() -> Self {
        Self {
            min_occurrences: 2,
            min_confidence: 0.6,
            max_inferences: 5,
        }
    }
}

/// Result of behavior inference.
#[derive(Debug, Clone)]
pub struct BehaviorInference {
    pub content: String,
    pub confidence: f64,
    pub occurrences: usize,
    pub keywords: Vec<String>,
}

/// Infer preferences from conversation patterns.
pub fn infer_preferences_from_behavior(
    messages: &[crate::providers::Message],
    config: &BehaviorInferenceConfig,
) -> Vec<BehaviorInference> {
    let mut inferences: Vec<BehaviorInference> = Vec::new();

    let user_texts: Vec<String> = messages.iter()
        .filter_map(|msg| {
            if msg.role == crate::providers::Role::User {
                match &msg.content {
                    crate::providers::MessageContent::Text(t) => Some(t.clone()),
                    crate::providers::MessageContent::Blocks(blocks) => {
                        Some(blocks.iter().filter_map(|b| {
                            if let crate::providers::ContentBlock::Text { text } = b {
                                Some(text.as_str())
                            } else {
                                None
                            }
                        }).collect::<Vec<_>>().join(" "))
                    }
                }
            } else {
                None
            }
        })
        .collect();

    if user_texts.len() < config.min_occurrences {
        return inferences;
    }

    let all_text = user_texts.join(" ");
    let all_text_lower = all_text.to_lowercase();

    let tech_patterns: Vec<(&str, &str)> = vec![
        ("rust", "Rust"), ("python", "Python"), ("react", "React"),
        ("vue", "Vue"), ("typescript", "TypeScript"), ("go", "Go"),
        ("docker", "Docker"), ("postgres", "PostgreSQL"), ("vim", "Vim"),
    ];

    let mut tech_counts: HashMap<&str, usize> = HashMap::new();
    for (pattern, _) in &tech_patterns {
        let count = all_text_lower.matches(pattern).count();
        if count >= config.min_occurrences {
            tech_counts.insert(pattern, count);
        }
    }

    for (pattern, name) in tech_patterns {
        if let Some(&count) = tech_counts.get(pattern) {
            let confidence = (count as f64 / user_texts.len() as f64).min(1.0);
            if confidence >= config.min_confidence {
                inferences.push(BehaviorInference {
                    content: format!("用户频繁提及 {}", name),
                    confidence,
                    occurrences: count,
                    keywords: vec![name.to_string()],
                });
            }
        }
    }

    inferences.truncate(config.max_inferences);
    inferences
}

/// Convert inference to memory entry.
pub fn inference_to_memory_entry(inference: &BehaviorInference) -> MemoryEntry {
    let mut entry = MemoryEntry::new(
        MemoryCategory::Preference,
        inference.content.clone(),
        None,
    );
    entry.importance = (inference.confidence * 70.0 + 30.0).min(80.0);
    entry.tags = inference.keywords.clone();
    entry
}

/// Apply behavior inferences to memory.
/// Returns the number of new entries added.
pub fn apply_behavior_inferences_to_memory(
    messages: &[crate::providers::Message],
    memory: &mut AutoMemory,
    config: Option<&BehaviorInferenceConfig>,
) -> usize {
    let cfg = config.cloned().unwrap_or_default();
    let inferences = infer_preferences_from_behavior(messages, &cfg);

    let mut added = 0;
    for inference in inferences {
        let entry = inference_to_memory_entry(&inference);
        // Check if similar entry already exists
        if !memory.entries.iter().any(|e| e.content == entry.content) {
            memory.entries.push(entry);
            added += 1;
        }
    }

    added
}