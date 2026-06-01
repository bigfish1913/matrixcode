//! Integrated Long Context Processor.
//!
//! This module integrates all long-context processing modules into a unified workflow:
//! - UnifiedExtractor: One-time extraction of all information
//! - AiFocusTracker: AI-driven focus detection and analysis
//! - CoherenceDetector: Semantic coherence segmentation
//! - ProgressiveCompressor: Segment-based compression
//! - PatternRegistry: Pattern learning from conversations

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::memory::{PatternRegistry, UnifiedExtractor, UnifiedExtractionResult, FocusDecision, FocusType as MemoryFocusType};
use crate::providers::{Message, Provider};
use crate::compress::{
    CoherenceDetector, ConversationFocus,
    FocusTracker, ProgressiveCompressor, FocusManager, FocusPoint, FocusStatus,
    TopicTransition, FocusType,
    hardcode_config::HardcodeConfig,
};

/// Configuration for the integrated processor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessorConfig {
    /// Coherence threshold (segments above this are kept intact).
    pub coherence_threshold: f32,
    /// Focus score threshold (segments above this are prioritized).
    pub focus_threshold: f32,
    /// Target token ratio after compression.
    pub target_ratio: f32,
    /// Whether to auto-learn patterns from conversations.
    pub auto_learn: bool,
    /// Maximum tokens before triggering compression.
    pub max_tokens_before_compression: u32,
    /// Number of recent messages to always preserve.
    pub preserve_last_n: usize,
    /// Whether to inject focus message into compressed output.
    pub inject_focus_message: bool,
}

impl Default for ProcessorConfig {
    fn default() -> Self {
        Self {
            coherence_threshold: 0.7,
            focus_threshold: 0.5,
            target_ratio: 0.6,
            auto_learn: true,
            max_tokens_before_compression: 12000,
            preserve_last_n: 3,
            inject_focus_message: true,
        }
    }
}

impl ProcessorConfig {
    /// Create config for simple conversations.
    pub fn simple_conversation() -> Self {
        Self {
            coherence_threshold: 0.6,
            focus_threshold: 0.4,
            target_ratio: 0.5,
            auto_learn: true,
            max_tokens_before_compression: 8000,
            preserve_last_n: 2,
            inject_focus_message: true,
        }
    }

    /// Create config for complex technical discussions.
    pub fn complex_technical() -> Self {
        Self {
            coherence_threshold: 0.8,
            focus_threshold: 0.6,
            target_ratio: 0.7,
            auto_learn: true,
            max_tokens_before_compression: 20000,
            preserve_last_n: 5,
            inject_focus_message: true,
        }
    }

    /// Create config from hardcode config.
    pub fn from_hardcode(hardcode: &HardcodeConfig) -> Self {
        Self {
            coherence_threshold: 0.7,
            focus_threshold: 0.5,
            target_ratio: 0.6,
            auto_learn: true,
            max_tokens_before_compression: 12000,
            preserve_last_n: hardcode.max_recent_context_count,
            inject_focus_message: true,
        }
    }

    /// Validate configuration.
    pub fn validate(&self) -> bool {
        self.coherence_threshold > 0.0
            && self.coherence_threshold <= 1.0
            && self.focus_threshold > 0.0
            && self.focus_threshold <= 1.0
            && self.target_ratio > 0.0
            && self.target_ratio <= 1.0
            && self.max_tokens_before_compression > 0
            && self.preserve_last_n > 0
    }
}

/// Integrated long context processor that orchestrates all modules.
///
/// This processor coordinates:
/// 1. Unified extraction (one AI call for all information)
/// 2. Rule-based focus tracking
/// 3. Coherence-based segmentation
/// 4. Progressive compression with focus priority
/// 5. Focus message injection
/// 6. Pattern learning
///
/// Note: This processor is fully implemented and tested but not yet integrated
/// into the main agent loop. Reserved for future use.
pub struct IntegratedLongContextProcessor {
    /// Unified extractor for one-time extraction.
    unified_extractor: UnifiedExtractor,
    /// Pattern registry for learning patterns.
    pattern_registry: PatternRegistry,
    /// Rule-based focus tracker.
    focus_tracker: FocusTracker,
    /// Focus manager for multi-focus tracking and selection.
    focus_manager: FocusManager,
    /// Coherence detector for segmentation.
    coherence_detector: CoherenceDetector,
    /// Progressive compressor for compression.
    progressive_compressor: ProgressiveCompressor,
    /// Processor configuration.
    config: ProcessorConfig,
    /// Hardcode configuration.
    hardcode_config: HardcodeConfig,
}

impl IntegratedLongContextProcessor {
    /// Create a new integrated processor.
    ///
    /// # Arguments
    /// * `provider` - Provider for AI extraction.
    /// * `model` - Model name for extraction.
    /// * `config` - Processor configuration.
    pub fn new(provider: Box<dyn Provider>, model: String, config: ProcessorConfig) -> Self {
        let hardcode_config = HardcodeConfig::default();

        Self {
            unified_extractor: UnifiedExtractor::new(provider, model),
            pattern_registry: PatternRegistry::new(),
            focus_tracker: FocusTracker::new(),
            focus_manager: FocusManager::new(),
            coherence_detector: CoherenceDetector::new_with_registry(
                config.coherence_threshold,
                PatternRegistry::new(),
            ),
            progressive_compressor: ProgressiveCompressor::default_config(),
            config,
            hardcode_config,
        }
    }

    /// Create with default configuration.
    pub fn with_defaults(provider: Box<dyn Provider>, model: String) -> Self {
        Self::new(provider, model, ProcessorConfig::default())
    }

    /// Create for simple conversations.
    pub fn for_simple_conversation(provider: Box<dyn Provider>, model: String) -> Self {
        Self::new(provider, model, ProcessorConfig::simple_conversation())
    }

    /// Create for complex technical discussions.
    pub fn for_complex_technical(provider: Box<dyn Provider>, model: String) -> Self {
        Self::new(provider, model, ProcessorConfig::complex_technical())
    }

    /// Create without AI focus tracking (rule-based only).
    pub fn without_ai_focus(provider: Box<dyn Provider>, model: String) -> Self {
        let hardcode_config = HardcodeConfig::default();

        Self {
            unified_extractor: UnifiedExtractor::new(provider, model),
            pattern_registry: PatternRegistry::new(),
            focus_tracker: FocusTracker::new(),
            focus_manager: FocusManager::new(),
            coherence_detector: CoherenceDetector::new_with_registry(
                ProcessorConfig::default().coherence_threshold,
                PatternRegistry::new(),
            ),
            progressive_compressor: ProgressiveCompressor::default_config(),
            config: ProcessorConfig::default(),
            hardcode_config,
        }
    }

    /// Set custom hardcode config.
    pub fn with_hardcode_config(mut self, config: HardcodeConfig) -> Self {
        self.hardcode_config = config.clone();
        self.progressive_compressor = ProgressiveCompressor::default_config()
            .with_hardcode_config(config);
        self
    }

    /// Get configuration reference.
    pub fn config(&self) -> &ProcessorConfig {
        &self.config
    }

    /// Get mutable configuration reference.
    pub fn config_mut(&mut self) -> &mut ProcessorConfig {
        &mut self.config
    }

    /// Get pattern registry reference.
    pub fn pattern_registry(&self) -> &PatternRegistry {
        &self.pattern_registry
    }

    /// Get mutable pattern registry reference.
    pub fn pattern_registry_mut(&mut self) -> &mut PatternRegistry {
        &mut self.pattern_registry
    }

    /// Process long context with the integrated workflow.
    ///
    /// # Workflow
    /// 1. Get existing focuses from FocusManager
    /// 2. Extract all information with focus selection (one AI call)
    /// 3. Update FocusManager based on AI's focus_decision
    /// 4. Segment messages by coherence
    /// 5. Compress segments with focus priority
    /// 6. Inject focus message
    /// 7. Learn patterns
    ///
    /// # Arguments
    /// * `messages` - Messages to process.
    /// * `session_id` - Optional session ID.
    /// * `project_path` - Optional project path.
    ///
    /// # Returns
    /// Processed messages and extraction result.
    pub async fn process(
        &mut self,
        messages: Vec<Message>,
        session_id: Option<&str>,
        project_path: Option<&str>,
    ) -> Result<ProcessedResult> {
        if messages.is_empty() {
            return Ok(ProcessedResult {
                messages: messages,
                extraction: UnifiedExtractionResult::default(),
                focus: ConversationFocus {
                    current_topic: None,
                    current_question: None,
                    recent_context: Vec::new(),
                    topic_transitions: Vec::new(),
                    detected_at: 0,
                },
                segments_count: 0,
                compression_ratio: 1.0,
            });
        }

        // Step 1: Get existing focuses from FocusManager for AI selection
        let existing_foci: Vec<(&str, &str, &[String])> = self.focus_manager
            .foci
            .iter()
            .filter(|(_, f)| f.status == FocusStatus::Active)
            .map(|(id, f)| {
                (id.as_str(), f.topic.as_str(), f.keywords.as_slice())
            })
            .collect();

        // Step 2: One-time extraction with focus selection
        let text = self.format_messages(&messages);
        let extraction = self.unified_extractor.extract_unified_with_foci(
            &text,
            &existing_foci,
            session_id,
            project_path,
        ).await?;

        // Step 3: Update FocusManager based on AI's focus_decision
        let focus = if let Some(decision) = &extraction.focus_decision {
            self.update_focus_from_decision(decision, messages.len().saturating_sub(1))
        } else {
            // Fallback: rule-based focus detection
            self.focus_tracker.set_current_keywords(&extraction.focus_keywords);
            self.focus_tracker.detect_focus(&messages)
        };

        // Step 4: Update coherence detector with pattern registry
        self.coherence_detector = CoherenceDetector::new_with_registry(
            self.config.coherence_threshold,
            self.pattern_registry.clone(),
        );

        // Step 5: Segment messages by coherence
        let segments = self.coherence_detector.segment_messages(&messages);
        let segments_count = segments.len();

        // Step 6: Compress segments with focus priority
        let compressed_messages = self.compress_segments_with_focus(segments, &focus)?;

        // Step 7: Inject focus message if configured
        let final_messages = if self.config.inject_focus_message {
            self.inject_focus_message(compressed_messages, &focus)
        } else {
            compressed_messages
        };

        // Step 8: Learn patterns from extraction
        if self.config.auto_learn {
            self.pattern_registry.learn_patterns(&extraction.conversation_patterns);
            // Save patterns to file (optional, may fail silently)
            if let Err(e) = self.pattern_registry.save_to_default_file() {
                log::warn!("Failed to save patterns: {}", e);
            }
        }

        // Calculate compression ratio
        let original_tokens = estimate_tokens(&messages);
        let final_tokens = estimate_tokens(&final_messages);
        let compression_ratio = if original_tokens > 0 {
            final_tokens as f32 / original_tokens as f32
        } else {
            1.0
        };

        Ok(ProcessedResult {
            messages: final_messages,
            extraction,
            focus,
            segments_count,
            compression_ratio,
        })
    }

    /// Update FocusManager based on AI's focus decision.
    ///
    /// This method handles:
    /// - Selecting an existing focus (switch_focus)
    /// - Creating a new focus (add_focus)
    /// - Recording topic transitions
    fn update_focus_from_decision(
        &mut self,
        decision: &FocusDecision,
        message_index: usize,
    ) -> ConversationFocus {
        use chrono::Utc;

        // Handle focus selection/creation
        if decision.need_new_focus {
            // Create new focus
            let new_focus = FocusPoint::new_with_ai(
                format!("focus-{}", Utc::now().timestamp()),
                decision.new_focus_topic.clone().unwrap_or_else(|| "未知话题".to_string()),
                decision.focus_keywords.clone(),
                decision.related_entities.clone(),
                decision.new_core_question.clone(),
                None, // semantic_summary
                Vec::new(), // related_files
                decision.confidence,
                match decision.focus_type {
                    MemoryFocusType::ProblemSolving => FocusType::ProblemSolving,
                    MemoryFocusType::TaskExecution => FocusType::TaskExecution,
                    MemoryFocusType::KnowledgeExploration => FocusType::KnowledgeExploration,
                    MemoryFocusType::DecisionMaking => FocusType::DecisionMaking,
                    MemoryFocusType::CodeOptimization => FocusType::CodeOptimization,
                    MemoryFocusType::General => FocusType::General,
                },
                message_index,
            );

            self.focus_manager.add_focus(new_focus);

            log::info!(
                "Created new focus: topic={}, confidence={}, reasoning={}",
                decision.new_focus_topic.as_ref().unwrap_or(&"unknown".to_string()),
                decision.confidence,
                decision.reasoning
            );
        } else if let Some(focus_id) = &decision.selected_focus_id {
            // Switch to existing focus
            self.focus_manager.switch_focus(focus_id);

            // Update focus keywords
            if let Some(focus) = self.focus_manager.current_focus_mut() {
                focus.add_keywords(&decision.focus_keywords);
                focus.update_message_range(message_index);
                focus.wake_up();
            }

            log::info!(
                "Selected existing focus: id={}, confidence={}, reasoning={}",
                focus_id,
                decision.confidence,
                decision.reasoning
            );
        }

        // Handle topic transition recording
        if decision.is_topic_switch && decision.previous_focus_id.is_some() {
            // Clone values before mutable borrow
            let prev_id = decision.previous_focus_id.clone();
            let curr_id = self.focus_manager.current_focus_id.clone();

            if let (Some(prev), Some(curr)) = (prev_id, curr_id) {
                self.focus_manager.add_focus_transition(&prev, &curr, decision.confidence);
            }
        }

        // Convert FocusManager state to ConversationFocus for compression
        self.focus_manager.current_focus()
            .map(|f| ConversationFocus {
                current_topic: Some(f.topic.clone()),
                current_question: f.core_question.clone(),
                recent_context: f.keywords.iter().take(5).cloned().collect(),
                topic_transitions: self.focus_manager.focus_history.iter()
                    .skip(1) // Skip first (initial focus)
                    .filter_map(|id| {
                        self.focus_manager.foci.get(id).and_then(|f| {
                            f.feedback_history.iter().rev().find(|fb| {
                                fb.feedback_type == crate::compress::FocusFeedbackType::AutoSwitched
                            }).map(|_| TopicTransition {
                                from_topic: f.topic.clone(), // Approximation
                                to_topic: f.topic.clone(),
                                message_index: f.message_range.start,
                                transition_keyword: "AI detected".to_string(),
                            })
                        })
                    })
                    .collect(),
                detected_at: message_index,
            })
            .unwrap_or_else(|| ConversationFocus {
                current_topic: decision.new_focus_topic.clone(),
                current_question: decision.new_core_question.clone(),
                recent_context: decision.focus_keywords.clone(),
                topic_transitions: Vec::new(),
                detected_at: message_index,
            })
    }

    /// Process with a provider reference (for progressive compressor).
    ///
    /// This variant allows using a provider for AI-based compression stages.
    pub async fn process_with_provider(
        &mut self,
        messages: Vec<Message>,
        provider: Option<&dyn Provider>,
        session_id: Option<&str>,
        project_path: Option<&str>,
    ) -> Result<ProcessedResult> {
        if messages.is_empty() {
            return Ok(ProcessedResult {
                messages: messages,
                extraction: UnifiedExtractionResult::default(),
                focus: ConversationFocus {
                    current_topic: None,
                    current_question: None,
                    recent_context: Vec::new(),
                    topic_transitions: Vec::new(),
                    detected_at: 0,
                },
                segments_count: 0,
                compression_ratio: 1.0,
            });
        }

        // Step 1: One-time extraction
        let text = self.format_messages(&messages);
        let extraction = self.unified_extractor.extract_unified(
            &text,
            session_id,
            project_path,
        ).await?;

        // Step 2: Set keywords
        self.focus_tracker.set_current_keywords(&extraction.focus_keywords);

        // Step 3: Detect focus
        let focus = self.focus_tracker.detect_focus(&messages);

        // Step 4: Segment by coherence
        self.coherence_detector = CoherenceDetector::new_with_registry(
            self.config.coherence_threshold,
            self.pattern_registry.clone(),
        );
        let segments = self.coherence_detector.segment_messages(&messages);
        let segments_count = segments.len();

        // Step 5: Check if compression is needed
        let current_tokens = estimate_tokens(&messages);
        let _target_tokens = (self.config.max_tokens_before_compression as f32 * self.config.target_ratio) as u32;

        let compressed_messages = if current_tokens > self.config.max_tokens_before_compression {
            // Compress using progressive compressor with provider
            self.progressive_compressor.set_focus_manager(
                crate::compress::focus_point::FocusManager::new()
            );

            // Use progressive compression
            self.progressive_compressor.compress(&messages, provider).await?
        } else {
            // Just apply segment-based focus prioritization
            self.compress_segments_with_focus(segments, &focus)?
        };

        // Step 6: Inject focus message
        let final_messages = if self.config.inject_focus_message {
            self.inject_focus_message(compressed_messages, &focus)
        } else {
            compressed_messages
        };

        // Step 7: Learn patterns
        if self.config.auto_learn {
            self.pattern_registry.learn_patterns(&extraction.conversation_patterns);
            if let Err(e) = self.pattern_registry.save_to_default_file() {
                log::warn!("Failed to save patterns: {}", e);
            }
        }

        // Calculate ratio
        let final_tokens = estimate_tokens(&final_messages);
        let compression_ratio = if current_tokens > 0 {
            final_tokens as f32 / current_tokens as f32
        } else {
            1.0
        };

        Ok(ProcessedResult {
            messages: final_messages,
            extraction,
            focus,
            segments_count,
            compression_ratio,
        })
    }

    /// Compress segments while considering focus priority.
    ///
    /// High coherence + high focus relevance = preserve intact.
    /// Low coherence or low focus = compress more aggressively.
    fn compress_segments_with_focus(
        &self,
        segments: Vec<Vec<Message>>,
        focus: &ConversationFocus,
    ) -> Result<Vec<Message>> {
        let mut result = Vec::new();

        for segment in segments {
            // Calculate coherence score for this segment
            let coherence_score = self.coherence_detector.calculate_coherence(&segment);

            // Calculate focus score for this segment
            let focus_score = self.calculate_segment_focus_score(&segment, focus);

            // Decision logic:
            // - High coherence (> threshold) + High focus (> threshold) = preserve intact
            // - High coherence + Low focus = preserve but may summarize
            // - Low coherence + High focus = keep key messages
            // - Low coherence + Low focus = aggressive compression

            if coherence_score >= self.config.coherence_threshold && focus_score >= self.config.focus_threshold {
                // High coherence + High focus: preserve intact
                log::debug!(
                    "Segment preserved intact: coherence={}, focus={}",
                    coherence_score, focus_score
                );
                result.extend(segment);
            } else if coherence_score >= self.config.coherence_threshold {
                // High coherence but lower focus: preserve mostly intact
                // Keep first and last messages, summarize middle if large
                if segment.len() <= 3 {
                    result.extend(segment);
                } else {
                    // Keep first and last, compress middle
                    result.push(segment[0].clone());
                    let middle = &segment[1..segment.len() - 1];
                    let summary = self.create_segment_summary(middle);
                    if let Some(summary_msg) = summary {
                        result.push(summary_msg);
                    }
                    result.push(segment[segment.len() - 1].clone());
                }
            } else if focus_score >= self.config.focus_threshold {
                // Low coherence but high focus: keep key messages
                for (i, msg) in segment.iter().enumerate() {
                    let msg_focus_score = self.focus_tracker.focus_score(msg, focus);
                    if msg_focus_score > self.config.focus_threshold * 0.5 || i == 0 || i == segment.len() - 1 {
                        result.push(msg.clone());
                    }
                }
            } else {
                // Low coherence + Low focus: aggressive compression
                // Create a single summary for the segment
                if segment.len() > 1 {
                    let summary = self.create_segment_summary(&segment);
                    if let Some(summary_msg) = summary {
                        result.push(summary_msg);
                    } else {
                        // Fallback: keep last message only
                        result.push(segment[segment.len() - 1].clone());
                    }
                } else {
                    result.extend(segment);
                }
            }
        }

        Ok(result)
    }

    /// Calculate focus score for a segment of messages.
    fn calculate_segment_focus_score(&self, segment: &[Message], focus: &ConversationFocus) -> f32 {
        if segment.is_empty() {
            return 0.0;
        }

        let mut total_score = 0.0;
        for msg in segment {
            total_score += self.focus_tracker.focus_score(msg, focus);
        }

        // Average score weighted by segment size
        total_score / segment.len() as f32
    }

    /// Create a summary message for a segment.
    fn create_segment_summary(&self, messages: &[Message]) -> Option<Message> {
        if messages.is_empty() {
            return None;
        }

        // Simple inline summary: concatenate key points
        let mut key_points = Vec::new();
        for msg in messages {
            if let Some(point) = self.extract_key_point(msg) {
                key_points.push(point);
            }
        }

        if key_points.is_empty() {
            return None;
        }

        // Limit summary length
        let summary_text = if key_points.len() > 3 {
            format!("[摘要] {} ...", key_points[..3].join(" | "))
        } else {
            format!("[摘要] {}", key_points.join(" | "))
        };

        Some(Message {
            role: crate::providers::Role::Assistant,
            content: crate::providers::MessageContent::Text(summary_text),
        })
    }

    /// Extract key point from a message.
    fn extract_key_point(&self, message: &Message) -> Option<String> {
        let text = match &message.content {
            crate::providers::MessageContent::Text(t) => t.clone(),
            crate::providers::MessageContent::Blocks(blocks) => {
                blocks.iter()
                    .filter_map(|b| {
                        if let crate::providers::ContentBlock::Text { text } = b {
                            Some(text.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            }
        };

        // Extract first sentence
        let sentence = text
            .split(|c| c == '.' || c == '。' || c == '\n')
            .next()
            .map(|s| s.trim().to_string())?;

        if sentence.len() > self.hardcode_config.min_substantial_text_length {
            Some(sentence)
        } else {
            None
        }
    }

    /// Inject focus message into the message list.
    ///
    /// The focus message is inserted after system messages to provide
    /// context about the current conversation focus.
    /// If a focus message already exists, it will be replaced (not duplicated).
    fn inject_focus_message(&self, messages: Vec<Message>, focus: &ConversationFocus) -> Vec<Message> {
        let focus_msg = self.focus_tracker.create_focus_message(focus);

        // Check if a focus message already exists (System message containing "焦点" or "Focus")
        let existing_focus_pos = messages.iter().position(|m| {
            if matches!(m.role, crate::providers::Role::System) {
                match &m.content {
                    crate::providers::MessageContent::Text(t) => {
                        t.contains("焦点") || t.contains("Focus") || t.contains("【焦点上下文】")
                    }
                    _ => false
                }
            } else {
                false
            }
        });

        let mut result = messages;

        if let Some(pos) = existing_focus_pos {
            // Replace existing focus message
            result[pos] = focus_msg;
            log::debug!("Focus message replaced at position {}", pos);
        } else {
            // Insert new focus message after system messages
            let insert_pos = result.iter()
                .position(|m| !matches!(m.role, crate::providers::Role::System))
                .unwrap_or(0);

            result.insert(insert_pos, focus_msg);
            log::debug!("Focus message injected at position {}", insert_pos);
        }

        result
    }

    /// Format messages for extraction.
    fn format_messages(&self, messages: &[Message]) -> String {
        messages.iter()
            .map(|m| {
                let role = match m.role {
                    crate::providers::Role::User => "User",
                    crate::providers::Role::Assistant => "Assistant",
                    crate::providers::Role::System => "System",
                    crate::providers::Role::Tool => "Tool",
                };
                let content = match &m.content {
                    crate::providers::MessageContent::Text(t) => t.clone(),
                    crate::providers::MessageContent::Blocks(blocks) => {
                        blocks.iter()
                            .filter_map(|b| {
                                if let crate::providers::ContentBlock::Text { text } = b {
                                    Some(text.clone())
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    }
                };
                format!("{}: {}", role, content)
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Quick process without AI extraction (for testing or fallback).
    ///
    /// This skips extraction step and uses only rule-based processing.
    pub fn quick_process(&mut self, messages: Vec<Message>) -> Result<ProcessedResult> {
        if messages.is_empty() {
            return Ok(ProcessedResult {
                messages: messages,
                extraction: UnifiedExtractionResult::default(),
                focus: ConversationFocus {
                    current_topic: None,
                    current_question: None,
                    recent_context: Vec::new(),
                    topic_transitions: Vec::new(),
                    detected_at: 0,
                },
                segments_count: 0,
                compression_ratio: 1.0,
            });
        }

        // Detect focus without keywords (uses fallback presets)
        let focus = self.focus_tracker.detect_focus(&messages);

        // Segment by coherence
        self.coherence_detector = CoherenceDetector::new_with_registry(
            self.config.coherence_threshold,
            self.pattern_registry.clone(),
        );
        let segments = self.coherence_detector.segment_messages(&messages);
        let segments_count = segments.len();

        // Compress segments
        let compressed = self.compress_segments_with_focus(segments, &focus)?;

        // Inject focus message
        let final_messages = if self.config.inject_focus_message {
            self.inject_focus_message(compressed, &focus)
        } else {
            compressed
        };

        // Calculate ratio
        let original_tokens = estimate_tokens(&messages);
        let final_tokens = estimate_tokens(&final_messages);
        let compression_ratio = if original_tokens > 0 {
            final_tokens as f32 / original_tokens as f32
        } else {
            1.0
        };

        Ok(ProcessedResult {
            messages: final_messages,
            extraction: UnifiedExtractionResult::default(),
            focus,
            segments_count,
            compression_ratio,
        })
    }
}

/// Result of processing long context.
#[derive(Debug, Clone)]
pub struct ProcessedResult {
    /// Processed messages.
    pub messages: Vec<Message>,
    /// Extraction result (memories, focus points, patterns, keywords).
    pub extraction: UnifiedExtractionResult,
    /// Detected conversation focus.
    pub focus: ConversationFocus,
    /// Number of segments identified.
    pub segments_count: usize,
    /// Compression ratio (final / original tokens).
    pub compression_ratio: f32,
}

impl ProcessedResult {
    /// Check if compression was applied.
    pub fn was_compressed(&self) -> bool {
        self.compression_ratio < 1.0
    }

    /// Get compression savings percentage.
    pub fn savings_percentage(&self) -> f32 {
        (1.0 - self.compression_ratio) * 100.0
    }

    /// Get extracted memories count.
    pub fn memories_count(&self) -> usize {
        self.extraction.memories.len()
    }

    /// Get extracted patterns count.
    pub fn patterns_count(&self) -> usize {
        self.extraction.conversation_patterns.len()
    }
}

/// Estimate tokens in messages (simple approximation).
fn estimate_tokens(messages: &[Message]) -> u32 {
    messages.iter()
        .map(|m| {
            let content = match &m.content {
                crate::providers::MessageContent::Text(text) => text.clone(),
                crate::providers::MessageContent::Blocks(blocks) => {
                    blocks.iter()
                        .filter_map(|b| {
                            if let crate::providers::ContentBlock::Text { text } = b {
                                Some(text.clone())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                }
            };
            // Rough estimation: 4 chars per token for English, 2 for Chinese
            // Use average of 3 chars per token
            (content.len() / 3) as u32 + 50 // +50 for metadata
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::{Message, MessageContent, Role};

    fn create_text_message(role: Role, text: &str) -> Message {
        Message {
            role,
            content: MessageContent::Text(text.to_string()),
        }
    }

    #[test]
    fn test_processor_config_default() {
        let config = ProcessorConfig::default();
        assert!(config.validate());
        assert_eq!(config.coherence_threshold, 0.7);
        assert_eq!(config.focus_threshold, 0.5);
        assert_eq!(config.target_ratio, 0.6);
    }

    #[test]
    fn test_processor_config_simple() {
        let config = ProcessorConfig::simple_conversation();
        assert!(config.validate());
        assert!(config.coherence_threshold < ProcessorConfig::default().coherence_threshold);
    }

    #[test]
    fn test_processor_config_complex() {
        let config = ProcessorConfig::complex_technical();
        assert!(config.validate());
        assert!(config.coherence_threshold > ProcessorConfig::default().coherence_threshold);
    }

    #[test]
    fn test_estimate_tokens() {
        let messages = vec![
            create_text_message(Role::User, "This is a test message"),
        ];
        let tokens = estimate_tokens(&messages);
        assert!(tokens > 0);
    }

    #[test]
    fn test_estimate_tokens_empty() {
        let messages: Vec<Message> = vec![];
        let tokens = estimate_tokens(&messages);
        assert_eq!(tokens, 0);
    }

    #[test]
    fn test_estimate_tokens_long() {
        let long_text = "x".repeat(1000);
        let messages = vec![
            create_text_message(Role::User, &long_text),
        ];
        let tokens = estimate_tokens(&messages);
        assert!(tokens > 300); // ~333 tokens for 1000 chars + 50 metadata
    }

    #[test]
    fn test_processed_result_was_compressed() {
        let result = ProcessedResult {
            messages: vec![],
            extraction: UnifiedExtractionResult::default(),
            focus: ConversationFocus {
                current_topic: None,
                current_question: None,
                recent_context: Vec::new(),
                topic_transitions: Vec::new(),
                detected_at: 0,
            },
            segments_count: 0,
            compression_ratio: 0.7,
        };
        assert!(result.was_compressed());
        // Use approximate comparison for floating point
        assert!((result.savings_percentage() - 30.0).abs() < 0.01);
    }

    #[test]
    fn test_processed_result_no_compression() {
        let result = ProcessedResult {
            messages: vec![],
            extraction: UnifiedExtractionResult::default(),
            focus: ConversationFocus {
                current_topic: None,
                current_question: None,
                recent_context: Vec::new(),
                topic_transitions: Vec::new(),
                detected_at: 0,
            },
            segments_count: 0,
            compression_ratio: 1.0,
        };
        assert!(!result.was_compressed());
        assert_eq!(result.savings_percentage(), 0.0);
    }

    #[test]
    fn test_processor_config_from_hardcode() {
        let hardcode = HardcodeConfig::complex_technical();
        let config = ProcessorConfig::from_hardcode(&hardcode);
        assert!(config.validate());
        assert_eq!(config.preserve_last_n, hardcode.max_recent_context_count);
    }

    #[test]
    fn test_quick_process_empty() {
        let mut processor = create_test_processor();
        let result = processor.quick_process(vec![]).unwrap();
        assert!(result.messages.is_empty());
        assert_eq!(result.compression_ratio, 1.0);
    }

    #[test]
    fn test_quick_process_single_message() {
        let mut processor = create_test_processor();
        let messages = vec![
            create_text_message(Role::User, "Test message"),
        ];
        let result = processor.quick_process(messages).unwrap();
        assert!(!result.messages.is_empty());
    }

    #[test]
    fn test_calculate_segment_focus_score_empty() {
        let processor = create_test_processor();
        let focus = ConversationFocus {
            current_topic: None,
            current_question: None,
            recent_context: Vec::new(),
            topic_transitions: Vec::new(),
            detected_at: 0,
        };
        let score = processor.calculate_segment_focus_score(&[], &focus);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_inject_focus_message() {
        let processor = create_test_processor();
        let focus = ConversationFocus {
            current_topic: Some("Testing".to_string()),
            current_question: Some("How to test?".to_string()),
            recent_context: vec!["Context 1".to_string()],
            topic_transitions: Vec::new(),
            detected_at: 0,
        };
        let messages = vec![
            create_text_message(Role::User, "First message"),
            create_text_message(Role::Assistant, "Response"),
        ];
        let result = processor.inject_focus_message(messages, &focus);
        assert_eq!(result.len(), 3);
        // Focus message should be first (no system messages)
        assert!(matches!(result[0].role, Role::System));
    }

    fn create_test_processor() -> IntegratedLongContextProcessor {
        // Create a minimal processor for testing
        let config = ProcessorConfig::default();
        let hardcode_config = HardcodeConfig::default();

        IntegratedLongContextProcessor {
            unified_extractor: UnifiedExtractor::new_minimal("test-model".to_string()),
            pattern_registry: PatternRegistry::new(),
            focus_tracker: FocusTracker::new(),
            focus_manager: FocusManager::new(),
            coherence_detector: CoherenceDetector::new(config.coherence_threshold),
            progressive_compressor: ProgressiveCompressor::default_config(),
            config,
            hardcode_config,
        }
    }
}