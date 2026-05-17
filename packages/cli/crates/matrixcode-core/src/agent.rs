//! Agent Core - Event-driven Agent without UI

use anyhow::Result;
use tokio::sync::mpsc;

use crate::event::{AgentEvent, EventCollector};
use crate::providers::{ChatRequest, ChatResponse, ContentBlock, Message, MessageContent, Provider, Role};
use crate::tools::Tool;
use crate::config::Config;
use crate::approval::{ApproveMode, needs_approval};

/// Agent that produces events instead of UI output
pub struct Agent {
    provider: Box<dyn Provider>,
    tools: Vec<Box<dyn Tool>>,
    messages: Vec<Message>,
    system_prompt: String,
    max_tokens: u32,
    think: bool,
    approve_mode: ApproveMode,
    event_tx: Option<mpsc::Sender<AgentEvent>>,
}

impl Agent {
    /// Create new agent
    pub fn new(provider: Box<dyn Provider>, config: Config) -> Self {
        Self {
            provider,
            tools: Vec::new(),
            messages: Vec::new(),
            system_prompt: config.system_prompt,
            max_tokens: config.max_tokens,
            think: config.think,
            approve_mode: config.approve_mode,
            event_tx: None,
        }
    }

    /// Set event sender for streaming events
    pub fn set_event_sender(&mut self, tx: mpsc::Sender<AgentEvent>) {
        self.event_tx = Some(tx);
    }

    /// Add a tool
    pub fn add_tool(&mut self, tool: Box<dyn Tool>) {
        self.tools.push(tool);
    }

    /// Run a chat request and produce events
    pub async fn chat(&mut self, input: String) -> Result<Vec<AgentEvent>> {
        let collector = EventCollector::new();
        
        // Send session started
        self.send_event(AgentEvent::session_started())?;

        // Add user message
        self.messages.push(Message {
            role: Role::User,
            content: MessageContent::Text(input.clone()),
        });

        // Build request
        let request = ChatRequest {
            system: Some(self.system_prompt.clone()),
            messages: self.messages.clone(),
            max_tokens: self.max_tokens,
            tools: self.tools.iter().map(|t| t.definition()).collect(),
            think: self.think,
            enable_caching: false,
            server_tools: Vec::new(),
        };

        // Send request to provider
        self.send_event(AgentEvent::progress("Sending request to AI...".to_string(), None))?;

        let response = self.provider.chat(request).await?;

        // Process response
        self.process_response(response).await?;

        // Send session ended
        self.send_event(AgentEvent::session_ended())?;

        Ok(collector.events().to_vec())
    }

    /// Process response from provider
    async fn process_response(&mut self, response: ChatResponse) -> Result<()> {
        // Send usage stats
        self.send_event(AgentEvent::usage(
            response.usage.input_tokens as u64,
            response.usage.output_tokens as u64,
        ))?;

        // Process content blocks
        for block in response.content {
            match block {
                ContentBlock::Text { text } => {
                    self.send_event(AgentEvent::text_start())?;
                    self.send_event(AgentEvent::text_delta(text))?;
                    self.send_event(AgentEvent::text_end())?;
                }
                ContentBlock::Thinking { thinking, signature } => {
                    self.send_event(AgentEvent::thinking_start())?;
                    self.send_event(AgentEvent::thinking_delta(thinking, signature))?;
                    self.send_event(AgentEvent::thinking_end())?;
                }
                ContentBlock::ToolUse { id, name, input } => {
                    self.send_event(AgentEvent::tool_use_start(id.clone(), name.clone()))?;
                    
                    // Execute tool
                    let result = self.execute_tool(&name, input.clone()).await?;
                    
                    self.send_event(AgentEvent::tool_result(id, result, false))?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Execute a tool
    async fn execute_tool(&mut self, name: &str, input: serde_json::Value) -> Result<String> {
        // Find tool
        let tool = self.tools.iter().find(|t| t.definition().name == name);
        
        if let Some(tool) = tool {
            // Check if needs approval
            if needs_approval(self.approve_mode, tool.risk_level()) {
                self.send_event(AgentEvent::progress(
                    format!("Tool '{}' requires approval", name),
                    None,
                ))?;
                
                // In daemon mode, auto-approve or reject
                // For now, we auto-approve safe tools
                // TODO: implement proper approval flow
            }

            // Execute
            self.send_event(AgentEvent::progress(
                format!("Executing tool: {}", name),
                None,
            ))?;

            tool.execute(input).await
        } else {
            Err(anyhow::anyhow!("Tool not found: {}", name))
        }
    }

    /// Send event to channel
    fn send_event(&self, event: AgentEvent) -> Result<()> {
        if let Some(tx) = &self.event_tx {
            tx.blocking_send(event)?;
        }
        Ok(())
    }

    /// Get messages
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Clear messages
    pub fn clear_messages(&mut self) {
        self.messages.clear();
    }
}

/// Agent builder
pub struct AgentBuilder {
    provider: Box<dyn Provider>,
    config: Config,
    tools: Vec<Box<dyn Tool>>,
}

impl AgentBuilder {
    pub fn new(provider: Box<dyn Provider>) -> Self {
        Self {
            provider,
            config: Config::default(),
            tools: Vec::new(),
        }
    }

    pub fn config(mut self, config: Config) -> Self {
        self.config = config;
        self
    }

    pub fn tool(mut self, tool: Box<dyn Tool>) -> Self {
        self.tools.push(tool);
        self
    }

    pub fn build(self) -> Agent {
        let mut agent = Agent::new(self.provider, self.config);
        for tool in self.tools {
            agent.add_tool(tool);
        }
        agent
    }
}