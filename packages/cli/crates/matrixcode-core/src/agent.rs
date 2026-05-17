//! Agent Core - Full Event-driven Implementation
//!
//! Complete agent with streaming, tool execution loop, and event output.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use anyhow::Result;
use tokio::sync::mpsc;

use crate::event::{AgentEvent, EventData, EventType};
use crate::providers::{ChatRequest, ChatResponse, ContentBlock, Message, MessageContent, Provider, Role, StreamEvent, StopReason, Usage};
use crate::tools::{Tool, ToolDefinition};
use crate::approval::{ApproveMode, RiskLevel, needs_approval};
use crate::compress::{CompressionConfig, should_compress};
use crate::cancel::CancellationToken;

const MAX_ITERATIONS: usize = 50;

/// Full Agent with event output
pub struct Agent {
    provider: Box<dyn Provider>,
    tools: Vec<Arc<dyn Tool>>,
    messages: Vec<Message>,
    system_prompt: String,
    max_tokens: u32,
    think: bool,
    approve_mode: ApproveMode,
    event_tx: mpsc::Sender<AgentEvent>,
    
    // New fields
    skills: Vec<crate::skills::Skill>,
    profile: crate::prompt::PromptProfile,
    project_overview: Option<String>,
    memory_summary: Option<String>,
    
    // State tracking
    total_input_tokens: AtomicU64,
    total_output_tokens: AtomicU64,
    cancel_token: Option<CancellationToken>,
    compression_config: CompressionConfig,
}

/// Agent builder
pub struct AgentBuilder {
    provider: Box<dyn Provider>,
    tools: Vec<Arc<dyn Tool>>,
    system_prompt: String,
    max_tokens: u32,
    think: bool,
    approve_mode: ApproveMode,
    // New fields
    skills: Vec<crate::skills::Skill>,
    profile: crate::prompt::PromptProfile,
    project_overview: Option<String>,
    memory_summary: Option<String>,
}

impl AgentBuilder {
    pub fn new(provider: Box<dyn Provider>) -> Self {
        Self {
            provider,
            tools: Vec::new(),
            system_prompt: "You are a helpful AI coding assistant.".to_string(),
            max_tokens: 4096,
            think: false,
            approve_mode: ApproveMode::Ask,
            skills: Vec::new(),
            profile: crate::prompt::PromptProfile::Default,
            project_overview: None,
            memory_summary: None,
        }
    }

    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
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

    /// Add skills
    pub fn skills(mut self, skills: Vec<crate::skills::Skill>) -> Self {
        self.skills = skills;
        self
    }

    /// Set prompt profile
    pub fn profile(mut self, profile: crate::prompt::PromptProfile) -> Self {
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

    pub fn build(self) -> Agent {
        Agent::new(self)
    }
}

impl Agent {
    fn new(builder: AgentBuilder) -> Self {
        let (event_tx, _) = mpsc::channel(100);
        
        Self {
            provider: builder.provider,
            tools: builder.tools,
            messages: Vec::new(),
            system_prompt: builder.system_prompt,
            max_tokens: builder.max_tokens,
            think: builder.think,
            approve_mode: builder.approve_mode,
            event_tx,
            skills: builder.skills,
            profile: builder.profile,
            project_overview: builder.project_overview,
            memory_summary: builder.memory_summary,
            total_input_tokens: AtomicU64::new(0),
            total_output_tokens: AtomicU64::new(0),
            cancel_token: None,
            compression_config: CompressionConfig::default(),
        }
    }

    /// Get event sender for streaming
    pub fn event_sender(&self) -> mpsc::Sender<AgentEvent> {
        self.event_tx.clone()
    }

    /// Set cancellation token
    pub fn set_cancel_token(&mut self, token: CancellationToken) {
        self.cancel_token = Some(token);
    }

    /// Build full system prompt with profile, overview, memory
    fn build_full_system_prompt(&self) -> String {
        use crate::prompt::build_system_prompt;
        
        build_system_prompt(
            &self.profile,
            &self.skills,
            self.project_overview.as_deref(),
            self.memory_summary.as_deref(),
        )
    }

    /// Run chat loop with tool execution
    pub async fn run(&mut self, user_input: String) -> Result<Vec<AgentEvent>> {
        let mut collector = EventCollector::new();
        
        // Send session started
        self.emit(AgentEvent::session_started())?;

        // Add user message
        self.messages.push(Message {
            role: Role::User,
            content: MessageContent::Text(user_input.clone()),
        });

        // Run agent loop (handle tool_use iterations)
        let mut iterations = 0;
        let mut should_continue = true;

        while should_continue && iterations < MAX_ITERATIONS {
            iterations += 1;
            
            // Check cancellation
            if let Some(token) = &self.cancel_token {
                if token.is_cancelled() {
                    self.emit(AgentEvent::error("Operation cancelled".to_string(), None, None))?;
                    break;
                }
            }

            // Build request
            let tool_defs: Vec<ToolDefinition> = self.tools.iter().map(|t| t.definition()).collect();
            let request = ChatRequest {
                system: Some(self.system_prompt.clone()),
                messages: self.messages.clone(),
                max_tokens: self.max_tokens,
                tools: tool_defs,
                think: self.think,
                enable_caching: false,
                server_tools: Vec::new(),
            };

            // Call provider
            self.emit(AgentEvent::progress(
                if iterations == 1 { "Thinking..." } else { "Processing..." },
                None,
            ))?;

            let response = self.provider.chat(request).await?;

            // Track usage
            self.track_usage(&response.usage);

            // Process response
            should_continue = self.process_response(&response).await?;

            // Check compression
            let context_size = self.estimate_context_size();
            let current_tokens = self.total_input_tokens.load(Ordering::Relaxed) as u32;
            if should_compress(current_tokens, Some(context_size), &self.compression_config) {
                self.emit(AgentEvent::progress("Compressing context...", None))?;
                // TODO: implement compression
            }
        }

        // Send final usage stats
        self.emit(AgentEvent::usage(
            self.total_input_tokens.load(Ordering::Relaxed),
            self.total_output_tokens.load(Ordering::Relaxed),
        ))?;

        // Send session ended
        self.emit(AgentEvent::session_ended())?;

        Ok(collector.events().to_vec())
    }

    /// Process response and handle tool_use
    async fn process_response(&mut self, response: &ChatResponse) -> Result<bool> {
        let mut has_tool_use = false;
        let mut assistant_content: Vec<ContentBlock> = Vec::new();

        for block in &response.content {
            match block {
                ContentBlock::Text { text } => {
                    self.emit(AgentEvent::text_start())?;
                    self.emit(AgentEvent::text_delta(text.clone()))?;
                    self.emit(AgentEvent::text_end())?;
                    assistant_content.push(ContentBlock::Text { text: text.clone() });
                }

                ContentBlock::Thinking { thinking, signature } => {
                    self.emit(AgentEvent::thinking_start())?;
                    self.emit(AgentEvent::thinking_delta(thinking.clone(), signature.clone()))?;
                    self.emit(AgentEvent::thinking_end())?;
                    assistant_content.push(ContentBlock::Thinking {
                        thinking: thinking.clone(),
                        signature: signature.clone(),
                    });
                }

                ContentBlock::ToolUse { id, name, input } => {
                    has_tool_use = true;
                    
                    self.emit(AgentEvent::tool_use_start(id.clone(), name.clone()))?;
                    
                    // Execute tool
                    let result = self.execute_tool(name, input.clone()).await;
                    
                    let (content, is_error) = match result {
                        Ok(output) => (output, false),
                        Err(e) => (e.to_string(), true),
                    };

                    self.emit(AgentEvent::tool_result(id.clone(), content.clone(), is_error))?;

                    // Add to message history
                    assistant_content.push(ContentBlock::ToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        input: input.clone(),
                    });

                    self.messages.push(Message {
                        role: Role::User,
                        content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                            tool_use_id: id.clone(),
                            content: format!("{}: {}", if is_error { "Error" } else { "Result" }, content),
                        }]),
                    });
                }

                _ => {}
            }
        }

        // Add assistant message to history
        if !assistant_content.is_empty() {
            self.messages.push(Message {
                role: Role::Assistant,
                content: MessageContent::Blocks(assistant_content),
            });
        }

        // Continue if there were tool calls
        Ok(has_tool_use)
    }

    /// Execute a tool
    async fn execute_tool(&self, name: &str, input: serde_json::Value) -> Result<String> {
        let tool = self.tools.iter().find(|t| t.definition().name == name);

        if let Some(tool) = tool {
            // Check approval
            if needs_approval(self.approve_mode, tool.risk_level()) {
                // In daemon mode, we auto-approve based on mode
                // Auto mode: approve all
                // Ask mode: approve safe/mutating, ask for dangerous
                // Strict mode: ask for all
                
                if self.approve_mode == ApproveMode::Strict ||
                   (self.approve_mode == ApproveMode::Ask && tool.risk_level() == RiskLevel::Dangerous) {
                    // Send approval request event
                    self.emit(AgentEvent::progress(
                        format!("Tool '{}' requires approval", name),
                        None,
                    ))?;
                    
                    // In daemon mode without interactive approval, we:
                    // - Auto-approve safe tools
                    // - Auto-approve mutating tools in Auto mode
                    // - Reject dangerous tools in Ask mode
                    
                    if tool.risk_level() == RiskLevel::Dangerous && self.approve_mode != ApproveMode::Auto {
                        return Err(anyhow::anyhow!(
                            "Tool '{}' requires manual approval (dangerous operation). Use --approve-mode auto to auto-approve.",
                            name
                        ));
                    }
                }
            }

            // Execute tool
            self.emit(AgentEvent::progress(format!("Executing: {}", name), None))?;
            tool.execute(input).await
        } else {
            Err(anyhow::anyhow!("Tool '{}' not found", name))
        }
    }

    /// Track token usage
    fn track_usage(&self, usage: &Usage) {
        self.total_input_tokens.fetch_add(usage.input_tokens as u64, Ordering::Relaxed);
        self.total_output_tokens.fetch_add(usage.output_tokens as u64, Ordering::Relaxed);
        
        // Emit usage event
        let _ = self.event_tx.blocking_send(AgentEvent::usage(
            usage.input_tokens as u64,
            usage.output_tokens as u64,
        ));
    }

    /// Estimate context size
    fn estimate_context_size(&self) -> u32 {
        // Rough estimate: each message ~100 tokens average
        (self.messages.len() as u32) * 100 + self.total_input_tokens.load(Ordering::Relaxed) as u32
    }

    /// Emit event
    fn emit(&self, event: AgentEvent) -> Result<()> {
        self.event_tx.blocking_send(event)?;
        Ok(())
    }

    /// Clear message history
    pub fn clear_history(&mut self) {
        self.messages.clear();
        self.total_input_tokens.store(0, Ordering::Relaxed);
        self.total_output_tokens.store(0, Ordering::Relaxed);
    }

    /// Get message count
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }
}

/// Event collector for gathering events
pub struct EventCollector {
    events: Vec<AgentEvent>,
}

impl EventCollector {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn events(&self) -> &[AgentEvent] {
        &self.events
    }
}