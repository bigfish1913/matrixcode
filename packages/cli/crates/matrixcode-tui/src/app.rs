use std::io::Stdout;
use std::time::{Duration, Instant};

use anyhow::Result;
use ratatui::{
    backend::CrosstermBackend,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind},
    Terminal,
};

use matrixcode_core::{AgentEvent, EventData, EventType, cancel::CancellationToken};
use ratatui::crossterm::event::MouseButton;

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
    // Input cursor position (character index in input string)
    pub(crate) cursor_pos: usize,
    // Scroll state
    pub(crate) scroll_offset: u16,
    pub(crate) auto_scroll: bool,
    pub(crate) max_scroll: std::cell::Cell<u16>,
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
            cursor_pos: 0,
            scroll_offset: 0,
            auto_scroll: true,
            max_scroll: std::cell::Cell::new(0),
            thinking_collapsed: false,  // Default: expanded
            approve_mode: ApproveMode::Ask,
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
            let content = match &msg.content {
                matrixcode_core::MessageContent::Text(t) => t.clone(),
                matrixcode_core::MessageContent::Blocks(blocks) => {
                    blocks.iter().filter_map(|b| match b {
                        matrixcode_core::ContentBlock::Text { text } => Some(text.clone()),
                        _ => None,
                    }).collect::<Vec<_>>().join("\n")
                }
            };
            if content.is_empty() { continue; }
            let role = match msg.role {
                matrixcode_core::Role::User => Role::User,
                matrixcode_core::Role::Assistant => Role::Assistant,
                matrixcode_core::Role::System => Role::System,
                matrixcode_core::Role::Tool => Role::Tool { name: "tool".into(), is_error: false },
            };
            self.messages.push(Message { role, content });
        }
        if !self.messages.is_empty() {
            self.show_welcome = false;
        }
    }

    /// Get selected text from messages
    fn get_selected_text(&self, selection: Selection) -> String {
        let norm = selection.normalized();
        
        // We need to reconstruct the text from rendered lines
        // This is tricky because we don't store the rendered lines
        // For simplicity, we'll extract from message content based on approximate line mapping
        
        let mut result = String::new();
        
        // Build all text lines from messages (approximate - doesn't account for wrapping)
        let mut all_text: Vec<String> = Vec::new();
        for msg in &self.messages {
            let icon = msg.role.icon();
            let label = msg.role.label();
            all_text.push(format!("{} {}", icon, label));
            for line in msg.content.lines() {
                all_text.push(format!("  {}", line));
            }
            all_text.push(String::new());  // Empty line between messages
        }
        
        // Extract selected range
        for i in norm.start_line..=norm.end_line {
            if let Some(line) = all_text.get(i) {
                if i == norm.start_line && i == norm.end_line {
                    // Single line selection
                    if norm.start_col < line.len() && norm.end_col <= line.len() {
                        result.push_str(&line[norm.start_col..norm.end_col]);
                    }
                } else if i == norm.start_line {
                    // First line of multi-line selection
                    if norm.start_col < line.len() {
                        result.push_str(&line[norm.start_col..]);
                    }
                    result.push('\n');
                } else if i == norm.end_line {
                    // Last line of multi-line selection
                    if norm.end_col <= line.len() {
                        result.push_str(&line[..norm.end_col]);
                    }
                } else {
                    // Middle line
                    result.push_str(line);
                    result.push('\n');
                }
            }
        }
        
        result
    }

    pub fn run(&mut self, term: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        loop {
            // Animation frame
            if self.last_anim.elapsed().as_millis() >= ANIM_MS as u128 {
                self.frame = (self.frame + 1) % 10;
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

    fn on_key(&mut self, k: KeyEvent) {
        if k.kind != KeyEventKind::Press { return; }

        match k.code {
            // Enter: send or newline
            KeyCode::Enter => {
                if k.modifiers.contains(KeyModifiers::SHIFT) {
                    // Shift+Enter: insert newline at cursor position
                    self.ensure_char_boundary();
                    self.input.insert(self.cursor_pos, '\n');
                    self.cursor_pos += 1;  // '\n' is 1 byte
                } else if !self.input.trim().is_empty() {
                    self.send_input();
                }
            }

            // Escape: interrupt or clear input
            KeyCode::Esc => {
                if self.activity == Activity::Asking {
                    // Abort approval request
                    self.waiting_for_ask = false;
                    self.activity = Activity::Idle;
                    self.messages.push(Message { role: Role::System, content: "⚠️ Approval aborted".into() });
                    if let Some(ask_tx) = &self.ask_tx {
                        ask_tx.try_send("abort".to_string()).ok();
                    }
                } else if self.activity != Activity::Idle {
                    self.cancel.cancel();
                    self.cancel.reset();
                    self.activity = Activity::Idle;
                    self.messages.push(Message { role: Role::System, content: "⚠️ Interrupted".into() });
                } else {
                    self.input.clear();
                    self.cursor_pos = 0;
                }
            }

            // Ctrl+C: copy selection or interrupt
            KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(sel) = self.selection {
                    // Copy selected text
                    let selected_text = self.get_selected_text(sel);
                    if !selected_text.is_empty() {
                        // Try to copy to clipboard
                        let clipboard_result = arboard::Clipboard::new()
                            .and_then(|mut cb| cb.set_text(&selected_text));
                        
                        match clipboard_result {
                            Ok(_) => {
                                self.messages.push(Message {
                                    role: Role::System,
                                    content: format!("📋 Copied {} chars to clipboard", selected_text.len())
                                });
                            }
                            Err(_) => {
                                self.messages.push(Message {
                                    role: Role::System,
                                    content: format!("📋 Copied {} chars (clipboard unavailable)", selected_text.len())
                                });
                            }
                        }
                        self.selection = None;
                        self.selecting = false;
                    }
                } else if self.activity != Activity::Idle {
                    self.cancel.cancel();
                    self.cancel.reset();
                    self.activity = Activity::Idle;
                    self.messages.push(Message { role: Role::System, content: "⚠️ Interrupted".into() });
                }
            }

            // Ctrl+D: exit
            KeyCode::Char('d') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                self.exit = true;
            }

            // Ctrl+V: paste from clipboard
            KeyCode::Char('v') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                // Try to get text from clipboard
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    if let Ok(text) = clipboard.get_text() {
                        self.on_paste(&text);
                    }
                }
            }

            // Backspace: delete char before cursor
            KeyCode::Backspace => {
                if self.cursor_pos > 0 {
                    let prev_pos = self.prev_char_boundary();
                    self.input.drain(prev_pos..self.cursor_pos);
                    self.cursor_pos = prev_pos;
                }
            }

            // Delete: delete char at cursor
            KeyCode::Delete => {
                if self.cursor_pos < self.input.len() {
                    let next_pos = self.next_char_boundary();
                    self.input.drain(self.cursor_pos..next_pos);
                }
            }

            // Left arrow: move cursor left (one character)
            KeyCode::Left => {
                if self.cursor_pos > 0 {
                    self.cursor_pos = self.prev_char_boundary();
                }
            }

            // Right arrow: move cursor right (one character)
            KeyCode::Right => {
                if self.cursor_pos < self.input.len() {
                    self.cursor_pos = self.next_char_boundary();
                }
            }

            // Up arrow: move cursor to previous line (in multiline input)
            KeyCode::Up if !k.modifiers.contains(KeyModifiers::ALT) => {
                if self.input.contains('\n') {
                    let (current_line_num, col_chars, _) = self.get_line_info();
                    if current_line_num > 1 {
                        let char_pos = self.byte_pos_to_char_pos();
                        let input_chars: Vec<char> = self.input.chars().collect();
                        let before_cursor_str: String = input_chars[..char_pos.min(input_chars.len())].iter().collect();
                        
                        // Previous line is before the last '\n' in before_cursor_str
                        let prev_lines_str = &before_cursor_str[..before_cursor_str.rfind('\n').unwrap_or(0)];
                        let prev_line_start_char = prev_lines_str.chars().count();
                        
                        // Find previous line length
                        let prev_line_end_char = char_pos.saturating_sub(col_chars).saturating_sub(1); // -1 for the newline
                        let prev_line_len_chars = prev_line_end_char.saturating_sub(prev_line_start_char);
                        
                        // Move to same column (or end if shorter)
                        let target_char_pos = prev_line_start_char + col_chars.min(prev_line_len_chars);
                        self.cursor_pos = self.char_pos_to_byte_pos(target_char_pos);
                    }
                }
            }

            // Down arrow: move cursor to next line (in multiline input)
            KeyCode::Down if !k.modifiers.contains(KeyModifiers::ALT) => {
                if self.input.contains('\n') {
                    let (current_line_num, col_chars, total_lines) = self.get_line_info();
                    if current_line_num < total_lines {
                        let char_pos = self.byte_pos_to_char_pos();
                        let input_chars: Vec<char> = self.input.chars().collect();
                        
                        // Boundary check: char_pos must not exceed input_chars.len()
                        let safe_char_pos = char_pos.min(input_chars.len());
                        
                        // Find next line start
                        let remaining_chars = &input_chars[safe_char_pos..];
                        let next_line_start_char = remaining_chars.iter().position(|c| *c == '\n')
                            .map(|i| safe_char_pos + i + 1)
                            .unwrap_or_else(|| input_chars.len());
                        
                        // Find next line end
                        let next_line_chars = &input_chars[next_line_start_char..];
                        let next_line_end_char = next_line_chars.iter().position(|c| *c == '\n')
                            .map(|i| next_line_start_char + i)
                            .unwrap_or_else(|| input_chars.len());
                        
                        let next_line_len_chars = next_line_end_char.saturating_sub(next_line_start_char);
                        
                        // Move to same column (or end if shorter)
                        let target_char_pos = next_line_start_char + col_chars.min(next_line_len_chars);
                        self.cursor_pos = self.char_pos_to_byte_pos(target_char_pos);
                    }
                }
            }

            // Regular character input (except when Alt/Ctrl is held)
            KeyCode::Char(c) if !k.modifiers.contains(KeyModifiers::ALT) && !k.modifiers.contains(KeyModifiers::CONTROL) => {
                self.ensure_char_boundary();
                self.input.insert(self.cursor_pos, c);
                self.cursor_pos += c.len_utf8();
            }

            // Alt+M: toggle approve mode
            KeyCode::Char('m') if k.modifiers.contains(KeyModifiers::ALT) => {
                self.approve_mode = self.approve_mode.next();
                self.tx.try_send(format!("/mode:{}", self.approve_mode.label())).ok();
            }

            // Alt+T: toggle thinking collapse
            KeyCode::Char('t') if k.modifiers.contains(KeyModifiers::ALT) => {
                self.thinking_collapsed = !self.thinking_collapsed;
            }

            // Shift+Tab / BackTab: toggle approve mode
            KeyCode::Tab if k.modifiers.contains(KeyModifiers::SHIFT) => {
                self.approve_mode = self.approve_mode.next();
                self.tx.try_send(format!("/mode:{}", self.approve_mode.label())).ok();
            }
            KeyCode::BackTab => {
                self.approve_mode = self.approve_mode.next();
                self.tx.try_send(format!("/mode:{}", self.approve_mode.label())).ok();
            }

            // Scroll: PageUp
            KeyCode::PageUp => {
                if self.auto_scroll {
                    self.scroll_offset = self.max_scroll.get();
                    self.auto_scroll = false;
                }
                self.scroll_offset = self.scroll_offset.saturating_sub(10);
            }

            // Scroll: PageDown
            KeyCode::PageDown => {
                if !self.auto_scroll {
                    self.scroll_offset = self.scroll_offset.saturating_add(10);
                    if self.scroll_offset >= self.max_scroll.get() {
                        self.auto_scroll = true;
                        self.scroll_offset = 0;
                    }
                }
            }

            // Scroll: Alt+Up (or Up when not idle)
            KeyCode::Up if k.modifiers.contains(KeyModifiers::ALT) => {
                if self.auto_scroll {
                    self.scroll_offset = self.max_scroll.get();
                    self.auto_scroll = false;
                }
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }

            // Scroll: Alt+Down (or Down when not idle)
            KeyCode::Down if k.modifiers.contains(KeyModifiers::ALT) => {
                if !self.auto_scroll {
                    self.scroll_offset = self.scroll_offset.saturating_add(1);
                    if self.scroll_offset >= self.max_scroll.get() {
                        self.auto_scroll = true;
                        self.scroll_offset = 0;
                    }
                }
            }

            // Home: move cursor to start (if input has content) or scroll to top
            KeyCode::Home => {
                if !self.input.is_empty() {
                    self.cursor_pos = 0;
                } else {
                    self.auto_scroll = false;
                    self.scroll_offset = 0;
                }
            }

            // End: move cursor to end (if input has content) or scroll to bottom
            KeyCode::End => {
                if !self.input.is_empty() {
                    self.cursor_pos = self.input.len();
                } else {
                    self.auto_scroll = true;
                    self.scroll_offset = 0;
                }
            }

            _ => {}
        }
    }

    // ============================================================================
    // Unicode-safe cursor position helpers
    // ============================================================================
    
    /// Ensure cursor_pos is at a valid UTF-8 character boundary.
    /// If not, move to the nearest valid boundary.
    fn ensure_char_boundary(&mut self) {
        if !self.input.is_char_boundary(self.cursor_pos) {
            self.cursor_pos = self.input.char_indices()
                .rfind(|(i, _)| *i <= self.cursor_pos)
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }
    
    /// Find the byte position of the previous character boundary.
    /// Returns 0 if cursor is at the start.
    fn prev_char_boundary(&self) -> usize {
        self.input.char_indices()
            .rfind(|(i, _)| *i < self.cursor_pos)
            .map(|(i, _)| i)
            .unwrap_or(0)
    }
    
    /// Find the byte position of the next character boundary.
    /// Returns input.len() if cursor is at the end.
    fn next_char_boundary(&self) -> usize {
        self.input.char_indices()
            .find(|(i, _)| *i > self.cursor_pos)
            .map(|(i, _)| i)
            .unwrap_or_else(|| self.input.len())
    }
    
    /// Convert byte position to character position (count of chars before cursor).
    fn byte_pos_to_char_pos(&self) -> usize {
        self.input.char_indices().take(self.cursor_pos).count()
    }
    
    /// Convert character position to byte position.
    fn char_pos_to_byte_pos(&self, char_pos: usize) -> usize {
        self.input.char_indices()
            .nth(char_pos)
            .map(|(i, _)| i)
            .unwrap_or_else(|| self.input.len())
    }
    
    /// Get current line info: (current_line_number, column_in_chars, total_lines)
    fn get_line_info(&self) -> (usize, usize, usize) {
        let char_pos = self.byte_pos_to_char_pos();
        let before_cursor_str: String = self.input.chars().take(char_pos).collect();
        let current_line_num = before_cursor_str.lines().count();
        let total_lines = self.input.lines().count();
        let current_line_start_char = before_cursor_str.rfind('\n')
            .map(|i| before_cursor_str[i+1..].chars().count())
            .unwrap_or(0);
        let col_chars = char_pos - current_line_start_char;
        (current_line_num, col_chars, total_lines)
    }

    fn send_input(&mut self) {
        self.show_welcome = false;
        let input = self.input.trim().to_string();
        self.input.clear();
        self.cursor_pos = 0;

        if self.waiting_for_ask {
            // Respond to approval/ask question
            self.waiting_for_ask = false;
            self.messages.push(Message { role: Role::User, content: input.clone() });
            if let Some(ask_tx) = &self.ask_tx {
                ask_tx.try_send(input).ok();
            }
            self.activity = Activity::Thinking;
            self.auto_scroll = true;
        } else if input.starts_with('/') {
            // Command
            self.handle_command(&input);
        } else if self.activity == Activity::Idle {
            // Send immediately
            self.messages.push(Message { role: Role::User, content: input.clone() });
            self.tx.try_send(input).ok();
            self.activity = Activity::Thinking;
            self.auto_scroll = true;
        } else {
            // Queue message (AI is processing)
            self.pending_messages.push(input.clone());
        }
    }

    fn on_mouse(&mut self, m: MouseEvent, msg_area_y: u16) {
        match m.kind {
            MouseEventKind::ScrollUp => {
                // Scroll up = view earlier content = decrease offset
                if self.auto_scroll {
                    self.scroll_offset = self.max_scroll.get();
                    self.auto_scroll = false;
                }
                self.scroll_offset = self.scroll_offset.saturating_sub(3);
                self.selection = None;  // Clear selection on scroll
            }
            MouseEventKind::ScrollDown => {
                // Scroll down = view newer content = increase offset
                if !self.auto_scroll {
                    self.scroll_offset = self.scroll_offset.saturating_add(3);
                    if self.scroll_offset >= self.max_scroll.get() {
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
                        self.scroll_offset = self.max_scroll.get();
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
                        self.scroll_offset = self.max_scroll.get();
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
            }
            _ => {}
        }
    }

    fn on_paste(&mut self, text: &str) {
        self.ensure_char_boundary();
        self.input.insert_str(self.cursor_pos, text);
        self.cursor_pos += text.len();  // cursor_pos is byte position
    }

    fn handle_command(&mut self, cmd: &str) {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        let command = parts.first().copied().unwrap_or("");
        let args = &parts[1..];

        match command {
            "/exit" | "/quit" | "/q" => {
                self.exit = true;
            }
            "/clear" => {
                if self.activity == Activity::Idle {
                    self.messages.clear();
                    self.pending_messages.clear();
                    self.messages.push(Message { role: Role::System, content: "✓ Messages cleared".into() });
                } else {
                    self.messages.push(Message { role: Role::System, content: "⚠️ Cannot clear while AI is processing".into() });
                }
                self.auto_scroll = true;
            }
            "/history" => {
                let user_count = self.messages.iter().filter(|m| m.role == Role::User).count();
                let assistant_count = self.messages.iter().filter(|m| m.role == Role::Assistant).count();
                let tool_count = self.messages.iter().filter(|m| matches!(m.role, Role::Tool { .. })).count();
                let queue_count = self.pending_messages.len();
                self.messages.push(Message {
                    role: Role::System,
                    content: format!(
                        "📊 Session: {} user, {} assistant, {} tools, {} queued, {} output tokens",
                        user_count, assistant_count, tool_count, queue_count, fmt_tokens(self.session_total_out)
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
                                content: "Invalid mode. Use: ask, auto, strict".into()
                            });
                            return;
                        }
                    }
                    self.tx.try_send(format!("/mode:{}", self.approve_mode.label())).ok();
                    self.messages.push(Message {
                        role: Role::System,
                        content: format!("✓ Mode: {}", self.approve_mode.label())
                    });
                }
                self.auto_scroll = true;
            }
            "/model" => {
                if args.is_empty() {
                    self.messages.push(Message {
                        role: Role::System,
                        content: format!("Model: {} (context: {})", self.model, fmt_tokens(self.context_size))
                    });
                } else if self.activity == Activity::Idle {
                    let new_model = args.join(" ");
                    self.model = new_model.clone();
                    self.messages.push(Message {
                        role: Role::System,
                        content: format!("✓ Model: {}", new_model)
                    });
                } else {
                    self.messages.push(Message {
                        role: Role::System,
                        content: "⚠️ Cannot change model while AI is processing".into()
                    });
                }
                self.auto_scroll = true;
            }
            "/compact" | "/compress" => {
                self.tx.try_send("/compact".to_string()).ok();
                self.auto_scroll = true;
            }
            "/init" => {
                // Initialize project configuration
                if args.is_empty() {
                    // Generate project overview
                    self.tx.try_send("/init".to_string()).ok();
                    self.messages.push(Message {
                        role: Role::System,
                        content: "🔄 Generating project overview...".into()
                    });
                } else if args[0] == "status" {
                    // Show project status
                    self.tx.try_send("/init status".to_string()).ok();
                    self.messages.push(Message {
                        role: Role::System,
                        content: "⏳ Checking project status...".into()
                    });
                } else if args[0] == "reset" || args[0] == "clear" {
                    // Reset configuration
                    self.tx.try_send("/init reset".to_string()).ok();
                    self.messages.push(Message {
                        role: Role::System,
                        content: "⏳ Resetting project configuration...".into()
                    });
                } else {
                    self.messages.push(Message {
                        role: Role::System,
                        content: "Unknown init command. Use: /init, /init status, /init reset".into()
                    });
                }
                self.auto_scroll = true;
            }
            "/debug" => {
                // Toggle debug mode
                self.debug_mode = !self.debug_mode;
                self.messages.push(Message {
                    role: Role::System,
                    content: format!("🔧 Debug mode: {} (api/tools counts {})",
                        if self.debug_mode { "ON" } else { "OFF" },
                        if self.debug_mode { "visible" } else { "hidden" }
                    )
                });
                self.auto_scroll = true;
            }
            "/retry" => {
                // Process pending queue
                if !self.pending_messages.is_empty() && self.activity == Activity::Idle {
                    let next_msg = self.pending_messages.remove(0);
                    self.messages.push(Message { role: Role::User, content: next_msg.clone() });
                    self.tx.try_send(next_msg).ok();
                    self.activity = Activity::Thinking;
                    self.auto_scroll = true;
                    self.messages.push(Message {
                        role: Role::System,
                        content: if self.pending_messages.is_empty() {
                            "✓ Retry: processing last queued message".into()
                        } else {
                            format!("⏳ Retry: {} messages remaining", self.pending_messages.len())
                        }
                    });
                } else if self.pending_messages.is_empty() {
                    self.messages.push(Message { role: Role::System, content: "No pending messages to retry".into() });
                } else {
                    self.messages.push(Message { role: Role::System, content: "AI is busy, please wait".into() });
                }
                self.auto_scroll = true;
            }
            "/new" => {
                if self.activity == Activity::Idle {
                    self.messages.clear();
                    self.pending_messages.clear();
                    self.tokens_in = 0;
                    self.tokens_out = 0;
                    self.session_total_out = 0;
                    self.tx.try_send("/new".to_string()).ok();
                    self.messages.push(Message { role: Role::System, content: "✓ New session".into() });
                } else {
                    self.messages.push(Message { role: Role::System, content: "⚠️ Cannot start new session while AI is processing".into() });
                }
                self.auto_scroll = true;
            }
            "/help" => {
                self.messages.push(Message {
                    role: Role::System,
                    content: concat!(
                        "📖 Commands:\n",
                        "  /help     - Show this help\n",
                        "  /exit     - Exit MatrixCode\n",
                        "  /clear    - Clear messages\n",
                        "  /history  - Show session history\n",
                        "  /mode     - Change approve mode (ask/auto/strict)\n",
                        "  /model    - Show/change model\n",
                        "  /compact  - Compress context\n",
                        "  /retry    - Retry last queued message\n",
                        "  /new      - Start new session\n",
                        "  /init     - Initialize/reset project\n",
                        "  /skills   - List loaded skills\n",
                        "  /memory   - View/manage memories\n",
                        "  /overview - View project overview\n",
                        "  /save     - Save current session\n",
                        "  /sessions - List saved sessions\n",
                        "  /load <id>- Load a session\n",
                        "  /debug    - Toggle debug mode\n",
                        "  /loop     - Start/stop loop task\n",
                        "  /cron     - Manage scheduled tasks\n",
                        "\n",
                        "⌨️ Shortcuts:\n",
                        "  Enter=send │ Shift+Enter=newline │ PgUp/PgDn=scroll\n",
                        "  Home/End=top/bot │ Alt+M=mode │ Alt+T=thinking\n",
                        "  Esc=interrupt │ Ctrl+D=exit"
                    ).into()
                });
                self.auto_scroll = true;
            }
            "/skills" => {
                // Send to backend for processing (shows loaded skills, not tools)
                self.tx.try_send("/skills".to_string()).ok();
                self.auto_scroll = true;
            }
            "/memory" => {
                // Send to backend for processing
                self.tx.try_send("/memory".to_string()).ok();
                self.auto_scroll = true;
            }
            "/overview" => {
                // Send to backend for processing
                self.tx.try_send("/overview".to_string()).ok();
                self.auto_scroll = true;
            }
            "/save" => {
                // Send to backend for processing
                self.tx.try_send("/save".to_string()).ok();
                self.auto_scroll = true;
            }
            "/sessions" | "/resume" => {
                // Send to backend for processing
                self.tx.try_send("/sessions".to_string()).ok();
                self.auto_scroll = true;
            }
            "/loop" => {
                if args.is_empty() {
                    self.messages.push(Message {
                        role: Role::System,
                        content: "/loop <message> [interval] [count] - Start loop\n/loop stop - Stop loop\n/loop status - Show status".into()
                    });
                } else if args[0] == "stop" {
                    // Take ownership to avoid borrow conflict
                    let task = self.loop_task.take();
                    if let Some(ref task) = task {
                        task.cancel_token.cancel();
                        self.messages.push(Message {
                            role: Role::System,
                            content: format!("✓ Loop stopped (executed {} times)", task.count)
                        });
                        self.loop_task = None;  // Already taken
                    } else {
                        self.messages.push(Message { role: Role::System, content: "No active loop".into() });
                    }
                } else if args[0] == "status" {
                    if let Some(ref task) = self.loop_task {
                        self.messages.push(Message {
                            role: Role::System,
                            content: format!(
                                "🔄 Loop active: '{}' every {}s, count {}{}",
                                truncate(&task.message, 30),
                                task.interval_secs,
                                task.count,
                                task.max_count.map(|m| format!(" (max {})", m)).unwrap_or_default()
                            )
                        });
                    } else {
                        self.messages.push(Message { role: Role::System, content: "No active loop".into() });
                    }
                } else {
                    // Start new loop: /loop "message" [interval] [max_count]
                    if self.loop_task.is_some() {
                        self.messages.push(Message { role: Role::System, content: "⚠️ Loop already active. Use /loop stop first".into() });
                    } else {
                        let message = args[0].to_string();
                        let interval_secs: u64 = args.get(1)
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(60);
                        let max_count: Option<u64> = args.get(2)
                            .and_then(|s| s.parse().ok());
                        
                        let cancel_token = CancellationToken::new();
                        self.loop_task = Some(LoopTask {
                            message: message.clone(),
                            interval_secs,
                            count: 0,
                            max_count,
                            cancel_token: cancel_token.clone(),
                        });
                        
                        // Spawn background task
                        let tx = self.tx.clone();
                        let msg = message.clone();
                        tokio::spawn(async move {
                            loop {
                                if cancel_token.is_cancelled() {
                                    break;
                                }
                                // Send message
                                tx.try_send(msg.clone()).ok();
                                // Wait interval
                                tokio::time::sleep(Duration::from_secs(interval_secs)).await;
                            }
                        });
                        
                        self.messages.push(Message {
                            role: Role::System,
                            content: format!(
                                "🔄 Loop started: '{}' every {}s{}",
                                truncate(&message, 30),
                                interval_secs,
                                max_count.map(|m| format!(" (max {})", m)).unwrap_or_default()
                            )
                        });
                    }
                }
                self.auto_scroll = true;
            }
            "/cron" => {
                if args.is_empty() {
                    self.messages.push(Message {
                        role: Role::System,
                        content: "/cron add <message> <minutes> - Add cron task\n/cron list - List tasks\n/cron remove <id> - Remove task\n/cron clear - Clear all".into()
                    });
                } else if args[0] == "list" {
                    if self.cron_tasks.is_empty() {
                        self.messages.push(Message { role: Role::System, content: "No cron tasks".into() });
                    } else {
                        let list: Vec<String> = self.cron_tasks.iter()
                            .map(|t| format!("#{}: '{}' every {}min", t.id, truncate(&t.message, 20), t.minute_interval))
                            .collect();
                        self.messages.push(Message {
                            role: Role::System,
                            content: format!("📋 Cron tasks:\n{}", list.join("\n"))
                        });
                    }
                } else if args[0] == "remove" || args[0] == "rm" {
                    let id: usize = args.get(1)
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                    if let Some(pos) = self.cron_tasks.iter().position(|t| t.id == id) {
                        let task = &self.cron_tasks[pos];
                        task.cancel_token.cancel();
                        self.cron_tasks.remove(pos);
                        self.messages.push(Message {
                            role: Role::System,
                            content: format!("✓ Cron task #{} removed", id)
                        });
                    } else {
                        self.messages.push(Message { role: Role::System, content: format!("Cron task #{} not found", id) });
                    }
                } else if args[0] == "clear" {
                    for task in &self.cron_tasks {
                        task.cancel_token.cancel();
                    }
                    let count = self.cron_tasks.len();
                    self.cron_tasks.clear();
                    self.messages.push(Message {
                        role: Role::System,
                        content: format!("✓ {} cron tasks cleared", count)
                    });
                } else if args[0] == "add" {
                    // /cron add "message" 5
                    if args.len() < 3 {
                        self.messages.push(Message {
                            role: Role::System,
                            content: "Usage: /cron add <message> <minutes>".into()
                        });
                    } else {
                        let message = args[1].to_string();
                        let minute_interval: u64 = args.get(2)
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(5);
                        
                        let id = self.cron_tasks.iter().map(|t| t.id).max().unwrap_or(0) + 1;
                        let cancel_token = CancellationToken::new();
                        
                        let task = CronTask {
                            id,
                            message: message.clone(),
                            minute_interval,
                            next_run: Instant::now() + Duration::from_secs(minute_interval * 60),
                            cancel_token: cancel_token.clone(),
                        };
                        
                        self.cron_tasks.push(task.clone());
                        
                        // Spawn background task
                        let tx = self.tx.clone();
                        let msg = message.clone();
                        let interval_secs = minute_interval * 60;
                        tokio::spawn(async move {
                            // Initial delay to next_run
                            tokio::time::sleep(Duration::from_secs(interval_secs)).await;
                            loop {
                                if cancel_token.is_cancelled() {
                                    break;
                                }
                                // Send message
                                tx.try_send(msg.clone()).ok();
                                // Wait interval
                                tokio::time::sleep(Duration::from_secs(interval_secs)).await;
                            }
                        });
                        
                        self.messages.push(Message {
                            role: Role::System,
                            content: format!("✓ Cron #{} added: '{}' every {}min", id, truncate(&message, 30), minute_interval)
                        });
                    }
                } else {
                    self.messages.push(Message {
                        role: Role::System,
                        content: "Unknown cron command. Use: add, list, remove, clear".into()
                    });
                }
                self.auto_scroll = true;
            }
            _ => {
                self.messages.push(Message {
                    role: Role::System,
                    content: format!("Unknown: {}. Type /help", command)
                });
                self.auto_scroll = true;
            }
        }
    }

    fn on_event(&mut self, e: AgentEvent) {
        match e.event_type {
            EventType::ThinkingStart => {
                self.activity = Activity::Thinking;
                self.thinking.clear();
            }
            EventType::ThinkingDelta => {
                if let Some(EventData::Thinking { delta, .. }) = e.data {
                    self.thinking.push_str(&delta);
                    self.activity = Activity::Thinking;
                }
            }
            EventType::ThinkingEnd => {
                if !self.thinking.is_empty() {
                    self.messages.push(Message { role: Role::Thinking, content: self.thinking.clone() });
                    self.thinking.clear();
                }
            }
            EventType::TextStart => {
                self.streaming.clear();
                self.activity = Activity::Thinking;
            }
            EventType::TextDelta => {
                if let Some(EventData::Text { delta }) = e.data {
                    self.streaming.push_str(&delta);
                    self.activity = Activity::Thinking;
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
                    self.activity_detail = extract_tool_detail(&name, input.as_ref());
                }
            }
            EventType::ToolResult => {
                if let Some(EventData::ToolResult { content, is_error, .. }) = e.data {
                    let tool_name = self.activity.label();
                    self.messages.push(Message {
                        role: Role::Tool { name: tool_name, is_error },
                        content: content  // Keep full content, draw.rs will summarize
                    });
                    self.tool_calls += 1;
                    self.activity = Activity::Thinking;
                    self.activity_detail.clear();
                }
            }
            EventType::SessionEnded => {
                // Flush remaining content
                if !self.streaming.is_empty() {
                    self.messages.push(Message { role: Role::Assistant, content: self.streaming.clone() });
                    self.streaming.clear();
                }
                if !self.thinking.is_empty() {
                    self.messages.push(Message { role: Role::Thinking, content: self.thinking.clone() });
                    self.thinking.clear();
                }

                // Process queue or go idle
                if !self.pending_messages.is_empty() {
                    let next_msg = self.pending_messages.remove(0);
                    self.messages.push(Message { role: Role::User, content: next_msg.clone() });
                    self.tx.try_send(next_msg).ok();
                    self.activity = Activity::Thinking;
                    self.auto_scroll = true;
                } else {
                    self.activity = Activity::Idle;
                }
                self.activity_detail.clear();
            }
            EventType::Error => {
                if let Some(EventData::Error { message, .. }) = e.data {
                    self.messages.push(Message { role: Role::System, content: format!("❌ Error: {}", message) });
                    self.streaming.clear();
                    self.thinking.clear();
                }
                self.activity = Activity::Idle;
                self.activity_detail.clear();
                
                // Check queue after error - user may want to retry
                if !self.pending_messages.is_empty() {
                    self.messages.push(Message {
                        role: Role::System,
                        content: format!("⚠️ Queue paused ({} messages). Send '/retry' to process.", self.pending_messages.len())
                    });
                }
            }
            EventType::Usage => {
                if let Some(EventData::Usage { input_tokens, output_tokens, cache_creation_input_tokens, cache_read_input_tokens }) = e.data {
                    self.tokens_in = input_tokens;
                    self.tokens_out = output_tokens;
                    self.session_total_out += output_tokens;
                    
                    // Update cache stats (only when actually reported by API)
                    // Note: DashScope doesn't support prompt caching, so these may always be 0
                    let cache_read = cache_read_input_tokens.unwrap_or(0);
                    let cache_created = cache_creation_input_tokens.unwrap_or(0);
                    
                    self.cache_read += cache_read;
                    self.cache_created += cache_created;
                    self.api_calls += 1;
                }
            }
            EventType::CompressionCompleted => {
                if let Some(EventData::Compression { original_tokens, compressed_tokens, ratio }) = e.data {
                    self.compressions += 1;
                    // Update token display to reflect compressed state
                    self.tokens_in = compressed_tokens;
                    self.messages.push(Message {
                        role: Role::System,
                        content: format!("📦 Compressed: {} → {} tokens ({:.0}% saved)\n  Context: {} tokens remaining",
                            fmt_tokens(original_tokens), fmt_tokens(compressed_tokens), (1.0 - ratio) * 100.0,
                            fmt_tokens(compressed_tokens))
                    });
                    self.auto_scroll = true;
                }
            }
            EventType::CompressionTriggered => {
                if let Some(EventData::Progress { message, .. }) = e.data {
                    self.messages.push(Message {
                        role: Role::System,
                        content: format!("⏳ {}", message)
                    });
                    self.auto_scroll = true;
                }
            }
            EventType::Progress => {
                if let Some(EventData::Progress { message, .. }) = e.data {
                    self.messages.push(Message {
                        role: Role::System,
                        content: message
                    });
                    self.auto_scroll = true;
                }
            }
            EventType::MemoryLoaded => {
                if let Some(EventData::Memory { entries_count, .. }) = e.data
                    && entries_count > 0 {
                    self.memory_saves += 1;
                    self.messages.push(Message {
                        role: Role::System,
                        content: format!("🧠 Memory: {} entries", entries_count)
                    });
                    self.auto_scroll = true;
                }
            }
            EventType::MemoryDetected => {
                if let Some(EventData::Memory { summary, entries_count }) = e.data {
                    self.memory_saves += 1;
                    self.messages.push(Message {
                        role: Role::System,
                        content: format!("🧠 Detected {} memories: {}", entries_count, summary)
                    });
                    self.auto_scroll = true;
                }
            }
            EventType::KeywordsExtracted => {
                // Keywords extraction is an internal operation, don't show to user
                // Only update internal state if needed (for debug mode, could show)
                if self.debug_mode {
                    if let Some(EventData::Keywords { keywords, source }) = e.data {
                        let preview = truncate(&source, 50);
                        self.messages.push(Message {
                            role: Role::System,
                            content: format!("🔍 Keywords: {} from '{}'", keywords.join(", "), preview)
                        });
                    }
                }
            }
            EventType::AskQuestion => {
                if let Some(EventData::AskQuestion { question, options }) = e.data {
                    // Detect if this is an approval request
                    let is_approval = question.contains("requires approval") || question.contains("Allow?");
                    let has_options = options.is_some();
                    
                    // Format the display
                    let mut content = if is_approval {
                        // Extract tool name and risk level
                        let lines: Vec<&str> = question.lines().collect();
                        let header = lines.first().copied().unwrap_or("");
                        let detail = lines.get(1).copied().unwrap_or("");
                        format!("{}\n{}", header, detail)
                    } else if has_options {
                        format!("❓ {}", question)
                    } else {
                        question.clone()
                    };
                    
                    // Show options if available
                    if let Some(ref opts) = options && let Some(arr) = opts.as_array() {
                        content.push_str("\n\nOptions:");
                        for opt in arr {
                            let id = opt["id"].as_str().unwrap_or("?");
                            let label = opt["label"].as_str().unwrap_or("");
                            let desc = opt["description"].as_str().unwrap_or("");
                            let desc_text = if desc.is_empty() { String::new() } else { format!("({})", desc) };
                            content.push_str(&format!("\n  [{}] {} {}", id, label, desc_text));
                        }
                    }
                    
                    // Add hint for approval
                    if is_approval {
                        content.push_str("\n\n[y] Approve  [n] Reject  [a] Abort");
                    } else if has_options {
                        content.push_str("\n\nEnter option ID (e.g., 'A' or 'B')");
                    }
                    
                    self.messages.push(Message { role: Role::System, content });
                    self.waiting_for_ask = true;
                    self.activity = Activity::Asking;
                    self.auto_scroll = true;
                }
            }
            _ => {}
        }
    }
}