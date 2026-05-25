//! Agent builder implementation.

use std::sync::Arc;

use crate::approval::ApproveMode;
use crate::event::AgentEvent;
use crate::prompt::PromptProfile;
use crate::providers::Provider;
use crate::skills::Skill;
use crate::tools::Tool;

use super::types::{Agent, AgentBuilder};

impl AgentBuilder {
    pub fn new(provider: Box<dyn Provider>) -> Self {
        Self {
            provider,
            model_name: "unknown".to_string(),
            tools: Vec::new(),
            system_prompt: "You are a helpful AI coding assistant.".to_string(),
            max_tokens: 4096,
            think: false,
            approve_mode: ApproveMode::Ask,
            event_tx: None,
            skills: Vec::new(),
            profile: PromptProfile::Default,
            project_overview: None,
            memory_summary: None,
            proxy_tools: Vec::new(),
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
    
    /// 添加代理工具
    pub fn proxy_tool(mut self, tool: crate::tools::proxy::ProxyTool) -> Self {
        self.proxy_tools.push(tool);
        self
    }
    
    /// 批量添加代理工具
    pub fn proxy_tools(mut self, tools: Vec<crate::tools::proxy::ProxyTool>) -> Self {
        self.proxy_tools.extend(tools);
        self
    }

    pub fn build(self) -> Agent {
        Agent::new(self)
    }
}
