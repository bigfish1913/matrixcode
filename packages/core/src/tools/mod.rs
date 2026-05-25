pub mod ask;
pub mod bash;
pub mod edit;
pub mod glob;
pub mod grep;
pub mod image_search;
pub mod ls;
pub mod monitor;
pub mod multi_edit;
pub mod plan_mode;
pub mod proxy;  // 代理工具模块
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
pub use proxy::{ProxyMetadata, ProxyTool, ProxyToolResponse};

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::approval::RiskLevel;
use crate::skills::Skill;

/// Type alias for boxed tool
pub type BoxedTool = Box<dyn Tool>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
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

/// Build the toolset and include the `skill` tool bound to the given
/// skills catalogue. The catalogue can be empty; the tool still works
/// but will only report "no skills loaded" if invoked.
pub fn all_tools_with_skills(skills: Arc<Vec<Skill>>) -> Vec<Box<dyn Tool>> {
    let mut tools: Vec<Box<dyn Tool>> = vec![
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
        Box::new(websearch::WebSearchTool),
        Box::new(webfetch::WebFetchTool),
        Box::new(skill::SkillTool::new(skills)),
        // New high-priority tools
        Box::new(task::TaskTool),
        Box::new(task::TaskCreateTool),
        Box::new(task::TaskGetTool),
        Box::new(task::TaskListTool),
        Box::new(task::TaskStopTool),
        Box::new(plan_mode::EnterPlanModeTool),
        Box::new(plan_mode::ExitPlanModeTool),
        Box::new(monitor::MonitorTool),
        // Box::new(image_search::ImageSearchTool),
        // Workflow-specific tools
    ];
    // Add workflow management tools (independent system like Skill)
    tools.extend(workflow::workflow_tools());
    tools
}

/// Build toolset with Provider for AI-powered tools
pub fn all_tools_with_provider(
    skills: Arc<Vec<Skill>>,
    provider: Arc<dyn crate::providers::Provider>,
) -> Vec<Box<dyn Tool>> {
    let mut tools = all_tools_with_skills(skills);
    // Add AI-powered workflow tools
    tools.extend(workflow::workflow_tools_with_provider(provider));
    tools
}

/// Generate tools description for system prompt
pub fn generate_tools_prompt() -> String {
    let tools = all_tools();
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
}
