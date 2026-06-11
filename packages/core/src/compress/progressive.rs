//! Progressive Compression Strategy: Multi-stage compression for optimal token management.
//!
//! This module implements a progressive compression approach that applies
//! compression in stages, preserving more information when possible.

use crate::providers::Message;
use crate::compress::CoherenceDetector;
use crate::compress::ConversationFocus;
use crate::compress::complexity::{ComplexityAnalyzer, ComplexityLevel};
use crate::compress::focus_point::{FocusManager};
use crate::compress::hardcode_config::HardcodeConfig;
use anyhow::Result;

/// Progressive compression stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionStage {
    /// Stage 1: Remove low-priority messages (greetings, simple questions)
    RemoveLowPriority,
    /// Stage 2: Summarize medium-priority messages
    SummarizeMedium,
    /// Stage 3: Compress high-priority but verbose messages
    CompressHighPriority,
    /// Stage 4: Emergency compression (aggressive summarization)
    EmergencyCompression,
}

impl CompressionStage {
    /// Get stage name.
    pub fn name(&self) -> &str {
        match self {
            CompressionStage::RemoveLowPriority => "Remove Low Priority",
            CompressionStage::SummarizeMedium => "Summarize Medium",
            CompressionStage::CompressHighPriority => "Compress High Priority",
            CompressionStage::EmergencyCompression => "Emergency Compression",
        }
    }

    /// Get stage priority (higher = more aggressive).
    pub fn priority(&self) -> u8 {
        match self {
            CompressionStage::RemoveLowPriority => 1,
            CompressionStage::SummarizeMedium => 2,
            CompressionStage::CompressHighPriority => 3,
            CompressionStage::EmergencyCompression => 4,
        }
    }
}

/// Progressive compression controller.
#[derive(Debug, Clone)]
pub struct ProgressiveCompressor {
    /// Coherence detector
    coherence: CoherenceDetector,
    /// Focus manager (optional)
    focus_manager: Option<FocusManager>,
    /// Configuration
    config: ProgressiveConfig,
    /// Hardcode configuration
    hardcode_config: HardcodeConfig,
}

/// Configuration for progressive compression.
#[derive(Debug, Clone)]
pub struct ProgressiveConfig {
    /// Token budget target
    target_budget: u32,
    /// Threshold to trigger stage 1
    stage1_threshold: u32,
    /// Threshold to trigger stage 2
    stage2_threshold: u32,
    /// Threshold to trigger stage 3
    stage3_threshold: u32,
    /// Threshold to trigger emergency
    emergency_threshold: u32,
    /// Preserve last N messages always
    preserve_last_n: usize,
    /// Minimum coherence threshold to keep together
    coherence_threshold: f32,
}

impl Default for ProgressiveConfig {
    fn default() -> Self {
        Self {
            target_budget: 8000,
            stage1_threshold: 12000,  // Remove low priority when >12k tokens
            stage2_threshold: 16000,  // Summarize medium when >16k tokens
            stage3_threshold: 20000,  // Compress high when >20k tokens
            emergency_threshold: 25000, // Emergency when >25k tokens
            preserve_last_n: 3,
            coherence_threshold: 0.7,
        }
    }
}

impl ProgressiveConfig {
    /// 创建基于对话复杂度的自适应配置
    /// 
    /// # Arguments
    /// * `messages` - 对话历史
    /// 
    /// # Returns
    /// 根据复杂度调整的压缩阈值配置
    pub fn adaptive_configure(messages: &[Message]) -> Self {
        let complexity = ComplexityAnalyzer::analyze(messages);
        
        // 基于复杂度动态调整阈值
        let (stage1, stage2, stage3, emergency, preserve_n) = match complexity {
            ComplexityLevel::High => {
                // 技术讨论密集 → 提前压缩，保留更多上下文
                log::info!("检测到高复杂度对话，采用激进压缩策略");
                (10000, 14000, 18000, 22000, 5)
            },
            ComplexityLevel::Medium => {
                // 普通对话 → 默认阈值
                log::info!("检测到中等复杂度对话，采用标准压缩策略");
                (12000, 16000, 20000, 25000, 3)
            },
            ComplexityLevel::Low => {
                // 闲聊 → 延迟压缩，减少 AI 调用
                log::info!("检测到低复杂度对话，采用保守压缩策略");
                (15000, 20000, 25000, 30000, 2)
            },
        };
        
        Self {
            target_budget: 8000,
            stage1_threshold: stage1,
            stage2_threshold: stage2,
            stage3_threshold: stage3,
            emergency_threshold: emergency,
            preserve_last_n: preserve_n,
            coherence_threshold: 0.7,
        }
    }
    
    /// 获取复杂度描述
    pub fn complexity_description(messages: &[Message]) -> &'static str {
        let complexity = ComplexityAnalyzer::analyze(messages);
        ComplexityAnalyzer::complexity_description(complexity)
    }
}

impl ProgressiveCompressor {
    /// Create a new progressive compressor.
    pub fn new(config: ProgressiveConfig) -> Self {
        Self {
            coherence: CoherenceDetector::default(),
            focus_manager: None,
            config,
            hardcode_config: HardcodeConfig::default(),
        }
    }

    /// Create with default configuration.
    pub fn default_config() -> Self {
        Self::new(ProgressiveConfig::default())
    }
    
    /// 创建自适应配置的压缩器（基于对话复杂度）
    pub fn adaptive_create(messages: &[Message]) -> Self {
        let config = ProgressiveConfig::adaptive_configure(messages);
        let mut instance = Self::new(config);
        // 根据复杂度配置 hardcode_config
        let complexity = ComplexityAnalyzer::analyze(messages);
        instance.hardcode_config = HardcodeConfig::from_complexity(complexity);
        instance
    }

    /// Set focus manager.
    pub fn set_focus_manager(&mut self, manager: FocusManager) {
        self.focus_manager = Some(manager);
    }
    
    /// Set custom hardcode config.
    pub fn with_hardcode_config(mut self, config: HardcodeConfig) -> Self {
        self.hardcode_config = config;
        self
    }

    /// Compress messages using progressive strategy.
    pub async fn compress(&mut self, messages: &[Message], provider: Option<&dyn crate::providers::Provider>) -> Result<Vec<Message>> {
        let current_tokens = estimate_tokens(messages);
        
        // If under budget, no compression needed
        if current_tokens <= self.config.target_budget {
            return Ok(messages.to_vec());
        }

        let mut result = messages.to_vec();
        let mut applied_stages = Vec::new();

        // Apply stages in order until target budget is reached
        for stage in &[
            CompressionStage::RemoveLowPriority,
            CompressionStage::SummarizeMedium,
            CompressionStage::CompressHighPriority,
            CompressionStage::EmergencyCompression,
        ] {
            let tokens = estimate_tokens(&result);
            
            // Check if we need this stage
            let threshold = self.get_threshold_for_stage(*stage);
            if tokens <= threshold && tokens <= self.config.target_budget {
                break;
            }

            // Apply stage
            result = self.apply_stage(result.clone(), *stage, provider).await?;
            applied_stages.push(*stage);

            // Check if we reached target
            if estimate_tokens(&result) <= self.config.target_budget {
                break;
            }
        }

        // Log compression result
        log::info!(
            "Progressive compression: {} -> {} tokens, stages applied: {}",
            current_tokens,
            estimate_tokens(&result),
            applied_stages.iter().map(|s| s.name()).collect::<Vec<_>>().join(", ")
        );

        Ok(result)
    }

    /// Get threshold for a compression stage.
    fn get_threshold_for_stage(&self, stage: CompressionStage) -> u32 {
        match stage {
            CompressionStage::RemoveLowPriority => self.config.stage1_threshold,
            CompressionStage::SummarizeMedium => self.config.stage2_threshold,
            CompressionStage::CompressHighPriority => self.config.stage3_threshold,
            CompressionStage::EmergencyCompression => self.config.emergency_threshold,
        }
    }

    /// Apply a single compression stage.
    async fn apply_stage(
        &mut self,
        messages: Vec<Message>,
        stage: CompressionStage,
        provider: Option<&dyn crate::providers::Provider>,
    ) -> Result<Vec<Message>> {
        match stage {
            CompressionStage::RemoveLowPriority => {
                self.remove_low_priority(messages)
            }
            CompressionStage::SummarizeMedium => {
                self.summarize_medium(messages, provider).await
            }
            CompressionStage::CompressHighPriority => {
                self.compress_high_priority(messages, provider).await
            }
            CompressionStage::EmergencyCompression => {
                self.emergency_compress(messages, provider).await
            }
        }
    }

    /// Stage 1: Remove low-priority messages (结合 Focus 相关性).
    fn remove_low_priority(&self, messages: Vec<Message>) -> Result<Vec<Message>> {
        let mut result = Vec::new();
        let mut removed_count = 0;

        // Segment messages by coherence
        let segments = self.coherence.segment_messages(&messages);

        for segment in segments {
            // 结合 Focus 相关性的优先级判断
            let filtered = segment.iter()
                .enumerate()
                .filter(|(i, msg)| {
                    // Always preserve last N messages
                    if messages.len() - i <= self.config.preserve_last_n {
                        return true;
                    }
                    
                    // Check Focus relevance if available
                    if let Some(focus_manager) = &self.focus_manager {
                        let content = self.get_message_content(msg);
                        let relevance = self.calculate_message_focus_relevance(&content, focus_manager);
                        
                        // High relevance (> 0.7) - always keep
                        if relevance > 0.7 {
                            return true;
                        }
                        
                        // Medium relevance (0.3-0.7) - keep if has code/questions
                        if relevance > 0.3
                            && (content.contains("```") || content.contains("?") || content.contains("？")) {
                                return true;
                            }
                        
                        // Low relevance (< 0.3) - can remove if short
                        if content.len() < 50 && !content.contains("```") {
                            removed_count += 1;
                            return false;
                        }
                        
                        true
                    } else {
                        // No focus manager, use simple heuristic
                        let content = self.get_message_content(msg);
                        if content.contains("```") || content.contains("function") || content.contains("fn ") {
                            return true;
                        }
                        
                        if content.contains("?") || content.contains("？") {
                            return true;
                        }
                        let content_lower = content.to_lowercase();
                        if content_lower.contains("how") || content_lower.contains("如何") {
                            return true;
                        }
                        
                        if content.len() < 50 {
                            removed_count += 1;
                            return false;
                        }
                        
                        true
                    }
                })
                .map(|(_, msg)| msg.clone())
                .collect::<Vec<_>>();

            result.extend(filtered);
        }

        log::debug!("Stage 1: Removed {} low-priority messages", removed_count);
        Ok(result)
    }
    
    /// Calculate message relevance to active focus points.
    fn calculate_message_focus_relevance(&self, content: &str, focus_manager: &FocusManager) -> f32 {
        if let Some(current_focus) = focus_manager.current_focus() {
            // Calculate relevance to current focus
            let mut score = 0.0_f32;
            
            // Keyword matching
            let content_lower = content.to_lowercase();
            for keyword in &current_focus.keywords {
                if content_lower.contains(&keyword.to_lowercase()) {
                    score += 0.2;
                }
            }
            
            // Entity matching (higher weight)
            for entity in &current_focus.entities {
                if content_lower.contains(&entity.to_lowercase()) {
                    score += 0.3;
                }
            }
            
            // File path matching
            for file in &current_focus.related_files {
                if content.contains(&*file.to_string_lossy()) {
                    score += 0.4;
                }
            }
            
            // Apply importance weighting
            score *= current_focus.importance;
            
            // Apply confidence weighting
            score *= current_focus.confidence;
            
            return score.min(1.0);
        }
        
        0.5 // Neutral if no active focus
    }

    /// Stage 2: Summarize medium-priority messages.
    async fn summarize_medium(&self, messages: Vec<Message>, provider: Option<&dyn crate::providers::Provider>) -> Result<Vec<Message>> {
        let mut result = Vec::new();
        let mut summarized_count = 0;

        // Segment messages by coherence
        let segments = self.coherence.segment_messages(&messages);

        for segment in segments {
            // Find medium-length messages (100-500 chars)
            let medium_indices = segment.iter()
                .enumerate()
                .filter(|(_, msg)| {
                    let content = self.get_message_content(msg);
                    content.len() >= 100 && content.len() <= 500 && !content.contains("```")
                })
                .map(|(i, _)| i)
                .collect::<Vec<_>>();

            if medium_indices.len() >= 2 {
                // Summarize medium-priority messages together
                let medium_messages = medium_indices.iter()
                    .map(|&i| segment[i].clone())
                    .collect::<Vec<_>>();

                if let Some(p) = provider {
                    let summary = self.generate_summary(&medium_messages, p).await?;
                    
                    // Create summary message
                    let summary_msg = Message {
                        role: crate::providers::Role::Assistant,
                        content: crate::providers::MessageContent::Text(format!("[摘要] {}", summary)),
                    };

                    // Replace medium messages with summary
                    let mut new_segment = Vec::new();
                    for (i, msg) in segment.iter().enumerate() {
                        if medium_indices.contains(&i) {
                            if i == medium_indices[0] {
                                new_segment.push(summary_msg.clone());
                                summarized_count += medium_indices.len();
                            }
                        } else {
                            new_segment.push(msg.clone());
                        }
                    }
                    result.extend(new_segment);
                } else {
                    // No provider, just compress inline
                    result.extend(self.compress_inline(&segment, &medium_indices));
                }
            } else {
                // No medium-priority messages in this segment
                result.extend(segment);
            }
        }

        log::debug!("Stage 2: Summarized {} medium-priority messages", summarized_count);
        Ok(result)
    }

    /// Stage 3: Compress high-priority but verbose messages.
    async fn compress_high_priority(&self, messages: Vec<Message>, provider: Option<&dyn crate::providers::Provider>) -> Result<Vec<Message>> {
        let mut result = Vec::new();
        let mut compressed_count = 0;

        for msg in messages {
            // Compress verbose messages (> threshold) with code or details
            let content = self.get_message_content(&msg);
            if content.len() > self.hardcode_config.code_content_threshold && (content.contains("```") || content.contains("fn ") || content.contains("function")) {
                if let Some(p) = provider {
                    let compressed = self.compress_single_message(&msg, p).await?;
                    result.push(compressed);
                    compressed_count += 1;
                } else {
                    result.push(self.trim_verbose_message(msg));
                }
            } else {
                result.push(msg);
            }
        }

        log::debug!("Stage 3: Compressed {} high-priority verbose messages", compressed_count);
        Ok(result)
    }

    /// Stage 4: Emergency compression (aggressive).
    async fn emergency_compress(&self, messages: Vec<Message>, provider: Option<&dyn crate::providers::Provider>) -> Result<Vec<Message>> {
        // Emergency: Summarize entire conversation
        if let Some(p) = provider {
            let emergency_summary = self.generate_emergency_summary(&messages, p).await?;
            
            // Keep only last 3 messages + summary
            let last_n = messages.iter().rev().take(self.config.preserve_last_n).rev().cloned().collect::<Vec<_>>();
            
            let summary_msg = Message {
                role: crate::providers::Role::Assistant,
                content: crate::providers::MessageContent::Text(format!("[对话摘要] {}", emergency_summary)),
            };

            let result = vec![summary_msg]
                .into_iter()
                .chain(last_n.into_iter())
                .collect();

            log::warn!("Emergency compression applied: {} messages -> summary + last {}", messages.len(), self.config.preserve_last_n);
            Ok(result)
        } else {
            // No provider, keep only last N messages
            Ok(messages.iter().rev().take(self.config.preserve_last_n).rev().cloned().collect())
        }
    }

    /// Generate summary for a group of messages.
    async fn generate_summary(&self, messages: &[Message], provider: &dyn crate::providers::Provider) -> Result<String> {
        let context = messages.iter()
            .map(|m| match &m.content {
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
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let request = crate::providers::ChatRequest {
            messages: vec![crate::providers::Message {
                role: crate::providers::Role::User,
                content: crate::providers::MessageContent::Text(format!(
                    "请简洁总结以下对话要点（不超过200字）：\n\n{}",
                    context
                )),
            }],
            tools: vec![],
            system: Some("你是对话摘要助手，生成简洁准确的总结。".to_string()),
            think: false,
            max_tokens: 300,
            server_tools: vec![],
            enable_caching: false,
        };

        let response = provider.chat(request).await?;
        
        let summary = response.content.iter()
            .filter_map(|b| {
                if let crate::providers::ContentBlock::Text { text } = b {
                    Some(text.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("");

        Ok(summary)
    }

    /// Generate emergency summary for entire conversation.
    async fn generate_emergency_summary(&self, messages: &[Message], provider: &dyn crate::providers::Provider) -> Result<String> {
        let context = messages.iter()
            .map(|m| match &m.content {
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
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        // Truncate if too long
        let truncated = if context.len() > self.hardcode_config.max_context_length {
            context.chars().take(self.hardcode_config.max_context_length).collect::<String>()
        } else {
            context
        };

        let request = crate::providers::ChatRequest {
            messages: vec![crate::providers::Message {
                role: crate::providers::Role::User,
                content: crate::providers::MessageContent::Text(format!(
                    "请生成紧急摘要，包含以下关键信息：\n1. 主要讨论主题\n2. 重要决策\n3. 待解决问题\n4. 当前状态\n\n对话内容：\n{}",
                    truncated
                )),
            }],
            tools: vec![],
            system: Some("你是紧急摘要助手，在对话过长时生成关键信息摘要。".to_string()),
            think: false,
            max_tokens: 500,
            server_tools: vec![],
            enable_caching: false,
        };

        let response = provider.chat(request).await?;
        
        let summary = response.content.iter()
            .filter_map(|b| {
                if let crate::providers::ContentBlock::Text { text } = b {
                    Some(text.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("");

        Ok(summary)
    }

    /// Compress a single verbose message.
    async fn compress_single_message(&self, msg: &Message, provider: &dyn crate::providers::Provider) -> Result<Message> {
        let content = match &msg.content {
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

        // Request concise version
        let request = crate::providers::ChatRequest {
            messages: vec![crate::providers::Message {
                role: crate::providers::Role::User,
                content: crate::providers::MessageContent::Text(format!(
                    "请将以下内容精简，保留核心信息：\n\n{}",
                    content
                )),
            }],
            tools: vec![],
            system: Some("你是内容精简助手，去除冗余保留要点。".to_string()),
            think: false,
            max_tokens: 200,
            server_tools: vec![],
            enable_caching: false,
        };

        let response = provider.chat(request).await?;
        
        let compressed = response.content.iter()
            .filter_map(|b| {
                if let crate::providers::ContentBlock::Text { text } = b {
                    Some(text.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("");

        Ok(Message {
            role: msg.role,
            content: crate::providers::MessageContent::Text(format!("[精简] {}", compressed)),
        })
    }

    /// Get text content from message.
    fn get_message_content(&self, msg: &Message) -> String {
        match &msg.content {
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
        }
    }

    /// Trim verbose message inline (without AI).
    fn trim_verbose_message(&self, msg: Message) -> Message {
        let content = match &msg.content {
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

        // Simple trim: keep first N chars
        let trimmed = if content.len() > self.hardcode_config.max_trimmed_content_length {
            format!("[精简] {}...", content.chars().take(self.hardcode_config.max_trimmed_content_length).collect::<String>())
        } else {
            content
        };

        Message {
            role: msg.role,
            content: crate::providers::MessageContent::Text(trimmed),
        }
    }

    /// Compress segments while maintaining coherence and focus priority.
    ///
    /// This method is designed to work with the integrated processor,
    /// compressing pre-segmented messages while considering focus relevance.
    ///
    /// # Arguments
    /// * `segments` - Pre-segmented message groups from CoherenceDetector.
    /// * `focus` - Current conversation focus.
    /// * `coherence` - Coherence detector for scoring.
    ///
    /// # Returns
    /// Compressed messages with focus-aware prioritization.
    pub fn compress_segments(
        &self,
        segments: Vec<Vec<Message>>,
        focus: &ConversationFocus,
        coherence: &CoherenceDetector,
    ) -> Result<Vec<Message>> {
        let mut result = Vec::new();

        for segment in segments {
            // Calculate coherence score for this segment
            let coherence_score = coherence.calculate_coherence(&segment);

            // Calculate focus score for this segment
            let focus_score = self.calculate_segment_focus_score(&segment, focus);

            // Decision logic based on coherence and focus
            if coherence_score > self.config.coherence_threshold && focus_score > 0.5 {
                // High coherence + High focus relevance: preserve intact
                log::debug!(
                    "Segment preserved: coherence={}, focus={}",
                    coherence_score, focus_score
                );
                result.extend(segment);
            } else if coherence_score > self.config.coherence_threshold {
                // High coherence but lower focus: mostly preserve
                if segment.len() <= 3 {
                    result.extend(segment);
                } else {
                    // Keep first and last, summarize middle
                    result.push(segment[0].clone());
                    // Middle messages can be summarized (inline compression)
                    let middle_indices: Vec<usize> = (1..segment.len() - 1).collect();
                    let compressed_middle = self.compress_inline(&segment, &middle_indices);
                    result.extend(compressed_middle.into_iter().skip(1).take(segment.len() - 3));
                    result.push(segment[segment.len() - 1].clone());
                }
            } else if focus_score > 0.5 {
                // Low coherence but high focus: keep key messages
                for msg in &segment {
                    let msg_focus = self.calculate_message_focus_score(msg, focus);
                    if msg_focus > 0.3 {
                        result.push(msg.clone());
                    }
                }
            } else {
                // Low coherence + Low focus: aggressive compression
                if !segment.is_empty() {
                    // Create inline summary for the segment
                    let all_indices: Vec<usize> = (0..segment.len()).collect();
                    let compressed = self.compress_inline(&segment, &all_indices);
                    result.extend(compressed);
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
            total_score += self.calculate_message_focus_score(msg, focus);
        }

        total_score / segment.len() as f32
    }

    /// Calculate focus score for a single message using keywords.
    fn calculate_message_focus_score(&self, message: &Message, focus: &ConversationFocus) -> f32 {
        // Get message content
        let content = self.get_message_content(message);
        let content_lower = content.to_lowercase();

        let mut score: f32 = 0.0;

        // Check if message matches current topic
        if let Some(topic) = &focus.current_topic {
            let topic_keywords: Vec<&str> = topic.split(", ").collect();
            for kw in topic_keywords {
                if content_lower.contains(&kw.to_lowercase()) {
                    score += 0.2;
                }
            }
        }

        // Check if message matches current question keywords
        if let Some(question) = &focus.current_question {
            let question_lower = question.to_lowercase();
            for word in question_lower.split_whitespace() {
                if word.len() > 3 && content_lower.contains(word) {
                    score += 0.1;
                }
            }
        }

        // Check focus manager if available
        if let Some(focus_manager) = &self.focus_manager {
            let relevance = self.calculate_message_focus_relevance(&content, focus_manager);
            score = score.max(relevance);
        }

        score.min(1.0)
    }

    /// Compress inline without AI.
    fn compress_inline(&self, messages: &[Message], indices: &[usize]) -> Vec<Message> {
        let mut result = Vec::new();
        let mut summary_parts = Vec::new();

        for (i, msg) in messages.iter().enumerate() {
            if indices.contains(&i) {
                // Collect summary parts
                let content = match &msg.content {
                    crate::providers::MessageContent::Text(text) => text.chars().take(100).collect::<String>(),
                    crate::providers::MessageContent::Blocks(_) => "...".to_string(),
                };
                summary_parts.push(content);
                
                if i == indices[indices.len() - 1] {
                    // Create summary
                    let summary = format!("[摘要] {}", summary_parts.join(" | "));
                    result.push(Message {
                        role: crate::providers::Role::Assistant,
                        content: crate::providers::MessageContent::Text(summary),
                    });
                }
            } else {
                result.push(msg.clone());
            }
        }

        result
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
            // Rough estimation: 4 chars per token
            (content.len() / 4) as u32 + 50 // +50 for metadata
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progressive_config_default() {
        let config = ProgressiveConfig::default();
        assert_eq!(config.target_budget, 8000);
        assert_eq!(config.preserve_last_n, 3);
    }

    #[test]
    fn test_compressor_creation() {
        let compressor = ProgressiveCompressor::default_config();
        assert!(compressor.focus_manager.is_none());
    }

    #[test]
    fn test_stage_ordering() {
        assert!(CompressionStage::RemoveLowPriority.priority() < CompressionStage::SummarizeMedium.priority());
        assert!(CompressionStage::SummarizeMedium.priority() < CompressionStage::CompressHighPriority.priority());
        assert!(CompressionStage::CompressHighPriority.priority() < CompressionStage::EmergencyCompression.priority());
    }

    #[test]
    fn test_estimate_tokens() {
        use crate::providers::{Message, MessageContent, Role};
        
        let messages = vec![
            Message {
                role: Role::User,
                content: MessageContent::Text("This is a test message".to_string()),
            },
        ];
        
        let tokens = estimate_tokens(&messages);
        assert!(tokens > 0);
    }
}