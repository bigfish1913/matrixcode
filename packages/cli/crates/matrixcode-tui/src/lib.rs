//! MatrixCode TUI - Clean Layout with Thinking in Messages

use anyhow::Result;
use ratatui::{
    backend::CrosstermBackend,
    crossterm::{
        event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind},
        terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
        execute, cursor::Show,
    },
    layout::{Constraint, Direction, Layout, Rect, Alignment},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Terminal,
};
use std::io::Stdout;
use std::time::{Duration, Instant};

pub use matrixcode_core::{AgentEvent, EventData, EventType, cancel::CancellationToken};

const ANIM_MS: u64 = 80;
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Activity state
#[derive(Debug, Clone, PartialEq, Default)]
enum Activity {
    #[default]
    Idle,
    Thinking,
    Reading,
    Writing,
    Editing,
    Searching,
    Running,
    WebSearch,
    WebFetch,
    Tool(String),
}

impl Activity {
    fn label(&self) -> String {
        match self {
            Activity::Idle => "Ready".into(),
            Activity::Thinking => "Thinking".into(),
            Activity::Reading => "📖 Reading".into(),
            Activity::Writing => "📝 Writing".into(),
            Activity::Editing => "✏️ Editing".into(),
            Activity::Searching => "🔍 Searching".into(),
            Activity::Running => "⚡ Running".into(),
            Activity::WebSearch => "🌐 WebSearch".into(),
            Activity::WebFetch => "🔗 Fetching".into(),
            Activity::Tool(name) => format!("🔧 {}", name),
        }
    }

    fn color(&self) -> Color {
        match self {
            Activity::Idle => Color::Green,
            Activity::Thinking => Color::Magenta,
            Activity::Reading | Activity::Searching => Color::Cyan,
            Activity::Writing | Activity::Editing => Color::Yellow,
            Activity::Running => Color::Red,
            Activity::WebSearch | Activity::WebFetch => Color::Blue,
            Activity::Tool(_) => Color::Cyan,
        }
    }

    fn from_tool(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "read" => Activity::Reading,
            "write" => Activity::Writing,
            "edit" | "multi_edit" => Activity::Editing,
            "search" | "glob" | "ls" => Activity::Searching,
            "bash" => Activity::Running,
            "websearch" => Activity::WebSearch,
            "webfetch" => Activity::WebFetch,
            other => Activity::Tool(other.to_string()),
        }
    }
}

/// Message role
#[derive(PartialEq)]
enum Role {
    User,
    Assistant,
    Thinking,  // Thinking 内容
    Tool { name: String, is_error: bool },
    System,
}

impl Role {
    fn icon(&self) -> &'static str {
        match self {
            Role::User => "👤",
            Role::Assistant => "🤖",
            Role::Thinking => "💭",
            Role::Tool { is_error, .. } => if *is_error { "❌" } else { "✅" },
            Role::System => "⚠️",
        }
    }

    fn label(&self) -> String {
        match self {
            Role::User => "You".into(),
            Role::Assistant => "Assistant".into(),
            Role::Thinking => "Thinking".into(),
            Role::Tool { name, .. } => name.clone(),
            Role::System => "System".into(),
        }
    }

    fn color(&self) -> Color {
        match self {
            Role::User => Color::Green,
            Role::Assistant => Color::Blue,
            Role::Thinking => Color::Magenta,
            Role::Tool { is_error, .. } => if *is_error { Color::Red } else { Color::Cyan },
            Role::System => Color::Yellow,
        }
    }
}

/// Message block
struct Message {
    role: Role,
    content: String,
}

/// TUI Application
/// Approval mode for tool execution
#[derive(Debug, Clone, PartialEq, Default)]
enum ApproveMode {
    #[default]
    Ask,     // Ask before dangerous operations
    Auto,    // Execute everything automatically
    Strict,  // Ask before every tool call
}

impl ApproveMode {
    fn label(&self) -> &'static str {
        match self {
            ApproveMode::Ask => "ask",
            ApproveMode::Auto => "auto",
            ApproveMode::Strict => "strict",
        }
    }
    
    fn color(&self) -> Color {
        match self {
            ApproveMode::Ask => Color::Yellow,
            ApproveMode::Auto => Color::Green,
            ApproveMode::Strict => Color::Red,
        }
    }
    
    fn next(&self) -> Self {
        match self {
            ApproveMode::Ask => ApproveMode::Auto,
            ApproveMode::Auto => ApproveMode::Strict,
            ApproveMode::Strict => ApproveMode::Ask,
        }
    }
}

pub struct TuiApp {
    activity: Activity,
    activity_detail: String,  // 工具执行明细 (文件名、搜索词等)
    messages: Vec<Message>,
    thinking: String,
    streaming: String,
    input: String,
    model: String,
    // Token stats
    tokens_in: u64,
    tokens_out: u64,
    session_total_out: u64,
    cache_read: u64,
    cache_created: u64,
    context_size: u64,
    // UI state
    frame: usize,
    last_anim: Instant,
    show_welcome: bool,
    exit: bool,
    // Scroll state
    scroll_offset: u16,
    auto_scroll: bool,
    // Thinking display state
    thinking_collapsed: bool,
    // Approval mode
    approve_mode: ApproveMode,
    // Ask tool channel
    ask_tx: Option<tokio::sync::mpsc::Sender<String>>,
    waiting_for_ask: bool,
    // Channels
    tx: tokio::sync::mpsc::Sender<String>,
    rx: tokio::sync::mpsc::Receiver<AgentEvent>,
    cancel: CancellationToken,
}

impl TuiApp {
    pub fn new(
        tx: tokio::sync::mpsc::Sender<String>,
        rx: tokio::sync::mpsc::Receiver<AgentEvent>,
        cancel: CancellationToken,
    ) -> Self {
        Self {
            activity: Activity::Idle,
            activity_detail: String::new(),
            messages: Vec::new(),
            thinking: String::new(),
            streaming: String::new(),
            input: String::new(),
            model: "claude-sonnet-4".into(),
            tokens_in: 0,
            tokens_out: 0,
            session_total_out: 0,
            cache_read: 0,
            cache_created: 0,
            context_size: 200_000,
            frame: 0,
            last_anim: Instant::now(),
            show_welcome: true,
            exit: false,
            scroll_offset: 0,
            auto_scroll: true,
            thinking_collapsed: true,
            approve_mode: ApproveMode::Ask,
            ask_tx: None,
            waiting_for_ask: false,
            tx, rx, cancel,
        }
    }

    /// Load restored messages from session (converts core Message to TUI Message)
    pub fn load_messages(&mut self, core_messages: Vec<matrixcode_core::Message>) {
        for msg in core_messages {
            // Convert MessageContent to String
            let content = match &msg.content {
                matrixcode_core::MessageContent::Text(t) => t.clone(),
                matrixcode_core::MessageContent::Blocks(blocks) => {
                    // Extract text from content blocks
                    blocks.iter()
                        .filter_map(|b| match b {
                            matrixcode_core::ContentBlock::Text { text } => Some(text.clone()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                }
            };
            
            if content.is_empty() { continue; }
            
            // Convert core Role to TUI Role
            let role = match msg.role {
                matrixcode_core::Role::User => Role::User,
                matrixcode_core::Role::Assistant => Role::Assistant,
                matrixcode_core::Role::System => Role::System,
                matrixcode_core::Role::Tool => Role::Tool { name: "tool".into(), is_error: false },
            };
            
            self.messages.push(Message { role, content });
        }
        
        // Hide welcome if we have messages
        if !self.messages.is_empty() {
            self.show_welcome = false;
        }
    }

    pub fn with_ask_channel(mut self, ask_tx: tokio::sync::mpsc::Sender<String>) -> Self {
        self.ask_tx = Some(ask_tx);
        self
    }

    pub fn with_config(mut self, model: &str, _think: bool, _max_tokens: u32, context_size: Option<u64>) -> Self {
        self.model = model.to_string();
        // Use provided context_size, or estimate from model name
        self.context_size = context_size.unwrap_or_else(|| {
            let m = model.to_ascii_lowercase();
            if m.contains("[1m]") || m.contains("opus-4-7") || m.contains("opus-4.7") {
                1_000_000
            } else if m.contains("claude-3") || m.contains("claude-4") || m.contains("claude-sonnet") || m.contains("claude-opus") {
                200_000
            } else if m.contains("kimi") {
                128_000
            } else if m.contains("deepseek") {
                64_000
            } else {
                128_000
            }
        });
        self
    }

    /// Restore messages from a previous session for display in TUI
    pub fn with_session_messages(mut self, messages: &[matrixcode_core::Message]) -> Self {
        use matrixcode_core::providers::{MessageContent, ContentBlock, Role as CoreRole};
        for msg in messages {
            let role = match msg.role {
                CoreRole::User => Role::User,
                CoreRole::Assistant => Role::Assistant,
                CoreRole::Tool => Role::Tool { name: "tool".into(), is_error: false },
                CoreRole::System => Role::System,
            };
            let content = match &msg.content {
                MessageContent::Text(t) => t.clone(),
                MessageContent::Blocks(blocks) => {
                    blocks.iter().filter_map(|b| match b {
                        ContentBlock::Text { text } => Some(text.as_str()),
                        _ => None,
                    }).collect::<Vec<_>>().join("\n")
                }
            };
            if !content.is_empty() {
                self.messages.push(Message { role, content });
            }
        }
        self.show_welcome = false;
        self
    }

    pub fn run(&mut self, term: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        loop {
            if self.last_anim.elapsed().as_millis() >= ANIM_MS as u128 {
                self.frame = (self.frame + 1) % 10;
                self.last_anim = Instant::now();
            }
            term.draw(|f| self.draw(f))?;
            if event::poll(Duration::from_millis(16))? {
                match event::read()? {
                    Event::Key(k) => self.on_key(k),
                    Event::Mouse(m) => self.on_mouse(m),
                    _ => {}
                }
            }
            while let Ok(e) = self.rx.try_recv() { self.on_event(e); }
            if self.exit { break; }
        }
        Ok(())
    }

    fn on_key(&mut self, k: KeyEvent) {
        if k.kind != KeyEventKind::Press { return; }
        match k.code {
            KeyCode::Enter if !self.input.trim().is_empty() && self.activity == Activity::Idle => {
                self.show_welcome = false;
                let input = self.input.trim().to_string();
                self.input.clear();
                
                // Check if waiting for ask tool response
                if self.waiting_for_ask {
                    self.waiting_for_ask = false;
                    self.messages.push(Message { role: Role::User, content: input.clone() });
                    // Send answer through ask channel
                    if let Some(ask_tx) = &self.ask_tx {
                        ask_tx.try_send(input).ok();
                    }
                    self.activity = Activity::Thinking;
                    self.auto_scroll = true;
                } else if input.starts_with('/') {
                    // Command
                    self.handle_command(&input);
                } else {
                    // Normal message
                    self.auto_scroll = true;
                    self.scroll_offset = 0;
                    self.messages.push(Message { role: Role::User, content: input.clone() });
                    self.tx.try_send(input).ok();
                    self.activity = Activity::Thinking;
                }
            }
            KeyCode::Esc => {
                if self.activity != Activity::Idle {
                    self.cancel.cancel();
                    self.cancel.reset();
                    self.activity = Activity::Idle;
                    self.messages.push(Message { role: Role::System, content: "Interrupted".into() });
                } else {
                    self.input.clear();
                }
            }
            KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) && self.activity != Activity::Idle => {
                self.cancel.cancel();
                self.cancel.reset();
                self.activity = Activity::Idle;
                self.messages.push(Message { role: Role::System, content: "Interrupted".into() });
            }
            KeyCode::Char('d') if k.modifiers.contains(KeyModifiers::CONTROL) => self.exit = true,
            KeyCode::Backspace => { self.input.pop(); }
            KeyCode::Char(c) if self.activity == Activity::Idle => self.input.push(c),
            // Scroll controls (任何时候都可用)
            KeyCode::PageUp => {
                // 向上滚动（查看历史）- 减小 offset（跳过更少行）
                self.auto_scroll = false;
                self.scroll_offset = self.scroll_offset.saturating_sub(10);
            }
            KeyCode::PageDown => {
                // 向下滚动（查看新消息）- 增加 offset（跳过更多行）
                self.scroll_offset = self.scroll_offset.saturating_add(10);
            }
            KeyCode::Up if k.modifiers.contains(KeyModifiers::ALT) || self.activity != Activity::Idle => {
                // Alt+Up: 向上滚动一行
                self.auto_scroll = false;
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }
            KeyCode::Down if k.modifiers.contains(KeyModifiers::ALT) || self.activity != Activity::Idle => {
                // Alt+Down: 向下滚动一行
                self.scroll_offset = self.scroll_offset.saturating_add(1);
            }
            KeyCode::Home => {
                // 滚动到顶部（查看最早的历史）
                self.auto_scroll = false;
                self.scroll_offset = 0;
            }
            KeyCode::End => {
                // 滚动到底部（自动滚动模式）
                self.auto_scroll = true;
                self.scroll_offset = 0;
            }
            // Toggle approve mode with Shift+Tab or Alt+M
            KeyCode::Tab if k.modifiers.contains(KeyModifiers::SHIFT) => {
                self.approve_mode = self.approve_mode.next();
                self.tx.try_send(format!("/mode:{}", self.approve_mode.label())).ok();
            }
            // BackTab 是 Shift+Tab 在某些终端中的表示
            KeyCode::BackTab => {
                self.approve_mode = self.approve_mode.next();
                self.tx.try_send(format!("/mode:{}", self.approve_mode.label())).ok();
            }
            KeyCode::Char('m') if k.modifiers.contains(KeyModifiers::ALT) => {
                self.approve_mode = self.approve_mode.next();
                self.tx.try_send(format!("/mode:{}", self.approve_mode.label())).ok();
            }
            // Toggle thinking collapse/expand with Alt+T
            KeyCode::Char('t') if k.modifiers.contains(KeyModifiers::ALT) => {
                self.thinking_collapsed = !self.thinking_collapsed;
            }
            _ => {}
        }
    }

    fn on_mouse(&mut self, m: MouseEvent) {
        match m.kind {
            MouseEventKind::ScrollUp => {
                // 向上滚动（查看历史）
                self.auto_scroll = false;
                self.scroll_offset = self.scroll_offset.saturating_add(3);
            }
            MouseEventKind::ScrollDown => {
                // 向下滚动（查看最新）
                self.scroll_offset = self.scroll_offset.saturating_sub(3);
                if self.scroll_offset == 0 {
                    self.auto_scroll = true;
                }
            }
            _ => {}
        }
    }

    fn handle_command(&mut self, cmd: &str) {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        let command = parts.get(0).map_or("", |v| v);
        let args = &parts[1..];

        match command {
            "/exit" | "/quit" | "/q" => {
                self.exit = true;
            }
            "/clear" => {
                self.messages.clear();
                self.thinking.clear();
                self.streaming.clear();
                self.tokens_in = 0;
                self.tokens_out = 0;
                self.session_total_out = 0;
                self.cache_read = 0;
                self.cache_created = 0;
                self.messages.push(Message { 
                    role: Role::System, 
                    content: "✓ Messages cleared".into() 
                });
            }
            "/history" => {
                let user_count = self.messages.iter().filter(|m| m.role == Role::User).count();
                let assistant_count = self.messages.iter().filter(|m| m.role == Role::Assistant).count();
                let thinking_count = self.messages.iter().filter(|m| m.role == Role::Thinking).count();
                let tool_count = self.messages.iter().filter(|m| matches!(m.role, Role::Tool { .. })).count();
                
                self.messages.push(Message { 
                    role: Role::System, 
                    content: format!(
                        "📊 Session History:\n  User: {}  Assistant: {}  Thinking: {}  Tools: {}\n  Total: {}  Output tokens: {}",
                        user_count, assistant_count, thinking_count, tool_count,
                        self.messages.len(),
                        fmt_tokens(self.session_total_out)
                    )
                });
                self.auto_scroll = true;
            }
            "/mode" => {
                if args.is_empty() {
                    self.messages.push(Message { 
                        role: Role::System, 
                        content: format!("Current mode: {} (use /mode ask|auto|strict)", self.approve_mode.label())
                    });
                } else {
                    match args[0] {
                        "ask" => self.approve_mode = ApproveMode::Ask,
                        "auto" => self.approve_mode = ApproveMode::Auto,
                        "strict" => self.approve_mode = ApproveMode::Strict,
                        _ => {
                            self.messages.push(Message { 
                                role: Role::System, 
                                content: "Invalid mode. Use: /mode ask|auto|strict".into()
                            });
                            return;
                        }
                    }
                    // Sync mode to agent
                    self.tx.try_send(format!("/mode:{}", self.approve_mode.label())).ok();
                    self.messages.push(Message { 
                        role: Role::System, 
                        content: format!("✓ Mode set to: {}", self.approve_mode.label())
                    });
                }
                self.auto_scroll = true;
            }
            "/model" => {
                if args.is_empty() {
                    self.messages.push(Message { 
                        role: Role::System, 
                        content: format!("Current model: {}  Context: {}", self.model, fmt_tokens(self.context_size))
                    });
                } else {
                    let new_model = args.join(" ");
                    self.model = new_model.clone();
                    self.context_size = if new_model.contains("opus") { 200_000 } 
                                       else if new_model.contains("sonnet") { 200_000 }
                                       else { 128_000 };
                    self.messages.push(Message { 
                        role: Role::System, 
                        content: format!("✓ Model: {}  Context: {}", new_model, fmt_tokens(self.context_size))
                    });
                }
                self.auto_scroll = true;
            }
            "/compact" => {
                // 发送压缩请求给 agent
                self.messages.push(Message { 
                    role: Role::System, 
                    content: "⏳ Requesting context compression...".into()
                });
                self.tx.try_send("/compact".to_string()).ok();
                self.activity = Activity::Thinking;
                self.auto_scroll = true;
            }
            "/new" => {
                // 新会话：清空所有状态
                self.messages.clear();
                self.thinking.clear();
                self.streaming.clear();
                self.tokens_in = 0;
                self.tokens_out = 0;
                self.session_total_out = 0;
                self.cache_read = 0;
                self.cache_created = 0;
                self.messages.push(Message { 
                    role: Role::System, 
                    content: "✓ New session started".into()
                });
                // 通知 agent 重置
                self.tx.try_send("/new".to_string()).ok();
                self.auto_scroll = true;
            }
            "/help" => {
                self.messages.push(Message { 
                    role: Role::System, 
                    content: concat!(
                        "📖 Commands:\n",
                        "  /help             Show this help\n",
                        "  /exit /quit /q    Exit MatrixCode\n",
                        "  /clear            Clear messages\n",
                        "  /history          Session statistics\n",
                        "  /mode <mode>      Set mode (ask/auto/strict)\n",
                        "  /model [name]     Show or switch model\n",
                        "  /compact          Compress context\n",
                        "  /new              Start new session\n",
                        "\n⌨️ Shortcuts:\n",
                        "  Enter             Send message\n",
                        "  Shift+Tab         Toggle mode\n",
                        "  PgUp/PgDn         Scroll history\n",
                        "  Home/End          Top/Bottom\n",
                        "  Esc               Clear input / Interrupt\n",
                        "  Ctrl+C            Interrupt request\n",
                        "  Ctrl+D            Exit",
                    ).into()
                });
                self.auto_scroll = true;
            }
            _ => {
                self.messages.push(Message { 
                    role: Role::System, 
                    content: format!("Unknown: {}  Type /help for commands", command)
                });
                self.auto_scroll = true;
            }
        }
    }

    fn on_event(&mut self, e: AgentEvent) {
        match e.event_type {
            EventType::ThinkingStart => {
                self.activity = Activity::Thinking;
                self.activity_detail.clear();
                self.thinking.clear();
            }
            EventType::ThinkingDelta => {
                if let Some(EventData::Thinking { delta, .. }) = e.data {
                    self.thinking.push_str(&delta);
                }
            }
            EventType::ThinkingEnd => {
                // Thinking 结束，保存到 messages（显示在 assistant 之前）
                if !self.thinking.is_empty() {
                    self.messages.push(Message { 
                        role: Role::Thinking,
                        content: self.thinking.clone() 
                    });
                    self.thinking.clear();
                }
            }
            EventType::TextStart => self.streaming.clear(),
            EventType::TextDelta => {
                if let Some(EventData::Text { delta }) = e.data {
                    self.streaming.push_str(&delta);
                }
            }
            EventType::TextEnd => {
                if !self.streaming.is_empty() {
                    self.messages.push(Message { role: Role::Assistant, content: self.streaming.clone() });
                    self.streaming.clear();
                }
            }
            EventType::ToolUseStart => {
                if let Some(EventData::ToolUse { name, input, .. }) = e.data {
                    self.activity = Activity::from_tool(&name);
                    // 提取工具执行明细
                    self.activity_detail = extract_tool_detail(&name, input.as_ref());
                }
            }
            EventType::ToolResult => {
                if let Some(EventData::ToolResult { content, is_error, .. }) = e.data {
                    let tool_name = match &self.activity {
                        Activity::Tool(name) => name.clone(),
                        Activity::Reading => "read".into(),
                        Activity::Writing => "write".into(),
                        Activity::Editing => "edit".into(),
                        Activity::Searching => "search".into(),
                        Activity::Running => "bash".into(),
                        Activity::WebSearch => "websearch".into(),
                        Activity::WebFetch => "webfetch".into(),
                        _ => "tool".into(),
                    };
                    self.messages.push(Message { 
                        role: Role::Tool { name: tool_name, is_error }, 
                        content: truncate(&content, 100) 
                    });
                    self.activity = Activity::Thinking;
                    self.activity_detail.clear();
                }
            }
            EventType::SessionEnded => {
                // Save any remaining streaming text
                if !self.streaming.is_empty() {
                    self.messages.push(Message { role: Role::Assistant, content: self.streaming.clone() });
                    self.streaming.clear();
                }
                // Save any remaining thinking
                if !self.thinking.is_empty() {
                    self.messages.push(Message { role: Role::Thinking, content: self.thinking.clone() });
                    self.thinking.clear();
                }
                self.activity = Activity::Idle;
                self.activity_detail.clear();
            }
            EventType::Error => {
                if let Some(EventData::Error { message, .. }) = e.data {
                    self.messages.push(Message { role: Role::System, content: format!("Error: {}", message) });
                }
                self.activity = Activity::Idle;
            }
            EventType::Usage => {
                if let Some(EventData::Usage { 
                    input_tokens, 
                    output_tokens,
                    cache_creation_input_tokens,
                    cache_read_input_tokens,
                }) = e.data {
                    // input_tokens 是当前请求的输入，包含历史消息，反映当前上下文大小
                    // 取最大值以显示会话的实际上下文使用峰值
                    self.tokens_in = self.tokens_in.max(input_tokens);
                    self.tokens_out = output_tokens;
                    self.session_total_out += output_tokens;
                    // 累积 cache 数据
                    self.cache_read += cache_read_input_tokens.unwrap_or(0);
                    self.cache_created += cache_creation_input_tokens.unwrap_or(0);
                }
            }
            EventType::SessionStarted => self.activity = Activity::Thinking,
            EventType::AskQuestion => {
                if let Some(EventData::AskQuestion { question, options }) = e.data {
                    // Display the question as a message
                    let mut content = format!("❓ {}", question);
                    if let Some(opts) = options {
                        if let Some(arr) = opts.as_array() {
                            content.push_str("\n\nOptions:");
                            for opt in arr {
                                let id = opt["id"].as_str().unwrap_or("?");
                                let label = opt["label"].as_str().unwrap_or("");
                                content.push_str(&format!("\n  {}) {}", id, label));
                            }
                        }
                    }
                    self.messages.push(Message { role: Role::System, content });
                    // Switch to waiting for ask input
                    self.waiting_for_ask = true;
                    self.activity = Activity::Idle;  // Allow input
                    self.auto_scroll = true;
                }
            }
            _ => {}
        }
    }

    fn draw(&self, f: &mut ratatui::Frame) {
        // 简化布局：Status + Messages + Usage + Input
        let constraints = vec![
            Constraint::Length(1),           // Status
            Constraint::Min(3),              // Messages (弹性高度，最大化)
            Constraint::Length(1),           // Usage + Hints
            Constraint::Length(1),           // Input
        ];

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(f.area());

        self.draw_status(f, chunks[0]);
        self.draw_messages(f, chunks[1]);
        self.draw_usage(f, chunks[2]);
        self.draw_input(f, chunks[3]);
    }

    fn draw_status(&self, f: &mut ratatui::Frame, area: Rect) {
        // 状态栏：MatrixCode + Model + Activity(detail) + mode
        let mut spans = vec![
            Span::styled(" MatrixCode ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled("│", Style::default().fg(Color::DarkGray)),
            Span::styled(format!(" {} ", self.model), Style::default().fg(Color::White)),
            Span::styled("│", Style::default().fg(Color::DarkGray)),
        ];
        
        // Activity + spinner + detail
        if self.activity != Activity::Idle {
            spans.push(Span::styled(
                format!(" {}", SPINNER[self.frame]),
                Style::default().fg(self.activity.color())
            ));
            spans.push(Span::styled(
                format!(" {} ", self.activity.label()),
                Style::default().fg(self.activity.color())
            ));
            if !self.activity_detail.is_empty() {
                spans.push(Span::styled(
                    format!("({})", self.activity_detail),
                    Style::default().fg(Color::DarkGray)
                ));
            }
        } else {
            spans.push(Span::styled(" Ready ", Style::default().fg(Color::Green)));
        }
        
        spans.push(Span::styled(" │", Style::default().fg(Color::DarkGray)));
        spans.push(Span::styled(
            format!(" mode:{} ", self.approve_mode.label()),
            Style::default().fg(self.approve_mode.color())
        ));
        
        f.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    fn draw_usage(&self, f: &mut ratatui::Frame, area: Rect) {
        // Usage + Hints 合并显示
        if self.tokens_in == 0 && self.tokens_out == 0 {
            // 只显示 hints
            let hints = " /help │ PgUp/PgDn: scroll │ Home/End: top/bot │ Alt+T: thinking";
            f.render_widget(Paragraph::new(Line::styled(hints, Style::default().fg(Color::DarkGray))), area);
            return;
        }
        
        // Usage bar: in 1.4K / out 38 (session out: 55.7K) | cache r/w 101.6K/0 | ctx 1.4K / 128.0K (1.1%) [░░░░░░]
        let context_pct = if self.context_size > 0 {
            (self.tokens_in as f64 / self.context_size as f64 * 100.0).min(100.0)
        } else { 0.0 };
        
        let ctx_color = if context_pct < 50.0 { Color::Green }
                       else if context_pct < 75.0 { Color::Yellow }
                       else { Color::Red };
        
        let bar = progress_bar(context_pct, 10);
        
        let mut parts: Vec<Span> = vec![
            Span::styled(
                format!("in {} / out {} (session: {})", 
                    fmt_tokens(self.tokens_in), 
                    fmt_tokens(self.tokens_out),
                    fmt_tokens(self.session_total_out)
                ),
                Style::default().fg(Color::Gray)
            ),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        ];
        
        // Cache info (始终显示)
        parts.push(Span::styled(
            format!("cache r/w {}/{}", fmt_tokens(self.cache_read), fmt_tokens(self.cache_created)),
            Style::default().fg(Color::Cyan)
        ));
        parts.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
        
        // Context info
        parts.push(Span::styled(
            format!("ctx {} / {} ({:.1}%) {}", 
                fmt_tokens(self.tokens_in),
                fmt_tokens(self.context_size),
                context_pct,
                bar
            ),
            Style::default().fg(ctx_color)
        ));
        
        f.render_widget(Paragraph::new(Line::from(parts)), area);
    }

    #[allow(dead_code)]
    fn draw_welcome(&self, f: &mut ratatui::Frame, area: Rect) {
        // 根据 show_welcome 状态决定显示内容，但始终保留空间避免布局跳动
        if !self.show_welcome {
            // 当 welcome 隐藏时，渲染空��保持布局稳定
            let empty_lines = vec![
                Line::raw(""),
                Line::raw(""),
                Line::raw(""),
                Line::raw(""),
                Line::raw(""),
                Line::raw(""),
                Line::raw(""),
                Line::raw(""),
                Line::raw(""),
            ];
            f.render_widget(Paragraph::new(empty_lines), area);
            return;
        }
        
        let cyan = Style::default().fg(Color::Cyan);
        let gray = Style::default().fg(Color::DarkGray);
        let green = Style::default().fg(Color::Green);
        
        let lines = vec![
            Line::styled("", Style::default()),
            Line::styled("   __  __       _             _____          __  __    ", cyan),
            Line::styled("  |  |/  | __ _| |_ ___ _ __  |  _  |_ _ ___ |  |/  |   ", cyan),
            Line::styled("  | |/| |/ _` | __/ _ \\ '__| | |_| | '_/ _ \\| |/| |   ", cyan),
            Line::styled("  | |  | | (_| | ||  __/ |    |  _  | ||  __/| |  | |   ", cyan),
            Line::styled("  |_|  |_|\\__,_|\\__\\___|_|    |_| |_|_| \\___|_|  |_|   ", cyan),
            Line::styled("", Style::default()),
            Line::styled(format!("   Model: {}  │  Tokens: 16K max", self.model), gray),
            Line::styled("   Press Enter to start...", green),
        ];
        
        f.render_widget(Paragraph::new(lines).alignment(Alignment::Center), area);
    }

    fn draw_messages(&self, f: &mut ratatui::Frame, area: Rect) {
        let mut lines: Vec<Line> = Vec::new();
        let max_w = area.width.saturating_sub(5) as usize;

        // Welcome 内容（只在初始状态显示）
        if self.show_welcome && self.messages.is_empty() {
            lines.push(Line::styled(
                "╭─────────────────────────────────────────────────────────────╮",
                Style::default().fg(Color::Cyan)
            ));
            lines.push(Line::styled(
                "│                     🤖 MatrixCode                           │",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            ));
            lines.push(Line::styled(
                "│   AI-powered coding assistant with extended thinking       │",
                Style::default().fg(Color::DarkGray)
            ));
            lines.push(Line::raw("│                                                             │"));
            lines.push(Line::styled(
                "│   Commands: /help /clear /history /mode /new /exit         │",
                Style::default().fg(Color::Gray)
            ));
            lines.push(Line::styled(
                "│   Shortcuts: Enter=send │ PgUp/PgDn=scroll │ Alt+T=thinking │",
                Style::default().fg(Color::Gray)
            ));
            lines.push(Line::styled(
                "╰─────────────────────────────────────────────────────────────╯",
                Style::default().fg(Color::Cyan)
            ));
            lines.push(Line::raw(""));
        }

        // Render all messages (including thinking as part of messages)
        for msg in &self.messages {
            let icon = msg.role.icon();
            let label = msg.role.label();
            let color = msg.role.color();
            
            lines.push(Line::from(vec![
                Span::styled(icon, Style::default().fg(color)),
                Span::raw(" "),
                Span::styled(label, Style::default().fg(color).add_modifier(Modifier::BOLD)),
            ]));
            
            // Thinking 内容：可通过 Alt+T 折叠/展开
            if matches!(msg.role, Role::Thinking) {
                if self.thinking_collapsed {
                    for line in msg.content.lines().take(2) {
                        // 自动换行而不是截断
                        for wrapped in wrap_line(line, max_w) {
                            lines.push(Line::styled(
                                format!("  {}", wrapped),
                                Style::default().fg(Color::DarkGray)
                            ));
                        }
                    }
                    if msg.content.lines().count() > 2 {
                        lines.push(Line::styled(
                            format!("  ... ({} lines)", msg.content.lines().count()),
                            Style::default().fg(Color::DarkGray)
                        ));
                    }
                } else {
                    for line in msg.content.lines() {
                        for wrapped in wrap_line(line, max_w) {
                            lines.push(Line::styled(
                                format!("  {}", wrapped),
                                Style::default().fg(Color::DarkGray)
                            ));
                        }
                    }
                }
            } else {
                // 非 Thinking 消息
                if msg.role == Role::Assistant {
                    // Assistant 消息使用 markdown 渲染
                    let md_lines = render_markdown(&msg.content, max_w);
                    lines.extend(md_lines);
                } else {
                    // User/Tool/System 消息：纯文本
                    for line in msg.content.lines() {
                        lines.push(Line::styled(
                            format!("  {}", truncate(line, max_w)),
                            Style::default().fg(Color::White)
                        ));
                    }
                }
            }
            
            lines.push(Line::raw(""));
        }

        // Current thinking (streaming, not yet saved)
        if !self.thinking.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("💭 ", Style::default().fg(Color::Magenta)),
                Span::styled("Thinking", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            ]));
            
            if self.thinking_collapsed {
                for line in self.thinking.lines().take(1) {
                    for wrapped in wrap_line(line, max_w) {
                        lines.push(Line::styled(
                            format!("  {}", wrapped),
                            Style::default().fg(Color::DarkGray)
                        ));
                    }
                }
                if self.thinking.lines().count() > 1 {
                    lines.push(Line::styled(
                        format!("  ... ({} lines)", self.thinking.lines().count()),
                        Style::default().fg(Color::DarkGray)
                    ));
                }
            } else {
                for line in self.thinking.lines() {
                    for wrapped in wrap_line(line, max_w) {
                        lines.push(Line::styled(
                            format!("  {}", wrapped),
                            Style::default().fg(Color::DarkGray)
                        ));
                    }
                }
            }
            lines.push(Line::raw(""));
        }

        // Streaming text (after thinking) - markdown rendered
        if !self.streaming.is_empty() {
            // Assistant 标题 + 动画
            let spinner = if self.activity != Activity::Idle {
                format!(" {} ", SPINNER[self.frame])
            } else {
                " ".to_string()
            };
            lines.push(Line::from(vec![
                Span::styled("🤖", Style::default().fg(Color::Blue)),
                Span::raw(" "),
                Span::styled("Assistant", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
                Span::styled(spinner, Style::default().fg(self.activity.color())),
            ]));
            let md_lines = render_markdown(&self.streaming, max_w);
            lines.extend(md_lines);
            // Cursor
            lines.push(Line::styled("  ▌", Style::default().fg(Color::Cyan)));
        }
        
        // 如果正在活动但没有 streaming/thinking，显示状态
        if self.activity != Activity::Idle && self.streaming.is_empty() && self.thinking.is_empty() {
            lines.push(Line::from(vec![
                Span::styled(SPINNER[self.frame], Style::default().fg(self.activity.color())),
                Span::raw(" "),
                Span::styled(self.activity.label(), Style::default().fg(self.activity.color())),
            ]));
        }

        // 计算滚动偏移 - 支持自动滚动和手动滚动
        let total_lines = lines.len() as u16;
        let visible_height = area.height;
        let max_scroll = if total_lines > visible_height {
            total_lines.saturating_sub(visible_height)
        } else {
            0
        };
        
        // 根据滚动模式计算偏移
        let scroll_offset = if self.auto_scroll {
            // 自动滚动：保持在底部，显示最新内容
            max_scroll
        } else {
            // 手动滚动：从顶部往下的偏移
            self.scroll_offset.min(max_scroll)
        };

        f.render_widget(
            Paragraph::new(lines)
                .scroll((scroll_offset, 0)),
            area
        );
    }

    fn draw_input(&self, f: &mut ratatui::Frame, area: Rect) {
        if self.activity == Activity::Idle {
            let mut spans: Vec<Span> = vec![];
            
            // Input prompt
            spans.push(Span::styled("❯ ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
            
            if self.input.is_empty() {
                spans.push(Span::styled("_", Style::default().fg(Color::Cyan)));
            } else {
                spans.push(Span::styled(&self.input, Style::default().fg(Color::White)));
            }
            
            // Scroll indicator (if not auto-scrolling)
            if !self.auto_scroll {
                spans.push(Span::styled(" [viewing history]", Style::default().fg(Color::DarkGray)));
            }
            
            f.render_widget(Paragraph::new(Line::from(spans)), area);
        } else {
            f.render_widget(Paragraph::new(Line::raw("")), area);
        }
    }
}

/// Simple markdown renderer for ratatui
fn render_markdown<'a>(text: &'a str, max_w: usize) -> Vec<Line<'a>> {
    let mut lines: Vec<Line> = Vec::new();
    let mut in_code_block = false;
    
    for line in text.lines() {
        if line.starts_with("```") {
            in_code_block = !in_code_block;
            if in_code_block {
                // Code block start - show language hint
                let lang = line.trim_start_matches("```").trim();
                if !lang.is_empty() {
                    lines.push(Line::styled(
                        format!("  ┌─ {} ", lang),
                        Style::default().fg(Color::DarkGray)
                    ));
                } else {
                    lines.push(Line::styled(
                        "  ┌─────",
                        Style::default().fg(Color::DarkGray)
                    ));
                }
            } else {
                lines.push(Line::styled(
                    "  └─────",
                    Style::default().fg(Color::DarkGray)
                ));
            }
            continue;
        }
        
        if in_code_block {
            // Code block content - cyan on dark background
            lines.push(Line::styled(
                format!("  │ {}", truncate(line, max_w.saturating_sub(4))),
                Style::default().fg(Color::Cyan)
            ));
            continue;
        }
        
        // Headers
        if line.starts_with("### ") {
            lines.push(Line::styled(
                format!("  {}", &line[4..]),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            ));
            continue;
        }
        if line.starts_with("## ") {
            lines.push(Line::styled(
                format!("  {}", &line[3..]),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            ));
            continue;
        }
        if line.starts_with("# ") {
            lines.push(Line::styled(
                format!("  {}", &line[2..]),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            ));
            continue;
        }
        
        // Bullet lists
        if line.starts_with("- ") || line.starts_with("* ") {
            let content = &line[2..];
            lines.push(Line::from(vec![
                Span::styled("  • ", Style::default().fg(Color::Green)),
                Span::styled(truncate(content, max_w.saturating_sub(4)), Style::default().fg(Color::White)),
            ]));
            continue;
        }
        
        // Numbered lists
        if line.len() > 2 && line.chars().next().map_or(false, |c| c.is_ascii_digit()) 
            && (line.contains(". ") || line.contains(") ")) {
            lines.push(Line::styled(
                format!("  {}", truncate(line, max_w.saturating_sub(2))),
                Style::default().fg(Color::White)
            ));
            continue;
        }
        
        // Regular text with inline formatting
        let spans = parse_inline_markdown(line, max_w);
        lines.push(Line::from(spans));
    }
    
    lines
}

/// Parse inline markdown (bold, code, etc.) into spans
fn parse_inline_markdown<'a>(line: &'a str, max_w: usize) -> Vec<Span<'a>> {
    // Truncate safely at char boundary
    let line = if line.chars().count() > max_w {
        let end = line.char_indices().nth(max_w).map(|(i, _)| i).unwrap_or(line.len());
        &line[..end]
    } else {
        line
    };
    let mut spans: Vec<Span> = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars().peekable();
    
    spans.push(Span::raw("  ")); // indent
    
    while let Some(ch) = chars.next() {
        match ch {
            '`' => {
                // Inline code
                if !current.is_empty() {
                    spans.push(Span::styled(current.clone(), Style::default().fg(Color::White)));
                    current.clear();
                }
                let mut code = String::new();
                while let Some(&next) = chars.peek() {
                    if next == '`' { chars.next(); break; }
                    code.push(chars.next().unwrap());
                }
                spans.push(Span::styled(code, Style::default().fg(Color::Cyan)));
            }
            '*' if chars.peek() == Some(&'*') => {
                // Bold
                chars.next(); // consume second *
                if !current.is_empty() {
                    spans.push(Span::styled(current.clone(), Style::default().fg(Color::White)));
                    current.clear();
                }
                let mut bold = String::new();
                while let Some(next) = chars.next() {
                    if next == '*' && chars.peek() == Some(&'*') { chars.next(); break; }
                    bold.push(next);
                }
                spans.push(Span::styled(bold, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));
            }
            _ => current.push(ch),
        }
    }
    
    if !current.is_empty() {
        spans.push(Span::styled(current, Style::default().fg(Color::White)));
    }
    
    if spans.len() == 1 {
        // Only indent, add empty content
        spans.push(Span::raw(""));
    }
    
    spans
}

/// 从工具输入中提取明细信息
fn extract_tool_detail(tool_name: &str, input: Option<&serde_json::Value>) -> String {
    let Some(input) = input else { return String::new() };
    match tool_name.to_lowercase().as_str() {
        "read" => input.get("path").and_then(|v| v.as_str())
            .map(|s| truncate(s, 40)).unwrap_or_default(),
        "write" => input.get("path").and_then(|v| v.as_str())
            .map(|s| truncate(s, 40)).unwrap_or_default(),
        "edit" | "multi_edit" => input.get("path").and_then(|v| v.as_str())
            .map(|s| truncate(s, 40)).unwrap_or_default(),
        "search" => input.get("pattern").and_then(|v| v.as_str())
            .map(|s| truncate(s, 30)).unwrap_or_default(),
        "glob" => input.get("pattern").and_then(|v| v.as_str())
            .map(|s| truncate(s, 30)).unwrap_or_default(),
        "ls" => input.get("path").and_then(|v| v.as_str())
            .map(|s| truncate(s, 40)).unwrap_or_default(),
        "bash" => input.get("command").and_then(|v| v.as_str())
            .map(|s| truncate(s, 40)).unwrap_or_default(),
        "websearch" => input.get("query").and_then(|v| v.as_str())
            .map(|s| truncate(s, 30)).unwrap_or_default(),
        "webfetch" => input.get("url").and_then(|v| v.as_str())
            .map(|s| truncate(s, 40)).unwrap_or_default(),
        _ => String::new(),
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.into() }
    else { s.chars().take(n.saturating_sub(3)).collect::<String>() + "..." }
}

/// Wrap a long line into multiple lines at char boundary
fn wrap_line(s: &str, max_w: usize) -> Vec<String> {
    if max_w == 0 { return vec![s.to_string()]; }
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_w {
        return vec![s.to_string()];
    }
    let mut result = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let end = (start + max_w).min(chars.len());
        result.push(chars[start..end].iter().collect());
        start = end;
    }
    result
}

fn fmt_tokens(n: u64) -> String {
    if n < 1_000 { n.to_string() }
    else if n < 1_000_000 { format!("{:.1}K", n as f64 / 1_000.0) }
    else { format!("{:.1}M", n as f64 / 1_000_000.0) }
}

fn progress_bar(pct: f64, width: usize) -> String {
    let filled = ((pct / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    let mut s = String::with_capacity(width);
    for i in 0..width {
        s.push(if i < filled { '█' } else { '░' });
    }
    s
}

pub fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    execute!(std::io::stdout(), event::EnableMouseCapture)?;
    let mut t = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;
    t.clear()?;
    Ok(t)
}

pub fn restore_terminal() -> Result<()> {
    disable_raw_mode()?;
    execute!(std::io::stdout(), event::DisableMouseCapture, Clear(ClearType::All), Show)?;
    Ok(())
}