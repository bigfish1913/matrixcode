use crate::providers::{Message, MessageContent, Role};
use crate::memory::MemoryEntry;
use super::focus_point::{FocusPoint, FocusStatus};
use super::prompts_zh::{EXTRACTION_PROMPT, CLASSIFICATION_PROMPT};
use chrono::Utc;
use std::collections::HashMap;

/// AI 聚焦点提取器
/// 
/// 负责：
/// 1. 从对话历史中提取聚焦点
/// 2. 判断用户输入属于哪个聚焦点
/// 3. 创建新聚焦点
/// 4. 更新聚焦点状态
pub struct FocusExtractor;

impl FocusExtractor {
    /// 从记忆条目中提取聚焦点
    /// 
    /// AI 在提取记忆时，同时提取聚焦点信息
    pub fn extract_from_memory(memory: &MemoryEntry) -> Option<FocusPoint> {
        // 从记忆的 tags 构建 keywords
        if memory.tags.is_empty() {
            return None;
        }
        
        // 从 content 提取主题（简单实现：取第一句）
        let topic = memory.content
            .split('\n')
            .next()
            .unwrap_or(&memory.content)
            .to_string();
        
        Some(FocusPoint::new(
            format!("focus-{}", memory.id),
            topic,
            memory.tags.clone(),
            vec![],
            None,
            0,
        ).with_importance((memory.importance / 100.0) as f32))
    }
    
    /// 从对话历史提取聚焦点（AI 分析）
    /// 
    /// 返回 JSON 格式的聚焦点信息，供 AI 模型解析
    pub fn create_extraction_prompt(messages: &[Message]) -> String {
        let conversation = Self::format_conversation(messages);
        EXTRACTION_PROMPT.replace("{conversation}", &conversation)
    }
    
    /// 从用户输入判断聚焦点归属
    pub fn create_classification_prompt(user_input: &str, existing_foci: &[FocusPoint]) -> String {
        let foci_description = Self::format_existing_foci(existing_foci);
        CLASSIFICATION_PROMPT
            .replace("{user_input}", user_input)
            .replace("{foci_description}", &foci_description)
    }
    
    /// 格式化对话历史
    fn format_conversation(messages: &[Message]) -> String {
        messages.iter()
            .map(|msg| {
                let role = match msg.role {
                    Role::User => "User",
                    Role::Assistant => "AI",
                    Role::System => "System",
                    Role::Tool => "Tool",
                };
                
                let content = match &msg.content {
                    MessageContent::Text(text) => text.clone(),
                    MessageContent::Blocks(blocks) => {
                        blocks.iter()
                            .filter_map(|b| {
                                match b {
                                    crate::providers::ContentBlock::Text { text } => Some(text.clone()),
                                    _ => None,
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
    
    /// 格式化现有聚焦点
    fn format_existing_foci(foci: &[FocusPoint]) -> String {
        foci.iter()
            .map(|f| {
                format!(
                    "- ID: {}\n  Topic: {}\n  Keywords: {}\n  Entities: {}\n  Status: {}\n  Importance: {}",
                    f.id,
                    f.topic,
                    f.keywords.join(", "),
                    f.entities.join(", "),
                    f.status,
                    f.importance
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }
    
    /// 解析 AI 返回的聚焦点 JSON
    pub fn parse_focus_response(response: &str) -> Result<Vec<FocusPoint>, String> {
        // 尝试提取 JSON 部分
        let json_str = Self::extract_json(response)?;
        
        let parsed: serde_json::Value = serde_json::from_str(&json_str)
            .map_err(|e| format!("JSON parse error: {}", e))?;
        
        let focuses = parsed["focuses"]
            .as_array()
            .ok_or("No focuses array in response")?;
        
        let mut result = Vec::new();
        
        for focus_json in focuses {
            let importance = focus_json["importance"]
                .as_f64()
                .unwrap_or(0.7) as f32;
            
            let focus = FocusPoint::new(
                format!("focus-{}", Utc::now().timestamp()),
                focus_json["topic"]
                    .as_str()
                    .ok_or("Missing topic")?
                    .to_string(),
                focus_json["keywords"]
                    .as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default(),
                focus_json["entities"]
                    .as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default(),
                focus_json["core_question"]
                    .as_str()
                    .map(String::from),
                0,
            ).with_importance(importance);
            
            // Set status
            if focus_json["is_current"].as_bool().unwrap_or(false) {
                result.push(focus);
            } else {
                let mut f = focus;
                f.status = FocusStatus::Suspended;
                result.push(f);
            }
        }
        
        Ok(result)
    }
    
    /// 解析分类响应
    pub fn parse_classification_response(response: &str) -> Result<ClassificationResult, String> {
        let json_str = Self::extract_json(response)?;
        
        let parsed: serde_json::Value = serde_json::from_str(&json_str)
            .map_err(|e| format!("JSON parse error: {}", e))?;
        
        let classification = &parsed["classification"];
        
        let matched_focus_id = classification["matched_focus_id"]
            .as_str()
            .map(String::from);
        
        let relevance_scores = classification["relevance_scores"]
            .as_object()
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| {
                        v.as_f64().map(|score| (k.clone(), score as f32))
                    })
                    .collect()
            })
            .unwrap_or_default();
        
        let is_new_focus = classification["is_new_focus"]
            .as_bool()
            .unwrap_or(false);
        
        let new_focus = if is_new_focus {
            let new_focus_json = &parsed["new_focus"];
            
            Some(FocusPoint::new(
                format!("focus-{}", Utc::now().timestamp()),
                new_focus_json["topic"]
                    .as_str()
                    .ok_or("Missing new focus topic")?
                    .to_string(),
                new_focus_json["keywords"]
                    .as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default(),
                new_focus_json["entities"]
                    .as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default(),
                new_focus_json["core_question"]
                    .as_str()
                    .map(String::from),
                0,
            ))
        } else {
            None
        };
        
        Ok(ClassificationResult {
            matched_focus_id,
            relevance_scores,
            is_new_focus,
            new_focus,
        })
    }
    
    /// 从响应中提取 JSON
    fn extract_json(response: &str) -> Result<String, String> {
        // 尝试找到 JSON 块
        let start = response.find('{')
            .ok_or("No JSON found in response")?;

        let mut depth = 0;
        let mut end = response.len(); // fallback to end of string

        // Use char_indices to get byte positions correctly
        for (byte_idx, ch) in response[start..].char_indices() {
            if ch == '{' {
                depth += 1;
            } else if ch == '}' {
                depth -= 1;
                if depth == 0 {
                    end = start + byte_idx + 1;
                    break;
                }
            }
        }

        Ok(response[start..end].to_string())
    }
}

/// 分类结果
#[derive(Debug, Clone)]
pub struct ClassificationResult {
    /// 匹配的聚焦点 ID
    pub matched_focus_id: Option<String>,
    
    /// 各聚焦点的相关性分数
    pub relevance_scores: HashMap<String, f32>,
    
    /// 是否是新聚焦点
    pub is_new_focus: bool,
    
    /// 新聚焦点（如果是）
    pub new_focus: Option<FocusPoint>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_create_extraction_prompt() {
        let messages = vec![
            Message {
                role: Role::User,
                content: MessageContent::Text("How to optimize Rust performance?".to_string()),
            },
            Message {
                role: Role::Assistant,
                content: MessageContent::Text("Use profiling tools.".to_string()),
            },
        ];
        
        let prompt = FocusExtractor::create_extraction_prompt(&messages);
        
        assert!(prompt.contains("分析对话内容并提取聚焦点"));
        assert!(prompt.contains("optimize Rust performance"));
        assert!(prompt.contains("\"focuses\":"));
    }
    
    #[test]
    fn test_parse_focus_response() {
        let response = r#"Based on the conversation, here are the focus points:
{
  "focuses": [
    {
      "topic": "Optimizing Rust performance",
      "keywords": ["performance", "rust", "optimization"],
      "entities": ["main.rs", "benchmark"],
      "core_question": "How to improve performance?",
      "importance": 0.85,
      "is_current": true
    }
  ]
}
"#;
        
        let result = FocusExtractor::parse_focus_response(response);
        
        assert!(result.is_ok());
        let focuses = result.unwrap();
        assert_eq!(focuses.len(), 1);
        assert_eq!(focuses[0].topic, "Optimizing Rust performance");
        assert_eq!(focuses[0].keywords.len(), 3);
    }
    
    #[test]
    fn test_create_classification_prompt() {
        let existing_foci = vec![
            FocusPoint::new(
                "focus-1".to_string(),
                "Database optimization".to_string(),
                vec!["database".to_string()],
                vec!["db.rs".to_string()],
                Some("Why is query slow?".to_string()),
                0,
            ),
        ];
        
        let prompt = FocusExtractor::create_classification_prompt(
            "The database query is still slow",
            &existing_foci
        );
        
        assert!(prompt.contains("判断用户输入属于哪个聚焦点"));
        assert!(prompt.contains("Database optimization"));
        assert!(prompt.contains("\"relevance_scores\":"));
    }
    
    #[test]
    fn test_parse_classification_response() {
        let response = r#"Classification result:
{
  "classification": {
    "matched_focus_id": "focus-1",
    "relevance_scores": {
      "focus-1": 0.85
    },
    "is_new_focus": false,
    "reason": "Input mentions database"
  }
}
"#;
        
        let result = FocusExtractor::parse_classification_response(response);
        
        assert!(result.is_ok());
        let classification = result.unwrap();
        assert_eq!(classification.matched_focus_id, Some("focus-1".to_string()));
        assert!(!classification.is_new_focus);
    }
}