//! Agent Core - Full Event-driven Implementation
//!
//! Complete agent with streaming, tool execution loop, and event output.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use anyhow::Result;
use tokio::sync::mpsc;

use crate::event::{AgentEvent, EventType, EventData};
use crate::providers::{ChatRequest, ChatResponse, ContentBlock, Message, MessageContent, Provider, Role, StopReason, Usage};
use crate::tools::{Tool, ToolDefinition};
use crate::approval::{ApproveMode, needs_approval};
use crate::compress::{CompressionConfig, should_compress};
use crate::cancel::CancellationToken;

const MAX_ITERATIONS: usize = 50;

/// Full Agent with event output
#[allow(dead_code)]  // Some fields are for future features
pub struct Agent {
    provider: Box<dyn Provider>,
    model_name: String,  // For debug logging
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
    /// The most recent API call's input_tokens — represents actual context window usage.
    last_input_tokens: AtomicU64,
    cancel_token: Option<CancellationToken>,
    compression_config: CompressionConfig,
    
    // Ask tool channel: receives user answers from TUI
    ask_rx: Option<mpsc::Receiver<String>>,
}

/// Agent builder
pub struct AgentBuilder {
    provider: Box<dyn Provider>,
    model_name: String,
    tools: Vec<Arc<dyn Tool>>,
    system_prompt: String,
    max_tokens: u32,
    think: bool,
    approve_mode: ApproveMode,
    event_tx: Option<mpsc::Sender<AgentEvent>>,
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
            model_name: "unknown".to_string(),
            tools: Vec::new(),
            system_prompt: "You are a helpful AI coding assistant.".to_string(),
            max_tokens: 4096,
            think: false,
            approve_mode: ApproveMode::Ask,
            event_tx: None,
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

    /// Set external event sender for streaming events
    pub fn event_tx(mut self, tx: mpsc::Sender<AgentEvent>) -> Self {
        self.event_tx = Some(tx);
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
        // Use external event_tx if provided, otherwise create internal one
        let event_tx = builder.event_tx.unwrap_or_else(|| {
            let (tx, _) = mpsc::channel(100);
            tx
        });

        Self {
            provider: builder.provider,
            model_name: builder.model_name,
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
            last_input_tokens: AtomicU64::new(0),
            cancel_token: None,
            compression_config: CompressionConfig::default(),
            ask_rx: None,
        }
    }

    /// Get event sender for streaming
    pub fn event_sender(&self) -> mpsc::Sender<AgentEvent> {
        self.event_tx.clone()
    }

    /// Set ask response channel (for TUI mode)
    pub fn set_ask_channel(&mut self, rx: mpsc::Receiver<String>) {
        self.ask_rx = Some(rx);
    }

    /// Set cancellation token
    pub fn set_cancel_token(&mut self, token: CancellationToken) {
        self.cancel_token = Some(token);
    }

    /// Set approve mode at runtime
    pub fn set_approve_mode(&mut self, mode: ApproveMode) {
        log::info!("Agent approve mode changed: {} -> {}", self.approve_mode, mode);
        self.approve_mode = mode;
    }

    /// Update memory summary and rebuild system prompt.
    /// This is called before each turn to use context-aware memory retrieval.
    pub fn update_memory_summary(&mut self, summary: Option<String>) {
        self.memory_summary = summary;
        // Rebuild system prompt with new memory summary
        self.system_prompt = crate::prompt::build_system_prompt(
            &self.profile,
            &self.skills,
            self.project_overview.as_deref(),
            self.memory_summary.as_deref(),
        );
    }

    /// Run chat loop with tool execution (streaming version)
    pub async fn run(&mut self, user_input: String) -> Result<Vec<AgentEvent>> {
        let collector = EventCollector::new();
        
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
            if let Some(token) = &self.cancel_token
                && token.is_cancelled()
            {
                self.emit(AgentEvent::error("Operation cancelled".to_string(), None, None))?;
                break;
            }

            // Build request
            let tool_defs: Vec<ToolDefinition> = self.tools.iter().map(|t| t.definition()).collect();
            let request = ChatRequest {
                system: Some(self.system_prompt.clone()),
                messages: self.messages.clone(),
                max_tokens: self.max_tokens,
                tools: tool_defs,
                think: self.think,
                enable_caching: true,
                server_tools: Vec::new(),
            };

            // Call provider with streaming
            self.emit(AgentEvent::progress(
                if iterations == 1 { "Thinking..." } else { "Processing..." },
                None,
            ))?;

            // Use streaming API for real-time output
            let response = self.call_streaming(&request).await?;

            // Track usage
            self.track_usage(&response.usage);

            // Debug log: API call
            crate::debug::debug_log().api_call(
                &self.model_name,
                response.usage.input_tokens,
                response.usage.cache_read_input_tokens > 0
            );

            // Process response
            should_continue = self.process_response(&response).await?;

            // Check compression (use last_input_tokens = actual context window usage)
            let context_size = self.provider.context_size();
            let current_tokens = self.last_input_tokens.load(Ordering::Relaxed) as u32;
            if should_compress(current_tokens, context_size, &self.compression_config) {
                self.emit(AgentEvent::progress("Compressing context...", None))?;
                
                let _original_count = self.messages.len();
                let original_tokens = current_tokens;
                
                // Use sliding window compression (no AI needed)
                match crate::compress::compress_messages(
                    &self.messages,
                    crate::compress::CompressionStrategy::SlidingWindow,
                    &self.compression_config,
                ) {
                    Ok(compressed) => {
                        let compressed_tokens = crate::compress::estimate_total_tokens(&compressed);
                        self.messages = compressed;
                        self.total_input_tokens.store(compressed_tokens as u64, Ordering::Relaxed);
                        self.last_input_tokens.store(compressed_tokens as u64, Ordering::Relaxed);
                        
                        // Debug log: compression
                        let ratio = compressed_tokens as f32 / original_tokens as f32;
                        crate::debug::debug_log().compression(original_tokens, compressed_tokens, ratio);
                        
                        self.emit(AgentEvent::with_data(
                            crate::event::EventType::CompressionCompleted,
                            crate::event::EventData::Compression {
                                original_tokens: original_tokens as u64,
                                compressed_tokens: compressed_tokens as u64,
                                ratio: compressed_tokens as f32 / original_tokens as f32,
                            },
                        ))?;
                    }
                    Err(e) => {
                        self.emit(AgentEvent::progress(
                            format!("Compression failed: {}", e),
                            None,
                        ))?;
                    }
                }
            }
        }

        // Send final usage stats (use last_input_tokens for accurate context display)
        self.emit(AgentEvent::usage_with_cache(
            self.last_input_tokens.load(Ordering::Relaxed),
            self.total_output_tokens.load(Ordering::Relaxed),
            0, 0,  // Cache info already sent per-request
        ))?;

        // Send session ended
        self.emit(AgentEvent::session_ended())?;

        Ok(collector.events().to_vec())
    }

    /// Call provider with streaming and emit events in real-time
    async fn call_streaming(&mut self, request: &ChatRequest) -> Result<ChatResponse> {
        use crate::providers::StreamEvent;
        
        const MAX_RETRIES: u32 = 5;
        const RETRY_DELAY_MS: u64 = 1000;  // 1 second base delay
        
        let mut attempt = 0;
        
        loop {
            attempt += 1;
            
            // Try to start streaming
            let rx_result = self.provider.chat_stream(request.clone()).await;
            
            match rx_result {
                Ok(mut rx) => {
                    // Successfully started streaming
                    let mut response_content: Vec<ContentBlock> = Vec::new();
                    let mut current_text = String::new();
                    let mut current_thinking = String::new();
                    let mut usage = Usage {
                        input_tokens: 0,
                        output_tokens: 0,
                        cache_creation_input_tokens: 0,
                        cache_read_input_tokens: 0,
                    };

                    while let Some(event) = rx.recv().await {
                        match event {
                            StreamEvent::FirstByte => {
                                // First byte received, streaming starts
                            }
                            StreamEvent::ThinkingDelta(delta) => {
                                if current_thinking.is_empty() {
                                    self.emit(AgentEvent::thinking_start())?;
                                }
                                current_thinking.push_str(&delta);
                                self.emit(AgentEvent::thinking_delta(delta, None))?;
                            }
                            StreamEvent::TextDelta(delta) => {
                                if current_text.is_empty() {
                                    self.emit(AgentEvent::text_start())?;
                                }
                                current_text.push_str(&delta);
                                self.emit(AgentEvent::text_delta(delta))?;
                            }
                            StreamEvent::ToolUseStart { id, name } => {
                                // Finish any pending text
                                if !current_text.is_empty() {
                                    self.emit(AgentEvent::text_end())?;
                                    response_content.push(ContentBlock::Text { text: current_text.clone() });
                                    current_text.clear();
                                }
                                // Finish any pending thinking
                                if !current_thinking.is_empty() {
                                    self.emit(AgentEvent::thinking_end())?;
                                    response_content.push(ContentBlock::Thinking {
                                        thinking: current_thinking.clone(),
                                        signature: None,
                                    });
                                    current_thinking.clear();
                                }
                                self.emit(AgentEvent::tool_use_start(&id, &name, None))?;
                            }
                            StreamEvent::ToolInputDelta { bytes_so_far: _ } => {
                                // Tool input progress - could emit progress event
                            }
                            StreamEvent::Done(resp) => {
                                // Finish any pending text
                                if !current_text.is_empty() {
                                    self.emit(AgentEvent::text_end())?;
                                    response_content.push(ContentBlock::Text { text: current_text.clone() });
                                }
                                // Finish any pending thinking
                                if !current_thinking.is_empty() {
                                    self.emit(AgentEvent::thinking_end())?;
                                    response_content.push(ContentBlock::Thinking {
                                        thinking: current_thinking.clone(),
                                        signature: None,
                                    });
                                }
                                // Add any remaining blocks from response
                                for block in &resp.content {
                                    if !response_content.iter().any(|b| b == block) {
                                        response_content.push(block.clone());
                                    }
                                }
                                usage = resp.usage;
                            }
                            StreamEvent::Error(msg) => {
                                // Stream error - might be retryable
                                if attempt < MAX_RETRIES {
                                    self.emit(AgentEvent::progress(
                                        format!("⚠️ Stream error, retrying ({}/{}): {}", attempt, MAX_RETRIES, &msg),
                                        None,
                                    ))?;
                                    // Exponential backoff
                                    let delay = RETRY_DELAY_MS * (1 << (attempt - 1));
                                    tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
                                    continue;  // Retry the outer loop
                                } else {
                                    self.emit(AgentEvent::error(msg.clone(), None, None))?;
                                    return Err(anyhow::anyhow!("Stream error after {} retries: {}", MAX_RETRIES, msg));
                                }
                            }
                        }
                    }

                    return Ok(ChatResponse {
                        content: response_content,
                        stop_reason: StopReason::EndTurn,
                        usage,
                    });
                }
                Err(e) => {
                    // Failed to start streaming
                    if attempt < MAX_RETRIES {
                        let error_msg = e.to_string();
                        self.emit(AgentEvent::progress(
                            format!("⚠️ API error, retrying ({}/{}): {}", attempt, MAX_RETRIES, &error_msg),
                            None,
                        ))?;
                        // Exponential backoff: 1s, 2s, 4s, 8s, 16s
                        let delay = RETRY_DELAY_MS * (1 << (attempt - 1));
                        tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
                    } else {
                        return Err(anyhow::anyhow!("API error after {} retries: {}", MAX_RETRIES, e));
                    }
                }
            }
        }
    }

    /// Process response and handle tool_use (Text/Thinking events already sent via streaming)
    async fn process_response(&mut self, response: &ChatResponse) -> Result<bool> {
        let mut has_tool_use = false;
        let mut assistant_content: Vec<ContentBlock> = Vec::new();
        let mut tool_results: Vec<Message> = Vec::new();

        for block in &response.content {
            match block {
                // Text and Thinking events already sent via streaming, just add to history
                ContentBlock::Text { text } => {
                    assistant_content.push(ContentBlock::Text { text: text.clone() });
                }

                ContentBlock::Thinking { thinking, signature } => {
                    assistant_content.push(ContentBlock::Thinking {
                        thinking: thinking.clone(),
                        signature: signature.clone(),
                    });
                }

                ContentBlock::ToolUse { id, name, input } => {
                    has_tool_use = true;
                    
                    self.emit(AgentEvent::tool_use_start(id.clone(), name.clone(), Some(input.clone())))?;
                    
                    // Execute tool
                    let result = self.execute_tool(name, input.clone()).await;
                    
                    let (content, is_error) = match result {
                        Ok(output) => (output, false),
                        Err(e) => (e.to_string(), true),
                    };

                    self.emit(AgentEvent::tool_result(id.clone(), content.clone(), is_error))?;

                    // Add tool_use to assistant content
                    assistant_content.push(ContentBlock::ToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        input: input.clone(),
                    });

                    // Collect tool results (will be added after assistant message)
                    tool_results.push(Message {
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

        // Add assistant message to history FIRST
        if !assistant_content.is_empty() {
            self.messages.push(Message {
                role: Role::Assistant,
                content: MessageContent::Blocks(assistant_content),
            });
        }

        // Then add tool results (User messages)
        for msg in tool_results {
            self.messages.push(msg);
        }

        // Continue if there were tool calls
        Ok(has_tool_use)
    }

    /// Execute a tool
    async fn execute_tool(&mut self, name: &str, input: serde_json::Value) -> Result<String> {
        let tool = self.tools.iter().find(|t| t.definition().name == name);

        if let Some(tool) = tool {
            // Debug: log approval check
            log::debug!(
                "Tool '{}' approval check: mode={}, risk={}, needs_approval={}",
                name, self.approve_mode, tool.risk_level(),
                needs_approval(self.approve_mode, tool.risk_level())
            );
            
            // Check approval
            if needs_approval(self.approve_mode, tool.risk_level()) {
                // Ask user for approval via TUI
                if self.ask_rx.is_some() {
                    // Build approval question with tool details
                    let detail = match name {
                        "bash" => format!("Command: {}", input["command"].as_str().unwrap_or("?")),
                        "write" => format!("File: {}", input["path"].as_str().unwrap_or("?")),
                        "edit" | "multi_edit" => format!("File: {}", input["path"].as_str().unwrap_or("?")),
                        _ => format!("Tool: {}", name),
                    };
                    
                    let question = format!(
                        "⚠️ Tool '{}' requires approval (risk: {})\n{}\n\nAllow? (y/n)",
                        name, tool.risk_level(), detail
                    );
                    
                    // Send approval request to TUI
                    self.emit(AgentEvent::with_data(
                        EventType::AskQuestion,
                        EventData::AskQuestion { question, options: None },
                    ))?;
                    
                    // Wait for user response
                    if let Some(rx) = &mut self.ask_rx {
                        match rx.recv().await {
                            Some(answer) => {
                                let answer_lower = answer.trim().to_lowercase();
                                // Check for abort
                                if matches!(answer_lower.as_str(), "a" | "abort" | "q" | "quit" | "stop") {
                                    self.emit(AgentEvent::with_data(
                                        EventType::Error,
                                        EventData::Error { message: "Aborted by user".into(), code: None, source: None },
                                    ))?;
                                    return Err(anyhow::anyhow!("Session aborted by user"));
                                }
                                // Check for approval
                                let approved = matches!(
                                    answer_lower.as_str(),
                                    "y" | "yes" | "ok" | "approve" | ""
                                );
                                if !approved {
                                    // Rejected - return error to AI so it can try alternative approach
                                    return Err(anyhow::anyhow!(
                                        "Tool '{}' rejected by user (answer: '{}')", name, answer_lower
                                    ));
                                }
                            }
                            None => {
                                return Err(anyhow::anyhow!("Approval channel closed"));
                            }
                        }
                    }
                } else {
                    // No ask channel - reject dangerous/mutating tools
                    return Err(anyhow::anyhow!(
                        "Tool '{}' requires manual approval (risk: {}). Use --approve-mode auto to auto-approve.",
                        name, tool.risk_level()
                    ));
                }
            }

            // Special handling for "ask" tool in TUI mode
            if name == "ask" && self.ask_rx.is_some() {
                let question = input["question"].as_str().unwrap_or("").to_string();
                let options = input.get("options").cloned();
                
                // Send AskQuestion event to TUI
                self.emit(AgentEvent::with_data(
                    EventType::AskQuestion,
                    EventData::AskQuestion { question, options },
                ))?;
                
                // Wait for user answer from TUI
                if let Some(rx) = &mut self.ask_rx {
                    match rx.recv().await {
                        Some(answer) => return Ok(answer),
                        None => return Err(anyhow::anyhow!("Ask channel closed")),
                    }
                }
            }

            // Execute tool normally
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
        // Store the latest request's input tokens — this is the actual context window usage.
        self.last_input_tokens.store(usage.input_tokens as u64, Ordering::Relaxed);

        // Emit usage event with cache info (use last_input_tokens for context display)
        let _ = self.event_tx.try_send(AgentEvent::usage_with_cache(
            usage.input_tokens as u64,
            usage.output_tokens as u64,
            usage.cache_read_input_tokens as u64,
            usage.cache_creation_input_tokens as u64,
        ));
    }

    /// Estimate context size
    #[allow(dead_code)]
    fn estimate_context_size(&self) -> u32 {
        // Rough estimate: each message ~100 tokens average
        (self.messages.len() as u32) * 100 + self.total_input_tokens.load(Ordering::Relaxed) as u32
    }

    /// Emit event (non-blocking)
    fn emit(&self, event: AgentEvent) -> Result<()> {
        // Use try_send to avoid blocking in async context
        match self.event_tx.try_send(event) {
            Ok(_) => Ok(()),
            Err(mpsc::error::TrySendError::Full(_)) => {
                // Channel full, drop event - not critical
                Ok(())
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                // Channel closed, receiver dropped
                Err(anyhow::anyhow!("Event channel closed"))
            }
        }
    }

    /// Restore message history (for session continue/resume)
    pub fn set_messages(&mut self, messages: Vec<Message>) {
        self.messages = messages;
    }

    /// Get current messages (for session saving)
    pub fn get_messages(&self) -> &[Message] {
        &self.messages
    }

    /// Get current token counts
    pub fn get_token_counts(&self) -> (u64, u64) {
        (
            self.total_input_tokens.load(Ordering::Relaxed),
            self.total_output_tokens.load(Ordering::Relaxed),
        )
    }

    /// Clear message history
    pub fn clear_history(&mut self) {
        self.messages.clear();
        self.total_input_tokens.store(0, Ordering::Relaxed);
        self.total_output_tokens.store(0, Ordering::Relaxed);
        self.last_input_tokens.store(0, Ordering::Relaxed);
    }

    /// Get message count
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }
}

/// Event collector for gathering events
#[derive(Default)]
pub struct EventCollector {
    events: Vec<AgentEvent>,
}

impl EventCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn events(&self) -> &[AgentEvent] {
        &self.events
    }
}