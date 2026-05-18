use std::io::Stdout;
use std::time::{Duration, Instant};

use anyhow::Result;
use ratatui::{
    backend::CrosstermBackend,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind},
    Terminal,
};

use matrixcode_core::{AgentEvent, EventData, EventType, cancel::CancellationToken};

use crate::types::{Activity, ApproveMode, Role, Message};
use crate::utils::{truncate, extract_tool_detail, fmt_tokens};
use crate::ANIM_MS;

pub struct TuiApp {
    pub(crate) activity: Activity,
    pub(crate) activity_detail: String,
    pub(crate) messages: Vec<Message>,
    pub(crate) thinking: String,
    pub(crate) streaming: String,
    pub(crate) input: String,
    pub(crate) model: String,
    // Token stats
    pub(crate) tokens_in: u64,
    pub(crate) tokens_out: u64,
    pub(crate) session_total_out: u64,
    pub(crate) cache_read: u64,
    pub(crate) cache_created: u64,
    pub(crate) context_size: u64,
    // Debug stats
    pub(crate) api_calls: u64,
    pub(crate) compressions: u64,
    pub(crate) memory_saves: u64,
    pub(crate) tool_calls: u64,
    // UI state
    pub(crate) frame: usize,
    pub(crate) last_anim: Instant,
    pub(crate) show_welcome: bool,
    pub(crate) exit: bool,
    // Scroll state
    pub(crate) scroll_offset: u16,
    pub(crate) auto_scroll: bool,
    pub(crate) max_scroll: std::cell::Cell<u16>,  // Interior mutability for draw to update
    // Thinking display state
    pub(crate) thinking_collapsed: bool,
    // Approval mode
    pub(crate) approve_mode: ApproveMode,
    // Ask tool channel
    pub(crate) ask_tx: Option<tokio::sync::mpsc::Sender<String>>,
    pub(crate) waiting_for_ask: bool,
    // Channels
    pub(crate) tx: tokio::sync::mpsc::Sender<String>,
    pub(crate) rx: tokio::sync::mpsc::Receiver<AgentEvent>,
    pub(crate) cancel: CancellationToken,
    // Message queue for pending inputs while AI is processing
    pub(crate) pending_messages: Vec<String>,
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
            api_calls: 0,
            compressions: 0,
            memory_saves: 0,
            tool_calls: 0,
            frame: 0,
            last_anim: Instant::now(),
            show_welcome: true,
            exit: false,
            scroll_offset: 0,
            auto_scroll: true,
            max_scroll: std::cell::Cell::new(0),
            thinking_collapsed: true,
            approve_mode: ApproveMode::Ask,
            ask_tx: None,
            waiting_for_ask: false,
            tx, rx, cancel,
            pending_messages: Vec::new(),
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
        // Use provided context_size, env var, or estimate from model name
        self.context_size = context_size.unwrap_or_else(|| {
            // Check env var first
            if let Ok(raw) = std::env::var("CONTEXT_SIZE")
                && let Ok(n) = raw.trim().parse::<u64>()
                    && n > 0 { return n; }
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
            KeyCode::Enter => {
                // Shift+Enter: add newline instead of sending
                if k.modifiers.contains(KeyModifiers::SHIFT) {
                    self.input.push('\n');
                } else if !self.input.trim().is_empty() {
                    self.show_welcome = false;
                    let input = self.input.trim().to_string();
                    self.input.clear();
                    
                    if self.activity == Activity::Idle {
                        // Can send immediately
                        if self.waiting_for_ask {
                            self.waiting_for_ask = false;
                            self.messages.push(Message { role: Role::User, content: input.clone() });
                            if let Some(ask_tx) = &self.ask_tx {
                                ask_tx.try_send(input).ok();
                            }
                            self.activity = Activity::Thinking;
                            self.auto_scroll = true;
                        } else if input.starts_with('/') {
                            self.handle_command(&input);
                        } else {
                            self.auto_scroll = true;
                            self.scroll_offset = 0;
                            self.messages.push(Message { role: Role::User, content: input.clone() });
                            self.tx.try_send(input).ok();
                            self.activity = Activity::Thinking;
                        }
                    } else if self.activity == Activity::Asking {
                        // Respond to approval question
                        self.waiting_for_ask = false;
                        self.messages.push(Message { role: Role::User, content: input.clone() });
                        if let Some(ask_tx) = &self.ask_tx {
                            ask_tx.try_send(input).ok();
                        }
                        self.activity = Activity::Thinking;
                        self.auto_scroll = true;
                    } else {
                        // AI is processing - queue the message
                        self.pending_messages.push(input);
                        self.messages.push(Message { 
                            role: Role::System, 
                            content: format!("⏳ Queued ({} pending)", self.pending_messages.len())
                        });
                    }
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
            KeyCode::Char(c) => self.input.push(c),  // Always allow input
            // Scroll controls (任何时候都可用)
            KeyCode::PageUp => {
                // PageUp = 查看更早的内容
                if self.auto_scroll {
                    self.scroll_offset = self.max_scroll.get();
                    self.auto_scroll = false;
                }
                self.scroll_offset = self.scroll_offset.saturating_sub(10);
            }
            KeyCode::PageDown => {
                // PageDown = 查看更新的内容
                if self.auto_scroll {
                    return; // 已经在底部
                }
                self.scroll_offset = self.scroll_offset.saturating_add(10);
                let max = self.max_scroll.get();
                if self.scroll_offset >= max {
                    self.scroll_offset = 0;
                    self.auto_scroll = true;
                }
            }
            KeyCode::Up if k.modifiers.contains(KeyModifiers::ALT) || self.activity != Activity::Idle => {
                // Alt+Up: 查看更早的内容
                if self.auto_scroll {
                    self.scroll_offset = self.max_scroll.get();
                    self.auto_scroll = false;
                }
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }
            KeyCode::Down if k.modifiers.contains(KeyModifiers::ALT) || self.activity != Activity::Idle => {
                // Alt+Down: 查看更新的内容
                if self.auto_scroll {
                    return; // 已经在底部
                }
                self.scroll_offset = self.scroll_offset.saturating_add(1);
                let max = self.max_scroll.get();
                if self.scroll_offset >= max {
                    self.scroll_offset = 0;
                    self.auto_scroll = true;
                }
            }
            KeyCode::Home => {
                // 滚动到顶部
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
                // 向上滚动 = 查看更早的历史
                // 先同步 offset 到当前位置（如果之前是 auto_scroll）
                if self.auto_scroll {
                    self.scroll_offset = self.max_scroll.get();
                    self.auto_scroll = false;
                }
                // 减少 offset = 显示更上面的内容
                self.scroll_offset = self.scroll_offset.saturating_sub(3);
            }
            MouseEventKind::ScrollDown => {
                // 向下滚动 = 查看更新的内容
                if self.auto_scroll {
                    // 已经在底部，不需要处理
                    return;
                }
                // 增加 offset = 显示更下面的内容
                self.scroll_offset = self.scroll_offset.saturating_add(3);
                // 如果到达底部，恢复自动滚动
                let max = self.max_scroll.get();
                if self.scroll_offset >= max {
                    self.scroll_offset = 0;  // 重置，让 auto_scroll 控制位置
                    self.auto_scroll = true;
                }
            }
            _ => {}
        }
    }

    fn handle_command(&mut self, cmd: &str) {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        let command = parts.first().map_or("", |v| v);
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
                    let m = new_model.to_ascii_lowercase();
                    self.context_size = if m.contains("[1m]") || m.contains("opus-4-7") || m.contains("opus-4.7") {
                        1_000_000
                    } else if m.contains("claude-3") || m.contains("claude-4") || m.contains("claude-sonnet") || m.contains("claude-opus") {
                        200_000
                    } else if m.contains("kimi") {
                        128_000
                    } else if m.contains("deepseek") {
                        64_000
                    } else {
                        128_000
                    };
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
                        "  Shift+Enter       Insert newline (multi-line input)\n",
                        "  Shift+Tab         Toggle mode\n",
                        "  PgUp/PgDn         Scroll history\n",
                        "  Home/End          Top/Bottom\n",
                        "  Esc               Clear input / Interrupt\n",
                        "  Ctrl+C            Interrupt request\n",
                        "  Ctrl+D            Exit\n",
                        "\n📝 Features:\n",
                        "  • Multi-line input: Shift+Enter to add newline\n",
                        "  • Message queue: Send while AI is processing\n",
                        "  • Text selection: Use terminal's native selection",
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
                    self.tool_calls += 1;
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
                
                // Check for pending messages in queue
                if !self.pending_messages.is_empty() {
                    let next_msg = self.pending_messages.remove(0);
                    self.messages.push(Message { role: Role::User, content: next_msg.clone() });
                    self.tx.try_send(next_msg).ok();
                    self.activity = Activity::Thinking;
                    self.auto_scroll = true;
                    if self.pending_messages.is_empty() {
                        self.messages.push(Message { role: Role::System, content: "✓ Queue cleared".into() });
                    } else {
                        self.messages.push(Message { 
                            role: Role::System, 
                            content: format!("⏳ Processing queued message ({} remaining)", self.pending_messages.len())
                        });
                    }
                } else {
                    self.activity = Activity::Idle;
                }
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
                    // input_tokens 反映当前上下文大小
                    // 压缩后会变小，所以直接更新（不再取最大值）
                    self.tokens_in = input_tokens;
                    self.tokens_out = output_tokens;
                    self.session_total_out += output_tokens;
                    // 累积 cache 数据
                    self.cache_read += cache_read_input_tokens.unwrap_or(0);
                    self.cache_created += cache_creation_input_tokens.unwrap_or(0);
                    // API call count
                    self.api_calls += 1;
                }
            }
            EventType::CompressionCompleted => {
                if let Some(EventData::Compression { original_tokens, compressed_tokens, ratio }) = e.data {
                    self.compressions += 1;
                    // Log compression info to messages (brief)
                    self.messages.push(Message { 
                        role: Role::System, 
                        content: format!("📦 Context compressed: {} → {} tokens ({:.0}% saved)", 
                            fmt_tokens(original_tokens), 
                            fmt_tokens(compressed_tokens), 
                            (1.0 - ratio) * 100.0)
                    });
                }
            }
            EventType::MemoryLoaded => {
                if let Some(EventData::Memory { summary: _, entries_count }) = e.data {
                    self.memory_saves += 1;
                    if entries_count > 0 {
                        self.messages.push(Message {
                            role: Role::System,
                            content: format!("🧠 Memory loaded: {} entries", entries_count)
                        });
                    }
                }
            }
            EventType::MemoryDetected => {
                if let Some(EventData::Memory { summary, entries_count }) = e.data {
                    self.memory_saves += 1;
                    self.messages.push(Message {
                        role: Role::System,
                        content: format!("🧠 Memory detected: {} entries ({})", entries_count, summary)
                    });
                    self.auto_scroll = true;
                }
            }
            EventType::SessionStarted => self.activity = Activity::Thinking,
            EventType::AskQuestion => {
                if let Some(EventData::AskQuestion { question, options }) = e.data {
                    // Check if this is an approval request (contains "requires approval")
                    let is_approval = question.contains("requires approval") || question.contains("Allow?");
                    
                    // Display the question with appropriate styling
                    let mut content = if is_approval {
                        format!("⚠️ APPROVAL REQUIRED\n\n{}", question)
                    } else {
                        format!("❓ {}", question)
                    };
                    
                    if let Some(opts) = options
                        && let Some(arr) = opts.as_array() {
                            content.push_str("\n\nOptions:");
                            for opt in arr {
                                let id = opt["id"].as_str().unwrap_or("?");
                                let label = opt["label"].as_str().unwrap_or("");
                                content.push_str(&format!("\n  {}) {}", id, label));
                            }
                    }
                    
                    // Use a distinctive marker for approval requests
                    self.messages.push(Message { 
                        role: if is_approval { Role::System } else { Role::System }, 
                        content 
                    });
                    
                    // Switch to asking state
                    self.waiting_for_ask = true;
                    self.activity = Activity::Asking;
                    self.auto_scroll = true;
                }
            }
            _ => {}
        }
    }

}
