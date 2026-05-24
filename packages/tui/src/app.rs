use std::collections::HashMap;
use std::io::Stdout;
use std::time::{Duration, Instant};

use anyhow::Result;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    crossterm::event::{self, Event, MouseEvent, MouseEventKind},
};

use matrixcode_core::{AgentEvent, cancel::CancellationToken};

use crate::ANIM_MS;
use crate::types::{Activity, ApproveMode, AskQuestion, Message, Role, SubmitMode};

pub struct TuiApp {
    pub(crate) activity: Activity,
    pub(crate) activity_detail: String,
    /// Full tool input for display (not truncated)
    pub(crate) activity_input: Option<serde_json::Value>,
    pub(crate) messages: Vec<Message>,
    pub(crate) thinking: String,
    pub(crate) streaming: String,
    pub(crate) input: String,
    pub(crate) model: String,
    // Token stats
    pub(crate) tokens_in: u64,
    pub(crate) tokens_out: u64,
    pub(crate) session_total_out: u64,
    pub(crate) current_request_tokens: u64, // Tokens for current request (real-time)
    pub(crate) cache_read: u64,
    pub(crate) cache_created: u64,
    pub(crate) context_size: u64,
    // Debug stats
    pub(crate) api_calls: u64,
    pub(crate) compressions: u64,
    pub(crate) memory_saves: u64,
    pub(crate) tool_calls: u64,
    // Timing
    pub(crate) request_start: Option<Instant>,
    pub(crate) tool_start: Option<Instant>, // When current tool execution started
    // UI state
    pub(crate) frame: usize,
    pub(crate) last_anim: Instant,
    pub(crate) show_welcome: bool,
    pub(crate) exit: bool,
    // Input cursor position (character index in input string)
    pub(crate) cursor_pos: usize,
    // Input history (Up/Down arrow navigation)
    pub(crate) input_history: Vec<String>,
    pub(crate) history_index: Option<usize>, // None = not browsing history
    pub(crate) history_draft: String,        // Saves current input when entering history mode
    // Scroll state
    pub(crate) scroll_offset: u16,
    pub(crate) auto_scroll: bool,
    pub(crate) max_scroll: std::cell::Cell<u16>,
    pub(crate) new_message_while_scrolled: std::cell::Cell<bool>, // Flag for notification when scrolled up
    // Thinking display state
    pub(crate) thinking_collapsed: bool,
    // Dirty flag for rendering optimization - only redraw when something changed
    pub(crate) dirty: std::cell::Cell<bool>,
    // Approval mode
    pub(crate) approve_mode: ApproveMode,
    // Shared approve mode atomic - directly updates agent's mode in real-time
    pub(crate) shared_approve_mode: Option<std::sync::Arc<std::sync::atomic::AtomicU8>>,
    // Ask tool channel
    pub(crate) ask_tx: Option<tokio::sync::mpsc::Sender<String>>,
    pub(crate) waiting_for_ask: bool,
    pub(crate) ask_options: Vec<crate::types::AskOption>,
    pub(crate) ask_selected_index: usize,
    pub(crate) ask_multi_select: bool, // Whether this is a multi-select question
    pub(crate) ask_submit_mode: SubmitMode, // How to submit selection
    pub(crate) ask_other_input_active: bool, // Whether user is typing custom input for "Other" option
    // Multi-question support
    pub(crate) ask_questions: Vec<AskQuestion>, // Queue of questions
    pub(crate) current_question_idx: usize,     // Current question index
    // Todo tracking for progress display
    pub(crate) todo_items: Vec<TodoItem>,
    // Channels
    pub(crate) tx: tokio::sync::mpsc::Sender<String>,
    pub(crate) rx: tokio::sync::mpsc::Receiver<AgentEvent>,
    pub(crate) cancel: CancellationToken,
    // Message queue for pending inputs while AI is processing
    pub(crate) pending_messages: Vec<String>,
    // Loop task state
    pub(crate) loop_task: Option<LoopTask>,
    // Cron tasks state
    pub(crate) cron_tasks: Vec<CronTask>,
    // Debug mode
    pub(crate) debug_mode: bool,
    // Debug panel state
    pub(crate) show_debug_panel: bool,
    pub(crate) debug_logs: Vec<String>,
    pub(crate) debug_scroll_offset: u16,
}

/// Todo item for progress tracking
#[derive(Clone)]
#[allow(dead_code)]  // Fields used in serialization, not directly read
pub struct TodoItem {
    pub content: String,
    pub status: String, // "pending", "in_progress", "completed"
}

/// Loop task - repeatedly send message
#[derive(Clone)]
pub struct LoopTask {
    pub message: String,
    pub interval_secs: u64,
    pub count: u64,
    pub max_count: Option<u64>,
    pub cancel_token: CancellationToken,
}

/// Cron task - scheduled message sending
#[derive(Clone)]
pub struct CronTask {
    pub id: usize,
    pub message: String,
    pub minute_interval: u64, // Simplified: run every N minutes
    #[allow(dead_code)]
    pub next_run: Instant, // For future use: precise scheduling
    pub cancel_token: CancellationToken,
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
            activity_input: None,
            messages: Vec::new(),
            thinking: String::new(),
            streaming: String::new(),
            input: String::new(),
            model: "claude-sonnet-4".into(),
            tokens_in: 0,
            tokens_out: 0,
            session_total_out: 0,
            current_request_tokens: 0,
            cache_read: 0,
            cache_created: 0,
            context_size: 200_000,
            api_calls: 0,
            compressions: 0,
            memory_saves: 0,
            tool_calls: 0,
            request_start: None,
            tool_start: None,
            frame: 0,
            last_anim: Instant::now(),
            show_welcome: true,
            exit: false,
            cursor_pos: 0,
            input_history: Vec::new(),
            history_index: None,
            history_draft: String::new(),
            scroll_offset: 0,
            auto_scroll: true,
            max_scroll: std::cell::Cell::new(0),
            new_message_while_scrolled: std::cell::Cell::new(false),
            thinking_collapsed: false, // Default: expanded to show thinking content
            dirty: std::cell::Cell::new(true), // Initial render needed
            approve_mode: ApproveMode::Ask,
            shared_approve_mode: None,
            ask_tx: None,
            waiting_for_ask: false,
            ask_options: Vec::new(),
            ask_selected_index: 0,
            ask_multi_select: false,
            ask_submit_mode: SubmitMode::default(),
            ask_other_input_active: false,
            ask_questions: Vec::new(),
            current_question_idx: 0,
            todo_items: Vec::new(),
            tx,
            rx,
            cancel,
            pending_messages: Vec::new(),
            loop_task: None,
            cron_tasks: Vec::new(),
            debug_mode: false,
            show_debug_panel: false,
            debug_logs: Vec::new(),
            debug_scroll_offset: 0,
        }
    }

    pub fn with_ask_channel(mut self, ask_tx: tokio::sync::mpsc::Sender<String>) -> Self {
        self.ask_tx = Some(ask_tx);
        self
    }

    /// Set shared approve mode atomic for real-time mode switching during agent execution.
    pub fn with_shared_approve_mode(
        mut self,
        shared: std::sync::Arc<std::sync::atomic::AtomicU8>,
    ) -> Self {
        self.shared_approve_mode = Some(shared);
        self
    }

    pub fn with_config(
        mut self,
        model: &str,
        _think: bool,
        _max_tokens: u32,
        context_size: Option<u64>,
    ) -> Self {
        self.model = model.to_string();
        self.context_size = context_size.unwrap_or_else(|| {
            let m = model.to_ascii_lowercase();
            if m.contains("1m") || m.contains("opus-4-7") {
                1_000_000
            } else if m.contains("claude-3")
                || m.contains("claude-4")
                || m.contains("claude-sonnet")
            {
                200_000
            } else {
                128_000
            }
        });
        self
    }

    /// Set debug mode from environment or config
    pub fn with_debug_mode(mut self, debug_mode: bool) -> Self {
        self.debug_mode = debug_mode;
        self
    }

    /// Toggle debug panel visibility
    pub fn toggle_debug_panel(&mut self) {
        self.show_debug_panel = !self.show_debug_panel;
        self.dirty.set(true);
    }

    /// Add a debug log entry
    pub fn add_debug_log(&mut self, log: String) {
        // Keep only last 100 logs to avoid memory issues
        if self.debug_logs.len() >= 100 {
            self.debug_logs.remove(0);
        }
        self.debug_logs.push(log);
        self.dirty.set(true);
    }

    /// Clear debug logs
    pub fn clear_debug_logs(&mut self) {
        self.debug_logs.clear();
        self.debug_scroll_offset = 0;
        self.dirty.set(true);
    }

    pub fn load_messages(&mut self, core_messages: Vec<matrixcode_core::Message>) {
        // Build mapping from tool_use_id to tool name
        let mut tool_names: HashMap<String, String> = HashMap::new();

        // First pass: collect tool names from ToolUse blocks
        for msg in &core_messages {
            if let matrixcode_core::MessageContent::Blocks(blocks) = &msg.content {
                for b in blocks {
                    if let matrixcode_core::ContentBlock::ToolUse { id, name, .. } = b {
                        tool_names.insert(id.clone(), name.clone());
                    }
                }
            }
        }

        // Second pass: process messages
        for msg in core_messages {
            // Handle different content block types separately
            match &msg.content {
                matrixcode_core::MessageContent::Text(t) => {
                    if t.is_empty() {
                        continue;
                    }
                    let role = match msg.role {
                        matrixcode_core::Role::User => Role::User,
                        matrixcode_core::Role::Assistant => Role::Assistant,
                        matrixcode_core::Role::System => Role::System,
                        matrixcode_core::Role::Tool => Role::Tool {
                            name: "tool".into(),
                            detail: None,
                            is_error: false,
                        },
                    };
                    // Restore input history from user messages
                    if role == Role::User
                        && !t.starts_with('/')
                        && self.input_history.last().map(|s| s.as_str()) != Some(t)
                    {
                        self.input_history.push(t.clone());
                    }
                    self.messages.push(Message {
                        role,
                        content: t.clone(),
                    });
                }
                matrixcode_core::MessageContent::Blocks(blocks) => {
                    // Process each block separately to maintain proper message types
                    for b in blocks {
                        match b {
                            matrixcode_core::ContentBlock::Text { text } => {
                                if text.is_empty() {
                                    continue;
                                }
                                let role = match msg.role {
                                    matrixcode_core::Role::User => Role::User,
                                    matrixcode_core::Role::Assistant => Role::Assistant,
                                    matrixcode_core::Role::System => Role::System,
                                    matrixcode_core::Role::Tool => Role::Tool {
                                        name: "tool".into(),
                                        detail: None,
                                        is_error: false,
                                    },
                                };
                                // Restore input history from user messages
                                if role == Role::User
                                    && !text.starts_with('/')
                                    && self.input_history.last().map(|s| s.as_str()) != Some(text)
                                {
                                    self.input_history.push(text.clone());
                                }
                                self.messages.push(Message {
                                    role,
                                    content: text.clone(),
                                });
                            }
                            matrixcode_core::ContentBlock::Thinking { thinking, .. } => {
                                if thinking.is_empty() {
                                    continue;
                                }
                                // Create separate Thinking message for proper rendering
                                self.messages.push(Message {
                                    role: Role::Thinking,
                                    content: thinking.clone(),
                                });
                            }
                            matrixcode_core::ContentBlock::ToolUse { name: _, .. } => {
                                // Skip tool_use blocks - metadata only (already collected in first pass)
                            }
                            matrixcode_core::ContentBlock::ToolResult {
                                content,
                                tool_use_id,
                                ..
                            } => {
                                if content.is_empty() {
                                    continue;
                                }
                                // Try to determine if this is an error from content
                                let is_error = content.contains("error")
                                    || content.contains("failed")
                                    || content.contains("Error");
                                // Use tool name from mapping, or fallback to tool_use_id
                                let name =
                                    tool_names.get(tool_use_id).cloned().unwrap_or_else(|| {
                                        // Fallback: try to guess from tool_use_id prefix
                                        if tool_use_id.starts_with("bash") {
                                            "bash".into()
                                        } else if tool_use_id.starts_with("read") {
                                            "read".into()
                                        } else if tool_use_id.starts_with("write") {
                                            "write".into()
                                        } else if tool_use_id.starts_with("edit") {
                                            "edit".into()
                                        } else {
                                            "tool".into()
                                        }
                                    });
                                self.messages.push(Message {
                                    role: Role::Tool {
                                        name,
                                        detail: None,
                                        is_error,
                                    },
                                    content: content.clone(),
                                });
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        if !self.messages.is_empty() {
            self.show_welcome = false;
        }
    }

    /// Set token stats from restored session metadata.
    pub fn set_token_stats(&mut self, input_tokens: u64, total_output_tokens: u64, _message_count: usize) {
        self.tokens_in = input_tokens;
        self.session_total_out = total_output_tokens;
    }

    pub fn run(&mut self, term: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        loop {
            // Animation frame - cycle through 10 frames for spinner
            // Always render when animation frame updates (for spinner)
            let anim_update = self.last_anim.elapsed().as_millis() >= ANIM_MS as u128;
            if anim_update {
                self.frame = (self.frame + 1) % 10;
                self.last_anim = Instant::now();
                self.dirty.set(true);
            }

            // Handle events - mark dirty on any user input
            if event::poll(Duration::from_millis(ANIM_MS as u64))? {
                match event::read()? {
                    Event::Key(k) => {
                        self.on_key(k);
                        self.dirty.set(true);
                    }
                    Event::Mouse(m) => {
                        self.on_mouse(m);
                        self.dirty.set(true);
                    }
                    Event::Paste(text) => {
                        self.on_paste(&text);
                        self.dirty.set(true);
                    }
                    _ => {}
                }
            }

            // Process agent events - mark dirty on any event
            let mut had_event = false;
            while let Ok(e) = self.rx.try_recv() {
                log::debug!("TUI received event: type={:?}", e.event_type);
                self.on_event(e);
                had_event = true;
            }
            if had_event {
                log::debug!("TUI: had events, marking dirty");
                self.dirty.set(true);
            }

            // Only render if dirty (something changed)
            if self.dirty.get() {
                term.draw(|f| self.draw(f))?;
                self.dirty.set(false);
            }

            if self.exit {
                break;
            }
        }
        Ok(())
    }
    fn on_mouse(&mut self, m: MouseEvent) {
        // If Shift is held, let terminal handle mouse for text selection
        if m.modifiers.contains(event::KeyModifiers::SHIFT) {
            return;
        }

        match m.kind {
            MouseEventKind::ScrollUp => {
                if self.auto_scroll {
                    self.auto_scroll = false;
                    self.scroll_offset = self.max_scroll.get().max(50);
                }
                self.scroll_offset = self.scroll_offset.saturating_sub(3);
            }
            MouseEventKind::ScrollDown => {
                if !self.auto_scroll {
                    self.scroll_offset = self.scroll_offset.saturating_add(3);
                    let max = self.max_scroll.get();
                    if max > 0 && self.scroll_offset >= max {
                        self.auto_scroll = true;
                        self.scroll_offset = 0;
                    }
                }
            }
            _ => {}
        }
    }
}
