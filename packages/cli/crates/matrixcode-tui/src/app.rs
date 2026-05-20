use std::io::Stdout;
use std::time::{Duration, Instant};

use anyhow::Result;
use ratatui::{
    backend::CrosstermBackend,
    crossterm::event::{self, Event, MouseEvent, MouseEventKind},
    Terminal,
};

use matrixcode_core::{AgentEvent, cancel::CancellationToken};
use ratatui::crossterm::event::MouseButton;

use crate::types::{Activity, ApproveMode, Role, Message};
use crate::utils::extract_by_visual_col;
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
    pub(crate) current_request_tokens: u64,  // Tokens for current request (real-time)
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
    // UI state
    pub(crate) frame: usize,
    pub(crate) last_anim: Instant,
    pub(crate) show_welcome: bool,
    pub(crate) exit: bool,
    // Input cursor position (character index in input string)
    pub(crate) cursor_pos: usize,
    // Input history (Up/Down arrow navigation)
    pub(crate) input_history: Vec<String>,
    pub(crate) history_index: Option<usize>,  // None = not browsing history
    pub(crate) history_draft: String,  // Saves current input when entering history mode
    // Scroll state
    pub(crate) scroll_offset: u16,
    pub(crate) auto_scroll: bool,
    pub(crate) max_scroll: std::cell::Cell<u16>,
    // Thinking display state
    pub(crate) thinking_collapsed: bool,
    // Approval mode
    pub(crate) approve_mode: ApproveMode,
    // Shared approve mode atomic - directly updates agent's mode in real-time
    pub(crate) shared_approve_mode: Option<std::sync::Arc<std::sync::atomic::AtomicU8>>,
    // Ask tool channel
    pub(crate) ask_tx: Option<tokio::sync::mpsc::Sender<String>>,
    pub(crate) waiting_for_ask: bool,
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
    // Selection state
    pub(crate) selection: Option<Selection>,
    pub(crate) selecting: bool,  // True while mouse dragging
    pub(crate) msg_area_top: std::cell::Cell<u16>,  // Messages area top Y (computed in draw)
    // Debug mode
    pub(crate) debug_mode: bool,
}

/// Text selection in messages area
#[derive(Clone, Copy, Debug)]
pub struct Selection {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

impl Selection {
    pub fn new(start_line: usize, start_col: usize) -> Self {
        Self {
            start_line,
            start_col,
            end_line: start_line,
            end_col: start_col,
        }
    }
    
    pub fn extend_to(&mut self, line: usize, col: usize) {
        self.end_line = line;
        self.end_col = col;
    }
    
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.start_line == self.end_line && self.start_col == self.end_col
    }
    
    pub fn normalized(&self) -> Self {
        // Normalize so start <= end
        if self.start_line > self.end_line || 
           (self.start_line == self.end_line && self.start_col > self.end_col) {
            Self {
                start_line: self.end_line,
                start_col: self.end_col,
                end_line: self.start_line,
                end_col: self.start_col,
            }
        } else {
            *self
        }
    }
    
    #[allow(dead_code)]
    pub fn contains(&self, line: usize, col: usize) -> bool {
        let norm = self.normalized();
        if line < norm.start_line || line > norm.end_line {
            return false;
        }
        if line == norm.start_line && line == norm.end_line {
            return col >= norm.start_col && col <= norm.end_col;
        }
        if line == norm.start_line {
            return col >= norm.start_col;
        }
        if line == norm.end_line {
            return col <= norm.end_col;
        }
        true  // Middle line
    }
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
    pub minute_interval: u64,  // Simplified: run every N minutes
    #[allow(dead_code)]
    pub next_run: Instant,  // For future use: precise scheduling
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
            thinking_collapsed: false,  // Default: expanded
            approve_mode: ApproveMode::Ask,
            shared_approve_mode: None,
            ask_tx: None,
            waiting_for_ask: false,
            tx, rx, cancel,
            pending_messages: Vec::new(),
            loop_task: None,
            cron_tasks: Vec::new(),
            selection: None,
            selecting: false,
            msg_area_top: std::cell::Cell::new(0),
            debug_mode: false,
        }
    }

    pub fn with_ask_channel(mut self, ask_tx: tokio::sync::mpsc::Sender<String>) -> Self {
        self.ask_tx = Some(ask_tx);
        self
    }

    /// Set shared approve mode atomic for real-time mode switching during agent execution.
    pub fn with_shared_approve_mode(mut self, shared: std::sync::Arc<std::sync::atomic::AtomicU8>) -> Self {
        self.shared_approve_mode = Some(shared);
        self
    }

    pub fn with_config(mut self, model: &str, _think: bool, _max_tokens: u32, context_size: Option<u64>) -> Self {
        self.model = model.to_string();
        self.context_size = context_size.unwrap_or_else(|| {
            let m = model.to_ascii_lowercase();
            if m.contains("1m") || m.contains("opus-4-7") {
                1_000_000
            } else if m.contains("claude-3") || m.contains("claude-4") || m.contains("claude-sonnet") {
                200_000
            } else {
                128_000
            }
        });
        self
    }

    pub fn load_messages(&mut self, core_messages: Vec<matrixcode_core::Message>) {
        for msg in core_messages {
            // Handle different content block types separately
            match &msg.content {
                matrixcode_core::MessageContent::Text(t) => {
                    if t.is_empty() { continue; }
                    let role = match msg.role {
                        matrixcode_core::Role::User => Role::User,
                        matrixcode_core::Role::Assistant => Role::Assistant,
                        matrixcode_core::Role::System => Role::System,
                        matrixcode_core::Role::Tool => Role::Tool { name: "tool".into(), is_error: false },
                    };
                    // Restore input history from user messages
                    if role == Role::User && !t.starts_with('/')
                        && self.input_history.last().map(|s| s.as_str()) != Some(t) {
                        self.input_history.push(t.clone());
                    }
                    self.messages.push(Message { role, content: t.clone() });
                }
                matrixcode_core::MessageContent::Blocks(blocks) => {
                    // Process each block separately to maintain proper message types
                    for b in blocks {
                        match b {
                            matrixcode_core::ContentBlock::Text { text } => {
                                if text.is_empty() { continue; }
                                let role = match msg.role {
                                    matrixcode_core::Role::User => Role::User,
                                    matrixcode_core::Role::Assistant => Role::Assistant,
                                    matrixcode_core::Role::System => Role::System,
                                    matrixcode_core::Role::Tool => Role::Tool { name: "tool".into(), is_error: false },
                                };
                                // Restore input history from user messages
                                if role == Role::User && !text.starts_with('/')
                                    && self.input_history.last().map(|s| s.as_str()) != Some(text) {
                                    self.input_history.push(text.clone());
                                }
                                self.messages.push(Message { role, content: text.clone() });
                            }
                            matrixcode_core::ContentBlock::Thinking { thinking, .. } => {
                                if thinking.is_empty() { continue; }
                                // Create separate Thinking message for proper rendering
                                self.messages.push(Message { role: Role::Thinking, content: thinking.clone() });
                            }
                            matrixcode_core::ContentBlock::ToolUse { name: _, .. } => {
                                // Skip tool_use blocks - metadata only
                            }
                            matrixcode_core::ContentBlock::ToolResult { content, tool_use_id, .. } => {
                                if content.is_empty() { continue; }
                                // Try to determine if this is an error from content
                                let is_error = content.contains("error") || content.contains("failed") || content.contains("Error");
                                self.messages.push(Message { 
                                    role: Role::Tool { 
                                        name: if tool_use_id.starts_with("bash") { "bash".into() } else { tool_use_id.clone() },
                                        is_error 
                                    }, 
                                    content: content.clone() 
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

    /// Get selected text from messages
    pub(crate) fn get_selected_text(&self, selection: Selection) -> String {
        let norm = selection.normalized();
        
        // We need to reconstruct the text from rendered lines
        // Build all text lines from messages (approximate - matches draw_messages logic)
        let mut all_text: Vec<String> = Vec::new();
        
        // Account for welcome message lines
        if self.show_welcome && self.messages.is_empty() {
            // 7 welcome lines + 1 empty
            for _ in 0..8 {
                all_text.push(String::new());
            }
        }
        
        for msg in &self.messages {
            let icon = msg.role.icon();
            let label = msg.role.label();
            all_text.push(format!("{} {}", icon, label));
            for line in msg.content.lines() {
                all_text.push(format!("  {}", line));
            }
            all_text.push(String::new());  // Empty line between messages
        }
        
        // Extract selected range using visual column positions
        let mut result = String::new();
        for i in norm.start_line..=norm.end_line {
            if let Some(line) = all_text.get(i) {
                let (start_col, end_col) = if i == norm.start_line && i == norm.end_line {
                    (norm.start_col, norm.end_col)
                } else if i == norm.start_line {
                    (norm.start_col, usize::MAX)
                } else if i == norm.end_line {
                    (0, norm.end_col)
                } else {
                    (0, usize::MAX)
                };
                
                // Convert visual column to char boundary
                let extracted = extract_by_visual_col(line, start_col, end_col);
                result.push_str(&extracted);
                if i != norm.end_line {
                    result.push('\n');
                }
            }
        }
        
        result
    }

    pub fn run(&mut self, term: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        loop {
            // Animation frame - cycle through 12 frames for spinner
            if self.last_anim.elapsed().as_millis() >= ANIM_MS as u128 {
                self.frame = (self.frame + 1) % 12;
                self.last_anim = Instant::now();
            }

            term.draw(|f| self.draw(f))?;

            // Handle events
            if event::poll(Duration::from_millis(16))? {
                match event::read()? {
                    Event::Key(k) => self.on_key(k),
                    Event::Mouse(m) => self.on_mouse(m, self.msg_area_top.get()),
                    Event::Paste(text) => self.on_paste(&text),
                    _ => {}
                }
            }

            // Process agent events
            while let Ok(e) = self.rx.try_recv() {
                self.on_event(e);
            }

            if self.exit { break; }
        }
        Ok(())
    }
    fn on_mouse(&mut self, m: MouseEvent, msg_area_y: u16) {
        match m.kind {
            MouseEventKind::ScrollUp => {
                // Scroll up = view earlier content = decrease offset
                // ratatui scroll(offset) skips first N lines, so:
                // - scroll_offset=0 shows top, scroll_offset=max shows bottom
                // - scroll up (earlier) = decrease offset
                if self.auto_scroll {
                    self.auto_scroll = false;
                    // We need to start from bottom, then scroll up
                    // Use max_scroll (will be updated in draw) or a large value
                    self.scroll_offset = self.max_scroll.get().max(50);
                }
                self.scroll_offset = self.scroll_offset.saturating_sub(3);
                self.selection = None;  // Clear selection on scroll
            }
            MouseEventKind::ScrollDown => {
                // Scroll down = view newer content = increase offset
                if !self.auto_scroll {
                    self.scroll_offset = self.scroll_offset.saturating_add(3);
                    // Check if we've scrolled to the bottom
                    // Use max_scroll if available, otherwise just keep scrolling
                    let max = self.max_scroll.get();
                    if max > 0 && self.scroll_offset >= max {
                        self.auto_scroll = true;
                        self.scroll_offset = 0;
                    }
                }
                self.selection = None;  // Clear selection on scroll
            }
            MouseEventKind::Down(MouseButton::Left) => {
                // Start selection in messages area
                if m.row >= msg_area_y {
                    // If auto_scroll is on, sync scroll_offset first before disabling it
                    if self.auto_scroll {
                        self.scroll_offset = self.max_scroll.get().max(50);
                    }
                    let line = self.scroll_offset as usize + (m.row - msg_area_y) as usize;
                    let col = m.column as usize;
                    self.selection = Some(Selection::new(line, col));
                    self.selecting = true;
                    self.auto_scroll = false;  // Stop auto scroll when selecting
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                // Extend selection
                if self.selecting && m.row >= msg_area_y {
                    // Sync scroll_offset if auto_scroll was on
                    if self.auto_scroll {
                        self.scroll_offset = self.max_scroll.get().max(50);
                        self.auto_scroll = false;
                    }
                    let line = self.scroll_offset as usize + (m.row - msg_area_y) as usize;
                    let col = m.column as usize;
                    if let Some(ref mut sel) = self.selection {
                        sel.extend_to(line, col);
                    }
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.selecting = false;
                // Auto-copy to clipboard on mouse release (like terminal behavior)
                if let Some(sel) = self.selection {
                    let text = self.get_selected_text(sel);
                    if !text.is_empty() {
                        let _ = arboard::Clipboard::new()
                            .and_then(|mut cb| cb.set_text(&text));
                    }
                }
            }
            _ => {}
        }
    }

}
