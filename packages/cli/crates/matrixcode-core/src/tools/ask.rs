//! Ask tool: allow the AI to proactively ask the user for clarification or choice.
//!
//! When the AI encounters ambiguity, multiple options, or uncertain decisions,
//! it can use this tool to pause and ask the user instead of guessing.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};
use std::io::{self, BufRead, Write as _};

use super::{Tool, ToolDefinition};
use crate::approval::RiskLevel;

// ============================================================================
// Tool Definition
// ============================================================================

pub struct AskTool;

const ASK_TOOL_DESCRIPTION: &str =
    "当遇到不确定或多种选择时，向用户提问以获取明确指示。\
     必须使用此工具的情况：(1) 用户请求含义模糊，(2) 存在多种可行方案需要选择，\
     (3) 决策可能对项目产生重大影响。\
     提供推荐方案及理由，让用户做出知情选择。\
     不确定时切勿猜测或直接推进。\
     支持单问题或多问题模式，多问题时用户可用 Tab 切换。";

#[async_trait]
impl Tool for AskTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "ask".to_string(),
            description: ASK_TOOL_DESCRIPTION.to_string(),
            parameters: ask_tool_schema(),
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        // Check for multi-question format
        if let Some(questions) = params.get("questions").and_then(|q| q.as_array()) {
            // Multi-question mode - render all questions
            render_multi_questions(questions);
            let answer = read_user_answer();
            println!();
            return Ok(answer);
        }

        // Single question mode
        let question = params["question"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'question'"))?;

        let options = params.get("options").and_then(|o| o.get("options")).and_then(|o| o.as_array())
            .or_else(|| params["options"].as_array());
        let recommendation = params["recommendation"].as_object();

        // Render the question UI
        render_question_ui(question, options, recommendation);

        // Read user answer
        let answer = read_user_answer();
        println!();

        Ok(answer)
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }
}

/// Get the JSON schema for ask tool parameters.
fn ask_tool_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "question": {
                "type": "string",
                "description": "要向用户提问的问题，需具体清晰（单问题模式）"
            },
            "options": {
                "type": "object",
                "description": "选项配置（单问题模式）",
                "properties": {
                    "multiSelect": {
                        "type": "boolean",
                        "description": "是否允许多选，默认 false"
                    },
                    "options": {
                        "type": "array",
                        "description": "可选方案列表",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": {
                                    "type": "string",
                                    "description": "选项短标识符（如 'A'、'B'、'1'、'2'）"
                                },
                                "label": {
                                    "type": "string",
                                    "description": "选项的可读标签"
                                },
                                "description": {
                                    "type": "string",
                                    "description": "该选项的简要说明"
                                }
                            },
                            "required": ["id", "label"]
                        }
                    }
                },
                "required": ["options"]
            },
            "recommendation": {
                "type": "object",
                "description": "你的推荐方案及理由",
                "properties": {
                    "option_id": {
                        "type": "string",
                        "description": "推荐选项的标识符"
                    },
                    "reason": {
                        "type": "string",
                        "description": "推荐该方案的理由"
                    }
                },
                "required": ["option_id", "reason"]
            },
            "questions": {
                "type": "array",
                "description": "多问题列表（多问题模式，用户可用 Tab 切换问题）",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": {
                            "type": "string",
                            "description": "问题唯一标识符"
                        },
                        "question": {
                            "type": "string",
                            "description": "问题内容"
                        },
                        "options": {
                            "type": "object",
                            "description": "选项配置",
                            "properties": {
                                "multiSelect": {
                                    "type": "boolean",
                                    "description": "是否允许多选，默认 false"
                                },
                                "options": {
                                    "type": "array",
                                    "description": "可选方案列表",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "id": { "type": "string", "description": "选项标识符" },
                                            "label": { "type": "string", "description": "选项标签" },
                                            "description": { "type": "string", "description": "选项说明" }
                                        },
                                        "required": ["id", "label"]
                                    }
                                }
                            },
                            "required": ["options"]
                        }
                    },
                    "required": ["id", "question"]
                }
            }
        },
        "oneOf": [
            { "required": ["question"] },
            { "required": ["questions"] }
        ]
    })
}

// ============================================================================
// UI Rendering
// ============================================================================

/// Render the question UI with options and recommendation.
fn render_question_ui(
    question: &str,
    options: Option<&Vec<Value>>,
    recommendation: Option<&serde_json::Map<String, Value>>,
) {
    println!();
    println!("┌─ AI 询问 ─────────────────────────────────────────");

    // Print the question
    for line in question.lines() {
        println!("│ {}", line);
    }
    println!("│");

    // Print options if provided
    if let Some(opts) = options
        && !opts.is_empty() {
            render_options(opts);
        }

    // Print recommendation if provided
    if let Some(rec) = recommendation {
        render_recommendation(rec, options);
    }

    println!("│ 请输入你的选择或补充想法：");
    println!("└────────────────────────────────────────────────────");
    print!("> ");
    let _ = io::stdout().flush();
}

/// Render the list of options.
fn render_options(opts: &[Value]) {
    println!("│ 可选方案：");
    for opt in opts {
        let id = opt["id"].as_str().unwrap_or("?");
        let label = opt["label"].as_str().unwrap_or("未命名");
        let desc = opt["description"].as_str();
        if let Some(d) = desc {
            println!("│   {}) {} - {}", id, label, d);
        } else {
            println!("│   {}) {}", id, label);
        }
    }
    println!("│");
}

/// Render multiple questions (CLI mode - non-TUI).
fn render_multi_questions(questions: &[Value]) {
    println!();
    println!("┌─ AI 询问 (共 {} 个问题) ───────────────────────────", questions.len());

    for (idx, q) in questions.iter().enumerate() {
        let question = q["question"].as_str().unwrap_or("");
        println!("│");
        println!("│ 【问题 {}】", idx + 1);
        for line in question.lines() {
            println!("│   {}", line);
        }

        // Render options if provided
        if let Some(opts_obj) = q.get("options") {
            let opts = opts_obj.get("options").and_then(|o| o.as_array());
            if let Some(opts) = opts {
                println!("│   可选方案：");
                for opt in opts {
                    let id = opt["id"].as_str().unwrap_or("?");
                    let label = opt["label"].as_str().unwrap_or("未命名");
                    println!("│     {} - {}", id, label);
                }
            }
        }
    }

    println!("│");
    println!("│ 请依次回答所有问题：");
    println!("└────────────────────────────────────────────────────");
    print!("> ");
    let _ = io::stdout().flush();
}

/// Render the recommendation with reasoning.
fn render_recommendation(
    rec: &serde_json::Map<String, Value>,
    options: Option<&Vec<Value>>,
) {
    let opt_id = rec["option_id"].as_str().unwrap_or("?");
    let reason = rec["reason"].as_str().unwrap_or("无理由");

    // Find the label for the recommended option
    let rec_label = options
        .and_then(|opts| opts.iter().find(|o| o["id"].as_str() == Some(opt_id)))
        .and_then(|o| o["label"].as_str())
        .unwrap_or(opt_id);

    println!("│ 💡 推荐方案：{} ({})", rec_label, opt_id);
    println!("│    理由：{}", reason);
    println!("│");
}

// ============================================================================
// Input Reading
// ============================================================================

/// Read user's answer from stdin.
fn read_user_answer() -> String {
    let stdin = io::stdin();
    let mut line = String::new();
    if stdin.lock().read_line(&mut line).is_err() {
        return "无法读取回答".to_string();
    }
    line.trim().to_string()
}