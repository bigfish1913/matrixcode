//! Adaptive Learning: User feedback learning and system adaptation.
//!
//! This module implements adaptive behavior based on user feedback:
//! - Track compression preferences
//! - Learn from accept/reject patterns
//! - Adjust focus detection sensitivity
//! - Customize retrieval weights

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Adaptive configuration learner.
pub struct AdaptiveLearner {
    /// Compression feedback history
    compression_feedback: Vec<CompressionFeedback>,
    /// Focus detection feedback
    focus_feedback: Vec<FocusFeedback>,
    /// Retrieval feedback
    retrieval_feedback: Vec<RetrievalFeedback>,
    /// Learned preferences
    preferences: AdaptivePreferences,
    /// Feedback statistics
    stats: FeedbackStats,
}

/// Feedback for compression quality.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionFeedback {
    /// Timestamp
    timestamp: DateTime<Utc>,
    /// Session ID
    session_id: String,
    /// Original token count
    original_tokens: u32,
    /// Compressed token count
    compressed_tokens: u32,
    /// Compression stage used
    stage: String,
    /// User rating (1-5)
    rating: u8,
    /// User comments (optional)
    comments: Option<String>,
    /// Accepted or rejected
    accepted: bool,
}

/// Feedback for focus detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusFeedback {
    /// Timestamp
    timestamp: DateTime<Utc>,
    /// Session ID
    session_id: String,
    /// Focus topic
    focus_topic: String,
    /// User rating (1-5)
    rating: u8,
    /// Was focus accurate?
    accurate: bool,
    /// Suggested correction (optional)
    suggested_correction: Option<String>,
}

/// Feedback for memory retrieval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalFeedback {
    /// Timestamp
    timestamp: DateTime<Utc>,
    /// Session ID
    session_id: String,
    /// Memory ID
    memory_id: String,
    /// Memory content (truncated)
    memory_content: String,
    /// User rating (1-5)
    rating: u8,
    /// Was retrieval relevant?
    relevant: bool,
    /// Suggested context (optional)
    suggested_context: Option<String>,
}

/// Learned adaptive preferences.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptivePreferences {
    /// Compression aggressiveness (0.0 = conservative, 1.0 = aggressive)
    compression_aggressiveness: f32,
    /// Focus detection sensitivity (0.0 = low, 1.0 = high)
    focus_sensitivity: f32,
    /// Preferred compression stage
    preferred_stage: String,
    /// Retrieval weight adjustments
    retrieval_weights: HashMap<String, f32>,
    /// Category preferences
    category_preferences: HashMap<String, f32>,
    /// Last update time
    last_updated: DateTime<Utc>,
}

impl Default for AdaptivePreferences {
    fn default() -> Self {
        Self {
            compression_aggressiveness: 0.5,
            focus_sensitivity: 0.7,
            preferred_stage: "RemoveLowPriority".to_string(),
            retrieval_weights: HashMap::new(),
            category_preferences: HashMap::new(),
            last_updated: Utc::now(),
        }
    }
}

/// Feedback statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackStats {
    /// Total compression feedback count
    compression_count: usize,
    /// Total focus feedback count
    focus_count: usize,
    /// Total retrieval feedback count
    retrieval_count: usize,
    /// Average compression rating
    avg_compression_rating: f32,
    /// Average focus rating
    avg_focus_rating: f32,
    /// Average retrieval rating
    avg_retrieval_rating: f32,
    /// Compression acceptance rate
    compression_accept_rate: f32,
    /// Focus accuracy rate
    focus_accuracy_rate: f32,
    /// Retrieval relevance rate
    retrieval_relevance_rate: f32,
}

impl Default for FeedbackStats {
    fn default() -> Self {
        Self {
            compression_count: 0,
            focus_count: 0,
            retrieval_count: 0,
            avg_compression_rating: 0.0,
            avg_focus_rating: 0.0,
            avg_retrieval_rating: 0.0,
            compression_accept_rate: 0.0,
            focus_accuracy_rate: 0.0,
            retrieval_relevance_rate: 0.0,
        }
    }
}

impl AdaptiveLearner {
    /// Create a new adaptive learner.
    pub fn new() -> Self {
        Self {
            compression_feedback: Vec::new(),
            focus_feedback: Vec::new(),
            retrieval_feedback: Vec::new(),
            preferences: AdaptivePreferences::default(),
            stats: FeedbackStats::default(),
        }
    }

    /// Record compression feedback.
    pub fn record_compression_feedback(
        &mut self,
        session_id: &str,
        original_tokens: u32,
        compressed_tokens: u32,
        stage: &str,
        rating: u8,
        accepted: bool,
        comments: Option<String>,
    ) {
        let feedback = CompressionFeedback {
            timestamp: Utc::now(),
            session_id: session_id.to_string(),
            original_tokens,
            compressed_tokens,
            stage: stage.to_string(),
            rating: rating.clamp(1, 5),
            comments,
            accepted,
        };

        self.compression_feedback.push(feedback);
        // Auto-prune to prevent unbounded growth
        if self.compression_feedback.len() > 100 {
            self.prune_old_feedback();
        }
        self.update_compression_preferences();
    }

    /// Record focus detection feedback.
    pub fn record_focus_feedback(
        &mut self,
        session_id: &str,
        focus_topic: &str,
        rating: u8,
        accurate: bool,
        suggested_correction: Option<String>,
    ) {
        let feedback = FocusFeedback {
            timestamp: Utc::now(),
            session_id: session_id.to_string(),
            focus_topic: focus_topic.to_string(),
            rating: rating.clamp(1, 5),
            accurate,
            suggested_correction,
        };

        self.focus_feedback.push(feedback);
        // Auto-prune to prevent unbounded growth
        if self.focus_feedback.len() > 100 {
            self.prune_old_feedback();
        }
        self.update_focus_preferences();
    }

    /// Record retrieval feedback.
    pub fn record_retrieval_feedback(
        &mut self,
        session_id: &str,
        memory_id: &str,
        memory_content: &str,
        rating: u8,
        relevant: bool,
        suggested_context: Option<String>,
    ) {
        // Truncate content for storage
        let truncated_content = if memory_content.len() > 100 {
            memory_content.chars().take(100).collect::<String>()
        } else {
            memory_content.to_string()
        };

        let feedback = RetrievalFeedback {
            timestamp: Utc::now(),
            session_id: session_id.to_string(),
            memory_id: memory_id.to_string(),
            memory_content: truncated_content,
            rating: rating.clamp(1, 5),
            relevant,
            suggested_context,
        };

        self.retrieval_feedback.push(feedback);
        // Auto-prune to prevent unbounded growth
        if self.retrieval_feedback.len() > 100 {
            self.prune_old_feedback();
        }
        self.update_retrieval_preferences();
    }

    /// Update compression preferences based on feedback.
    fn update_compression_preferences(&mut self) {
        if self.compression_feedback.len() < 5 {
            return; // Need at least 5 feedbacks
        }

        // Calculate acceptance rate by stage
        let stage_acceptance: HashMap<String, f32> = self
            .compression_feedback
            .iter()
            .fold(HashMap::new(), |mut acc, f| {
                let entry = acc.entry(f.stage.clone()).or_insert((0usize, 0usize));
                if f.accepted {
                    entry.0 += 1;
                }
                entry.1 += 1;
                acc
            })
            .iter()
            .map(|(stage, (accepted, total))| {
                (stage.clone(), *accepted as f32 / *total as f32)
            })
            .collect();

        // Find best performing stage
        let best_stage = stage_acceptance
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(s, _)| s.clone())
            .unwrap_or_else(|| "RemoveLowPriority".to_string());

        self.preferences.preferred_stage = best_stage;

        // Calculate average rating and acceptance rate
        let total_rating = self.compression_feedback.iter().map(|f| f.rating as f32).sum::<f32>();
        let total_accepted = self.compression_feedback.iter().filter(|f| f.accepted).count();

        self.stats.avg_compression_rating = total_rating / self.compression_feedback.len() as f32;
        self.stats.compression_accept_rate = total_accepted as f32 / self.compression_feedback.len() as f32;

        // Adjust aggressiveness based on ratings
        if self.stats.avg_compression_rating > 4.0 && self.stats.compression_accept_rate > 0.8 {
            // User likes aggressive compression
            self.preferences.compression_aggressiveness = 0.7;
        } else if self.stats.avg_compression_rating < 3.0 || self.stats.compression_accept_rate < 0.5 {
            // User prefers conservative compression
            self.preferences.compression_aggressiveness = 0.3;
        }

        self.preferences.last_updated = Utc::now();
        self.stats.compression_count = self.compression_feedback.len();
    }

    /// Update focus preferences based on feedback.
    fn update_focus_preferences(&mut self) {
        if self.focus_feedback.len() < 5 {
            return;
        }

        // Calculate accuracy rate
        let accurate_count = self.focus_feedback.iter().filter(|f| f.accurate).count();
        self.stats.focus_accuracy_rate = accurate_count as f32 / self.focus_feedback.len() as f32;

        // Calculate average rating
        let total_rating = self.focus_feedback.iter().map(|f| f.rating as f32).sum::<f32>();
        self.stats.avg_focus_rating = total_rating / self.focus_feedback.len() as f32;

        // Adjust sensitivity based on accuracy
        if self.stats.focus_accuracy_rate > 0.9 {
            // High accuracy, keep current sensitivity
            self.preferences.focus_sensitivity = 0.7;
        } else if self.stats.focus_accuracy_rate < 0.5 {
            // Low accuracy, increase sensitivity to detect more
            self.preferences.focus_sensitivity = 0.9;
        } else if self.stats.focus_accuracy_rate > 0.7 && self.stats.avg_focus_rating > 4.0 {
            // Good performance, reduce sensitivity to reduce noise
            self.preferences.focus_sensitivity = 0.5;
        }

        self.preferences.last_updated = Utc::now();
        self.stats.focus_count = self.focus_feedback.len();
    }

    /// Update retrieval preferences based on feedback.
    fn update_retrieval_preferences(&mut self) {
        if self.retrieval_feedback.len() < 5 {
            return;
        }

        // Calculate relevance rate
        let relevant_count = self.retrieval_feedback.iter().filter(|f| f.relevant).count();
        self.stats.retrieval_relevance_rate = relevant_count as f32 / self.retrieval_feedback.len() as f32;

        // Calculate average rating
        let total_rating = self.retrieval_feedback.iter().map(|f| f.rating as f32).sum::<f32>();
        self.stats.avg_retrieval_rating = total_rating / self.retrieval_feedback.len() as f32;

        // Adjust weights based on performance
        if self.stats.retrieval_relevance_rate < 0.5 {
            // Low relevance, increase focus weight
            self.preferences.retrieval_weights.insert("focus".to_string(), 0.35);
            self.preferences.retrieval_weights.insert("tfidf".to_string(), 0.25);
        } else if self.stats.retrieval_relevance_rate > 0.8 && self.stats.avg_retrieval_rating > 4.0 {
            // High performance, balance weights
            self.preferences.retrieval_weights.insert("focus".to_string(), 0.25);
            self.preferences.retrieval_weights.insert("tfidf".to_string(), 0.30);
        }

        self.preferences.last_updated = Utc::now();
        self.stats.retrieval_count = self.retrieval_feedback.len();
    }

    /// Get current preferences.
    pub fn get_preferences(&self) -> &AdaptivePreferences {
        &self.preferences
    }

    /// Get current statistics.
    pub fn get_stats(&self) -> &FeedbackStats {
        &self.stats
    }

    /// Get compression aggressiveness.
    pub fn get_compression_aggressiveness(&self) -> f32 {
        self.preferences.compression_aggressiveness
    }

    /// Get focus sensitivity.
    pub fn get_focus_sensitivity(&self) -> f32 {
        self.preferences.focus_sensitivity
    }

    /// Get preferred compression stage.
    pub fn get_preferred_stage(&self) -> &str {
        &self.preferences.preferred_stage
    }

    /// Get retrieval weight for a specific factor.
    pub fn get_retrieval_weight(&self, factor: &str) -> f32 {
        self.preferences.retrieval_weights.get(factor).copied().unwrap_or(0.25)
    }

    /// Export feedback history for analysis.
    pub fn export_feedback(&self) -> FeedbackExport {
        FeedbackExport {
            compression_feedback: self.compression_feedback.clone(),
            focus_feedback: self.focus_feedback.clone(),
            retrieval_feedback: self.retrieval_feedback.clone(),
            preferences: self.preferences.clone(),
            stats: self.stats.clone(),
        }
    }

    /// Import feedback history.
    pub fn import_feedback(&mut self, export: FeedbackExport) {
        self.compression_feedback = export.compression_feedback;
        self.focus_feedback = export.focus_feedback;
        self.retrieval_feedback = export.retrieval_feedback;
        self.preferences = export.preferences;
        self.stats = export.stats;

        // Re-apply learning
        self.update_compression_preferences();
        self.update_focus_preferences();
        self.update_retrieval_preferences();
    }

    /// Clear old feedback (keep last 100).
    pub fn prune_old_feedback(&mut self) {
        if self.compression_feedback.len() > 100 {
            let start = self.compression_feedback.len() - 100;
            self.compression_feedback.drain(..start);
        }
        if self.focus_feedback.len() > 100 {
            let start = self.focus_feedback.len() - 100;
            self.focus_feedback.drain(..start);
        }
        if self.retrieval_feedback.len() > 100 {
            let start = self.retrieval_feedback.len() - 100;
            self.retrieval_feedback.drain(..start);
        }
    }

    /// Generate adaptation report.
    pub fn generate_report(&self) -> String {
        let mut report = String::from("【自适应学习报告】\n\n");

        report.push_str(&format!(
            "压缩偏好:\n  激进程度: {:.0}%\n  首选阶段: {}\n  平均评分: {:.1}\n  接受率: {:.0}%\n\n",
            self.preferences.compression_aggressiveness * 100.0,
            self.preferences.preferred_stage,
            self.stats.avg_compression_rating,
            self.stats.compression_accept_rate * 100.0
        ));

        report.push_str(&format!(
            "聚焦检测:\n  灵敏度: {:.0}%\n  准确率: {:.0}%\n  平均评分: {:.1}\n\n",
            self.preferences.focus_sensitivity * 100.0,
            self.stats.focus_accuracy_rate * 100.0,
            self.stats.avg_focus_rating
        ));

        report.push_str(&format!(
            "记忆检索:\n  相关率: {:.0}%\n  平均评分: {:.1}\n\n",
            self.stats.retrieval_relevance_rate * 100.0,
            self.stats.avg_retrieval_rating
        ));

        report.push_str(&format!(
            "反馈统计:\n  压缩反馈: {} 条\n  聚焦反馈: {} 条\n  检索反馈: {} 条\n",
            self.stats.compression_count,
            self.stats.focus_count,
            self.stats.retrieval_count
        ));

        report
    }
}

impl Default for AdaptiveLearner {
    fn default() -> Self {
        Self::new()
    }
}

/// Exportable feedback data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackExport {
    compression_feedback: Vec<CompressionFeedback>,
    focus_feedback: Vec<FocusFeedback>,
    retrieval_feedback: Vec<RetrievalFeedback>,
    preferences: AdaptivePreferences,
    stats: FeedbackStats,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adaptive_learner_creation() {
        let learner = AdaptiveLearner::new();
        assert_eq!(learner.preferences.compression_aggressiveness, 0.5);
    }

    #[test]
    fn test_compression_feedback_recording() {
        let mut learner = AdaptiveLearner::new();
        learner.record_compression_feedback(
            "test-session",
            10000,
            8000,
            "RemoveLowPriority",
            4,
            true,
            None,
        );
        assert_eq!(learner.compression_feedback.len(), 1);
    }

    #[test]
    fn test_focus_feedback_recording() {
        let mut learner = AdaptiveLearner::new();
        learner.record_focus_feedback(
            "test-session",
            "database optimization",
            5,
            true,
            None,
        );
        assert_eq!(learner.focus_feedback.len(), 1);
    }

    #[test]
    fn test_retrieval_feedback_recording() {
        let mut learner = AdaptiveLearner::new();
        learner.record_retrieval_feedback(
            "test-session",
            "memory-123",
            "Test memory content",
            4,
            true,
            None,
        );
        assert_eq!(learner.retrieval_feedback.len(), 1);
    }

    #[test]
    fn test_preferences_default() {
        let prefs = AdaptivePreferences::default();
        assert_eq!(prefs.compression_aggressiveness, 0.5);
        assert_eq!(prefs.focus_sensitivity, 0.7);
    }

    #[test]
    fn test_feedback_pruning() {
        let mut learner = AdaptiveLearner::new();
        
        // Add 150 feedbacks
        for i in 0..150 {
            learner.record_compression_feedback(
                &format!("session-{}", i),
                10000,
                8000,
                "RemoveLowPriority",
                4,
                true,
                None,
            );
        }
        
        learner.prune_old_feedback();
        assert_eq!(learner.compression_feedback.len(), 100);
    }

    #[test]
    fn test_report_generation() {
        let learner = AdaptiveLearner::new();
        let report = learner.generate_report();
        assert!(report.contains("自适应学习报告"));
    }
}