pub mod ask;
pub mod bash;
pub mod codegraph;
pub mod edit;
pub mod glob;
pub mod grep;
pub mod ls;
pub mod monitor;
pub mod multi_edit;
pub mod plan_mode;
pub mod toolproxy;  // 代理工具模块
pub mod read;
pub mod registry;  // 工具注册中心
pub mod search;
pub mod skill;
pub mod task;
pub mod todo_write;
pub mod webfetch;
pub mod websearch;
pub mod workflow;
pub mod write;

// Re-export proxy types for convenience
pub use toolproxy::{ProxyToolExecutor, ProxyToolDef, ProxyMetadata, ProxyTool, ProxyToolResponse, ProxyToolRequest};

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::approval::RiskLevel;
use crate::skills::Skill;
use std::path::PathBuf;

/// Type alias for boxed tool
pub type BoxedTool = Box<dyn Tool>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
    /// 是否为优先工具。true 时会在描述前添加 "[优先]" 提示，
    /// 让 LLM 更倾向选择此工具。默认 false。
    #[serde(default)]
    pub is_priority: bool,
}

impl Default for ToolDefinition {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            parameters: json!({"type": "object"}),
            is_priority: false,
        }
    }
}

impl ToolDefinition {
    /// 获取发送给 LLM 的描述（带优先标记）
    pub fn description_for_llm(&self) -> String {
        if self.is_priority {
            format!("[优先] {}", self.description)
        } else {
            self.description.clone()
        }
    }
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn definition(&self) -> ToolDefinition;
    async fn execute(&self, params: Value) -> Result<String>;

    /// Risk level of this tool. Defaults to Safe (read-only).
    /// Override in tools that modify state.
    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }
}

/// Default toolset without any skill integration. Kept for callers
/// (and the existing tests) that don't care about skills.
pub fn all_tools() -> Vec<Box<dyn Tool>> {
    all_tools_with_skills(Arc::new(Vec::new()))
}

/// Base toolset without workflow tools (to avoid duplicates).
fn base_tools(skills: Arc<Vec<Skill>>) -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(ask::AskTool),
        Box::new(read::ReadTool),
        Box::new(write::WriteTool),
        Box::new(edit::EditTool),
        Box::new(multi_edit::MultiEditTool),
        Box::new(search::SearchTool),
        Box::new(grep::GrepTool),
        Box::new(glob::GlobTool),
        Box::new(ls::LsTool),
        Box::new(bash::BashTool),
        Box::new(todo_write::TodoWriteTool),
        Box::new(websearch::WebSearchTool::new()),
        Box::new(webfetch::WebFetchTool),
        Box::new(skill::SkillTool::new(skills)),
        Box::new(task::TaskTool),
        Box::new(task::TaskCreateTool),
        Box::new(task::TaskGetTool),
        Box::new(task::TaskListTool),
        Box::new(task::TaskStopTool),
        Box::new(plan_mode::EnterPlanModeTool),
        Box::new(plan_mode::ExitPlanModeTool),
        Box::new(monitor::MonitorTool),
    ]
}

/// Build the toolset with skill support but without provider.
pub fn all_tools_with_skills(skills: Arc<Vec<Skill>>) -> Vec<Box<dyn Tool>> {
    let mut tools = base_tools(skills);
    // Add workflow tools without provider
    tools.extend(workflow::workflow_tools());
    tools
}

/// Build toolset with Provider for AI-powered tools.
pub fn all_tools_with_provider(
    skills: Arc<Vec<Skill>>,
    provider: Arc<dyn crate::providers::Provider>,
) -> Vec<Box<dyn Tool>> {
    let mut tools = base_tools(skills);
    // Add AI-powered workflow tools (with provider)
    tools.extend(workflow::workflow_tools_with_provider(provider));
    tools
}

/// Generate tools description for system prompt
pub fn generate_tools_prompt() -> String {
    generate_tools_prompt_with_path(None)
}

/// Generate tools description with optional CodeGraph support
pub fn generate_tools_prompt_with_path(project_path: Option<&PathBuf>) -> String {
    let mut tools = base_tools(Arc::new(Vec::new()));

    // Add CodeGraph tools only if initialized (CLI installed + .codegraph exists)
    if let Some(path) = project_path
        && codegraph::should_inject_codegraph_tools(path) {
        tools.extend(codegraph::codegraph_tools_with_auto_detect(path));
    }

    // Add workflow tools
    tools.extend(workflow::workflow_tools());

    let mut lines = vec!["可用工具：".to_string()];

    for tool in tools {
        let def = tool.definition();
        // Extract brief description (first sentence or up to 50 chars)
        let brief = def
            .description
            .split('.')
            .next()
            .or_else(|| def.description.split('\n').next())
            .unwrap_or(&def.description);
        let brief = if brief.len() > 60 {
            format!("{}...", brief.chars().take(57).collect::<String>())
        } else {
            brief.to_string()
        };
        lines.push(format!("- {}: {}", def.name, brief));
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_all_tools_includes_workflow_tools() {
        let tools = all_tools();
        let tool_names: Vec<String> = tools.iter().map(|t| t.definition().name).collect();

        // Verify workflow tools are present
        assert!(tool_names.contains(&"workflow_discover".to_string()), "workflow_discover should be in tools");
        assert!(tool_names.contains(&"workflow_run".to_string()), "workflow_run should be in tools");
        assert!(tool_names.contains(&"workflow_match".to_string()), "workflow_match should be in tools");
    }

    #[test]
    fn test_generate_tools_prompt_includes_workflow() {
        let prompt = generate_tools_prompt();

        // Verify workflow tools appear in prompt
        assert!(prompt.contains("workflow_discover"), "prompt should mention workflow_discover");
        assert!(prompt.contains("workflow_run"), "prompt should mention workflow_run");
        assert!(prompt.contains("workflow_match"), "prompt should mention workflow_match");
    }

    #[test]
    fn test_generate_tools_prompt_with_path_includes_codegraph() {
        let path = PathBuf::from(".");
        let prompt = generate_tools_prompt_with_path(Some(&path));

        // Verify codegraph tools appear in prompt when path provided
        assert!(prompt.contains("code_search"), "prompt should mention code_search");
        assert!(prompt.contains("code_callers"), "prompt should mention code_callers");
        assert!(prompt.contains("code_callees"), "prompt should mention code_callees");
        assert!(prompt.contains("code_status"), "prompt should mention code_status");
    }

    #[test]
    fn test_generate_tools_prompt_without_path_excludes_codegraph() {
        let prompt = generate_tools_prompt();

        // Verify codegraph tools NOT in prompt without path
        assert!(!prompt.contains("code_search"), "prompt should NOT mention code_search without path");
    }
}

/// Build toolset with Arc Provider (preferred method)
pub fn all_tools_with_arc_provider(
    skills: Arc<Vec<Skill>>,
    provider: Arc<dyn crate::providers::Provider>,
) -> Vec<Box<dyn Tool>> {
    all_tools_with_provider(skills, provider)
}

/// Build toolset with Box Provider (for CLI compatibility - safe implementation)
/// Uses clone_arc to safely convert Box to Arc without unsafe code.
pub fn all_tools_with_box_provider(
    skills: Arc<Vec<Skill>>,
    boxed_provider: Box<dyn crate::providers::Provider>,
) -> Vec<Box<dyn Tool>> {
    // Safe conversion: clone_arc creates a new Arc without unsafe pointer manipulation
    let arc_provider = boxed_provider.clone_arc();
    all_tools_with_provider(skills, arc_provider)
}

/// Build toolset with project path for CodeGraph integration.
pub fn all_tools_with_project_path(
    skills: Arc<Vec<Skill>>,
    project_path: PathBuf,
) -> Vec<Box<dyn Tool>> {
    let mut tools = base_tools(skills);
    // Add CodeGraph tools
    tools.extend(codegraph::codegraph_tools(&project_path));
    // Add workflow tools
    tools.extend(workflow::workflow_tools());
    tools
}

/// Build full toolset with provider and project path.
pub fn all_tools_full(
    skills: Arc<Vec<Skill>>,
    provider: Arc<dyn crate::providers::Provider>,
    project_path: PathBuf,
) -> Vec<Box<dyn Tool>> {
    let mut tools = base_tools(skills);
    // Add CodeGraph tools
    tools.extend(codegraph::codegraph_tools(&project_path));
    // Add AI-powered workflow tools
    tools.extend(workflow::workflow_tools_with_provider(provider));
    tools
}
