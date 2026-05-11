use std::io::{Write as _, stdout};
use std::time::Duration;

use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};

use crate::providers::{
    ChatRequest, ChatResponse, ContentBlock, Message, MessageContent, Provider, Role, StopReason,
    StreamEvent,
};
use crate::tools::{self, Tool};

const SYSTEM_PROMPT: &str = r#"You are a helpful code agent. You can read, write, edit, and search files, as well as fetch web content.

When using tools, think step by step:
1. Understand what the user wants
2. Decide which tool(s) to use
3. Execute the tool(s) and observe results
4. Continue until the task is complete

Always explain what you're doing before using a tool."#;

const MAX_ITERATIONS: usize = 20;

// ANSI dim italic for thinking, reset at end. Kept minimal to avoid pulling in a color crate.
const DIM: &str = "\x1b[2;3m";
const RESET: &str = "\x1b[0m";

pub struct Agent {
    provider: Box<dyn Provider>,
    tools: Vec<Box<dyn Tool>>,
    think: bool,
    messages: Vec<Message>,
}

impl Agent {
    pub fn new(provider: Box<dyn Provider>) -> Self {
        Self::with_options(provider, true)
    }

    pub fn with_options(provider: Box<dyn Provider>, think: bool) -> Self {
        Self {
            provider,
            tools: tools::all_tools(),
            think,
            messages: Vec::new(),
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

        let tool_defs: Vec<_> = self.tools.iter().map(|t| t.definition()).collect();

        for iteration in 0..MAX_ITERATIONS {
            let request = ChatRequest {
                messages: self.messages.clone(),
                tools: tool_defs.clone(),
                system: Some(SYSTEM_PROMPT.to_string()),
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
        let mut final_response: Option<ChatResponse> = None;

        while let Some(event) = rx.recv().await {
            match event {
                StreamEvent::FirstByte => {
                    spinner.finish_and_clear();
                }
                StreamEvent::ThinkingDelta(t) => {
                    if in_text {
                        // thinking can resume between text blocks; add a gap
                        println!();
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
                    print!("{}", t);
                    let _ = stdout().flush();
                }
                StreamEvent::ToolUseStart { name, .. } => {
                    if in_thinking {
                        print!("{}\n\n", RESET);
                        in_thinking = false;
                    }
                    if in_text {
                        println!();
                        in_text = false;
                    }
                    println!("[tool: {}]", name);
                }
                StreamEvent::Done(resp) => {
                    if in_thinking {
                        print!("{}", RESET);
                    }
                    println!();
                    final_response = Some(resp);
                    break;
                }
                StreamEvent::Error(e) => {
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
