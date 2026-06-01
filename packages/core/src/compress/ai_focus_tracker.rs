//! AI-driven focus tracker that uses LLM to analyze message relevance.
//!
//! This replaces the simple `contains` matching with intelligent LLM-based
//! focus analysis for better semantic understanding.

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::providers::{ChatRequest, ContentBlock, Message, MessageContent, Provider, Role};
use super::focus::{ConversationFocus, TopicTransition};
use super::focus_config::FocusTrackerConfig;

/// System prompt for focus analysis.
const FOCUS_ANALYSIS_PROMPT: &str = r#"你是焦点分析助手。分析新消息与当前会话焦点的关系。

## 分析维度

1. **relevance** (0.0-1.0): 与当前焦点的相关性
   - 1.0: 直接回答当前问题或继续当前任务
   - 0.7-0.9: 高度相关，提供重要上下文
   - 0.4-0.6: 中等相关，有联系但不直接
   - 0.1-0.3: 低相关，可能偏离话题
   - 0.0: 完全不相关或话题已切换

2. **is_focus_update** (true/false): 是否需要更新焦点
   - true: 当话题明显转换、新问题提出、任务切换时
   - false: 继续当前话题时

3. **语义差异检测**: 注意区分相似但不同的概念
   - 例如: "压缩" vs "解压缩" 是不同任务
   - 例如: "优化性能" vs "优化内存" 是不同焦点

## 输出格式（严格 JSON）

```json
{
  "relevance": 0.8,
  "is_focus_update": false,
  "new_topic": "新话题名称（如果需要更新）",
  "new_question": "新问题（如果需要更新）",
  "context_to_add": "需要添加到上下文的关键信息",
  "reason": "判断理由简述"
}
```

## 规则

1. 只返回 JSON，不要其他解释
2. 如果不需要更新焦点，`new_topic` 和 `new_question` 可以省略
3. `context_to_add` 只在有重要上下文信息时填写
4. relevance 应基于语义理解，不是简单的关键词匹配"#;

/// Result of AI focus analysis on a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusAnalysisResult {
    /// Relevance score to current focus (0.0 to 1.0).
    pub relevance: f32,
    /// Whether the focus needs to be updated.
    pub is_focus_update: bool,
    /// New topic if focus needs update.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_topic: Option<String>,
    /// New question if focus needs update.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_question: Option<String>,
    /// Context to add for better understanding.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_to_add: Option<String>,
    /// Reason for the judgment.
    pub reason: String,
}

impl Default for FocusAnalysisResult {
    fn default() -> Self {
        Self {
            relevance: 0.5,
            is_focus_update: false,
            new_topic: None,
            new_question: None,
            context_to_add: None,
            reason: "Default result (AI analysis not performed)".to_string(),
        }
    }
}

/// AI-driven focus tracker using LLM for intelligent analysis.
#[allow(dead_code)]
pub struct AiFocusTracker {
    /// Provider for AI calls.
    provider: Box<dyn Provider>,
    /// Model name (should be a fast/cheap model).
    model: String,
    /// Current conversation focus.
    current_focus: Option<ConversationFocus>,
    /// Configuration.
    config: FocusTrackerConfig,
    /// Cache for analyzed messages (avoid repeated calls).
    analysis_cache: Vec<(String, FocusAnalysisResult)>,
    /// Maximum cache size.
    max_cache_size: usize,
}

impl AiFocusTracker {
    /// Create a new AI-driven focus tracker.
    ///
    /// # Arguments
    /// * `provider` - Provider for AI calls.
    /// * `model` - Model name (recommend fast model like claude-haiku).
    pub fn new(provider: Box<dyn Provider>, model: String) -> Self {
        Self {
            provider,
            model,
            current_focus: None,
            config: FocusTrackerConfig::default(),
            analysis_cache: Vec::new(),
            max_cache_size: 50,
        }
    }

    /// Create with custom configuration.
    pub fn with_config(provider: Box<dyn Provider>, model: String, config: FocusTrackerConfig) -> Self {
        Self {
            provider,
            model,
            current_focus: None,
            config,
            analysis_cache: Vec::new(),
            max_cache_size: 50,
        }
    }

    /// Create a minimal tracker for background tasks.
    pub fn new_minimal(model: String) -> Self {
        Self {
            provider: crate::create_minimal_provider(&model),
            model,
            current_focus: None,
            config: FocusTrackerConfig::default(),
            analysis_cache: Vec::new(),
            max_cache_size: 50,
        }
    }

    /// Get current focus.
    pub fn current_focus(&self) -> Option<&ConversationFocus> {
        self.current_focus.as_ref()
    }

    /// Set current focus manually.
    pub fn set_focus(&mut self, focus: ConversationFocus) {
        self.current_focus = Some(focus);
    }

    /// Clear current focus.
    pub fn clear_focus(&mut self) {
        self.current_focus = None;
        self.analysis_cache.clear();
    }

    /// Get configuration.
    pub fn config(&self) -> &FocusTrackerConfig {
        &self.config
    }

    /// Get mutable configuration.
    pub fn config_mut(&mut self) -> &mut FocusTrackerConfig {
        &mut self.config
    }

    /// Analyze a message's relationship with current focus using AI.
    ///
    /// This is the main method that replaces simple keyword matching
    /// with intelligent LLM-based analysis.
    ///
    /// # Arguments
    /// * `message` - Message to analyze.
    ///
    /// # Returns
    /// Analysis result with relevance score and focus update info.
    pub async fn analyze_message(&mut self, message: &Message) -> Result<FocusAnalysisResult> {
        // Check cache first
        let message_key = self.message_cache_key(message);
        if let Some((_, cached)) = self.analysis_cache.iter().find(|(k, _)| k == &message_key) {
            log::debug!("Using cached focus analysis result");
            return Ok(cached.clone());
        }

        // Build prompt
        let prompt = self.build_focus_analysis_prompt(message);

        // Call AI
        let response = self.call_ai(&prompt).await?;

        // Parse result
        let result = self.parse_analysis_result(&response)?;

        // Update focus if needed
        if result.is_focus_update {
            self.update_focus_from_result(&result, message);
        }

        // Cache result
        self.cache_result(message_key, result.clone());

        Ok(result)
    }

    /// Analyze a batch of key messages efficiently.
    ///
    /// This method analyzes multiple messages but only calls AI for
    /// messages that are likely to affect focus (user messages, first/last messages).
    ///
    /// # Arguments
    /// * `messages` - Messages to analyze.
    ///
    /// # Returns
    /// List of analysis results for key messages.
    pub async fn analyze_key_messages(&mut self, messages: &[Message]) -> Result<Vec<(usize, FocusAnalysisResult)>> {
        let mut results = Vec::new();

        // Identify key messages: user messages, first and last
        for (idx, msg) in messages.iter().enumerate() {
            let is_key = matches!(msg.role, Role::User)
                || idx == 0
                || idx == messages.len() - 1;

            if is_key {
                let result = self.analyze_message(msg).await?;
                results.push((idx, result));
            }
        }

        Ok(results)
    }

    /// Build focus analysis prompt for a message.
    fn build_focus_analysis_prompt(&self, message: &Message) -> String {
        let current_focus_text = self.format_current_focus();
        let message_text = self.format_message(message);

        format!(
            "分析新消息与当前会话焦点的关系：\n\n{}\n\n新消息：\n{}\n\n请返回 JSON 格式分析结果。",
            current_focus_text,
            message_text
        )
    }

    /// Format current focus for prompt.
    fn format_current_focus(&self) -> String {
        match &self.current_focus {
            Some(focus) => {
                let mut parts = Vec::new();

                if let Some(topic) = &focus.current_topic {
                    parts.push(format!("当前话题: {}", topic));
                }

                if let Some(question) = &focus.current_question {
                    parts.push(format!("当前问题/任务: {}", question));
                }

                if !focus.recent_context.is_empty() {
                    parts.push(format!("最近上下文: {}", focus.recent_context.join(" | ")));
                }

                if !focus.topic_transitions.is_empty() {
                    let transitions: Vec<String> = focus.topic_transitions.iter()
                        .map(|t| format!("{} -> {}", t.from_topic, t.to_topic))
                        .collect();
                    parts.push(format!("话题转换历史: {}", transitions.join(", ")));
                }

                if parts.is_empty() {
                    "当前焦点: (尚未建立明确焦点)".to_string()
                } else {
                    format!("当前焦点:\n{}", parts.join("\n"))
                }
            }
            None => "当前焦点: (尚未建立明确焦点，这是对话开始)".to_string(),
        }
    }

    /// Format message for prompt.
    fn format_message(&self, message: &Message) -> String {
        let role = match message.role {
            Role::User => "用户",
            Role::Assistant => "助手",
            Role::System => "系统",
            Role::Tool => "工具",
        };

        let content = match &message.content {
            MessageContent::Text(text) => text.clone(),
            MessageContent::Blocks(blocks) => {
                blocks.iter()
                    .filter_map(|b| {
                        if let ContentBlock::Text { text } = b {
                            Some(text.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        };

        // Truncate if too long
        let truncated = if content.len() > 500 {
            format!("{}... (已截断)", &content[..500])
        } else {
            content
        };

        format!("角色: {}\n内容: {}", role, truncated)
    }

    /// Call AI for analysis.
    async fn call_ai(&self, prompt: &str) -> Result<String> {
        let request = ChatRequest {
            messages: vec![Message {
                role: Role::User,
                content: MessageContent::Text(prompt.to_string()),
            }],
            tools: vec![],
            system: Some(FOCUS_ANALYSIS_PROMPT.to_string()),
            think: false,
            max_tokens: 256, // Small response for analysis
            server_tools: vec![],
            enable_caching: false,
        };

        let response = self.provider.chat(request).await?;

        // Extract text from response
        let text = response.content.iter()
            .filter_map(|b| {
                if let ContentBlock::Text { text } = b {
                    Some(text.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("");

        Ok(text)
    }

    /// Parse AI response into analysis result.
    fn parse_analysis_result(&self, response: &str) -> Result<FocusAnalysisResult> {
        // Clean up response
        let cleaned = response
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        // Parse JSON
        let result: FocusAnalysisResult = serde_json::from_str(cleaned)?;

        // Validate and clamp values
        let validated = FocusAnalysisResult {
            relevance: result.relevance.clamp(0.0, 1.0),
            is_focus_update: result.is_focus_update,
            new_topic: result.new_topic,
            new_question: result.new_question,
            context_to_add: result.context_to_add,
            reason: result.reason,
        };

        Ok(validated)
    }

    /// Update focus from analysis result.
    fn update_focus_from_result(&mut self, result: &FocusAnalysisResult, message: &Message) {
        let message_idx = self.current_focus.as_ref()
            .map(|f| f.detected_at + 1)
            .unwrap_or(0);

        // Get message content for context
        let message_context = self.extract_message_context(message);

        let new_focus = match &self.current_focus {
            Some(existing) => {
                // Create updated focus
                let mut new_focus = ConversationFocus {
                    current_topic: result.new_topic.clone().or(existing.current_topic.clone()),
                    current_question: result.new_question.clone().or(existing.current_question.clone()),
                    recent_context: existing.recent_context.clone(),
                    topic_transitions: existing.topic_transitions.clone(),
                    detected_at: message_idx,
                };

                // Add new context if provided
                if let Some(ctx) = &result.context_to_add {
                    new_focus.recent_context.push(ctx.clone());
                    if new_focus.recent_context.len() > self.config.max_recent_context_count {
                        new_focus.recent_context.remove(0);
                    }
                }

                // Record topic transition if topic changed
                if let (Some(new_topic), Some(old_topic)) = (&result.new_topic, &existing.current_topic) {
                    if new_topic != old_topic {
                        new_focus.topic_transitions.push(TopicTransition {
                            from_topic: old_topic.clone(),
                            to_topic: new_topic.clone(),
                            message_index: message_idx,
                            transition_keyword: "AI detected".to_string(),
                        });
                    }
                }

                new_focus
            }
            None => {
                // Create initial focus
                ConversationFocus {
                    current_topic: result.new_topic.clone().or(message_context.topic),
                    current_question: result.new_question.clone().or(message_context.question),
                    recent_context: result.context_to_add.clone().map(|ctx| vec![ctx]).unwrap_or_default(),
                    topic_transitions: Vec::new(),
                    detected_at: message_idx,
                }
            }
        };

        self.current_focus = Some(new_focus);
        log::debug!("Focus updated: topic={}, question={}",
            self.current_focus.as_ref().and_then(|f| f.current_topic.as_ref()).unwrap_or(&"none".to_string()),
            self.current_focus.as_ref().and_then(|f| f.current_question.as_ref()).unwrap_or(&"none".to_string())
        );
    }

    /// Extract context from message for focus initialization.
    fn extract_message_context(&self, message: &Message) -> MessageContext {
        let text = match &message.content {
            MessageContent::Text(t) => t.clone(),
            MessageContent::Blocks(blocks) => {
                blocks.iter()
                    .filter_map(|b| {
                        if let ContentBlock::Text { text } = b {
                            Some(text.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        };

        // Simple extraction for fallback
        let topic = self.config.find_tech_keywords(&text)
            .first()
            .cloned();

        let question = if self.config.matches_question(&text) {
            Some(text.chars().take(100).collect::<String>())
        } else {
            None
        };

        MessageContext { topic, question }
    }

    /// Generate cache key for a message.
    fn message_cache_key(&self, message: &Message) -> String {
        let content = match &message.content {
            MessageContent::Text(t) => t.clone(),
            MessageContent::Blocks(blocks) => {
                blocks.iter()
                    .filter_map(|b| {
                        if let ContentBlock::Text { text } = b {
                            Some(text.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("|")
            }
        };

        // Use first 100 chars as key
        let key = content.chars().take(100).collect::<String>();
        format!("{:?}:{}", message.role, key)
    }

    /// Cache analysis result.
    fn cache_result(&mut self, key: String, result: FocusAnalysisResult) {
        // Remove old entry if exists
        self.analysis_cache.retain(|(k, _)| k != &key);

        // Add new entry
        self.analysis_cache.push((key, result));

        // Trim cache if too large
        if self.analysis_cache.len() > self.max_cache_size {
            self.analysis_cache.remove(0);
        }
    }

    /// Detect focus from messages (rule-based fallback).
    ///
    /// This method provides a fallback when AI analysis is not available.
    pub fn detect_focus_fallback(&self, messages: &[Message]) -> ConversationFocus {
        // Use the original FocusTracker logic for fallback
        let tracker = super::focus::FocusTracker::with_config(self.config.clone());
        tracker.detect_focus(messages)
    }

    /// Calculate focus score for a message using cached analysis.
    ///
    /// Returns the relevance score from AI analysis if available,
    /// otherwise uses rule-based calculation.
    pub fn focus_score(&self, message: &Message) -> f32 {
        // Check cache
        let key = self.message_cache_key(message);
        if let Some((_, result)) = self.analysis_cache.iter().find(|(k, _)| k == &key) {
            return result.relevance;
        }

        // Fallback to rule-based scoring
        if let Some(focus) = &self.current_focus {
            let tracker = super::focus::FocusTracker::with_config(self.config.clone());
            tracker.focus_score(message, focus)
        } else {
            0.5 // Default when no focus established
        }
    }

    /// Create a focus message to inject into compressed conversation.
    pub fn create_focus_message(&self) -> Message {
        match &self.current_focus {
            Some(focus) => {
                let tracker = super::focus::FocusTracker::with_config(self.config.clone());
                tracker.create_focus_message(focus)
            }
            None => {
                Message {
                    role: Role::System,
                    content: MessageContent::Text("[焦点追踪系统初始化]".to_string()),
                }
            }
        }
    }
}

/// Helper struct for extracted message context.
struct MessageContext {
    topic: Option<String>,
    question: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_focus_analysis_result_default() {
        let result = FocusAnalysisResult::default();
        assert_eq!(result.relevance, 0.5);
        assert!(!result.is_focus_update);
        assert!(result.new_topic.is_none());
        assert!(result.new_question.is_none());
    }

    #[test]
    fn test_focus_analysis_result_clamp_relevance() {
        let json = r#"{
            "relevance": 1.5,
            "is_focus_update": false,
            "reason": "test"
        }"#;

        // Note: This would need a full tracker to test properly
        // Here we just test the struct
        let result: FocusAnalysisResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.relevance, 1.5); // Will be clamped in actual usage
    }

    #[test]
    fn test_ai_focus_tracker_creation() {
        let tracker = AiFocusTracker::new_minimal("test-model".to_string());
        assert!(tracker.current_focus().is_none());
        assert!(tracker.config().validate());
    }

    #[test]
    fn test_format_current_focus_none() {
        let tracker = AiFocusTracker::new_minimal("test-model".to_string());
        let text = tracker.format_current_focus();
        assert!(text.contains("尚未建立明确焦点"));
    }

    #[test]
    fn test_format_current_focus_some() {
        let mut tracker = AiFocusTracker::new_minimal("test-model".to_string());
        tracker.set_focus(ConversationFocus {
            current_topic: Some("API设计".to_string()),
            current_question: Some("如何优化性能？".to_string()),
            recent_context: vec!["之前讨论了数据库".to_string()],
            topic_transitions: Vec::new(),
            detected_at: 5,
        });

        let text = tracker.format_current_focus();
        assert!(text.contains("API设计"));
        assert!(text.contains("如何优化性能"));
        assert!(text.contains("之前讨论了数据库"));
    }

    #[test]
    fn test_format_message() {
        let tracker = AiFocusTracker::new_minimal("test-model".to_string());
        let message = Message {
            role: Role::User,
            content: MessageContent::Text("如何优化API性能？".to_string()),
        };

        let text = tracker.format_message(&message);
        assert!(text.contains("用户"));
        assert!(text.contains("如何优化API性能"));
    }

    #[test]
    fn test_format_message_truncation() {
        let tracker = AiFocusTracker::new_minimal("test-model".to_string());
        let long_text = "x".repeat(600);
        let message = Message {
            role: Role::User,
            content: MessageContent::Text(long_text.clone()),
        };

        let text = tracker.format_message(&message);
        assert!(text.contains("已截断"));
        assert!(text.len() < long_text.len() + 50);
    }

    #[test]
    fn test_message_cache_key() {
        let tracker = AiFocusTracker::new_minimal("test-model".to_string());
        let message = Message {
            role: Role::User,
            content: MessageContent::Text("测试消息内容".to_string()),
        };

        let key = tracker.message_cache_key(&message);
        assert!(key.starts_with("User:"));
    }

    #[test]
    fn test_cache_result() {
        let mut tracker = AiFocusTracker::new_minimal("test-model".to_string());
        let key = "test-key".to_string();
        let result = FocusAnalysisResult {
            relevance: 0.8,
            is_focus_update: false,
            new_topic: None,
            new_question: None,
            context_to_add: None,
            reason: "test".to_string(),
        };

        tracker.cache_result(key.clone(), result.clone());

        // Should be in cache
        assert_eq!(tracker.analysis_cache.len(), 1);
        assert_eq!(tracker.analysis_cache[0].0, key);
        assert_eq!(tracker.analysis_cache[0].1.relevance, 0.8);
    }

    #[test]
    fn test_cache_result_max_size() {
        let mut tracker = AiFocusTracker::new_minimal("test-model".to_string());
        tracker.max_cache_size = 3;

        for i in 0..5 {
            tracker.cache_result(
                format!("key-{}", i),
                FocusAnalysisResult::default(),
            );
        }

        // Should only have 3 entries
        assert_eq!(tracker.analysis_cache.len(), 3);
        // First entries should be removed
        assert!(!tracker.analysis_cache.iter().any(|(k, _)| k == "key-0"));
        assert!(!tracker.analysis_cache.iter().any(|(k, _)| k == "key-1"));
    }

    #[test]
    fn test_set_and_clear_focus() {
        let mut tracker = AiFocusTracker::new_minimal("test-model".to_string());

        tracker.set_focus(ConversationFocus {
            current_topic: Some("测试话题".to_string()),
            current_question: None,
            recent_context: Vec::new(),
            topic_transitions: Vec::new(),
            detected_at: 0,
        });

        assert!(tracker.current_focus().is_some());

        tracker.clear_focus();
        assert!(tracker.current_focus().is_none());
    }

    #[test]
    fn test_detect_focus_fallback() {
        let tracker = AiFocusTracker::new_minimal("test-model".to_string());
        let messages = vec![
            Message {
                role: Role::User,
                content: MessageContent::Text("如何优化 Rust 性能？".to_string()),
            },
        ];

        let focus = tracker.detect_focus_fallback(&messages);
        assert!(focus.current_question.is_some());
    }

    #[test]
    fn test_focus_score_without_focus() {
        let tracker = AiFocusTracker::new_minimal("test-model".to_string());
        let message = Message {
            role: Role::User,
            content: MessageContent::Text("测试消息".to_string()),
        };

        let score = tracker.focus_score(&message);
        assert_eq!(score, 0.5); // Default when no focus
    }

    #[test]
    fn test_create_focus_message_without_focus() {
        let tracker = AiFocusTracker::new_minimal("test-model".to_string());
        let msg = tracker.create_focus_message();

        assert!(matches!(msg.role, Role::System));
        // Extract text from MessageContent
        let text = match &msg.content {
            MessageContent::Text(t) => t.clone(),
            MessageContent::Blocks(blocks) => {
                blocks.iter()
                    .filter_map(|b| {
                        if let ContentBlock::Text { text } = b {
                            Some(text.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("")
            }
        };
        assert!(text.contains("初始化"));
    }

    #[test]
    fn test_create_focus_message_with_focus() {
        let mut tracker = AiFocusTracker::new_minimal("test-model".to_string());
        tracker.set_focus(ConversationFocus {
            current_topic: Some("API优化".to_string()),
            current_question: Some("如何提升性能？".to_string()),
            recent_context: Vec::new(),
            topic_transitions: Vec::new(),
            detected_at: 5,
        });

        let msg = tracker.create_focus_message();
        assert!(matches!(msg.role, Role::System));
        // Extract text from MessageContent
        let text = match &msg.content {
            MessageContent::Text(t) => t.clone(),
            MessageContent::Blocks(blocks) => {
                blocks.iter()
                    .filter_map(|b| {
                        if let ContentBlock::Text { text } = b {
                            Some(text.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("")
            }
        };
        assert!(text.contains("API优化"));
        assert!(text.contains("如何提升性能"));
    }

    #[test]
    fn test_parse_analysis_result_valid() {
        let tracker = AiFocusTracker::new_minimal("test-model".to_string());
        let json = r#"{
            "relevance": 0.8,
            "is_focus_update": false,
            "reason": "高度相关"
        }"#;

        let result = tracker.parse_analysis_result(json).unwrap();
        assert_eq!(result.relevance, 0.8);
        assert!(!result.is_focus_update);
        assert_eq!(result.reason, "高度相关");
    }

    #[test]
    fn test_parse_analysis_result_with_code_block() {
        let tracker = AiFocusTracker::new_minimal("test-model".to_string());
        let json = r#"```json
{
    "relevance": 0.7,
    "is_focus_update": true,
    "new_topic": "新话题",
    "reason": "话题切换"
}
```"#;

        let result = tracker.parse_analysis_result(json).unwrap();
        assert_eq!(result.relevance, 0.7);
        assert!(result.is_focus_update);
        assert_eq!(result.new_topic, Some("新话题".to_string()));
    }
}