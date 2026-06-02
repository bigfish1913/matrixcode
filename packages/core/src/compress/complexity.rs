//! Conversation Complexity Analyzer
//! 
//! Analyzes conversation complexity to adapt compression thresholds dynamically.

use crate::providers::{Message, MessageContent};

/// Conversation complexity level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComplexityLevel {
    /// Technical discussion with code, errors, tools
    High,
    /// Mixed technical and general conversation
    Medium,
    /// Casual conversation, simple questions
    Low,
}

/// Complexity analyzer configuration
#[derive(Debug, Clone)]
pub struct ComplexityConfig {
    /// Weight for code presence
    code_weight: f32,
    /// Weight for tool usage
    tool_weight: f32,
    /// Weight for technical keywords
    keyword_weight: f32,
    /// Weight for error mentions
    error_weight: f32,
    /// Threshold for high complexity
    high_threshold: f32,
    /// Threshold for medium complexity
    medium_threshold: f32,
}

impl Default for ComplexityConfig {
    fn default() -> Self {
        Self {
            code_weight: 0.3,
            tool_weight: 0.25,
            keyword_weight: 0.15,
            error_weight: 0.2,
            high_threshold: 5.0,
            medium_threshold: 2.0,
        }
    }
}

/// Analyzes conversation complexity
pub struct ComplexityAnalyzer {
    config: ComplexityConfig,
    /// Technical keywords (中英文)
    tech_keywords: Vec<String>,
}

impl Default for ComplexityAnalyzer {
    fn default() -> Self {
        Self::new(ComplexityConfig::default())
    }
}

impl ComplexityAnalyzer {
    pub fn new(config: ComplexityConfig) -> Self {
        Self {
            config,
            tech_keywords: vec![
                // 中文关键词
                "函数".to_string(),
                "优化".to_string(),
                "性能".to_string(),
                "错误".to_string(),
                "测试".to_string(),
                "架构".to_string(),
                "数据库".to_string(),
                "算法".to_string(),
                "重构".to_string(),
                "调试".to_string(),
                "部署".to_string(),
                "缓存".to_string(),
                "并发".to_string(),
                "异步".to_string(),
                // 英文关键词
                "function".to_string(),
                "optimize".to_string(),
                "performance".to_string(),
                "error".to_string(),
                "test".to_string(),
                "architecture".to_string(),
                "database".to_string(),
                "algorithm".to_string(),
                "refactor".to_string(),
                "debug".to_string(),
                "deploy".to_string(),
                "cache".to_string(),
                "async".to_string(),
                "concurrent".to_string(),
            ],
        }
    }

    /// Analyze conversation complexity
    pub fn analyze(messages: &[Message]) -> ComplexityLevel {
        let analyzer = Self::default();
        analyzer.analyze_complexity(messages)
    }

    /// Calculate complexity score
    pub fn analyze_complexity(&self, messages: &[Message]) -> ComplexityLevel {
        if messages.is_empty() {
            return ComplexityLevel::Low;
        }

        let mut score = 0.0;

        // 1. Code density detection
        let code_count = messages.iter()
            .filter(|m| self.has_code(m))
            .count();
        score += code_count as f32 * self.config.code_weight;

        // 2. Tool usage frequency
        let tool_count = messages.iter()
            .filter(|m| self.has_tool_use(m))
            .count();
        score += tool_count as f32 * self.config.tool_weight;

        // 3. Technical keyword density
        let keyword_hits = messages.iter()
            .map(|m| self.count_keywords(m))
            .sum::<usize>();
        score += keyword_hits as f32 * self.config.keyword_weight;

        // 4. Error mentions
        let error_count = messages.iter()
            .filter(|m| self.has_error(m))
            .count();
        score += error_count as f32 * self.config.error_weight;

        // Normalize by message count
        score /= messages.len() as f32;

        // Determine level
        if score >= self.config.high_threshold {
            ComplexityLevel::High
        } else if score >= self.config.medium_threshold {
            ComplexityLevel::Medium
        } else {
            ComplexityLevel::Low
        }
    }

    /// Check if message contains code
    fn has_code(&self, message: &Message) -> bool {
        let content = self.get_text_content(message);
        content.contains("```") || 
            content.contains("fn ") || 
            content.contains("function ") ||
            content.contains("class ") ||
            content.contains("struct ")
    }

    /// Check if message has tool use
    fn has_tool_use(&self, message: &Message) -> bool {
        matches!(message.content, MessageContent::Blocks(_)) ||
            self.get_text_content(message).contains("tool") ||
            self.get_text_content(message).contains("工具")
    }

    /// Check if message mentions error
    fn has_error(&self, message: &Message) -> bool {
        let content = self.get_text_content(message);
        content.contains("error") ||
            content.contains("failed") ||
            content.contains("错误") ||
            content.contains("失败") ||
            content.contains("异常") ||
            content.contains("exception")
    }

    /// Count technical keywords in message
    fn count_keywords(&self, message: &Message) -> usize {
        let content = self.get_text_content(message).to_lowercase();
        self.tech_keywords.iter()
            .filter(|kw| content.contains(&kw.to_lowercase()))
            .count()
    }

    /// Extract text content from message
    fn get_text_content(&self, message: &Message) -> String {
        match &message.content {
            MessageContent::Text(text) => text.clone(),
            MessageContent::Blocks(blocks) => {
                blocks.iter()
                    .filter_map(|block| {
                        if let crate::providers::ContentBlock::Text { text } = block {
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

    /// Get complexity description
    pub fn complexity_description(level: ComplexityLevel) -> &'static str {
        match level {
            ComplexityLevel::High => "技术讨论密集：大量代码、工具使用、错误处理",
            ComplexityLevel::Medium => "混合对话：部分技术内容",
            ComplexityLevel::Low => "简单对话：少量技术内容",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::Role;

    #[test]
    fn test_empty_messages() {
        let level = ComplexityAnalyzer::analyze(&[]);
        assert_eq!(level, ComplexityLevel::Low);
    }

    #[test]
    fn test_high_complexity() {
        // Use analyzer with lower thresholds for testing
        let config = ComplexityConfig {
            high_threshold: 0.5,
            medium_threshold: 0.3,
            ..Default::default()
        };
        let analyzer = ComplexityAnalyzer::new(config);

        let messages = vec![
            Message {
                role: Role::User,
                content: MessageContent::Text("这个函数性能有问题，需要优化算法".to_string()),
            },
            Message {
                role: Role::Assistant,
                content: MessageContent::Text("好的，我来优化这个函数:\n```rust\nfn optimize() {}\n```".to_string()),
            },
            Message {
                role: Role::User,
                content: MessageContent::Text("测试失败了，出现错误".to_string()),
            },
        ];

        let level = analyzer.analyze_complexity(&messages);
        assert_eq!(level, ComplexityLevel::High);
    }

    #[test]
    fn test_medium_complexity() {
        // Use analyzer with very low thresholds for testing
        let config = ComplexityConfig {
            high_threshold: 0.5,
            medium_threshold: 0.05,  // Very low threshold
            ..Default::default()
        };
        let analyzer = ComplexityAnalyzer::new(config);

        let messages = vec![
            Message {
                role: Role::User,
                content: MessageContent::Text("如何在数据库中查询数据？".to_string()),
            },
            Message {
                role: Role::Assistant,
                content: MessageContent::Text("你可以使用 SQL 查询".to_string()),
            },
        ];

        let level = analyzer.analyze_complexity(&messages);
        assert_eq!(level, ComplexityLevel::Medium);
    }

    #[test]
    fn test_low_complexity() {
        let messages = vec![
            Message {
                role: Role::User,
                content: MessageContent::Text("你好".to_string()),
            },
            Message {
                role: Role::Assistant,
                content: MessageContent::Text("你好！有什么可以帮助你的？".to_string()),
            },
        ];
        
        let level = ComplexityAnalyzer::analyze(&messages);
        assert_eq!(level, ComplexityLevel::Low);
    }
}