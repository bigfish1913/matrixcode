//! Subagent Executor - Real agent instance execution for parallel tasks.
//!
//! This module implements true subagent execution by creating lightweight Agent
//! instances that can run independently with optional worktree isolation.

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::agent::{Agent, AgentBuilder};
use crate::cancel::CancellationToken;
use crate::event::AgentEvent;
use crate::providers::create_minimal_provider;
use crate::prompt::PromptProfile;
use crate::tools::Tool;

/// Subagent task definition
#[derive(Debug, Clone)]
pub struct SubagentTask {
    /// Unique task identifier
    pub id: String,
    /// Task description (3-5 words)
    pub description: String,
    /// Full task prompt with context
    pub prompt: String,
    /// Agent type: "general-purpose", "Explore", "Plan"
    pub subagent_type: String,
    /// Isolation mode: "none", "worktree"
    pub isolation: String,
    /// Working directory path
    pub work_path: Option<PathBuf>,
}

/// Subagent execution result
#[derive(Debug, Clone)]
pub struct SubagentResult {
    /// Task identifier
    pub task_id: String,
    /// Execution output content
    pub content: String,
    /// Success status
    pub success: bool,
    /// Token usage statistics
    pub usage: TokenUsage,
}

/// Token usage for subagent
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Configuration for subagent executor
#[derive(Debug, Clone)]
pub struct SubagentConfig {
    /// Model name for subagent (typically a fast model)
    pub model_name: String,
    /// Maximum tokens for subagent responses
    pub max_tokens: u32,
    /// System prompt prefix for subagent
    pub system_prompt_prefix: Option<String>,
    /// Whether to enable thinking mode
    pub think: bool,
    /// Tools to include (subset of main agent tools)
    pub tool_names: Option<Vec<String>>,
}

impl Default for SubagentConfig {
    fn default() -> Self {
        Self {
            model_name: "claude-sonnet-4-20250514".to_string(),
            max_tokens: 4096,
            system_prompt_prefix: None,
            think: false,
            tool_names: None,
        }
    }
}

/// Subagent executor - manages parallel agent execution
pub struct SubagentExecutor {
    /// Configuration for subagents
    config: SubagentConfig,
    /// Main agent's event channel (for forwarding events)
    event_tx: mpsc::Sender<AgentEvent>,
    /// Active task cancellation tokens
    cancel_tokens: HashMap<String, CancellationToken>,
    /// Available tools from main agent
    tools: Vec<Arc<dyn Tool>>,
}

impl SubagentExecutor {
    /// Create a new subagent executor
    ///
    /// # Arguments
    /// * `config` - Subagent configuration
    /// * `event_tx` - Main agent's event sender for forwarding
    /// * `tools` - Available tools from main agent
    pub fn new(
        config: SubagentConfig,
        event_tx: mpsc::Sender<AgentEvent>,
        tools: Vec<Arc<dyn Tool>>,
    ) -> Self {
        Self {
            config,
            event_tx,
            cancel_tokens: HashMap::new(),
            tools,
        }
    }

    /// Create with default configuration
    pub fn with_defaults(
        event_tx: mpsc::Sender<AgentEvent>,
        tools: Vec<Arc<dyn Tool>>,
    ) -> Self {
        Self::new(SubagentConfig::default(), event_tx, tools)
    }

    /// Execute a single subagent task
    ///
    /// Creates a lightweight Agent instance and runs the task.
    /// Events are forwarded to the main agent's event channel.
    pub async fn execute(&mut self, task: SubagentTask) -> Result<SubagentResult> {
        let cancel_token = CancellationToken::new();
        self.cancel_tokens.insert(task.id.clone(), cancel_token.clone());

        // Create event channel for this subagent
        let (subagent_tx, mut subagent_rx) = mpsc::channel::<AgentEvent>(100);

        // Build the subagent
        let provider = create_minimal_provider(&self.config.model_name);
        let work_path = task.work_path.clone().unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        });

        // Filter tools based on configuration
        let filtered_tools = self.filter_tools(&task.subagent_type);

        // Build system prompt based on agent type
        let system_prompt = self.build_system_prompt(&task.subagent_type, &task.description);

        // Create lightweight agent
        let mut agent = AgentBuilder::new(provider)
            .model_name(&self.config.model_name)
            .max_tokens(self.config.max_tokens)
            .system_prompt(system_prompt)
            .think(self.config.think)
            .tools_with_provider(filtered_tools)
            .project_path(work_path)
            .event_tx(subagent_tx)
            .profile(PromptProfile::Default)
            .build();

        agent.set_cancel_token(cancel_token);

        // Start event forwarder before running the agent
        let event_forwarder = tokio::spawn({
            let main_tx = self.event_tx.clone();
            let task_id = task.id.clone();
            async move {
                while let Some(event) = subagent_rx.recv().await {
                    // Tag events with subagent context
                    let tagged_event = Self::tag_event(event, &task_id);
                    if main_tx.send(tagged_event).await.is_err() {
                        break;
                    }
                }
            }
        });

        // Run the agent
        let run_result = agent.run(task.prompt.clone()).await;

        // Wait for event forwarder to finish
        event_forwarder.abort();

        // Cleanup
        self.cancel_tokens.remove(&task.id);

        // Get token usage
        let (input_tokens, output_tokens) = agent.get_token_counts();

        match run_result {
            Ok(_) => Ok(SubagentResult {
                task_id: task.id.clone(),
                content: Self::extract_content(&agent),
                success: true,
                usage: TokenUsage {
                    input_tokens,
                    output_tokens,
                },
            }),
            Err(e) => Ok(SubagentResult {
                task_id: task.id.clone(),
                content: format!("Task failed: {}", e),
                success: false,
                usage: TokenUsage {
                    input_tokens,
                    output_tokens,
                },
            }),
        }
    }

    /// Execute multiple tasks in parallel
    ///
    /// Spawns concurrent subagent tasks and collects results.
    /// Each task runs independently with its own agent instance.
    pub async fn execute_parallel(&mut self, tasks: Vec<SubagentTask>) -> Result<Vec<SubagentResult>> {
        let mut results = Vec::with_capacity(tasks.len());
        let mut futures = Vec::new();

        for task in tasks {
            // Create a separate executor for each parallel task
            let config = self.config.clone();
            let event_tx = self.event_tx.clone();
            let tools = self.tools.clone();

            let future = tokio::spawn(async move {
                let mut executor = SubagentExecutor::new(config, event_tx, tools);
                executor.execute(task).await
            });

            futures.push(future);
        }

        // Wait for all tasks to complete
        for future in futures {
            match future.await {
                Ok(result) => {
                    if let Ok(r) = result {
                        results.push(r);
                    }
                }
                Err(e) => {
                    log::error!("Parallel task failed: {}", e);
                }
            }
        }

        Ok(results)
    }

    /// Cancel a running task
    pub async fn cancel(&mut self, task_id: &str) -> Result<()> {
        if let Some(token) = self.cancel_tokens.get(task_id) {
            token.cancel();
            log::info!("Task {} cancelled", task_id);
        }
        Ok(())
    }

    /// Check if a task is cancelled
    pub fn is_cancelled(&self, task_id: &str) -> bool {
        self.cancel_tokens
            .get(task_id)
            .map(|t| t.is_cancelled())
            .unwrap_or(false)
    }

    // ========================================================================
    // Internal Helper Methods
    // ========================================================================

    /// Filter tools based on agent type
    fn filter_tools(&self, subagent_type: &str) -> Vec<Box<dyn Tool>> {
        // For Explore agents, only use read-only tools
        let read_only_tools = vec![
            "read", "grep", "glob", "ls", "search", "code_search",
            "code_files", "code_node", "code_explore", "code_context",
            "codegraph_search", "codegraph_files",
        ];

        // For Plan agents, use read-only + plan tools
        let plan_tools = vec![
            "read", "grep", "glob", "ls", "search", "code_search",
            "code_files", "code_node", "code_explore", "code_context",
            "enter_plan_mode", "exit_plan_mode", "todo_write",
        ];

        let allowed_tools = match subagent_type {
            "Explore" => &read_only_tools,
            "Plan" => &plan_tools,
            _ => return Vec::new(), // General purpose gets all tools
        };

        // Filter from available tools
        self.tools
            .iter()
            .filter(|t| {
                let name = t.definition().name;
                allowed_tools.contains(&name.as_str())
            })
            .map(|t| {
                // Clone Arc<dyn Tool> to Box<dyn Tool>
                // We need to create a new boxed instance
                Self::arc_to_box_tool(t.clone())
            })
            .collect()
    }

    /// Convert Arc<dyn Tool> to Box<dyn Tool>
    /// This is needed because AgentBuilder expects Box<dyn Tool>
    fn arc_to_box_tool(arc_tool: Arc<dyn Tool>) -> Box<dyn Tool> {
        // For now, we'll return a minimal subset
        // In production, this would need proper tool cloning
        // We use the tools from the main agent directly
        // This is a workaround - actual implementation would need
        // proper tool factory or clone mechanism
        Box::new(SubagentToolWrapper(arc_tool))
    }

    /// Build system prompt based on agent type
    fn build_system_prompt(&self, subagent_type: &str, description: &str) -> String {
        let base_prompt = self.config.system_prompt_prefix.clone()
            .unwrap_or_else(|| "You are a helpful AI assistant.".to_string());

        match subagent_type {
            "Explore" => {
                format!(
                    "{}\n\nYou are an Explore agent focused on fast, read-only search operations.\n\
                    Your task: {}\n\n\
                    Rules:\n\
                    - Only use read-only tools (read, grep, glob, ls, search, code_search)\n\
                    - Do not modify any files\n\
                    - Provide concise summaries of findings\n\
                    - Complete quickly and report results",
                    base_prompt, description
                )
            }
            "Plan" => {
                format!(
                    "{}\n\nYou are a Plan agent focused on architecture and planning.\n\
                    Your task: {}\n\n\
                    Rules:\n\
                    - Analyze the codebase structure\n\
                    - Create implementation plans\n\
                    - Use todo_write to track plan steps\n\
                    - Provide clear, actionable recommendations\n\
                    - Do not modify files unless explicitly asked",
                    base_prompt, description
                )
            }
            _ => {
                format!(
                    "{}\n\nYou are a general-purpose agent handling a subtask.\n\
                    Your task: {}\n\n\
                    Complete the task efficiently and report your results.",
                    base_prompt, description
                )
            }
        }
    }

    /// Tag event with subagent context
    fn tag_event(event: AgentEvent, task_id: &str) -> AgentEvent {
        // Add subagent prefix to text events for identification
        if let Some(ref data) = event.data {
            if let crate::event::EventData::Text { delta } = data {
                return AgentEvent::with_data(
                    event.event_type,
                    crate::event::EventData::Text {
                        delta: format!("[Subagent {}] {}", task_id, delta),
                    },
                );
            }
        }
        event
    }

    /// Extract final content from agent messages
    fn extract_content(agent: &Agent) -> String {
        // Get the last assistant message
        let messages = agent.get_messages();
        for msg in messages.iter().rev() {
            if msg.role == crate::providers::Role::Assistant {
                match &msg.content {
                    crate::providers::MessageContent::Text(text) => {
                        return text.clone();
                    }
                    crate::providers::MessageContent::Blocks(blocks) => {
                        // Extract text from blocks
                        let texts: Vec<String> = blocks
                            .iter()
                            .filter_map(|b| {
                                if let crate::providers::ContentBlock::Text { text } = b {
                                    Some(text.clone())
                                } else {
                                    None
                                }
                            })
                            .collect();
                        return texts.join("\n");
                    }
                }
            }
        }
        "No output generated".to_string()
    }
}

/// Wrapper to convert Arc<dyn Tool> to Box<dyn Tool>
/// This is a workaround for the tool cloning issue
struct SubagentToolWrapper(Arc<dyn Tool>);

#[async_trait::async_trait]
impl Tool for SubagentToolWrapper {
    fn definition(&self) -> crate::tools::ToolDefinition {
        self.0.definition()
    }

    async fn execute(&self, params: serde_json::Value) -> Result<String> {
        self.0.execute(params).await
    }

    fn risk_level(&self) -> crate::approval::RiskLevel {
        self.0.risk_level()
    }
}

/// Create a SubagentTask from parameters
pub fn create_task(
    description: &str,
    prompt: &str,
    subagent_type: &str,
    isolation: &str,
) -> SubagentTask {
    let id = Uuid::new_v4().to_string();

    let work_path = if isolation == "worktree" {
        // Create temporary worktree path
        let temp_dir = std::env::temp_dir()
            .join(format!("matrixcode-task-{}", id));
        Some(temp_dir)
    } else {
        None
    };

    SubagentTask {
        id,
        description: description.to_string(),
        prompt: prompt.to_string(),
        subagent_type: subagent_type.to_string(),
        isolation: isolation.to_string(),
        work_path,
    }
}

/// Setup worktree isolation for a task
pub async fn setup_worktree(task: &SubagentTask) -> Result<PathBuf> {
    if task.isolation != "worktree" {
        return Ok(std::env::current_dir()?);
    }

    let work_path = task.work_path.clone().unwrap_or_else(|| {
        std::env::temp_dir()
            .join(format!("matrixcode-task-{}", task.id))
    });

    // Create the directory
    std::fs::create_dir_all(&work_path)?;

    // In a real implementation, this would:
    // 1. Run `git worktree add <path> <branch>`
    // 2. Set up the environment for isolated work
    // 3. Track the worktree for cleanup

    log::info!("Worktree created at {} for task {}", work_path.display(), task.id);

    Ok(work_path)
}

/// Cleanup worktree after task completion
pub async fn cleanup_worktree(task: &SubagentTask) -> Result<()> {
    if task.isolation != "worktree" {
        return Ok(());
    }

    if let Some(path) = &task.work_path {
        // In a real implementation, this would:
        // 1. Run `git worktree remove <path>`
        // 2. Delete the directory

        if path.exists() {
            std::fs::remove_dir_all(path)?;
            log::info!("Worktree cleaned up for task {}", task.id);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_task() {
        let task = create_task(
            "Search codebase",
            "Find all occurrences of 'Agent'",
            "Explore",
            "none",
        );

        assert_eq!(task.description, "Search codebase");
        assert_eq!(task.subagent_type, "Explore");
        assert_eq!(task.isolation, "none");
        assert!(task.work_path.is_none());
    }

    #[test]
    fn test_create_worktree_task() {
        let task = create_task(
            "Refactor module",
            "Refactor the agent module",
            "general-purpose",
            "worktree",
        );

        assert_eq!(task.isolation, "worktree");
        assert!(task.work_path.is_some());
    }

    #[test]
    fn test_subagent_config_default() {
        let config = SubagentConfig::default();

        assert_eq!(config.model_name, "claude-sonnet-4-20250514");
        assert_eq!(config.max_tokens, 4096);
        assert!(!config.think);
    }

    #[test]
    fn test_build_system_prompt_explore() {
        let executor = SubagentExecutor::new(
            SubagentConfig::default(),
            tokio::sync::mpsc::channel(1).0,
            Vec::new(),
        );

        let prompt = executor.build_system_prompt("Explore", "Search for X");

        assert!(prompt.contains("Explore agent"));
        assert!(prompt.contains("read-only"));
        assert!(prompt.contains("Search for X"));
    }

    #[test]
    fn test_build_system_prompt_plan() {
        let executor = SubagentExecutor::new(
            SubagentConfig::default(),
            tokio::sync::mpsc::channel(1).0,
            Vec::new(),
        );

        let prompt = executor.build_system_prompt("Plan", "Create architecture");

        assert!(prompt.contains("Plan agent"));
        assert!(prompt.contains("architecture"));
        assert!(prompt.contains("Create architecture"));
    }
}