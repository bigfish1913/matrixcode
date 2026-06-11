//! Semantic compression using AI summarization.
//!
//! Instead of simple truncation, this module uses a small model to summarize
//! historical messages while preserving key information.

use crate::providers::{Message, MessageContent, Role};
use crate::compress::hardcode_config::HardcodeConfig;
use super::prompts_zh::SUMMARY_PROMPT;
use serde::{Deserialize, Serialize};

/// Summary of a conversation segment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationSummary {
    /// Key decisions made
    pub decisions: Vec<String>,
    /// Important facts discovered
    pub facts: Vec<String>,
    /// Tools used and their results
    pub tool_usage: Vec<ToolUsage>,
    /// Errors encountered and how they were resolved
    pub issues: Vec<Issue>,
    /// Overall summary text
    pub summary: String,
}

/// Record of tool usage in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUsage {
    pub tool_name: String,
    pub purpose: String,
    pub outcome: String,
}

/// Record of an issue and its resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub problem: String,
    pub solution: String,
}

/// Semantic compressor that uses AI to summarize messages.
#[derive(Default)]
pub struct SemanticCompressor {
    /// Hardcode configuration
    hardcode_config: HardcodeConfig,
}


impl SemanticCompressor {
    pub fn new() -> Self {
        Self {
            hardcode_config: HardcodeConfig::default(),
        }
    }

    /// Extract key information from a message.
    pub fn extract_key_info(message: &Message) -> KeyInfo {
        let mut info = KeyInfo::default();

        // Check content for important patterns
        if let MessageContent::Text(text) = &message.content {
            // Detect decisions (中英文)
            if text.contains("decided") || text.contains("decision") 
                || text.contains("决定") || text.contains("choose") || text.contains("selected") {
                info.has_decision = true;
            }

            // Detect errors (中英文)
            if text.contains("error") || text.contains("failed") 
                || text.contains("错误") || text.contains("失败") || text.contains("异常") {
                info.has_error = true;
            }

            // Detect tool usage
            if text.contains("tool") || text.contains("function") {
                info.has_tool_use = true;
            }

            // Detect code
            if text.contains("```") || text.contains("fn ") || text.contains("function ") {
                info.has_code = true;
            }
        }

        // Check for tool blocks
        if let MessageContent::Blocks(blocks) = &message.content {
            for block in blocks {
                match block {
                    crate::providers::ContentBlock::ToolUse { name, .. } => {
                        info.tool_names.push(name.clone());
                        info.has_tool_use = true;
                    }
                    crate::providers::ContentBlock::ToolResult { content, .. } => {
                        if content.contains("error") || content.contains("failed") {
                            info.has_error = true;
                        }
                    }
                    _ => {}
                }
            }
        }

        info
    }

    /// Check if messages should be semantically compressed.
    pub fn should_summarize(&self, messages: &[Message]) -> bool {
        if messages.is_empty() {
            return false;
        }

        // Check if there's enough content to summarize
        let has_substantial_content = messages.iter().any(|m| {
            matches!(&m.content, MessageContent::Text(t) if t.len() > self.hardcode_config.summary_length_threshold)
        });

        // Check if there are multiple messages
        has_substantial_content && messages.len() >= 3
    }

    /// Generate a summary prompt for the messages.
    pub fn create_summary_prompt(messages: &[Message]) -> String {
        let mut conversation = String::new();
        
        for msg in messages {
            let role = match msg.role {
                Role::User => "用户",
                Role::Assistant => "助手",
                Role::System => "系统",
                Role::Tool => "工具",
            };

            if let MessageContent::Text(text) = &msg.content {
                conversation.push_str(&format!("{}: {}\n", role, text));
            } else if let MessageContent::Blocks(blocks) = &msg.content {
                for block in blocks {
                    if let crate::providers::ContentBlock::Text { text } = block {
                        conversation.push_str(&format!("{}: {}\n", role, text));
                    }
                }
            }
        }

        SUMMARY_PROMPT.replace("{conversation}", &conversation)
    }

    /// Create a compressed summary message.
    pub fn create_summary_message(summary: ConversationSummary) -> Message {
        let mut content = String::new();
        content.push_str("📝 **对话摘要**\n\n");

        if !summary.decisions.is_empty() {
            content.push_str("**决策：**\n");
            for decision in &summary.decisions {
                content.push_str(&format!("- {}\n", decision));
            }
            content.push('\n');
        }

        if !summary.facts.is_empty() {
            content.push_str("**关键事实：**\n");
            for fact in &summary.facts {
                content.push_str(&format!("- {}\n", fact));
            }
            content.push('\n');
        }

        if !summary.tool_usage.is_empty() {
            content.push_str("**使用的工具：**\n");
            for tool in &summary.tool_usage {
                content.push_str(&format!("- {}: {}\n", tool.tool_name, tool.outcome));
            }
            content.push('\n');
        }

        if !summary.issues.is_empty() {
            content.push_str("**解决的问题：**\n");
            for issue in &summary.issues {
                content.push_str(&format!("- 问题: {}\n  解决: {}\n", issue.problem, issue.solution));
            }
            content.push('\n');
        }

        content.push_str(&format!("**Overall:** {}", summary.summary));

        Message {
            role: Role::System,
            content: MessageContent::Text(content),
        }
    }
}

/// Key information extracted from a message.
#[derive(Debug, Default)]
pub struct KeyInfo {
    pub has_decision: bool,
    pub has_error: bool,
    pub has_tool_use: bool,
    pub has_code: bool,
    pub tool_names: Vec<String>,
}

/// Strategy for semantic compression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum SemanticStrategy {
    /// Don't use semantic compression (truncate only)
    None,
    /// Use semantic compression for old messages
    #[default]
    OldOnly,
    /// Use semantic compression for all compressible messages
    Aggressive,
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::{ContentBlock, Message, MessageContent, Role};

    #[test]
    fn test_extract_key_info_decision() {
        let msg = Message {
            role: Role::Assistant,
            content: MessageContent::Text("I decided to use Rust for the project.".to_string()),
        };
        let info = SemanticCompressor::extract_key_info(&msg);
        assert!(info.has_decision);
    }

    #[test]
    fn test_extract_key_info_error() {
        let msg = Message {
            role: Role::Assistant,
            content: MessageContent::Text("The operation failed with error code 404.".to_string()),
        };
        let info = SemanticCompressor::extract_key_info(&msg);
        assert!(info.has_error);
    }

    #[test]
    fn test_extract_key_info_tool() {
        let msg = Message {
            role: Role::Assistant,
            content: MessageContent::Blocks(vec![ContentBlock::ToolUse {
                id: "tool_1".to_string(),
                name: "bash".to_string(),
                input: serde_json::json!({"command": "ls"}),
            }]),
        };
        let info = SemanticCompressor::extract_key_info(&msg);
        assert!(info.has_tool_use);
        assert!(info.tool_names.contains(&"bash".to_string()));
    }

    #[test]
    fn test_should_summarize() {
        // Too few messages
        let messages = vec![Message {
            role: Role::User,
            content: MessageContent::Text("Hello".to_string()),
        }];
        let compressor = SemanticCompressor::default();
        assert!(!compressor.should_summarize(&messages));

        // Enough messages with substantial content (需要超过 200 字符)
        let messages = vec![
            Message {
                role: Role::User,
                content: MessageContent::Text("This is a longer message with more than two hundred characters to test the substantial content check. We need to make sure it's long enough. Adding more text to ensure the message has sufficient length for the test requirement.".to_string()),
            },
            Message {
                role: Role::Assistant,
                content: MessageContent::Text("Response 1".to_string()),
            },
            Message {
                role: Role::User,
                content: MessageContent::Text("Query 2".to_string()),
            },
        ];
        let compressor = SemanticCompressor::default();
        assert!(compressor.should_summarize(&messages));
    }

    #[test]
    fn test_create_summary_message() {
        let summary = ConversationSummary {
            decisions: vec!["Use Rust for backend".to_string()],
            facts: vec!["Project uses PostgreSQL".to_string()],
            tool_usage: vec![ToolUsage {
                tool_name: "bash".to_string(),
                purpose: "Run tests".to_string(),
                outcome: "All tests passed".to_string(),
            }],
            issues: vec![Issue {
                problem: "Compilation error".to_string(),
                solution: "Fixed missing import".to_string(),
            }],
            summary: "Completed initial setup and testing.".to_string(),
        };

        let msg = SemanticCompressor::create_summary_message(summary);
        assert!(matches!(msg.role, Role::System));
        
        if let MessageContent::Text(text) = &msg.content {
            assert!(text.contains("决策"));
            assert!(text.contains("关键事实"));
            assert!(text.contains("使用的工具"));
            assert!(text.contains("解决的问题"));
        } else {
            panic!("Expected text content");
        }
    }
}