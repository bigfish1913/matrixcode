use std::io::{Write as _, stdout};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};

use crate::compress::{CompressionConfig, CompressionStrategy, should_compress};
use crate::markdown;
use crate::models::{MultiModelConfig, Planner, TaskPlan, TaskComplexity};
use crate::providers::{
    ChatRequest, ChatResponse, ContentBlock, Message, MessageContent, Provider, Role, ServerTool,
    StopReason, StreamEvent, Usage,
};
use crate::skills::{self, Skill};
use crate::tools::{self, Tool};
use termimad::MadSkin;

pub use crate::prompt::PromptProfile;

const MAX_ITERATIONS: usize = 200;

/// Token usage statistics for the current session.
pub struct TokenStats {
    pub last_input_tokens: u32,
    pub total_output_tokens: u64,
    pub context_size: Option<u32>,
}

// ANSI dim italic for thinking, reset at end. Kept minimal to avoid pulling in a color crate.
const DIM: &str = "\x1b[2;3m";
const RESET: &str = "\x1b[0m";

pub struct Agent {
    provider: Box<dyn Provider>,
    /// Optional secondary providers for specific tasks.
    compress_provider: Option<Box<dyn Provider>>,
    plan_provider: Option<Box<dyn Provider>>,
    /// Multi-model configuration.
    model_config: MultiModelConfig,
    tools: Vec<Box<dyn Tool>>,
    /// Server-side tools that are executed by the API provider (e.g., web_search).
    server_tools: Vec<ServerTool>,
    think: bool,
    max_tokens: u32,
    messages: Vec<Message>,
    /// Whether to re-render assistant text as markdown when a text block ends.
    markdown_enabled: bool,
    /// Cached skin; cheap to build but we only need one per agent.
    skin: MadSkin,
    /// Final system prompt with any skills catalogue already appended.
    system_prompt: String,
    /// Project overview content, injected into system prompt when present.
    project_overview: Option<String>,
    /// Profile used to build the static system prompt.
    profile: PromptProfile,
    /// Skills catalogue for the skill tool.
    skills: Arc<Vec<Skill>>,
    /// Cumulative output tokens across the whole session. (Input tokens for
    /// the next turn include prior output already — they are reported
    /// directly by the provider — so we track input via "latest turn" and
    /// output via running sum.)
    total_output_tokens: u64,
    /// Latest `input_tokens` reported by the provider, which equals the
    /// number of tokens currently resident in the context window.
    last_input_tokens: u32,
    /// Compression configuration for context management.
    compression_config: CompressionConfig,
    /// Enable prompt caching (Anthropic-specific).
    enable_caching: bool,
    /// Last compression result (if any), for session recording.
    last_compression_result: Option<crate::compress::CompressionResult>,
    /// Last task plan (if any), for tracking execution.
    last_plan: Option<TaskPlan>,
}

impl Agent {
    pub fn new(provider: Box<dyn Provider>) -> Self {
        Self::with_options(provider, true)
    }

    pub fn with_options(provider: Box<dyn Provider>, think: bool) -> Self {
        Self::with_full_options(provider, think, true)
    }

    pub fn with_full_options(
        provider: Box<dyn Provider>,
        think: bool,
        markdown_enabled: bool,
    ) -> Self {
        Self::with_profile(provider, think, markdown_enabled, PromptProfile::Default)
    }

    pub fn with_profile(
        provider: Box<dyn Provider>,
        think: bool,
        markdown_enabled: bool,
        profile: PromptProfile,
    ) -> Self {
        Self::with_profile_and_skills(provider, think, markdown_enabled, profile, Vec::new())
    }

    /// Full constructor. The `skills` list is advertised in the system
    /// prompt and bound to the `skill` tool so the model can pull any
    /// one of them into the conversation on demand.
    pub fn with_skills(
        provider: Box<dyn Provider>,
        think: bool,
        markdown_enabled: bool,
        skills: Vec<Skill>,
    ) -> Self {
        Self::with_profile_and_skills(
            provider,
            think,
            markdown_enabled,
            PromptProfile::Default,
            skills,
        )
    }

    /// Full constructor with an explicit prompt profile.
    pub fn with_profile_and_skills(
        provider: Box<dyn Provider>,
        think: bool,
        markdown_enabled: bool,
        profile: PromptProfile,
        skills: Vec<Skill>,
    ) -> Self {
        Self::with_profile_and_skills_and_max_tokens(
            provider,
            think,
            markdown_enabled,
            profile,
            skills,
            16384, // default max_tokens
        )
    }

    /// Full constructor with all options including max_tokens.
    pub fn with_profile_and_skills_and_max_tokens(
        provider: Box<dyn Provider>,
        think: bool,
        markdown_enabled: bool,
        profile: PromptProfile,
        skills: Vec<Skill>,
        max_tokens: u32,
    ) -> Self {
        Self::with_profile_and_skills_and_max_tokens_and_overview(
            provider,
            think,
            markdown_enabled,
            profile,
            skills,
            max_tokens,
            None,
        )
    }

    /// Full constructor with all options including project overview.
    pub fn with_profile_and_skills_and_max_tokens_and_overview(
        provider: Box<dyn Provider>,
        think: bool,
        markdown_enabled: bool,
        profile: PromptProfile,
        skills: Vec<Skill>,
        max_tokens: u32,
        project_overview: Option<&str>,
    ) -> Self {
        let skills_arc = Arc::new(skills);
        let system_prompt = build_system_prompt(profile, &skills_arc, project_overview);
        Self {
            provider,
            compress_provider: None,
            plan_provider: None,
            model_config: MultiModelConfig::default(),
            tools: tools::all_tools_with_skills(skills_arc.clone()),
            server_tools: Vec::new(),
            think,
            max_tokens,
            messages: Vec::new(),
            markdown_enabled: markdown::should_render(markdown_enabled),
            skin: markdown::default_skin(),
            system_prompt,
            project_overview: project_overview.map(|s| s.to_string()),
            profile,
            skills: skills_arc,
            total_output_tokens: 0,
            last_input_tokens: 0,
            compression_config: CompressionConfig::default(),
            enable_caching: true,
            last_compression_result: None,
            last_plan: None,
        }
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
        self.system_prompt = build_system_prompt(self.profile, &self.skills, Some(overview));
    }

    /// Clear the project overview and rebuild system prompt.
    pub fn clear_project_overview(&mut self) {
        self.project_overview = None;
        self.system_prompt = build_system_prompt(self.profile, &self.skills, None);
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

            let response = self.stream_one_turn(request).await?;

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

    /// Check if context compression is needed and perform it.
    /// Returns compression result if compression was performed.
    fn check_and_compress(&mut self) {
        use crate::compress::{CompressionResult, compress_messages};
        
        // Clear previous compression result
        self.last_compression_result = None;
        
        let context_size = self.provider.context_size();
        let current_tokens = self.last_input_tokens;
        
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

    /// Drive one streaming turn: show spinner while waiting, then print
    /// thinking deltas (dim) and text deltas (normal) as they arrive.
    /// Returns the assembled final response.
    async fn stream_one_turn(&self, request: ChatRequest) -> Result<ChatResponse> {
        let spinner = make_spinner("thinking");
        let mut rx = self.provider.chat_stream(request).await?;

        let mut in_thinking = false;
        let mut in_text = false;
        // Raw markdown accumulated for the current text block. Re-rendered
        // over the printed plaintext when the block closes.
        let mut text_buffer = String::new();
        let mut tool_spinner: Option<(ProgressBar, String)> = None;
        let mut last_shown_bytes: usize = 0;
        let mut final_response: Option<ChatResponse> = None;

        while let Some(event) = rx.recv().await {
            match event {
                StreamEvent::FirstByte => {
                    spinner.finish_and_clear();
                }
                StreamEvent::ThinkingDelta(t) => {
                    if in_text {
                        // thinking can resume between text blocks; add a gap
                        self.flush_text_block(&mut text_buffer);
                        in_text = false;
                    }
                    if !in_thinking {
                        print!("{}[thinking] ", DIM);
                        in_thinking = true;
                    }
                    print!("{}", t);
                    let _ = stdout().flush();
                }
                StreamEvent::TextDelta(t) => {
                    if in_thinking {
                        print!("{}\n\n", RESET);
                        in_thinking = false;
                    }
                    in_text = true;
                    text_buffer.push_str(&t);
                    print!("{}", t);
                    let _ = stdout().flush();
                }
                StreamEvent::ToolUseStart { name, .. } => {
                    if in_thinking {
                        print!("{}\n\n", RESET);
                        in_thinking = false;
                    }
                    if in_text {
                        self.flush_text_block(&mut text_buffer);
                        in_text = false;
                    }
                    if let Some((sp, _)) = tool_spinner.take() {
                        sp.finish_and_clear();
                    }
                    println!("[tool: {}]", name);
                    let sp = make_spinner(&format!("streaming {} input (0 B)", name));
                    tool_spinner = Some((sp, name));
                    last_shown_bytes = 0;
                }
                StreamEvent::ToolInputDelta { bytes_so_far } => {
                    // Throttle: only refresh the spinner label when the size
                    // has grown by at least ~1 KB, to avoid noisy redraws
                    // when the model streams many small partial_json chunks.
                    const REFRESH_STEP: usize = 1024;
                    if bytes_so_far >= last_shown_bytes + REFRESH_STEP {
                        if let Some((sp, name)) = tool_spinner.as_ref() {
                            sp.set_message(format!(
                                "streaming {} input ({})",
                                name,
                                format_bytes(bytes_so_far)
                            ));
                            last_shown_bytes = bytes_so_far;
                        }
                    }
                }
                StreamEvent::Done(resp) => {
                    if let Some((sp, _)) = tool_spinner.take() {
                        sp.finish_and_clear();
                    }
                    if in_thinking {
                        print!("{}", RESET);
                    }
                    if in_text {
                        self.flush_text_block(&mut text_buffer);
                    } else {
                        println!();
                    }
                    final_response = Some(resp);
                    break;
                }
                StreamEvent::Error(e) => {
                    if let Some((sp, _)) = tool_spinner.take() {
                        sp.finish_and_clear();
                    }
                    if in_thinking {
                        print!("{}", RESET);
                    }
                    spinner.finish_and_clear();
                    anyhow::bail!("stream error: {}", e);
                }
            }
        }

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

    async fn execute_tool_calls(&self, content: &[ContentBlock]) -> Vec<ContentBlock> {
        let mut results = Vec::new();

        for block in content {
            match block {
                ContentBlock::ToolUse { id, name, input } => {
                    println!(
                        "[tool-input: {}] {}",
                        name,
                        serde_json::to_string_pretty(input).unwrap_or_default()
                    );
                    let spinner = make_spinner(&format!("running {}", name));

                    let result = self.execute_single_tool(name, input).await;
                    spinner.finish_and_clear();

                    let output = match result {
                        Ok(output) => {
                            println!("[result: {}] {}", name, truncate(&output, 500));
                            output
                        }
                        Err(e) => {
                            let err_msg = format!("Error: {}", e);
                            println!("[error: {}] {}", name, err_msg);
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
                    println!(
                        "[server-tool: {}] {}",
                        name,
                        serde_json::to_string_pretty(input).unwrap_or_default()
                    );
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
                            println!("    {}", truncate(snippet, 200));
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
            format_tokens(usage.input_tokens as u64),
            format_tokens(usage.output_tokens as u64),
            format_tokens(self.total_output_tokens),
        ));
        if usage.cache_read_input_tokens > 0 || usage.cache_creation_input_tokens > 0 {
            parts.push(format!(
                "cache r/w {}/{}",
                format_tokens(usage.cache_read_input_tokens as u64),
                format_tokens(usage.cache_creation_input_tokens as u64),
            ));
        }
        if let Some(ctx) = self.provider.context_size() {
            let used = usage.input_tokens;
            let pct = (used as f64 / ctx as f64 * 100.0).min(100.0);
            parts.push(format!(
                "ctx {} / {} ({:.1}%) {}",
                format_tokens(used as u64),
                format_tokens(ctx as u64),
                pct,
                bar(pct, 20),
            ));
        }

        println!("{}{}{}", DIM, parts.join(" | "), RESET);
    }
}

/// Render a 0–100 percentage into a 20-char unicode progress bar.
fn bar(pct: f64, width: usize) -> String {
    let filled = ((pct / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    let mut s = String::with_capacity(width + 2);
    s.push('[');
    for i in 0..width {
        s.push(if i < filled { '█' } else { '░' });
    }
    s.push(']');
    s
}

fn format_tokens(n: u64) -> String {
    if n < 1_000 {
        n.to_string()
    } else if n < 1_000_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{:.2}M", n as f64 / 1_000_000.0)
    }
}

fn make_spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner())
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(Duration::from_millis(80));
    pb.tick(); // force an immediate draw so fast responses still show the spinner
    pb
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

fn format_bytes(n: usize) -> String {
    const KB: usize = 1024;
    const MB: usize = 1024 * 1024;
    if n < KB {
        format!("{} B", n)
    } else if n < MB {
        format!("{:.1} KB", n as f64 / KB as f64)
    } else {
        format!("{:.2} MB", n as f64 / MB as f64)
    }
}

/// Build the system prompt with optional project overview section.
fn build_system_prompt(
    profile: PromptProfile,
    skills: &Arc<Vec<Skill>>,
    project_overview: Option<&str>,
) -> String {
    use crate::prompt::{PromptContext, SystemPromptBuilder, SECTION_PROJECT_CONTEXT};

    let mut prompt_context = PromptContext::new()
        .with_available_skills(skills::format_catalogue(skills).unwrap_or_default());

    if let Some(overview) = project_overview {
        prompt_context = prompt_context.with_section(SECTION_PROJECT_CONTEXT, overview);
    }

    SystemPromptBuilder::new(profile)
        .with_context(prompt_context)
        .build()
}

#[cfg(test)]
mod tests {
    use super::truncate;

    #[test]
    fn truncate_ascii_under_max() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_ascii_over_max() {
        assert_eq!(truncate("hello world", 5), "hello");
    }

    #[test]
    fn truncate_multibyte_mid_char_does_not_panic() {
        let s = "中文".repeat(200);
        let t = truncate(&s, 500);
        assert!(t.len() <= 500);
        assert!(s.starts_with(t));
    }

    #[test]
    fn truncate_zero_max() {
        assert_eq!(truncate("中", 0), "");
    }
}
