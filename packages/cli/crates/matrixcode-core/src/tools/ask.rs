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
    "Ask the user a question when you are uncertain or there are multiple options. \
     ALWAYS use this tool when: (1) the user's request is ambiguous, \
     (2) there are multiple viable approaches and you need to choose one, \
     (3) a decision could significantly impact the project. \
     Provide your recommendation with reasoning so the user can make an informed choice. \
     Do NOT guess or proceed without clarification when uncertain.";

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
        let question = params["question"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'question'"))?;

        let options = params["options"].as_array();
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
                "description": "The question to ask the user. Be specific and clear."
            },
            "options": {
                "type": "array",
                "description": "List of available options/choices",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": {
                            "type": "string",
                            "description": "Short identifier for the option (e.g., 'A', 'B', '1', '2')"
                        },
                        "label": {
                            "type": "string",
                            "description": "Human-readable label for the option"
                        },
                        "description": {
                            "type": "string",
                            "description": "Brief description of what this option entails"
                        }
                    },
                    "required": ["id", "label"]
                }
            },
            "recommendation": {
                "type": "object",
                "description": "Your recommended option with reasoning",
                "properties": {
                    "option_id": {
                        "type": "string",
                        "description": "The id of your recommended option"
                    },
                    "reason": {
                        "type": "string",
                        "description": "Why you recommend this option"
                    }
                },
                "required": ["option_id", "reason"]
            }
        },
        "required": ["question"]
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