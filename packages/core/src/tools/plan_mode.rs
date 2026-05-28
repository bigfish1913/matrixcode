use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::Mutex;

use super::{Tool, ToolDefinition};
use crate::approval::RiskLevel;

/// Planning mode state
#[derive(Debug, Clone, PartialEq)]
pub enum PlanState {
    None,
    Active,
    Committed,
}

/// Plan info
#[derive(Debug, Clone)]
pub struct PlanInfo {
    pub state: PlanState,
    pub plan_content: String,
    pub files_to_modify: Vec<String>,
    pub created_at: Option<std::time::Instant>,
}

static PLAN_STATE: std::sync::OnceLock<Arc<Mutex<PlanInfo>>> = std::sync::OnceLock::new();

fn get_plan_state() -> Arc<Mutex<PlanInfo>> {
    PLAN_STATE
        .get_or_init(|| {
            Arc::new(Mutex::new(PlanInfo {
                state: PlanState::None,
                plan_content: String::new(),
                files_to_modify: Vec::new(),
                created_at: None,
            }))
        })
        .clone()
}

/// EnterPlanMode tool - enter planning mode for designing implementation
pub struct EnterPlanModeTool;

#[async_trait]
impl Tool for EnterPlanModeTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "enter_plan_mode".to_string(),
            description: "进入规划模式，在执行前设计实现方案。适用于：(1) 需规划的非 trivial 实现任务；(2) 可受益于架构考量的问题；(3) 可能产生重大影响的变更。返回分步计划并识别关键文件。".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
            ..Default::default()
        }
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe // Read-only mode
    }

    async fn execute(&self, _params: Value) -> Result<String> {
        let plan = get_plan_state();
        let mut state = plan.lock().await;

        if state.state == PlanState::Active {
            return Ok(
                "Already in plan mode. Continue planning or use exit_plan_mode to finish."
                    .to_string(),
            );
        }

        state.state = PlanState::Active;
        state.plan_content = String::new();
        state.files_to_modify = Vec::new();
        state.created_at = Some(std::time::Instant::now());

        Ok("Entered plan mode. Design your implementation approach:\n\n1. Analyze the task requirements\n2. Identify key files and components\n3. Consider architectural trade-offs\n4. Create step-by-step implementation plan\n5. Use exit_plan_mode to commit and execute\n\nNote: In plan mode, focus on analysis and design. Tool executions will be limited to read-only operations.".to_string())
    }
}

/// ExitPlanMode tool - exit planning mode and commit plan
pub struct ExitPlanModeTool;

#[async_trait]
impl Tool for ExitPlanModeTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "exit_plan_mode".to_string(),
            description: "退出规划模式。若计划被批准，代理将执行计划的变更；若被拒绝，计划将被丢弃且不做任何修改。注意：用户批准工具执行即视为批准计划。".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "plan": {
                        "type": "string",
                        "description": "要提交的实现计划（可选，若已记录）"
                    },
                    "files_to_modify": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "将要修改的文件列表（可选）"
                    }
                    // NOTE: 'approved' parameter removed - user approval of tool execution = plan approval
                    // This prevents AI from setting approved=false while user approves execution
                }
            }),
            ..Default::default()
        }
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Mutating
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let plan_content = params["plan"].as_str();
        let files_to_modify = params["files_to_modify"].as_array().map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        });
        // User approval of tool execution = plan approval
        // If this tool is executed, it means user approved the plan
        let approved = true;

        let plan = get_plan_state();
        let mut state = plan.lock().await;

        if state.state != PlanState::Active {
            return Ok("Not in plan mode. Use enter_plan_mode first.".to_string());
        }

        // Update plan content if provided
        if let Some(content) = plan_content {
            state.plan_content = content.to_string();
        }
        if let Some(files) = files_to_modify {
            state.files_to_modify = files;
        }

        if approved {
            state.state = PlanState::Committed;

            let files_str = if state.files_to_modify.is_empty() {
                "No specific files identified".to_string()
            } else {
                state.files_to_modify.join(", ")
            };

            Ok(format!(
                "Plan committed. Ready to execute.\n\nPlan: {}\nFiles to modify: {}\n\nNow proceeding with implementation...",
                state.plan_content, files_str
            ))
        } else {
            state.state = PlanState::None;
            state.plan_content.clear();
            state.files_to_modify.clear();

            Ok(
                "Plan rejected and discarded. Returning to normal mode without making changes."
                    .to_string(),
            )
        }
    }
}

/// Check if currently in plan mode
pub fn is_in_plan_mode() -> bool {
    // This is a synchronous check - for async use get_plan_state().lock().await
    false // Placeholder - real check would need async context
}

/// Get current plan state (async)
pub async fn get_current_plan_state() -> PlanState {
    let plan = get_plan_state();
    let state = plan.lock().await;
    state.state.clone()
}
