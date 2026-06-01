pub mod ask;
pub mod bash;
pub mod browser;
pub mod codegraph;
pub mod edit;
pub mod glob;
pub mod grep;
pub mod ls;
pub mod monitor;
pub mod multi_edit;
pub mod plan_mode;
pub mod read;
pub mod registry; // 工具注册中心
pub mod search;
pub mod skill;
pub mod task;
pub mod todo_write;
pub mod toolproxy; // 代理工具模块
pub mod webfetch;
pub mod websearch;
pub mod workflow;
pub mod write;

// Re-export proxy types for convenience
pub use toolproxy::{
    ProxyMetadata, ProxyTool, ProxyToolDef, ProxyToolExecutor, ProxyToolRequest, ProxyToolResponse,
};

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::approval::RiskLevel;
use crate::skills::Skill;
use std::path::PathBuf;

/// Context for tool definition generation
/// Used to customize tool descriptions based on available features
#[derive(Debug, Clone, Default)]
pub struct ToolContext {
    /// Whether CodeGraph tools are available
    pub codegraph_available: bool,
}

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
    /// Get tool definition (must implement)
    fn definition(&self) -> ToolDefinition;

    /// Get tool definition with context (for dynamic descriptions)
    ///
    /// Default implementation calls definition(). Override this method
    /// if you need context-aware descriptions (e.g., different text
    /// when CodeGraph is available).
    fn definition_with_context(&self, _ctx: &ToolContext) -> ToolDefinition {
        self.definition()
    }

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
        Box::new(browser::BrowserOpenTool),
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
    generate_tools_prompt_with_path_and_lsp(None, None)
}

/// Generate tools description with optional CodeGraph support
pub fn generate_tools_prompt_with_path(project_path: Option<&PathBuf>) -> String {
    generate_tools_prompt_with_path_and_lsp(project_path, None)
}

/// Generate tools description with optional CodeGraph and LSP support
pub fn generate_tools_prompt_with_path_and_lsp(
    project_path: Option<&PathBuf>,
    lsp_registry: Option<Arc<crate::lsp::LspClientRegistry>>,
) -> String {
    // Build tool context based on CodeGraph availability
    let ctx = ToolContext {
        codegraph_available: project_path
            .map(|p| codegraph::should_inject_codegraph_tools(p))
            .unwrap_or(false),
    };

    let mut tools = base_tools(Arc::new(Vec::new()));

    // Add CodeGraph tools only if initialized (CLI installed + .codegraph exists)
    if ctx.codegraph_available {
        if let Some(path) = project_path {
            tools.extend(codegraph::codegraph_tools_with_auto_detect(path));
        }
    }

    // Add LSP tools if registry is provided
    if let Some(registry) = lsp_registry {
        tools.extend(crate::lsp::tools::lsp_tools(registry));
    }

    // Add workflow tools
    tools.extend(workflow::workflow_tools());

    // 🎯 分类显示：优先工具 + 其他工具
    let mut priority_tools = Vec::new();
    let mut normal_tools = Vec::new();

    for tool in tools {
        // Use definition_with_context for dynamic descriptions
        let def = tool.definition_with_context(&ctx);
        if def.is_priority {
            priority_tools.push(def);
        } else {
            normal_tools.push(def);
        }
    }

    let mut lines = vec!["可用工具：".to_string()];

    // 优先工具（完整描述，包含适用场景）
    if !priority_tools.is_empty() {
        lines.push("\n【优先工具 - 必须优先考虑】".to_string());
        for def in priority_tools {
            // 使用 description_for_llm() 自动添加 [优先] 标记
            let full_desc = def.description_for_llm();
            // 优先工具保留完整描述（最多150字符）
            let desc = full_desc.split('\n').next().unwrap_or(&full_desc);
            if desc.len() > 150 {
                lines.push(format!(
                    "  {}: {}...",
                    def.name,
                    desc.chars().take(147).collect::<String>()
                ));
            } else {
                lines.push(format!("  {}: {}", def.name, desc));
            }
        }
    }

    // 其他工具（简要描述）
    if !normal_tools.is_empty() {
        lines.push("\n【其他工具】".to_string());
        for def in normal_tools {
            // 其他工具保持简要描述（前60字符）
            let desc = def
                .description
                .split('.')
                .next()
                .or_else(|| def.description.split('\n').next())
                .unwrap_or(&def.description);
            if desc.len() > 60 {
                lines.push(format!(
                    "  {}: {}...",
                    def.name,
                    desc.chars().take(57).collect::<String>()
                ));
            } else {
                lines.push(format!("  {}: {}", def.name, desc));
            }
        }
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
        assert!(
            tool_names.contains(&"workflow_discover".to_string()),
            "workflow_discover should be in tools"
        );
        assert!(
            tool_names.contains(&"workflow_run".to_string()),
            "workflow_run should be in tools"
        );
        assert!(
            tool_names.contains(&"workflow_match".to_string()),
            "workflow_match should be in tools"
        );
    }

    #[test]
    fn test_generate_tools_prompt_includes_workflow() {
        let prompt = generate_tools_prompt();

        // Verify workflow tools appear in prompt
        assert!(
            prompt.contains("workflow_discover"),
            "prompt should mention workflow_discover"
        );
        assert!(
            prompt.contains("workflow_run"),
            "prompt should mention workflow_run"
        );
        assert!(
            prompt.contains("workflow_match"),
            "prompt should mention workflow_match"
        );
    }

    #[test]
    fn test_generate_tools_prompt_with_path_includes_codegraph() {
        let path = PathBuf::from(".");
        let prompt = generate_tools_prompt_with_path(Some(&path));

        // CodeGraph tools are only included when:
        // 1. CodeGraph CLI is installed
        // 2. Project has .codegraph directory
        // So we check based on actual conditions
        if codegraph::should_inject_codegraph_tools(&path) {
            assert!(
                prompt.contains("code_search"),
                "prompt should mention code_search when conditions met"
            );
            assert!(
                prompt.contains("code_callers"),
                "prompt should mention code_callers when conditions met"
            );
        } else {
            // When conditions not met, codegraph tools should NOT appear
            assert!(
                !prompt.contains("code_search"),
                "prompt should NOT mention code_search without .codegraph"
            );
        }
    }

    #[test]
    fn test_generate_tools_prompt_without_path_excludes_codegraph() {
        let prompt = generate_tools_prompt();

        // Verify codegraph tools NOT in prompt without path
        assert!(
            !prompt.contains("code_search"),
            "prompt should NOT mention code_search without path"
        );
    }

    #[test]
    fn test_tool_context_affects_grep_description() {
        use crate::tools::grep::GrepTool;

        // Without CodeGraph - should suggest using grep for definitions
        let ctx_no_codegraph = ToolContext {
            codegraph_available: false,
        };
        let def_no_cg = GrepTool.definition_with_context(&ctx_no_codegraph);
        assert!(
            def_no_cg.description.contains("用 grep 搜索"),
            "Without CodeGraph, grep should suggest using grep for definitions"
        );
        assert!(
            !def_no_cg.description.contains("code_search"),
            "Without CodeGraph, grep description should not mention code_search"
        );

        // With CodeGraph - should recommend code_search
        let ctx_with_codegraph = ToolContext {
            codegraph_available: true,
        };
        let def_with_cg = GrepTool.definition_with_context(&ctx_with_codegraph);
        assert!(
            def_with_cg.description.contains("code_search"),
            "With CodeGraph, grep should recommend code_search"
        );
        assert!(
            def_with_cg.description.contains("快10-100倍"),
            "With CodeGraph, grep should mention speed advantage"
        );
    }

    #[test]
    fn test_tool_context_affects_search_description() {
        use crate::tools::search::SearchTool;

        // Without CodeGraph
        let ctx_no_codegraph = ToolContext {
            codegraph_available: false,
        };
        let def_no_cg = SearchTool.definition_with_context(&ctx_no_codegraph);
        assert!(
            def_no_cg.description.contains("search 的适用场景"),
            "Without CodeGraph, search should show its own applicable scenarios"
        );

        // With CodeGraph
        let ctx_with_codegraph = ToolContext {
            codegraph_available: true,
        };
        let def_with_cg = SearchTool.definition_with_context(&ctx_with_codegraph);
        assert!(
            def_with_cg.description.contains("优先使用 code_search"),
            "With CodeGraph, search should mention code_search priority"
        );
    }

    #[test]
    fn test_tool_context_affects_glob_description() {
        use crate::tools::glob::GlobTool;

        // Without CodeGraph
        let ctx_no_codegraph = ToolContext {
            codegraph_available: false,
        };
        let def_no_cg = GlobTool.definition_with_context(&ctx_no_codegraph);
        assert!(
            def_no_cg.description.contains("glob 的适用场景"),
            "Without CodeGraph, glob should show its own applicable scenarios"
        );

        // With CodeGraph
        let ctx_with_codegraph = ToolContext {
            codegraph_available: true,
        };
        let def_with_cg = GlobTool.definition_with_context(&ctx_with_codegraph);
        assert!(
            def_with_cg.description.contains("优先使用 code_files"),
            "With CodeGraph, glob should mention code_files priority"
        );
    }

    #[test]
    fn test_generate_tools_prompt_dynamic_descriptions() {
        let path = PathBuf::from(".");
        let prompt = generate_tools_prompt_with_path(Some(&path));

        // Check based on actual CodeGraph availability
        if codegraph::should_inject_codegraph_tools(&path) {
            // When CodeGraph is available, grep should mention code_search
            assert!(
                prompt.contains("code_search") || prompt.contains("grep"),
                "Prompt should contain grep tool"
            );
        }

        // Both grep and search should always be present
        assert!(prompt.contains("grep"), "Prompt should contain grep tool");
        assert!(
            prompt.contains("search"),
            "Prompt should contain search tool"
        );
        assert!(prompt.contains("glob"), "Prompt should contain glob tool");
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
    all_tools_with_project_path_and_lsp(skills, project_path, None)
}

/// Build toolset with project path and optional LSP registry.
pub fn all_tools_with_project_path_and_lsp(
    skills: Arc<Vec<Skill>>,
    project_path: PathBuf,
    lsp_registry: Option<Arc<crate::lsp::LspClientRegistry>>,
) -> Vec<Box<dyn Tool>> {
    let mut tools = base_tools(skills);
    // Add CodeGraph tools
    tools.extend(codegraph::codegraph_tools(&project_path));
    // Add LSP tools if registry is provided
    if let Some(registry) = lsp_registry {
        tools.extend(crate::lsp::tools::lsp_tools(registry));
    }
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
    all_tools_full_with_lsp(skills, provider, project_path, None)
}

/// Build full toolset with provider, project path, and optional LSP registry.
pub fn all_tools_full_with_lsp(
    skills: Arc<Vec<Skill>>,
    provider: Arc<dyn crate::providers::Provider>,
    project_path: PathBuf,
    lsp_registry: Option<Arc<crate::lsp::LspClientRegistry>>,
) -> Vec<Box<dyn Tool>> {
    let mut tools = base_tools(skills);
    // Add CodeGraph tools only if initialized (CLI installed + .codegraph exists)
    if codegraph::should_inject_codegraph_tools(&project_path) {
        tools.extend(codegraph::codegraph_tools(&project_path));
    }
    // Add LSP tools if registry is provided
    if let Some(registry) = lsp_registry {
        tools.extend(crate::lsp::tools::lsp_tools(registry));
    }
    // Add AI-powered workflow tools
    tools.extend(workflow::workflow_tools_with_provider(provider));
    tools
}
