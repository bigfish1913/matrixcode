use std::io::{Write as _, stdout};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};

use crate::markdown;
use crate::providers::{
    ChatRequest, ChatResponse, ContentBlock, Message, MessageContent, Provider, Role, StopReason,
    StreamEvent,
};
use crate::skills::{self, Skill};
use crate::tools::{self, Tool};
use termimad::MadSkin;

const BASE_SYSTEM_PROMPT: &str = r#"You are a helpful code agent with tool use.

Available tools:
- read / write / edit / multi_edit: file I/O. Prefer edit or multi_edit over write for changes to existing files.
- ls: list a directory (non-recursive).
- glob: find files by name pattern (e.g. **/*.rs).
- search: grep for a regex pattern inside files.
- bash: run shell commands (builds, tests, git, package managers).
- todo_write: maintain a structured todo list for multi-step tasks (3+ steps). Update status as you progress.
- webfetch: fetch a URL.
- skill: load the full instructions for one of the skills listed below. Prefer this over guessing when a skill name matches the task.

When using tools, think step by step:
1. Understand what the user wants
2. Decide which tool(s) to use
3. Execute the tool(s) and observe results
4. Continue until the task is complete

Always explain what you're doing before using a tool."#;

const MAX_ITERATIONS: usize = 200;

// ANSI dim italic for thinking, reset at end. Kept minimal to avoid pulling in a color crate.
const DIM: &str = "\x1b[2;3m";
const RESET: &str = "\x1b[0m";

pub struct Agent {
    provider: Box<dyn Provider>,
    tools: Vec<Box<dyn Tool>>,
    think: bool,
    messages: Vec<Message>,
    /// Whether to re-render assistant text as markdown when a text block ends.
    markdown_enabled: bool,
    /// Cached skin; cheap to build but we only need one per agent.
    skin: MadSkin,
    /// Final system prompt with any skills catalogue already appended.
    system_prompt: String,
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
        Self::with_skills(provider, think, markdown_enabled, Vec::new())
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
        let mut system_prompt = String::from(BASE_SYSTEM_PROMPT);
        if let Some(cat) = skills::format_catalogue(&skills) {
            system_prompt.push_str(&cat);
        }
        let skills_arc = Arc::new(skills);
        Self {
            provider,
            tools: tools::all_tools_with_skills(skills_arc),
            think,
            messages: Vec::new(),
            markdown_enabled: markdown::should_render(markdown_enabled),
            skin: markdown::default_skin(),
            system_prompt,
        }
    }

    /// Borrow the accumulated conversation for persistence.
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Replace the accumulated conversation, e.g. when resuming a session.
    pub fn set_messages(&mut self, messages: Vec<Message>) {
        self.messages = messages;
    }

    /// Run a single user turn, re-using accumulated conversation history.
    /// The agent keeps looping through tool_use turns internally until it
    /// produces a non-tool-use response, then returns control to the caller.
    pub async fn chat_once(&mut self, user_input: &str) -> Result<()> {
        self.messages.push(Message {
            role: Role::User,
            content: MessageContent::Text(user_input.to_string()),
        });

        let tool_defs: Vec<_> = self.tools.iter().map(|t| t.definition()).collect();

        for iteration in 0..MAX_ITERATIONS {
            let request = ChatRequest {
                messages: self.messages.clone(),
                tools: tool_defs.clone(),
                system: Some(self.system_prompt.clone()),
                think: self.think,
            };

            let response = self.stream_one_turn(request).await?;

            self.messages.push(Message {
                role: Role::Assistant,
                content: MessageContent::Blocks(response.content.clone()),
            });

            if response.stop_reason != StopReason::ToolUse {
                return Ok(());
            }

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
        }

        Ok(())
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
            if let ContentBlock::ToolUse { id, name, input } = block {
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
