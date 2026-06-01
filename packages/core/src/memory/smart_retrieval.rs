//! Smart Memory Retrieval: Advanced retrieval with focus relevance and time decay.
//!
//! This module implements intelligent memory retrieval using multiple factors:
//! - TF-IDF similarity
//! - Focus point relevance
//! - Time decay
//! - Importance weighting
//! - Usage frequency

use chrono::{DateTime, Utc};
use std::collections::HashMap;

use super::entry::{MemoryCategory, MemoryEntry};
use super::retrieval::{TfIdfSearch, expand_semantic_keywords};
use crate::compress::{FocusPoint, FocusStatus};

/// Smart memory retriever with advanced scoring.
pub struct SmartMemoryRetriever {
    /// Time decay configuration
    time_decay_config: TimeDecayConfig,
    /// Focus relevance weight
    focus_weight: f32,
    /// Time decay weight
    time_decay_weight: f32,
    /// Importance weight
    importance_weight: f32,
    /// TF-IDF weight
    tfidf_weight: f32,
    /// Usage frequency weight
    usage_weight: f32,
}

/// Time decay configuration.
#[derive(Debug, Clone)]
pub struct TimeDecayConfig {
    /// Half-life in hours (time for relevance to decay to 50%)
    half_life_hours: f32,
    /// Minimum decay factor (don't decay below this)
    min_decay: f32,
    /// Boost for recent entries (< 1 hour)
    recent_boost: f32,
}

impl Default for TimeDecayConfig {
    fn default() -> Self {
        Self {
            half_life_hours: 24.0, // Decay to 50% after 24 hours
            min_decay: 0.3,        // Keep at least 30% relevance
            recent_boost: 1.5,     // Boost recent entries by 50%
        }
    }
}

impl Default for SmartMemoryRetriever {
    fn default() -> Self {
        Self {
            time_decay_config: TimeDecayConfig::default(),
            focus_weight: 0.25,
            time_decay_weight: 0.15,
            importance_weight: 0.20,
            tfidf_weight: 0.30,
            usage_weight: 0.10,
        }
    }
}

impl SmartMemoryRetriever {
    /// Create a new smart retriever.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with custom weights.
    pub fn with_weights(
        focus_weight: f32,
        time_decay_weight: f32,
        importance_weight: f32,
        tfidf_weight: f32,
        usage_weight: f32,
    ) -> Self {
        // Normalize weights to sum to 1.0
        let total = focus_weight + time_decay_weight + importance_weight + tfidf_weight + usage_weight;
        Self {
            time_decay_config: TimeDecayConfig::default(),
            focus_weight: focus_weight / total,
            time_decay_weight: time_decay_weight / total,
            importance_weight: importance_weight / total,
            tfidf_weight: tfidf_weight / total,
            usage_weight: usage_weight / total,
        }
    }

    /// Retrieve memories with smart scoring.
    pub fn retrieve(
        &self,
        entries: &[MemoryEntry],
        context_keywords: &[String],
        active_foci: &[FocusPoint],
        max_entries: usize,
    ) -> Vec<MemoryEntry> {
        if entries.is_empty() {
            return Vec::new();
        }

        // Expand keywords semantically
        let expanded_keywords = expand_semantic_keywords(context_keywords);

        // Build TF-IDF index
        let _tfidf = TfIdfSearch::new();
        // Note: We need to adapt TfIdfSearch to work with entries directly
        // For now, we'll use a simplified approach

        // Calculate scores for each entry
        let mut scored_entries: Vec<(MemoryEntry, f32)> = entries
            .iter()
            .map(|entry| {
                let score = self.calculate_entry_score(
                    entry,
                    &expanded_keywords,
                    active_foci,
                    Utc::now(),
                );
                (entry.clone(), score)
            })
            .collect();

        // Sort by score (descending)
        scored_entries.sort_by(|a, b| {
            b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Take top entries
        scored_entries
            .into_iter()
            .take(max_entries)
            .map(|(entry, _)| entry)
            .collect()
    }

    /// Calculate comprehensive score for an entry.
    fn calculate_entry_score(
        &self,
        entry: &MemoryEntry,
        keywords: &[String],
        active_foci: &[FocusPoint],
        now: DateTime<Utc>,
    ) -> f32 {
        // 1. TF-IDF / keyword relevance
        let relevance_score = self.calculate_relevance_score(entry, keywords);

        // 2. Focus relevance
        let focus_score = self.calculate_focus_relevance(entry, active_foci);

        // 3. Time decay
        let time_score = self.calculate_time_decay(entry, now);

        // 4. Importance
        let importance_score = entry.importance as f32 / 100.0;

        // 5. Usage frequency
        let usage_score = self.calculate_usage_score(entry);

        // Combine scores with weights
        let combined = 
            relevance_score * self.tfidf_weight +
            focus_score * self.focus_weight +
            time_score * self.time_decay_weight +
            importance_score * self.importance_weight +
            usage_score * self.usage_weight;

        // Apply manual boost
        if entry.is_manual {
            combined * 1.5
        } else {
            combined
        }
    }

    /// Calculate keyword relevance score.
    fn calculate_relevance_score(&self, entry: &MemoryEntry, keywords: &[String]) -> f32 {
        let entry_lower = entry.content.to_lowercase();
        let mut score = 0.0;

        for keyword in keywords {
            let kw_lower = keyword.to_lowercase();
            if entry_lower.contains(&kw_lower) {
                // Exact match: full score
                score += 1.0;
            } else {
                // Check tags
                if entry.tags.iter().any(|t| t.to_lowercase().contains(&kw_lower)) {
                    score += 0.7;
                }
            }
        }

        // Normalize by number of keywords
        if !keywords.is_empty() {
            (score / keywords.len() as f32).min(1.0)
        } else {
            0.0
        }
    }

    /// Calculate focus relevance score.
    fn calculate_focus_relevance(&self, entry: &MemoryEntry, active_foci: &[FocusPoint]) -> f32 {
        if active_foci.is_empty() {
            return 0.5; // Neutral if no active focus
        }

        let entry_lower = entry.content.to_lowercase();
        let mut max_score: f32 = 0.0;

        for focus in active_foci {
            let mut score: f32 = 0.0;

            // Keyword match
            for keyword in &focus.keywords {
                if entry_lower.contains(&keyword.to_lowercase()) {
                    score += 0.3;
                }
            }

            // Entity match (higher weight)
            for entity in &focus.entities {
                if entry_lower.contains(&entity.to_lowercase()) {
                    score += 0.5;
                }
            }

            // Topic overlap
            let topic_words = focus.topic.split_whitespace()
                .map(|w| w.to_lowercase())
                .collect::<Vec<_>>();
            for word in &topic_words {
                if entry_lower.contains(word) {
                    score += 0.2;
                }
            }

            // Weight by focus importance
            score *= focus.importance;

            // Weight by focus status
            if focus.status == FocusStatus::Active {
                score *= 1.2;
            }

            max_score = max_score.max(score);
        }

        max_score.min(1.0_f32)
    }

    /// Calculate time decay score.
    fn calculate_time_decay(&self, entry: &MemoryEntry, now: DateTime<Utc>) -> f32 {
        let hours_since_created = (now - entry.created_at).num_seconds() as f32 / 3600.0;

        // Apply exponential decay
        let decay_factor = 0.5_f32.powf(hours_since_created / self.time_decay_config.half_life_hours);

        // Apply minimum threshold
        let decayed = decay_factor.max(self.time_decay_config.min_decay);

        // Apply recent boost
        if hours_since_created < 1.0 {
            decayed * self.time_decay_config.recent_boost
        } else {
            decayed
        }
    }

    /// Calculate usage frequency score.
    fn calculate_usage_score(&self, entry: &MemoryEntry) -> f32 {
        // Normalize reference count
        let ref_score = (entry.reference_count as f32 / 10.0).min(1.0);

        // New entries have neutral score
        if entry.reference_count == 0 {
            0.5
        } else {
            ref_score
        }
    }

    /// Generate smart summary with focus awareness.
    pub fn generate_smart_summary(
        &self,
        entries: &[MemoryEntry],
        context_keywords: &[String],
        active_foci: &[FocusPoint],
        max_entries: usize,
    ) -> String {
        let selected = self.retrieve(entries, context_keywords, active_foci, max_entries);

        if selected.is_empty() {
            return String::new();
        }

        let mut summary = String::from("【智能记忆检索】\n\n");

        // Group by category
        let mut by_cat: HashMap<MemoryCategory, Vec<&MemoryEntry>> = HashMap::new();
        for entry in &selected {
            by_cat.entry(entry.category).or_default().push(entry);
        }

        // Add focus context if available
        if !active_foci.is_empty() {
            summary.push_str("当前聚焦:\n");
            for focus in active_foci {
                summary.push_str(&format!("  • {} (重要性: {:.0}%)\n", focus.topic, focus.importance * 100.0));
            }
            summary.push_str("\n");
        }

        // Add memories by category
        for (cat, entries) in by_cat {
            summary.push_str(&format!("{} {}:\n", cat.icon(), cat.display_name()));
            for entry in entries {
                summary.push_str(&format!("  {}\n", entry.format_for_prompt()));
            }
            summary.push_str("\n");
        }

        summary
    }

    /// Get retrieval statistics.
    pub fn get_retrieval_stats(
        &self,
        entries: &[MemoryEntry],
        context_keywords: &[String],
        active_foci: &[FocusPoint],
    ) -> RetrievalStats {
        let expanded = expand_semantic_keywords(context_keywords);

        let mut stats = RetrievalStats {
            total_entries: entries.len(),
            keyword_matches: 0,
            focus_matches: 0,
            recent_entries: 0,
            highly_important: 0,
            frequently_used: 0,
            avg_score: 0.0,
        };

        let now = Utc::now();
        let mut total_score = 0.0;

        for entry in entries {
            let score = self.calculate_entry_score(entry, &expanded, active_foci, now);
            total_score += score;

            // Count matches
            if self.calculate_relevance_score(entry, &expanded) > 0.5 {
                stats.keyword_matches += 1;
            }
            if self.calculate_focus_relevance(entry, active_foci) > 0.5 {
                stats.focus_matches += 1;
            }
            if (now - entry.created_at).num_hours() < 1 {
                stats.recent_entries += 1;
            }
            if entry.importance > 70.0 {
                stats.highly_important += 1;
            }
            if entry.reference_count > 5 {
                stats.frequently_used += 1;
            }
        }

        if !entries.is_empty() {
            stats.avg_score = total_score / entries.len() as f32;
        }

        stats
    }
}

/// Retrieval statistics.
#[derive(Debug, Clone)]
pub struct RetrievalStats {
    pub total_entries: usize,
    pub keyword_matches: usize,
    pub focus_matches: usize,
    pub recent_entries: usize,
    pub highly_important: usize,
    pub frequently_used: usize,
    pub avg_score: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smart_retriever_creation() {
        let retriever = SmartMemoryRetriever::new();
        assert_eq!(retriever.focus_weight + retriever.time_decay_weight + retriever.importance_weight + retriever.tfidf_weight + retriever.usage_weight, 1.0);
    }

    #[test]
    fn test_time_decay_config() {
        let config = TimeDecayConfig::default();
        assert_eq!(config.half_life_hours, 24.0);
        assert_eq!(config.min_decay, 0.3);
    }

    #[test]
    fn test_empty_retrieval() {
        let retriever = SmartMemoryRetriever::new();
        let result = retriever.retrieve(&[], &[], &[], 5);
        assert!(result.is_empty());
    }

    #[test]
    fn test_empty_summary() {
        let retriever = SmartMemoryRetriever::new();
        let summary = retriever.generate_smart_summary(&[], &[], &[], 5);
        assert!(summary.is_empty());
    }
}