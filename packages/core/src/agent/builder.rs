//! Agent builder implementation.

use std::path::PathBuf;
use std::sync::Arc;

use crate::approval::ApproveMode;
use crate::constants::QUICK_ACTION_MAX_TOKENS;
use crate::event::AgentEvent;
use crate::prompt::PromptProfile;
use crate::providers::Provider;
use crate::skills::Skill;
use crate::tools::Tool;
use crate::tools::toolproxy::{ProxyToolExecutor, ProxyToolDef};

use super::types::{Agent, AgentBuilder};

impl AgentBuilder {
    pub fn new(provider: Box<dyn Provider>) -> Self {
        Self {
            provider,
            model_name: "unknown".to_string(),
            tools: Vec::new(),
            system_prompt: "You are a helpful AI coding assistant.".to_string(),
            max_tokens: QUICK_ACTION_MAX_TOKENS,
            think: false,
            approve_mode: ApproveMode::Ask,
            event_tx: None,
            skills: Vec::new(),
            profile: PromptProfile::Default,
            project_overview: None,
            memory_summary: None,
            project_path: None,
            proxy_tool_defs: Vec::new(),
            proxy_executor: None,
            mcp_registry: None,
        }
    }

    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
        self
    }

    pub fn model_name(mut self, name: impl Into<String>) -> Self {
        self.model_name = name.into();
        self
    }

    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = tokens;
        self
    }

    pub fn think(mut self, enabled: bool) -> Self {
        self.think = enabled;
        self
    }

    pub fn approve_mode(mut self, mode: ApproveMode) -> Self {
        self.approve_mode = mode;
        self
    }

    pub fn tool(mut self, tool: Arc<dyn Tool>) -> Self {
        self.tools.push(tool);
        self
    }

    /// Add multiple tools
    pub fn tools(mut self, tools: Vec<Box<dyn Tool>>) -> Self {
        self.tools.extend(tools.into_iter().map(Arc::from));
        self
    }

    /// Add multiple tools with provider support
    pub fn tools_with_provider(mut self, tools: Vec<Box<dyn Tool>>) -> Self {
        self.tools.extend(tools.into_iter().map(Arc::from));
        self
    }

    /// Set external event sender for streaming events
    pub fn event_tx(mut self, tx: tokio::sync::mpsc::Sender<AgentEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }

    /// Add skills
    pub fn skills(mut self, skills: Vec<Skill>) -> Self {
        self.skills = skills;
        self
    }

    /// Set prompt profile
    pub fn profile(mut self, profile: PromptProfile) -> Self {
        self.profile = profile;
        self
    }

    /// Set project overview
    pub fn overview(mut self, overview: impl Into<String>) -> Self {
        self.project_overview = Some(overview.into());
        self
    }

    /// Set memory summary
    pub fn memory(mut self, summary: impl Into<String>) -> Self {
        self.memory_summary = Some(summary.into());
        self
    }

    /// Set project path (for dynamic tool injection like CodeGraph)
    pub fn project_path(mut self, path: PathBuf) -> Self {
        self.project_path = Some(path);
        self
    }

    /// 设置代理工具执行器
    ///
    /// # Example
    /// ```ignore
    /// use std::sync::Arc;
    /// use serde_json::json;
    /// use matrixcode_core::tools::toolproxy::{ProxyToolExecutor, ProxyToolDef};
    ///
    /// let executor = Arc::new(MyProxyExecutor);
    /// let tool_def = ProxyToolDef::new("image_search", "搜索图片", json!({...}))
    ///     .with_priority(true);
    ///
    /// builder.proxy_executor(executor, vec![tool_def])
    /// ```
    pub fn proxy_executor(mut self, executor: Arc<dyn ProxyToolExecutor>, tool_defs: Vec<ProxyToolDef>) -> Self {
        self.proxy_executor = Some(executor);
        self.proxy_tool_defs = tool_defs;
        self
    }

    pub fn build(self) -> Agent {
        Agent::new(self)
    }

    /// 设置 MCP 工具注册表
    ///
    /// # Example
    /// ```ignore
    /// use std::sync::Arc;
    /// use matrixcode_core::mcp::McpToolRegistry;
    ///
    /// let registry = Arc::new(tokio::sync::RwLock::new(McpToolRegistry::new()));
    /// builder.mcp_registry(registry)
    /// ```
    pub fn mcp_registry(mut self, registry: Arc<tokio::sync::RwLock<crate::mcp::McpToolRegistry>>) -> Self {
        self.mcp_registry = Some(registry);
        self
    }
}
