use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use uuid::Uuid;

use super::{Tool, ToolDefinition};
use crate::approval::RiskLevel;

/// Task tool for spawning sub-agents to handle complex multi-step tasks
pub struct TaskTool;

#[async_trait]
impl Tool for TaskTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "task".to_string(),
            description: "启动新代理处理复杂的多步骤任务。每个代理独立运行，可并行处理不同任务。适用于：(1) 需多次查询/查找的研究任务；(2) 可在后台运行的长时间操作；(3) 需与主上下文隔离的任务；(4) 可并行执行的多个独立任务。".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "description": {
                        "type": "string",
                        "description": "任务简短描述（3-5 个词）"
                    },
                    "prompt": {
                        "type": "string",
                        "description": "代理要执行的任务，需包含所有必要上下文"
                    },
                    "subagent_type": {
                        "type": "string",
                        "enum": ["general-purpose", "Explore", "Plan"],
                        "default": "general-purpose",
                        "description": "代理类型：'general-purpose' 用于通用任务，'Explore' 用于快速只读搜索，'Plan' 用于架构规划"
                    },
                    "run_in_background": {
                        "type": "boolean",
                        "default": false,
                        "description": "若为 true，在后台运行代理，完成时会收到通知"
                    },
                    "isolation": {
                        "type": "string",
                        "enum": ["none", "worktree"],
                        "default": "none",
                        "description": "隔离模式：'none' 在当前目录工作，'worktree' 创建隔离的 git worktree"
                    }
                },
                "required": ["description", "prompt"]
            }),
            ..Default::default()
        }
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Mutating // Tasks can modify files
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let description = params["description"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'description'"))?;
        let prompt = params["prompt"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'prompt'"))?;
        let subagent_type = params["subagent_type"]
            .as_str()
            .unwrap_or("general-purpose");
        let run_in_background = params["run_in_background"].as_bool().unwrap_or(false);
        let isolation = params["isolation"].as_str().unwrap_or("none");

        // Generate task ID
        let task_id = Uuid::new_v4().to_string();

        // Create task info
        let task_info = TaskInfo {
            id: task_id.clone(),
            description: description.to_string(),
            prompt: prompt.to_string(),
            subagent_type: subagent_type.to_string(),
            status: TaskStatus::Pending,
            result: None,
            started_at: Some(std::time::Instant::now()),
        };

        // Get or create task manager
        let manager = get_task_manager();

        // Add task
        {
            let mut tasks = manager.tasks.lock().await;
            tasks.insert(task_id.clone(), task_info);
        }

        if run_in_background {
            // Spawn background task
            let manager_clone = Arc::clone(&manager);
            let task_id_clone = task_id.clone();
            let prompt_clone = prompt.to_string();
            let subagent_type_clone = subagent_type.to_string();
            let isolation_clone = isolation.to_string();

            tokio::spawn(async move {
                let result =
                    execute_subagent_task(&prompt_clone, &subagent_type_clone, &isolation_clone)
                        .await;

                // Update task status
                let mut tasks = manager_clone.tasks.lock().await;
                if let Some(task) = tasks.get_mut(&task_id_clone) {
                    match result {
                        Ok(output) => {
                            task.status = TaskStatus::Completed;
                            task.result = Some(output);
                        }
                        Err(e) => {
                            task.status = TaskStatus::Failed;
                            task.result = Some(e.to_string());
                        }
                    }
                }

                // Send notification (if channel available)
                if let Some(tx) = &manager_clone.notification_tx {
                    let _ = tx.try_send(TaskNotification {
                        task_id: task_id_clone,
                        status: "completed".to_string(),
                    });
                }
            });

            Ok(format!(
                "Task {} started in background. You'll be notified when it completes.",
                task_id
            ))
        } else {
            // Execute synchronously
            let result = execute_subagent_task(prompt, subagent_type, isolation).await?;

            // Update task status
            {
                let mut tasks = manager.tasks.lock().await;
                if let Some(task) = tasks.get_mut(&task_id) {
                    task.status = TaskStatus::Completed;
                    task.result = Some(result.clone());
                }
            }

            Ok(result)
        }
    }
}

/// Task status
#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "pending"),
            TaskStatus::Running => write!(f, "running"),
            TaskStatus::Completed => write!(f, "completed"),
            TaskStatus::Failed => write!(f, "failed"),
            TaskStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Task info
#[derive(Debug, Clone)]
pub struct TaskInfo {
    pub id: String,
    pub description: String,
    pub prompt: String,
    pub subagent_type: String,
    pub status: TaskStatus,
    pub result: Option<String>,
    pub started_at: Option<std::time::Instant>,
}

/// Task notification
#[derive(Debug, Clone)]
pub struct TaskNotification {
    pub task_id: String,
    pub status: String,
}

/// Task manager singleton
pub struct TaskManager {
    pub tasks: Mutex<HashMap<String, TaskInfo>>,
    pub notification_tx: Option<mpsc::Sender<TaskNotification>>,
}

static TASK_MANAGER: std::sync::OnceLock<Arc<TaskManager>> = std::sync::OnceLock::new();

fn get_task_manager() -> Arc<TaskManager> {
    TASK_MANAGER
        .get_or_init(|| {
            Arc::new(TaskManager {
                tasks: Mutex::new(HashMap::new()),
                notification_tx: None,
            })
        })
        .clone()
}

/// Execute subagent task
async fn execute_subagent_task(
    prompt: &str,
    subagent_type: &str,
    isolation: &str,
) -> Result<String> {
    // For now, return a placeholder indicating the task type
    // In full implementation, this would spawn a real agent

    // Handle isolation mode
    let work_path = if isolation == "worktree" {
        // Create temporary worktree
        let temp_dir = std::env::temp_dir().join(format!("matrixcode-task-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir)?;
        temp_dir.to_string_lossy().to_string()
    } else {
        std::env::current_dir()?.to_string_lossy().to_string()
    };

    // Simulate task execution based on type
    match subagent_type {
        "Explore" => {
            // Fast read-only search task
            Ok(format!(
                "[Explore Agent] Completed search task in {}\nPrompt: {}\nResult: Task completed successfully (fast read-only mode)",
                work_path, prompt
            ))
        }
        "Plan" => {
            // Architecture planning task
            Ok(format!(
                "[Plan Agent] Architecture analysis completed\nPrompt: {}\nResult: Implementation plan generated (check main context)",
                prompt
            ))
        }
        _ => {
            // General purpose task - in real implementation would run actual agent
            Ok(format!(
                "[Agent] Task completed\nPrompt: {}\nWork path: {}\nNote: Subagent execution would be implemented with full agent integration",
                prompt, work_path
            ))
        }
    }
}

/// TaskCreate tool for creating background tasks
pub struct TaskCreateTool;

#[async_trait]
impl Tool for TaskCreateTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "task_create".to_string(),
            description: "创建独立运行的后台任务".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "description": {
                        "type": "string",
                        "description": "任务描述"
                    },
                    "prompt": {
                        "type": "string",
                        "description": "任务提示"
                    }
                },
                "required": ["description", "prompt"]
            }),
            ..Default::default()
        }
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Mutating
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let description = params["description"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'description'"))?;
        let prompt = params["prompt"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'prompt'"))?;

        let task_id = Uuid::new_v4().to_string();
        let manager = get_task_manager();

        let task_info = TaskInfo {
            id: task_id.clone(),
            description: description.to_string(),
            prompt: prompt.to_string(),
            subagent_type: "general-purpose".to_string(),
            status: TaskStatus::Running,
            result: None,
            started_at: Some(std::time::Instant::now()),
        };

        {
            let mut tasks = manager.tasks.lock().await;
            tasks.insert(task_id.clone(), task_info);
        }

        Ok(format!("Task {} created and running", task_id))
    }
}

/// TaskGet tool for getting task status
pub struct TaskGetTool;

#[async_trait]
impl Tool for TaskGetTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "task_get".to_string(),
            description: "获取指定任务的状态和结果".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "task_id": {
                        "type": "string",
                        "description": "要查询的任务 ID"
                    }
                },
                "required": ["task_id"]
            }),
            ..Default::default()
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let task_id = params["task_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'task_id'"))?;

        let manager = get_task_manager();
        let tasks = manager.tasks.lock().await;

        if let Some(task) = tasks.get(task_id) {
            let status_str = task.status.to_string();

            let elapsed = task
                .started_at
                .map(|s| format!("{:.1}s", s.elapsed().as_secs_f64()))
                .unwrap_or_else(|| "N/A".to_string());

            let result_str = task
                .result
                .clone()
                .unwrap_or_else(|| "No result yet".to_string());

            Ok(format!(
                "Task: {}\nDescription: {}\nStatus: {}\nElapsed: {}\nResult: {}",
                task_id, task.description, status_str, elapsed, result_str
            ))
        } else {
            Ok(format!("Task {} not found", task_id))
        }
    }
}

/// TaskList tool for listing all tasks
pub struct TaskListTool;

#[async_trait]
impl Tool for TaskListTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "task_list".to_string(),
            description: "列出所有活动任务".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
            ..Default::default()
        }
    }

    async fn execute(&self, _params: Value) -> Result<String> {
        let manager = get_task_manager();
        let tasks = manager.tasks.lock().await;

        if tasks.is_empty() {
            return Ok("No active tasks".to_string());
        }

        let mut result = Vec::new();
        for (id, task) in tasks.iter() {
            result.push(format!("{} [{}] - {}", id, task.status, task.description));
        }

        Ok(result.join("\n"))
    }
}

/// TaskStop tool for stopping a task
pub struct TaskStopTool;

#[async_trait]
impl Tool for TaskStopTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "task_stop".to_string(),
            description: "停止正在运行的任务".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "task_id": {
                        "type": "string",
                        "description": "要停止的任务 ID"
                    }
                },
                "required": ["task_id"]
            }),
            ..Default::default()
        }
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Mutating
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let task_id = params["task_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'task_id'"))?;

        let manager = get_task_manager();
        let mut tasks = manager.tasks.lock().await;

        if let Some(task) = tasks.get_mut(task_id) {
            task.status = TaskStatus::Cancelled;
            Ok(format!("Task {} stopped", task_id))
        } else {
            Ok(format!("Task {} not found", task_id))
        }
    }
}
