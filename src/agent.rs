use std::io::{Write as _, stdout};
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::mpsc;

use crate::approval::{ApproveMode, ApprovalAnswer, build_approval_request, needs_approval, prompt_approval};
use crate::compress::{CompressionConfig, CompressionStrategy, should_compress};
use crate::markdown;
use crate::cancel::CancellationToken;
use crate::models::{MultiModelConfig, Planner, TaskPlan, TaskComplexity};
use crate::providers::{
    ChatRequest, ChatResponse, ContentBlock, Message, MessageContent, Provider, Role, ServerTool,
    StopReason, StreamEvent, Usage,
};
use crate::skills::{self, Skill};
use crate::tools::{self, Tool};
use crate::tools::spinner::ToolSpinner;
use crate::ui;
use termimad::MadSkin;

pub use crate::prompt::PromptProfile;

const MAX_ITERATIONS: usize = 200;

// ============================================================================
// Token Statistics
// ============================================================================

/// Token usage statistics for the current session.
pub struct TokenStats {
    pub last_input_tokens: u32,
    pub total_output_tokens: u64,
    pub context_size: Option<u32>,
}

// ============================================================================
// Agent Structure
// ============================================================================

pub struct Agent {
    provider: Box<dyn Provider>,
    compress_provider: Option<Box<dyn Provider>>,
    plan_provider: Option<Box<dyn Provider>>,
    model_config: MultiModelConfig,
    tools: Vec<Box<dyn Tool>>,
    server_tools: Vec<ServerTool>,
    think: bool,
    max_tokens: u32,
    messages: Vec<Message>,
    markdown_enabled: bool,
    skin: MadSkin,
    system_prompt: String,
    project_overview: Option<String>,
    memory_summary: Option<String>,  // 跨会话记忆摘要
    profile: PromptProfile,
    skills: Arc<Vec<Skill>>,
    total_output_tokens: u64,
    last_input_tokens: u32,
    compression_config: CompressionConfig,
    enable_caching: bool,
    last_compression_result: Option<crate::compress::CompressionResult>,
    last_plan: Option<TaskPlan>,
    cancel_token: Option<CancellationToken>,
    approve_mode: ApproveMode,
}

// ============================================================================
// Agent Builder
// ============================================================================

/// Builder for creating an Agent with custom configuration.
/// 
/// # Example
/// ```ignore
/// let agent = Agent::builder(provider)
///     .think(true)
///     .markdown(true)
///     .profile(PromptProfile::Default)
///     .skills(skills)
///     .max_tokens(16384)
///     .build();
/// ```
pub struct AgentBuilder {
    provider: Box<dyn Provider>,
    think: bool,
    markdown_enabled: bool,
    profile: PromptProfile,
    skills: Vec<Skill>,
    max_tokens: u32,
    project_overview: Option<String>,
    memory_summary: Option<String>,  // 跨会话记忆摘要
}

impl AgentBuilder {
    /// Create a new builder with the required provider.
    pub fn new(provider: Box<dyn Provider>) -> Self {
        Self {
            provider,
            think: true,
            markdown_enabled: true,
            profile: PromptProfile::Default,
            skills: Vec::new(),
            max_tokens: 16384,
            project_overview: None,
            memory_summary: None,
        }
    }

    /// Enable or disable extended thinking mode.
    pub fn think(mut self, enabled: bool) -> Self {
        self.think = enabled;
        self
    }

    /// Enable or disable markdown rendering.
    pub fn markdown(mut self, enabled: bool) -> Self {
        self.markdown_enabled = enabled;
        self
    }

    /// Set the prompt profile.
    pub fn profile(mut self, profile: PromptProfile) -> Self {
        self.profile = profile;
        self
    }

    /// Set the skills list.
    pub fn skills(mut self, skills: Vec<Skill>) -> Self {
        self.skills = skills;
        self
    }

    /// Set maximum output tokens.
    pub fn max_tokens(mut self, max: u32) -> Self {
        self.max_tokens = max;
        self
    }

    /// Set project overview content.
    pub fn overview(mut self, overview: impl Into<String>) -> Self {
        self.project_overview = Some(overview.into());
        self
    }

    /// Set memory summary content (accumulated memories across sessions).
    pub fn memory(mut self, summary: impl Into<String>) -> Self {
        self.memory_summary = Some(summary.into());
        self
    }

    /// Build the Agent instance.
    pub fn build(self) -> Agent {
        let skills_arc = Arc::new(self.skills);
        let system_prompt = build_system_prompt(
            self.profile,
            &skills_arc,
            self.project_overview.as_deref(),
            self.memory_summary.as_deref(),
        );
        Agent {
            provider: self.provider,
            compress_provider: None,
            plan_provider: None,
            model_config: MultiModelConfig::default(),
            tools: tools::all_tools_with_skills(skills_arc.clone()),
            server_tools: Vec::new(),
            think: self.think,
            max_tokens: self.max_tokens,
            messages: Vec::new(),
            markdown_enabled: markdown::should_render(self.markdown_enabled),
            skin: markdown::default_skin(),
            system_prompt,
            project_overview: self.project_overview,
            memory_summary: self.memory_summary,
            profile: self.profile,
            skills: skills_arc,
            total_output_tokens: 0,
            last_input_tokens: 0,
            compression_config: CompressionConfig::default(),
            enable_caching: true,
            last_compression_result: None,
            last_plan: None,
            cancel_token: None,
            approve_mode: ApproveMode::Ask,
        }
    }
}

// ============================================================================
// Agent Implementation - Constructors
// ============================================================================

impl Agent {
    /// Create a builder for constructing an Agent.
    pub fn builder(provider: Box<dyn Provider>) -> AgentBuilder {
        AgentBuilder::new(provider)
    }

    /// Simple constructor with default settings.
    pub fn new(provider: Box<dyn Provider>) -> Self {
        Self::builder(provider).build()
    }

    /// Constructor with thinking option.
    pub fn with_options(provider: Box<dyn Provider>, think: bool) -> Self {
        Self::builder(provider).think(think).build()
    }

    /// Constructor with full basic options.
    pub fn with_full_options(provider: Box<dyn Provider>, think: bool, markdown_enabled: bool) -> Self {
        Self::builder(provider)
            .think(think)
            .markdown(markdown_enabled)
            .build()
    }

    /// Constructor with prompt profile.
    pub fn with_profile(provider: Box<dyn Provider>, think: bool, markdown_enabled: bool, profile: PromptProfile) -> Self {
        Self::builder(provider)
            .think(think)
            .markdown(markdown_enabled)
            .profile(profile)
            .build()
    }

    /// Constructor with skills list.
    pub fn with_skills(provider: Box<dyn Provider>, think: bool, markdown_enabled: bool, skills: Vec<Skill>) -> Self {
        Self::builder(provider)
            .think(think)
            .markdown(markdown_enabled)
            .skills(skills)
            .build()
    }

    /// Full constructor with profile and skills.
    pub fn with_profile_and_skills(
        provider: Box<dyn Provider>,
        think: bool,
        markdown_enabled: bool,
        profile: PromptProfile,
        skills: Vec<Skill>,
    ) -> Self {
        Self::builder(provider)
            .think(think)
            .markdown(markdown_enabled)
            .profile(profile)
            .skills(skills)
            .build()
    }

    /// Full constructor with max_tokens.
    pub fn with_profile_and_skills_and_max_tokens(
        provider: Box<dyn Provider>,
        think: bool,
        markdown_enabled: bool,
        profile: PromptProfile,
        skills: Vec<Skill>,
        max_tokens: u32,
    ) -> Self {
        Self::builder(provider)
            .think(think)
            .markdown(markdown_enabled)
            .profile(profile)
            .skills(skills)
            .max_tokens(max_tokens)
            .build()
    }

    /// Full constructor with project overview.
    pub fn with_profile_and_skills_and_max_tokens_and_overview(
        provider: Box<dyn Provider>,
        think: bool,
        markdown_enabled: bool,
        profile: PromptProfile,
        skills: Vec<Skill>,
        max_tokens: u32,
        project_overview: Option<&str>,
    ) -> Self {
        Self::with_memory_and_overview(
            provider,
            think,
            markdown_enabled,
            profile,
            skills,
            max_tokens,
            project_overview,
            None,  // No memory summary
        )
    }

    /// Constructor with full options including memory summary.
    pub fn with_memory_and_overview(
        provider: Box<dyn Provider>,
        think: bool,
        markdown_enabled: bool,
        profile: PromptProfile,
        skills: Vec<Skill>,
        max_tokens: u32,
        project_overview: Option<&str>,
        memory_summary: Option<&str>,
    ) -> Self {
        let mut builder = Self::builder(provider)
            .think(think)
            .markdown(markdown_enabled)
            .profile(profile)
            .skills(skills)
            .max_tokens(max_tokens);
        if let Some(overview) = project_overview {
            builder = builder.overview(overview);
        }
        if let Some(memory) = memory_summary {
            builder = builder.memory(memory);
        }
        builder.build()
    }

    // ========================================================================
    // Configuration Methods (Setters/Getters)
    // ========================================================================

    /// Set cancellation token for interrupting operations.
    pub fn set_cancel_token(&mut self, token: CancellationToken) {
        self.cancel_token = Some(token);
    }

    /// Set the approval mode for tool execution.
    pub fn set_approve_mode(&mut self, mode: ApproveMode) {
        self.approve_mode = mode;
    }

    /// Get the current approval mode.
    pub fn approve_mode(&self) -> ApproveMode {
        self.approve_mode
    }

    /// Toggle approval mode: Ask -> Auto -> Strict -> Ask
    pub fn toggle_approve_mode(&mut self) {
        self.approve_mode = self.approve_mode.next();
    }

    /// Clear the cancellation token.
    pub fn clear_cancel_token(&mut self) {
        self.cancel_token = None;
    }

    /// Check if the current operation should be cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancel_token.as_ref().map(|t| t.is_cancelled()).unwrap_or(false)
    }

    /// Set compress provider for AI summarization.
    pub fn with_compress_provider(mut self, provider: Box<dyn Provider>) -> Self {
        self.compress_provider = Some(provider);
        self
    }

    /// Set plan provider for task planning.
    pub fn with_plan_provider(mut self, provider: Box<dyn Provider>) -> Self {
        self.plan_provider = Some(provider);
        self
    }

    /// Set multi-model configuration.
    pub fn with_model_config(mut self, config: MultiModelConfig) -> Self {
        self.model_config = config;
        self
    }

    /// Get model configuration.
    pub fn model_config(&self) -> &MultiModelConfig {
        &self.model_config
    }

    /// Get loaded skills list.
    pub fn skills(&self) -> &[Skill] {
        &self.skills
    }

    /// Get current model name (main model).
    pub fn current_model(&self) -> &str {
        &self.model_config.main.name
    }

    /// Enable server-side web search tool. This allows the model to perform
    /// web searches directly via the API provider without client intervention.
    pub fn with_web_search(mut self, max_uses: Option<u32>) -> Self {
        self.server_tools.push(ServerTool::web_search(max_uses));
        self
    }

    /// Enable or disable prompt caching.
    pub fn with_caching(mut self, enable: bool) -> Self {
        self.enable_caching = enable;
        self
    }

    /// Set caching flag.
    pub fn set_caching(&mut self, enable: bool) {
        self.enable_caching = enable;
    }

    /// Set server tools explicitly.
    pub fn set_server_tools(&mut self, server_tools: Vec<ServerTool>) {
        self.server_tools = server_tools;
    }

    /// Set or update the project overview and rebuild system prompt.
    pub fn set_project_overview(&mut self, overview: &str) {
        self.project_overview = Some(overview.to_string());
        self.system_prompt = build_system_prompt(
            self.profile,
            &self.skills,
            Some(overview),
            self.memory_summary.as_deref(),
        );
    }

    /// Clear the project overview and rebuild system prompt.
    pub fn clear_project_overview(&mut self) {
        self.project_overview = None;
        self.system_prompt = build_system_prompt(
            self.profile,
            &self.skills,
            None,
            self.memory_summary.as_deref(),
        );
    }

    /// Set or update the memory summary and rebuild system prompt.
    pub fn set_memory_summary(&mut self, summary: &str) {
        self.memory_summary = Some(summary.to_string());
        self.system_prompt = build_system_prompt(
            self.profile,
            &self.skills,
            self.project_overview.as_deref(),
            Some(summary),
        );
    }

    /// Clear the memory summary and rebuild system prompt.
    pub fn clear_memory_summary(&mut self) {
        self.memory_summary = None;
        self.system_prompt = build_system_prompt(
            self.profile,
            &self.skills,
            self.project_overview.as_deref(),
            None,
        );
    }

    /// Get current memory summary.
    pub fn memory_summary(&self) -> Option<&str> {
        self.memory_summary.as_deref()
    }

    /// Set compression configuration.
    pub fn set_compression_config(&mut self, config: CompressionConfig) {
        self.compression_config = config;
    }

    /// Get compression configuration.
    pub fn compression_config(&self) -> &CompressionConfig {
        &self.compression_config
    }

    /// Borrow the accumulated conversation for persistence.
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Get a reference to the provider.
    pub fn provider(&self) -> &dyn Provider {
        self.provider.as_ref()
    }

    /// Replace the accumulated conversation, e.g. when resuming a session.
    pub fn set_messages(&mut self, messages: Vec<Message>) {
        self.messages = messages;
    }

    /// Clear the conversation history.
    pub fn clear_messages(&mut self) {
        self.messages.clear();
    }

    /// Get the number of messages in the conversation.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Get token usage statistics.
    pub fn token_stats(&self) -> TokenStats {
        TokenStats {
            last_input_tokens: self.last_input_tokens,
            total_output_tokens: self.total_output_tokens,
            context_size: self.provider.context_size(),
        }
    }

    // ========================================================================
    // Core Chat Methods
    // ========================================================================

    /// Run a single user turn, re-using accumulated conversation history.
    /// The agent keeps looping through tool_use turns internally until it
    /// produces a non-tool-use response, then returns control to the caller.
    pub async fn chat_once(&mut self, user_input: &str) -> Result<()> {
        self.messages.push(Message {
            role: Role::User,
            content: MessageContent::Text(user_input.to_string()),
        });

        // Check if context compression is needed before sending request
        self.check_and_compress();

        let tool_defs: Vec<_> = self.tools.iter().map(|t| t.definition()).collect();

        // Track max_tokens continuation count to avoid infinite loops
        let mut continuation_count = 0;
        const MAX_CONTINUATIONS: usize = 5;

        // Track whether we've already retried after an input-length error
        let mut retried_after_length_error = false;

        for iteration in 0..MAX_ITERATIONS {
            let request = ChatRequest {
                messages: self.messages.clone(),
                tools: tool_defs.clone(),
                system: Some(self.system_prompt.clone()),
                think: self.think,
                max_tokens: self.max_tokens,
                server_tools: self.server_tools.clone(),
                enable_caching: self.enable_caching,
            };

            let response = match self.stream_one_turn(request).await {
                Ok(r) => r,
                Err(e) if !retried_after_length_error && is_input_length_error(&e) => {
                    retried_after_length_error = true;
                    eprintln!("\n[error] input too long for API, force-compressing context...");
                    self.force_compress();
                    continue;
                }
                Err(e) => return Err(e),
            };

            self.record_usage(&response.usage);
            self.print_usage_line(&response.usage);

            self.messages.push(Message {
                role: Role::Assistant,
                content: MessageContent::Blocks(response.content.clone()),
            });

            if response.stop_reason == StopReason::ToolUse {
                let tool_results = self.execute_tool_calls(&response.content).await;

                self.messages.push(Message {
                    role: Role::Tool,
                    content: MessageContent::Blocks(tool_results),
                });

                if iteration + 1 == MAX_ITERATIONS {
                    eprintln!(
                        "\n[warn] reached MAX_ITERATIONS ({}), stopping without a final reply",
                        MAX_ITERATIONS
                    );
                }
                continue;
            }

            // Handle max_tokens truncation: ask model to continue
            if response.stop_reason == StopReason::MaxTokens {
                if continuation_count >= MAX_CONTINUATIONS {
                    eprintln!(
                        "\n[warn] reached max continuation limit ({}), output may be incomplete",
                        MAX_CONTINUATIONS
                    );
                    return Ok(());
                }
                continuation_count += 1;
                println!("\n[output truncated, auto-continuing ({}/{})...]", continuation_count, MAX_CONTINUATIONS);
                self.messages.push(Message {
                    role: Role::User,
                    content: MessageContent::Text("请继续完成你的回复。".to_string()),
                });
                continue;
            }

            return Ok(());
        }

        Ok(())
    }

    // ========================================================================
    // Context Compression Methods
    // ========================================================================

    /// Check if context compression is needed and perform it.
    /// Returns compression result if compression was performed.
    fn check_and_compress(&mut self) {
        use crate::compress::{CompressionResult, compress_messages};

        // Clear previous compression result
        self.last_compression_result = None;

        let context_size = self.provider.context_size();
        // Use last_input_tokens if available (from a prior turn), otherwise
        // estimate from message content. This handles the case where we resume
        // a large session and haven't made an API call yet.
        let current_tokens = if self.last_input_tokens > 0 {
            self.last_input_tokens
        } else {
            crate::compress::estimate_total_tokens(&self.messages)
        };

        if should_compress(current_tokens, context_size, &self.compression_config) {
            let original_count = self.messages.len();
            let original_tokens = crate::compress::estimate_total_tokens(&self.messages);
            
            println!(
                "\n[compressing context: {} tokens / {} max ({:.0}%)]",
                current_tokens,
                context_size.unwrap_or(0),
                (current_tokens as f64 / context_size.unwrap_or(1) as f64 * 100.0)
            );
            
            let strategy = if self.compression_config.use_summarization {
                CompressionStrategy::SlidingWindow
            } else {
                CompressionStrategy::Truncate
            };
            
            match compress_messages(&self.messages, strategy, &self.compression_config) {
                Ok(compressed) => {
                    let new_count = compressed.len();
                    let new_tokens = crate::compress::estimate_total_tokens(&compressed);
                    let tokens_saved = original_tokens.saturating_sub(new_tokens);
                    
                    self.messages = compressed;
                    
                    println!(
                        "[compressed: {} messages → {} messages (~{} tokens saved)]",
                        original_count, new_count, tokens_saved
                    );
                    
                    self.last_compression_result = Some(CompressionResult::new(
                        original_count,
                        new_count,
                        tokens_saved,
                        None,
                        strategy,
                    ));
                }
                Err(e) => {
                    eprintln!("[warn] compression failed: {}", e);
                }
            }
        }
    }

    /// Force-compress context aggressively when the API rejects input as too long.
    /// Uses a lower threshold and more aggressive settings to guarantee size reduction.
    fn force_compress(&mut self) {
        use crate::compress::{CompressionResult, CompressionStrategy, compress_messages};

        let original_count = self.messages.len();
        let original_tokens = crate::compress::estimate_total_tokens(&self.messages);

        // Use aggressive settings: keep only min_preserve_messages, target 30% of original
        let mut config = self.compression_config.clone();
        config.target_ratio = 0.3;

        let strategy = if config.use_summarization {
            CompressionStrategy::SlidingWindow
        } else {
            CompressionStrategy::Truncate
        };

        println!(
            "[force-compressing: {} messages, ~{} estimated tokens]",
            original_count, original_tokens
        );

        match compress_messages(&self.messages, strategy, &config) {
            Ok(compressed) => {
                let new_count = compressed.len();
                let new_tokens = crate::compress::estimate_total_tokens(&compressed);
                let tokens_saved = original_tokens.saturating_sub(new_tokens);

                self.messages = compressed;
                // Reset last_input_tokens so the next check_and_compress uses estimation
                self.last_input_tokens = 0;

                println!(
                    "[force-compressed: {} → {} messages (~{} tokens saved)]",
                    original_count, new_count, tokens_saved
                );

                self.last_compression_result = Some(CompressionResult::new(
                    original_count,
                    new_count,
                    tokens_saved,
                    None,
                    strategy,
                ));
            }
            Err(e) => {
                eprintln!("[warn] force-compression failed: {}", e);
                // Last resort: truncate to min_preserve_messages
                let keep = config.min_preserve_messages.min(self.messages.len());
                let removed = self.messages.len() - keep;
                self.messages = self.messages.split_off(self.messages.len() - keep);
                self.last_input_tokens = 0;
                eprintln!(
                    "[fallback: dropped {} oldest messages, kept {}]",
                    removed, keep
                );
            }
        }
    }

    /// Get the last compression result (if any).
    pub fn last_compression_result(&self) -> Option<&crate::compress::CompressionResult> {
        self.last_compression_result.as_ref()
    }

    /// Get the last task plan (if any).
    pub fn last_plan(&self) -> Option<&TaskPlan> {
        self.last_plan.as_ref()
    }

    /// Generate a task plan using the plan model.
    /// Returns the plan if a plan provider is available.
    pub async fn plan_task(&mut self, request: &str) -> Result<Option<TaskPlan>> {
        if let Some(ref plan_provider) = self.plan_provider {
            let planner = Planner::new(
                plan_provider.clone_box(),
                self.model_config.plan.clone(),
            );
            
            // Get available tool names
            let tool_names: Vec<String> = self.tools.iter()
                .map(|t| t.definition().name.clone())
                .collect();
            let tool_names_refs: Vec<&str> = tool_names.iter().map(|s| s.as_str()).collect();
            
            println!("[planning task with {}...]", self.model_config.plan.display_name());
            
            let plan = planner.plan(request, &tool_names_refs).await?;
            
            println!("[plan generated: {} steps, complexity: {}]", 
                plan.steps.len(), 
                plan.complexity.display()
            );
            
            self.last_plan = Some(plan.clone());
            Ok(Some(plan))
        } else {
            // No plan provider, return None
            Ok(None)
        }
    }

    /// Quick complexity assessment using fast model.
    pub async fn assess_complexity(&self, request: &str) -> Result<TaskComplexity> {
        // Use main provider if no plan provider
        let provider = self.plan_provider.as_ref()
            .map(|p| p.as_ref())
            .unwrap_or(self.provider.as_ref());
        
        let planner = Planner::new(
            provider.clone_box(),
            self.model_config.fast.clone(),
        );
        
        planner.assess_complexity(request).await
    }

    /// Get suggested next action based on current plan.
    pub fn get_next_step(&self) -> Option<&crate::models::PlanStep> {
        self.last_plan.as_ref()
            .and_then(|plan| {
                // Find the first pending step (assuming we track progress)
                plan.steps.first()
            })
    }

    /// Manually compress context with specified bias.
    /// Returns compression result if compression was performed.
    pub fn compress_with_bias(&mut self, bias_spec: Option<&str>) -> Result<Option<crate::compress::CompressionResult>> {
        use crate::compress::{CompressionBias, CompressionResult, CompressionStrategy, compress_messages};
        
        // Parse bias specification
        let bias = if let Some(spec) = bias_spec {
            CompressionBias::parse(spec)?
        } else {
            self.compression_config.bias.clone()
        };

        // Update config with new bias temporarily
        let mut config = self.compression_config.clone();
        config.bias = bias.clone();

        let original_count = self.messages.len();
        if original_count <= config.min_preserve_messages {
            println!("[no need to compress: only {} messages]", original_count);
            return Ok(None);
        }

        let original_tokens = crate::compress::estimate_total_tokens(&self.messages);

        println!(
            "\n[manual compression: {} messages, ~{} tokens]",
            original_count,
            crate::compress::format_tokens(original_tokens)
        );
        println!("[bias: {}]", bias.format());

        let strategy = CompressionStrategy::BiasBased;

        match compress_messages(&self.messages, strategy, &config) {
            Ok(compressed) => {
                let new_count = compressed.len();
                let new_tokens = crate::compress::estimate_total_tokens(&compressed);
                let tokens_saved = original_tokens.saturating_sub(new_tokens);

                self.messages = compressed;
                self.compression_config.bias = bias; // Persist the bias

                println!(
                    "[compressed: {} → {} messages (~{} tokens saved)]",
                    original_count, new_count,
                    crate::compress::format_tokens(tokens_saved)
                );

                let result = CompressionResult::new(
                    original_count,
                    new_count,
                    tokens_saved,
                    None,
                    strategy,
                );
                self.last_compression_result = Some(result.clone());

                Ok(Some(result))
            }
            Err(e) => {
                eprintln!("[error] compression failed: {}", e);
                Err(e)
            }
        }
    }

    /// One-shot convenience: run a single prompt and discard agent state.
    pub async fn run(&mut self, prompt: &str) -> Result<()> {
        self.chat_once(prompt).await
    }

    // ========================================================================
    // Streaming Response Processing
    // ========================================================================

    /// Drive one streaming turn: show spinner while waiting, then print
    /// thinking deltas (dim) and text deltas (normal) as they arrive.
    /// Returns the assembled final response.
    async fn stream_one_turn(&self, request: ChatRequest) -> Result<ChatResponse> {
        let mut spinner = Some(ToolSpinner::new("thinking"));
        let mut rx = self.request_with_retry(&request, &mut spinner).await?;

        let mut in_thinking = false;
        let mut in_text = false;
        // Raw markdown accumulated for the current text block. Re-rendered
        // over the printed plaintext when the block closes.
        let mut text_buffer = String::new();
        let mut tool_spinner: Option<ToolSpinner> = None;
        let mut current_tool_name: Option<String> = None;
        let mut last_shown_bytes: usize = 0;
        let mut final_response: Option<ChatResponse> = None;

        loop {
            // Check for cancellation at the start of each iteration
            if self.is_cancelled() {
                // Spinner cleanup handled by Drop (RAII)
                spinner.take();
                tool_spinner.take();
                if in_thinking {
                    print!("{}", ui::RESET);
                }
                if in_text {
                    self.flush_text_block(&mut text_buffer);
                }
                println!("\n[interrupted]");
                // Return a minimal response to end the turn gracefully
                return Ok(ChatResponse {
                    content: vec![ContentBlock::Text { text: "[interrupted]".to_string() }],
                    stop_reason: StopReason::EndTurn,
                    usage: Usage::default(),
                });
            }

            // Use a shorter timeout (10ms) for more responsive cancellation
            let event = tokio::select! {
                evt = rx.recv() => evt,
                _ = tokio::time::sleep(std::time::Duration::from_millis(10)) => {
                    // Timeout - continue loop to check cancellation
                    // Also show "processing" spinner if text just ended and we're waiting for next event
                    if !in_text && !in_thinking && tool_spinner.is_none() && spinner.is_none() {
                        tool_spinner = Some(ToolSpinner::new("processing"));
                    }
                    continue;
                }
            };

            // Check cancellation again immediately after receiving an event
            if self.is_cancelled() {
                // Spinner cleanup handled by Drop (RAII)
                spinner.take();
                tool_spinner.take();
                if in_thinking {
                    print!("{}", ui::RESET);
                }
                if in_text {
                    self.flush_text_block(&mut text_buffer);
                }
                println!("\n[interrupted]");
                return Ok(ChatResponse {
                    content: vec![ContentBlock::Text { text: "[interrupted]".to_string() }],
                    stop_reason: StopReason::EndTurn,
                    usage: Usage::default(),
                });
            }

            match event {
                Some(StreamEvent::FirstByte) => {
                    // Main spinner done, clear it (cleanup handled by Drop)
                    spinner.take();
                }
                Some(StreamEvent::ThinkingDelta(t)) => {
                    if in_text {
                        // Text ended, thinking starts
                        self.flush_text_block(&mut text_buffer);
                        in_text = false;
                        // Show preparing spinner during the gap
                        if tool_spinner.is_none() {
                            tool_spinner = Some(ToolSpinner::new("processing"));
                        }
                    }
                    // Clear preparing spinner when thinking actually starts
                    tool_spinner.take();
                    if !in_thinking {
                        print!("{}[thinking] ", ui::DIM);
                        in_thinking = true;
                    }
                    print!("{}", t);
                    let _ = stdout().flush();
                }
                Some(StreamEvent::TextDelta(t)) => {
                    if in_thinking {
                        print!("{}\n\n", ui::RESET);
                        in_thinking = false;
                    }
                    // Clear preparing spinner when text resumes
                    tool_spinner.take();
                    in_text = true;
                    text_buffer.push_str(&t);
                    print!("{}", t);
                    let _ = stdout().flush();
                }
                Some(StreamEvent::ToolUseStart { name, .. }) => {
                    if in_thinking {
                        print!("{}\n\n", ui::RESET);
                        in_thinking = false;
                    }
                    if in_text {
                        self.flush_text_block(&mut text_buffer);
                        in_text = false;
                        // Show preparing spinner briefly while transitioning
                        if tool_spinner.is_none() {
                            tool_spinner = Some(ToolSpinner::new("preparing tool call"));
                        }
                    }
                    // Replace preparing spinner with tool-specific spinner
                    tool_spinner.take();
                    println!("[tool: {}]", name);
                    tool_spinner = Some(ToolSpinner::new(&format!("streaming {} input", name)));
                    current_tool_name = Some(name.clone());
                    last_shown_bytes = 0;
                }
                Some(StreamEvent::ToolInputDelta { bytes_so_far }) => {
                    // Throttle: only refresh the spinner label when the size
                    // has grown by at least ~1 KB, to avoid noisy redraws
                    // when the model streams many small partial_json chunks.
                    const REFRESH_STEP: usize = 1024;
                    if bytes_so_far >= last_shown_bytes + REFRESH_STEP {
                        if let Some(ref sp) = tool_spinner {
                            if let Some(ref name) = current_tool_name {
                                sp.set_message(&format!(
                                    "streaming {} input ({})",
                                    name,
                                    ui::format_bytes(bytes_so_far)
                                ));
                                last_shown_bytes = bytes_so_far;
                            }
                        }
                    }
                }
                Some(StreamEvent::Done(resp)) => {
                    // Tool spinner cleanup handled by Drop (RAII)
                    tool_spinner.take();
                    if in_thinking {
                        print!("{}", ui::RESET);
                    }
                    if in_text {
                        self.flush_text_block(&mut text_buffer);
                    } else {
                        println!();
                    }
                    final_response = Some(resp);
                    break;
                }
                Some(StreamEvent::Error(e)) => {
                    // All spinners cleanup handled by Drop (RAII)
                    spinner.take();
                    tool_spinner.take();
                    if in_thinking {
                        print!("{}", ui::RESET);
                    }
                    anyhow::bail!("stream error: {}", e);
                }
                None => break,
            }
        }

        // Main spinner cleanup handled by Drop (RAII) if still present
        spinner.take();
        
        final_response.ok_or_else(|| anyhow::anyhow!("stream ended without Done event"))
    }

    /// Close the current text block. If markdown rendering is active, erase
    /// the raw text we printed during streaming and redraw it through the
    /// markdown skin. Otherwise just emit a trailing newline so the next
    /// section starts on a fresh row.
    fn flush_text_block(&self, buffer: &mut String) {
        if buffer.is_empty() {
            println!();
            return;
        }
        if self.markdown_enabled {
            let width = markdown::term_width();
            markdown::rerender_over(buffer, &self.skin, width);
        } else {
            println!();
        }
        buffer.clear();
    }

    /// Maximum number of retries for transient API errors.
    const MAX_RETRIES: u32 = 3;

    /// Request the LLM with automatic retry on transient errors.
    /// Uses exponential backoff: 1s, 2s, 4s between retries.
    async fn request_with_retry(
        &self,
        request: &ChatRequest,
        spinner: &mut Option<ToolSpinner>,
    ) -> Result<mpsc::Receiver<StreamEvent>> {
        let mut last_err = None;

        for attempt in 0..=Self::MAX_RETRIES {
            if attempt > 0 {
                let delay_secs = 1u64 << (attempt - 1); // 1, 2, 4
                // Update spinner to show retry status
                if let Some(s) = spinner.as_ref() {
                    s.set_message(&format!(
                        "retrying ({}/{}) in {}s...",
                        attempt, Self::MAX_RETRIES, delay_secs
                    ));
                }
                eprintln!(
                    "\n[retry {}/{}] waiting {}s before retrying...",
                    attempt, Self::MAX_RETRIES, delay_secs
                );
                tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;

                // Restore spinner message
                if let Some(s) = spinner.as_ref() {
                    s.set_message("thinking");
                }
            }

            match self.provider.chat_stream(request.clone()).await {
                Ok(rx) => return Ok(rx),
                Err(e) => {
                    if Self::is_retryable_error(&e) && attempt < Self::MAX_RETRIES {
                        eprintln!("\n[error] transient API error: {}", e);
                        last_err = Some(e);
                        continue;
                    }
                    return Err(e);
                }
            }
        }

        // Should not reach here, but just in case
        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("request failed after retries")))
    }

    /// Determine if an error is transient and worth retrying.
    fn is_retryable_error(err: &anyhow::Error) -> bool {
        let msg = err.to_string().to_lowercase();

        // Keywords that indicate retryable errors
        const RETRYABLE_KEYWORDS: &[&str] = &[
            // HTTP status codes and messages
            "429", "rate limit",
            "500", "502", "503", "504",
            "internal server error", "bad gateway", 
            "service unavailable", "gateway timeout",
            // Network / connection errors
            "connection", "timeout", "timed out",
            "reset by peer", "broken pipe",
            "dns", "resolve",
            // Anthropic specific
            "overloaded", "capacity",
        ];

        RETRYABLE_KEYWORDS.iter().any(|kw| msg.contains(kw))
    }

    // ========================================================================
    // Tool Execution Methods
    // ========================================================================

    async fn execute_tool_calls(&self, content: &[ContentBlock]) -> Vec<ContentBlock> {
        let mut results = Vec::new();

        for block in content {
            match block {
                ContentBlock::ToolUse { id, name, input } => {
                    // Create transition spinner to fill the gap between Done event and tool execution
                    let mut transition_spinner = ToolSpinner::new(&format!("executing {}", name));

                    // Print tool input with nice formatting
                    ui::print_tool_input(name, input);

                    // Clear transition spinner before tool creates its own spinner
                    transition_spinner.finish_clear();

                    // Execute the tool (spinner is created immediately in tool's execute method)
                    let result = self.execute_single_tool(name, input).await;

                    let output = match result {
                        Ok(output) => {
                            // Print result header
                            println!("[result: {}]", name);
                            // Print result with indentation, truncate if too long
                            let truncated = ui::truncate(&output, 1000);
                            for line in truncated.lines() {
                                println!("  {}", line);
                            }
                            if output.len() > 1000 {
                                println!("  ... (truncated, {} bytes total)", output.len());
                            }
                            output
                        }
                        Err(e) => {
                            let err_msg = format!("Error: {}", e);
                            println!("[error: {}]", name);
                            println!("  {}", err_msg);
                            err_msg
                        }
                    };

                    results.push(ContentBlock::ToolResult {
                        tool_use_id: id.clone(),
                        content: output,
                    });
                }
                ContentBlock::ServerToolUse { id: _, name, input } => {
                    // Server tool use is just informational - the server executes it.
                    ui::print_tool_input(name, input);
                    // Server tools don't need client-side execution or result blocks
                    // The server will return web_search_tool_result directly.
                }
                ContentBlock::WebSearchResult { tool_use_id: _, content } => {
                    // Web search result from the server - display it.
                    println!("[web-search-result: {} results]", content.results.len());
                    for result in &content.results {
                        println!(
                            "  - {}",
                            result.title.as_deref().unwrap_or("(no title)")
                        );
                        println!("    {}", result.url);
                        if let Some(snippet) = &result.snippet {
                            println!("    {}", ui::truncate(snippet, 200));
                        }
                    }
                    // Web search results are already in the message, no need to add tool_result
                }
                _ => {}
            }
        }

        results
    }

    async fn execute_single_tool(&self, name: &str, input: &serde_json::Value) -> Result<String> {
        let tool = self
            .tools
            .iter()
            .find(|t| t.definition().name == name)
            .ok_or_else(|| anyhow::anyhow!("Unknown tool: {}", name))?;

        // Approval gate: check if user confirmation is needed
        let risk = tool.risk_level();
        if needs_approval(self.approve_mode, risk) {
            let request = build_approval_request(name, risk, input);
            match prompt_approval(&request) {
                ApprovalAnswer::Yes => { /* proceed */ }
                ApprovalAnswer::No => {
                    return Ok("用户拒绝了此操作，请调整方案或询问用户意见。".to_string());
                }
                ApprovalAnswer::Abort => {
                    anyhow::bail!("用户中止了本轮执行。");
                }
            }
        }

        // Special handling for 'ask' tool in auto mode: skip user interaction
        if name == "ask" && self.approve_mode == ApproveMode::Auto {
            // Extract recommendation if provided
            let recommendation = input["recommendation"]["option_id"].as_str();
            let reason = input["recommendation"]["reason"].as_str();
            let question = input["question"].as_str().unwrap_or("未提供问题");

            println!("[ask: auto mode, skipping user interaction]");
            
            if recommendation.is_some() {
                let rec = recommendation.unwrap();
                let r = reason.unwrap_or("无理由");
                return Ok(format!(
                    "自动模式下无法询问用户。\n问题：{}\n已自动采纳推荐方案：{}\n理由：{}\n请继续执行推荐方案。",
                    question, rec, r
                ));
            } else {
                return Ok(format!(
                    "自动模式下无法询问用户。问题：{}\n请根据你的判断选择最合适的方案继续执行。",
                    question
                ));
            }
        }

        tool.execute(input.clone()).await
    }

    fn record_usage(&mut self, usage: &Usage) {
        self.last_input_tokens = usage.input_tokens;
        self.total_output_tokens = self
            .total_output_tokens
            .saturating_add(usage.output_tokens as u64);
    }

    /// Print a compact one-liner summarising this turn's token usage and the
    /// current context-window fullness. Silent when the provider returned
    /// nothing usable (e.g. a proxied endpoint that strips `usage`).
    fn print_usage_line(&self, usage: &Usage) {
        if usage.input_tokens == 0 && usage.output_tokens == 0 {
            return;
        }

        let mut parts: Vec<String> = Vec::with_capacity(4);
        parts.push(format!(
            "in {} / out {} (session out: {})",
            ui::format_tokens(usage.input_tokens as u64),
            ui::format_tokens(usage.output_tokens as u64),
            ui::format_tokens(self.total_output_tokens),
        ));
        if usage.cache_read_input_tokens > 0 || usage.cache_creation_input_tokens > 0 {
            parts.push(format!(
                "cache r/w {}/{}",
                ui::format_tokens(usage.cache_read_input_tokens as u64),
                ui::format_tokens(usage.cache_creation_input_tokens as u64),
            ));
        }
        if let Some(ctx) = self.provider.context_size() {
            let used = usage.input_tokens;
            let pct = (used as f64 / ctx as f64 * 100.0).min(100.0);
            parts.push(format!(
                "ctx {} / {} ({:.1}%) {}",
                ui::format_tokens(used as u64),
                ui::format_tokens(ctx as u64),
                pct,
                ui::bar(pct, 20),
            ));
        }

        println!("{}{}{}", ui::DIM, parts.join(" | "), ui::RESET);
    }
}

// ============================================================================
// Helper Functions (Utilities)
// ============================================================================

/// Detect API errors caused by input exceeding the model's max input length.
fn is_input_length_error(err: &anyhow::Error) -> bool {
    let msg = err.to_string();
    msg.contains("Range of input length should be")
        || msg.contains("InvalidParameter")
        || (msg.contains("400") && msg.contains("input length"))
}

/// Build the system prompt with optional project overview section.
fn build_system_prompt(
    profile: PromptProfile,
    skills: &Arc<Vec<Skill>>,
    project_overview: Option<&str>,
    memory_summary: Option<&str>,
) -> String {
    use crate::prompt::{PromptContext, SystemPromptBuilder, SECTION_PROJECT_CONTEXT, SECTION_ACCUMULATED_MEMORY};

    let mut prompt_context = PromptContext::new()
        .with_available_skills(skills::format_catalogue(skills).unwrap_or_default());

    if let Some(overview) = project_overview {
        prompt_context = prompt_context.with_section(SECTION_PROJECT_CONTEXT, overview);
    }

    if let Some(memory) = memory_summary {
        prompt_context = prompt_context.with_section(SECTION_ACCUMULATED_MEMORY, memory);
    }

    SystemPromptBuilder::new(profile)
        .with_context(prompt_context)
        .build()
}

#[cfg(test)]
mod tests {
    #[test]
    fn truncate_ascii_under_max() {
        assert_eq!(crate::ui::truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_ascii_over_max() {
        assert_eq!(crate::ui::truncate("hello world", 5), "hello");
    }

    #[test]
    fn truncate_multibyte_mid_char_does_not_panic() {
        let s = "中文".repeat(200);
        let t = crate::ui::truncate(&s, 500);
        assert!(t.len() <= 500);
        assert!(s.starts_with(t));
    }

    #[test]
    fn truncate_zero_max() {
        assert_eq!(crate::ui::truncate("中", 0), "");
    }
}
