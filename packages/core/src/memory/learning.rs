//! Feedback learning and behavior inference.
//!
//! This module provides high-level learning mechanisms that let the model
//! understand patterns from interactions, rather than hardcoded rules.

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

/// Detect user feedback patterns - generic detection, not exhaustive.
/// The model should use its understanding to detect nuances.
pub fn detect_feedback_patterns(text: &str) -> Vec<FeedbackResult> {
    let mut results = Vec::new();
    let text_lower = text.to_lowercase();

    // Generic correction signals
    let correction_signals = ["不对", "错了", "不是", "no", "wrong", "should be"];
    for signal in correction_signals {
        if text_lower.contains(signal) {
            let content = extract_feedback_content(text, signal);
            if content.len() >= MIN_MEMORY_CONTENT_LENGTH {
                results.push(FeedbackResult {
                    action: FeedbackAction::Correct,
                    category: Some(infer_category_from_content(&content)),
                    new_content: Some(content.clone()),
                    search_keywords: extract_context_keywords(&content),
                    original_text: text.to_string(),
                });
                break; // Only one correction per message
            }
        }
    }

    // Generic delete signals
    let delete_signals = ["不要", "删掉", "remove", "delete", "don't need"];
    for signal in delete_signals {
        if text_lower.contains(signal) {
            let content = extract_feedback_content(text, signal);
            results.push(FeedbackResult {
                action: FeedbackAction::Delete,
                category: None,
                new_content: None,
                search_keywords: if content.is_empty() {
                    vec![signal.to_string()]
                } else {
                    extract_context_keywords(&content)
                },
                original_text: text.to_string(),
            });
            break;
        }
    }

    // Generic add signals
    let add_signals = ["记住", "记一下", "remember", "note"];
    for signal in add_signals {
        if text_lower.contains(signal) {
            let content = extract_feedback_content(text, signal);
            if content.len() >= MIN_MEMORY_CONTENT_LENGTH {
                results.push(FeedbackResult {
                    action: FeedbackAction::Add,
                    category: Some(infer_category_from_content(&content)),
                    new_content: Some(content),
                    search_keywords: vec![],
                    original_text: text.to_string(),
                });
                break;
            }
        }
    }

    // Generic negative preference signals
    let negative_signals = ["不喜欢", "讨厌", "dislike", "hate", "don't like"];
    for signal in negative_signals {
        if text_lower.contains(signal) {
            let content = extract_feedback_content(text, signal);
            if content.len() >= MIN_MEMORY_CONTENT_LENGTH {
                results.push(FeedbackResult {
                    action: FeedbackAction::NegativePreference,
                    category: Some(MemoryCategory::Preference),
                    new_content: Some(format!("不喜欢: {}", content)),
                    search_keywords: extract_context_keywords(&content),
                    original_text: text.to_string(),
                });
                break;
            }
        }
    }

    results
}

fn extract_feedback_content(text: &str, pattern: &str) -> String {
    // Use case-insensitive search but track position in original text
    // to avoid Unicode byte length mismatches from lowercase conversion
    let text_lower = text.to_lowercase();
    let pattern_lower = pattern.to_lowercase();

    let pos = match text_lower.find(&pattern_lower) {
        Some(p) => p,
        None => return String::new(),
    };

    // Find the actual position in original text by counting chars
    // (lowercase conversion can change byte lengths for some Unicode chars)
    let char_pos = text_lower[..pos].chars().count();
    let start_char_idx = char_pos + pattern.chars().count();

    // Get remaining text by char indices
    let remaining: String = text.chars().skip(start_char_idx).collect();
    if remaining.is_empty() {
        return String::new();
    }

    // Find end delimiter (first ., 。, or \n, or up to 100 chars)
    let end_char_count = remaining
        .find(['.', '。', '\n'])
        .map(|i| remaining[..i].chars().count())
        .unwrap_or(remaining.chars().count().min(100));

    remaining.chars().take(end_char_count).collect::<String>().trim().to_string()
}

/// Apply feedback to memory.
pub fn apply_feedback_to_memory(memory: &mut AutoMemory, feedback: &FeedbackResult) -> usize {
    let mut changes = 0;

    match feedback.action {
        FeedbackAction::Correct => {
            if let Some(ref content) = feedback.new_content {
                for entry in &mut memory.entries {
                    if feedback
                        .search_keywords
                        .iter()
                        .any(|k| entry.content.to_lowercase().contains(&k.to_lowercase()))
                    {
                        entry.content = content.clone();
                        entry.importance = entry.importance.max(80.0);
                        changes += 1;
                    }
                }
                if changes == 0 {
                    let category = feedback.category.unwrap_or(MemoryCategory::Finding);
                    memory.add_memory(category, content.clone(), None);
                    changes += 1;
                }
            }
        }
        FeedbackAction::Delete => {
            let ids_to_delete: Vec<String> = memory
                .entries
                .iter()
                .filter(|e| {
                    feedback
                        .search_keywords
                        .iter()
                        .any(|k| e.content.to_lowercase().contains(&k.to_lowercase()))
                })
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
                let entry = MemoryEntry::manual_global(category, content.clone());
                memory.add(entry);
                changes += 1;
            }
        }
        FeedbackAction::NegativePreference => {
            if let Some(ref content) = feedback.new_content {
                let mut entry = MemoryEntry::manual_global(MemoryCategory::Preference, content.clone());
                entry.tags.push("negative".to_string());
                memory.add(entry);
                changes += 1;
            }
        }
    }

    changes
}

// ============================================================================
// Behavior Inference - Generic Pattern Detection
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

/// Infer patterns from behavior - generic word frequency analysis.
/// Let the model decide what's meaningful, not hardcoded tech patterns.
pub fn infer_preferences_from_behavior(
    messages: &[crate::providers::Message],
    config: &BehaviorInferenceConfig,
) -> Vec<BehaviorInference> {
    let user_texts: Vec<String> = messages
        .iter()
        .filter_map(|msg| {
            if msg.role == crate::providers::Role::User {
                match &msg.content {
                    crate::providers::MessageContent::Text(t) => Some(t.clone()),
                    crate::providers::MessageContent::Blocks(blocks) => Some(
                        blocks
                            .iter()
                            .filter_map(|b| {
                                if let crate::providers::ContentBlock::Text { text } = b {
                                    Some(text.as_str())
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>()
                            .join(" "),
                    ),
                }
            } else {
                None
            }
        })
        .collect();

    if user_texts.len() < config.min_occurrences {
        return Vec::new();
    }

    // Generic word frequency analysis
    let mut word_freq: HashMap<String, usize> = HashMap::new();
    for text in &user_texts {
        for word in text.to_lowercase().split_whitespace() {
            if word.len() > 3 { // Skip short words
                *word_freq.entry(word.to_string()).or_default() += 1;
            }
        }
    }

    // Extract high-frequency words as potential preferences
    let inferences: Vec<BehaviorInference> = word_freq
        .iter()
        .filter(|(_, count)| **count >= config.min_occurrences)
        .map(|(word, count)| {
            let confidence = (*count as f64 / user_texts.len() as f64).min(1.0);
            BehaviorInference {
                content: format!("用户多次提及 '{}'", word),
                confidence,
                occurrences: *count,
                keywords: vec![word.clone()],
            }
        })
        .filter(|inf| inf.confidence >= config.min_confidence)
        .take(config.max_inferences)
        .collect();

    inferences
}

/// Convert inference to memory entry (global preference).
pub fn inference_to_memory_entry(inference: &BehaviorInference) -> MemoryEntry {
    let mut entry = MemoryEntry::new(MemoryCategory::Preference, inference.content.clone(), None, None);
    entry.importance = (inference.confidence * 70.0 + 30.0).min(80.0);
    entry.tags = inference.keywords.clone();
    entry
}

/// Apply behavior inferences to memory.
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
        if !memory.entries.iter().any(|e| e.content == entry.content) {
            memory.entries.push(entry);
            added += 1;
        }
    }

    added
}

/// Generic tool learning - let model decide what to remember.
/// This is a placeholder for future AI-driven learning.
pub fn apply_tool_learning_to_memory(
    _tool_name: &str,
    _tool_input: &serde_json::Value,
    _tool_result: &str,
    _is_error: bool,
    _memory: &mut AutoMemory,
) -> usize {
    // Future: Use AI to analyze tool execution and extract learnings
    // Current: Let the model handle this through its own analysis
    0
}