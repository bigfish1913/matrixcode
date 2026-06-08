//! `agent` tool - Spawn subagents to execute tasks.
//!
//! This tool allows the model to spawn independent subagent instances
//! for parallel task execution, matching Claude Code's Agent tool interface.

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};

use super::{Tool, ToolDefinition, subagent_tools_arc};
use crate::tools::subagent_executor::{SubagentExecutor, SubagentTask, SubagentConfig};
use crate::event::AgentEvent;
use crate::skills::Skill;
use tokio::sync::mpsc;

/// Agent tool - spawns subagents for task execution
pub struct AgentTool {
    /// Event channel for forwarding subagent events
    event_tx: Option<mpsc::Sender<AgentEvent>>,
    /// Skills for subagent tool creation
    skills: Arc<Vec<Skill>>,
}

impl AgentTool {
    /// Create a new Agent tool
    pub fn new() -> Self {
        Self {
            event_tx: None,
            skills: Arc::new(Vec::new()),
        }
    }

    /// Create with event channel for event forwarding
    pub fn with_event_tx(event_tx: mpsc::Sender<AgentEvent>) -> Self {
        Self {
            event_tx: Some(event_tx),
            skills: Arc::new(Vec::new()),
        }
    }

    /// Create with skills (for subagent tool creation)
    pub fn with_skills(skills: Arc<Vec<Skill>>) -> Self {
        Self {
            event_tx: None,
            skills,
        }
    }

    /// Create with both event channel and skills
    pub fn with_event_tx_and_skills(
        event_tx: mpsc::Sender<AgentEvent>,
        skills: Arc<Vec<Skill>>,
    ) -> Self {
        Self {
            event_tx: Some(event_tx),
            skills,
        }
    }
}

impl Default for AgentTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for AgentTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "Agent".to_string(),
            description: "启动一个新的子代理来处理复杂、多步骤任务。每个子代理类型有特定能力和工具。

可用的子代理类型：
- claude: 通用代理，适合任何任务（默认）
- Explore: 只读搜索代理，用于广泛搜索文件/目录/命名约定
- Plan: 软件架构代理，用于设计实现计划
- general-purpose: 通用代理，用于复杂搜索和多步骤任务

使用场景：
- 当任务匹配可用代理类型时
- 当有独立工作可并行运行时
- 当回答需要跨多个文件搜索时

重要：
- 代理的最终消息作为工具结果返回
- 子代理类型通过 subagent_type 参数选择
- 可选 isolation: \"worktree\" 创建独立 git 工树
- 可选 run_in_background: true 异步运行代理".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "subagent_type": {
                        "type": "string",
                        "description": "子代理类型（默认 general-purpose）",
                        "enum": ["claude", "Explore", "Plan", "general-purpose"],
                        "default": "general-purpose"
                    },
                    "description": {
                        "type": "string",
                        "description": "任务的简短描述（3-5词）"
                    },
                    "prompt": {
                        "type": "string",
                        "description": "子代理要执行的任务"
                    },
                    "isolation": {
                        "type": "string",
                        "enum": ["worktree"],
                        "description": "隔离模式。\"worktree\" 创建临时 git 工树"
                    },
                    "run_in_background": {
                        "type": "boolean",
                        "default": false,
                        "description": "设为 true 异步运行代理"
                    }
                },
                "required": ["description", "prompt"]
            }),
            ..Default::default()
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let subagent_type = params["subagent_type"]
            .as_str()
            .unwrap_or("general-purpose");

        let description = params["description"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'description'"))?;

        let prompt = params["prompt"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'prompt'"))?;

        let isolation = params["isolation"]
            .as_str()
            .unwrap_or("none");

        let run_in_background = params["run_in_background"]
            .as_bool()
            .unwrap_or(false);

        // Create subagent task
        let task = SubagentTask {
            id: uuid::Uuid::new_v4().to_string(),
            description: description.to_string(),
            prompt: prompt.to_string(),
            subagent_type: subagent_type.to_string(),
            isolation: isolation.to_string(),
            work_path: None,
        };

        // Create event channel if not provided
        let (event_tx, _event_rx) = mpsc::channel::<AgentEvent>(100);
        let tx = self.event_tx.clone().unwrap_or(event_tx);

        // Configure subagent based on type
        let config = match subagent_type {
            "Explore" => SubagentConfig {
                model_name: "claude-sonnet-4-20250514".to_string(),
                max_tokens: 4096,
                system_prompt_prefix: Some("You are a fast, read-only search agent.".to_string()),
                think: false,
                tool_names: Some(vec![
                    "read".to_string(), "grep".to_string(), "glob".to_string(),
                    "ls".to_string(), "search".to_string(),
                    "code_search".to_string(), "code_callers".to_string(),
                    "code_callees".to_string(), "code_status".to_string(),
                ]),
            },
            "Plan" => SubagentConfig {
                model_name: "claude-sonnet-4-20250514".to_string(),
                max_tokens: 8192,
                system_prompt_prefix: Some("You are a software architecture planning agent.".to_string()),
                think: true,
                tool_names: Some(vec![
                    "read".to_string(), "grep".to_string(), "glob".to_string(),
                    "ls".to_string(), "search".to_string(),
                    "code_search".to_string(), "code_callers".to_string(),
                    "code_callees".to_string(), "code_status".to_string(),
                    "enter_plan_mode".to_string(), "exit_plan_mode".to_string(),
                    "todo_write".to_string(),
                ]),
            },
            _ => SubagentConfig {
                model_name: "claude-sonnet-4-20250514".to_string(),
                max_tokens: 4096,
                system_prompt_prefix: None,
                think: false,
                tool_names: None,
            },
        };

        if run_in_background {
            // Spawn background task
            let task_clone = task.clone();
            let config_clone = config.clone();
            let tools = subagent_tools_arc(self.skills.clone());

            tokio::spawn(async move {
                let mut executor = SubagentExecutor::new(config_clone, tx, tools);
                let result = executor.execute(task_clone).await;

                if let Ok(r) = result {
                    log::info!("Background agent task {} completed: success={}",
                        r.task_id, r.success);
                }
            });

            Ok(format!("Agent task '{}' started in background. Task ID: {}",
                description, task.id))
        } else {
            // Execute synchronously
            let tools = subagent_tools_arc(self.skills.clone());
            let mut executor = SubagentExecutor::new(config, tx, tools);
            let result = executor.execute(task).await?;

            if result.success {
                Ok(result.content)
            } else {
                Ok(format!("Agent task failed: {}", result.content))
            }
        }
    }

    fn risk_level(&self) -> crate::approval::RiskLevel {
        // Agent tool spawns independent processes, medium risk
        crate::approval::RiskLevel::Mutating
    }
}