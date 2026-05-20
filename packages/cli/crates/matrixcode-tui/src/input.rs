use std::time::Instant;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::types::{Activity, ApproveMode, Message, Role};
use crate::app::TuiApp;

impl TuiApp {
    pub(crate) fn on_key(&mut self, k: KeyEvent) {
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
                    // Signal cancellation - backend will respond with Error event
                    // The events.rs handler will then process queue
                    self.cancel.cancel();
                    self.messages.push(Message { role: Role::System, content: "⚡ Interrupting...".into() });
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
                        
                        // Copy to clipboard (silent)
                        let _ = clipboard_result;
                        self.selection = None;
                        self.selecting = false;
                    }
                } else if self.activity != Activity::Idle {
                    // Signal cancellation - backend will respond with Error event
                    self.cancel.cancel();
                    self.messages.push(Message { role: Role::System, content: "⚡ Interrupting...".into() });
                }
            }

            // Ctrl+D: exit
            KeyCode::Char('d') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                self.exit = true;
            }

            // Ctrl+V: paste from clipboard
            KeyCode::Char('v') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                // Try to get text from clipboard
                if let Ok(mut clipboard) = arboard::Clipboard::new()
                    && let Ok(text) = clipboard.get_text() {
                        self.on_paste(&text);
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

            // Up arrow: history navigation (single-line) or move cursor (multiline)
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
                } else if !self.input_history.is_empty() {
                    // Single-line: browse history
                    match self.history_index {
                        None => {
                            // Entering history mode: save current input as draft
                            self.history_draft = self.input.clone();
                            self.history_index = Some(self.input_history.len() - 1);
                            self.input = self.input_history[self.input_history.len() - 1].clone();
                        }
                        Some(idx) if idx > 0 => {
                            self.history_index = Some(idx - 1);
                            self.input = self.input_history[idx - 1].clone();
                        }
                        _ => {} // Already at oldest entry
                    }
                    self.cursor_pos = self.input.len();
                }
            }

            // Down arrow: history navigation (single-line) or move cursor (multiline)
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
                } else if self.history_index.is_some() {
                    // Single-line: browse history forward
                    let idx = self.history_index.unwrap();
                    if idx + 1 < self.input_history.len() {
                        self.history_index = Some(idx + 1);
                        self.input = self.input_history[idx + 1].clone();
                    } else {
                        // Back to draft (current unsent input)
                        self.history_index = None;
                        self.input = self.history_draft.clone();
                        self.history_draft.clear();
                    }
                    self.cursor_pos = self.input.len();
                }
            }

            // Regular character input (except when Alt/Ctrl is held)
            KeyCode::Char(c) if !k.modifiers.contains(KeyModifiers::ALT) && !k.modifiers.contains(KeyModifiers::CONTROL) => {
                self.ensure_char_boundary();
                self.input.insert(self.cursor_pos, c);
                self.cursor_pos += c.len_utf8();
                // Exit history browsing mode on any character input
                if self.history_index.is_some() {
                    self.history_index = None;
                    self.history_draft.clear();
                }
            }

            // Alt+M: toggle approve mode
            KeyCode::Char('m') if k.modifiers.contains(KeyModifiers::ALT) => {
                self.approve_mode = self.approve_mode.next();
                self.sync_approve_mode();
            }

            // Alt+T: toggle thinking collapse
            KeyCode::Char('t') if k.modifiers.contains(KeyModifiers::ALT) => {
                self.thinking_collapsed = !self.thinking_collapsed;
            }

            // Shift+Tab / BackTab: toggle approve mode
            KeyCode::Tab if k.modifiers.contains(KeyModifiers::SHIFT) => {
                self.approve_mode = self.approve_mode.next();
                self.sync_approve_mode();
            }
            KeyCode::BackTab => {
                self.approve_mode = self.approve_mode.next();
                self.sync_approve_mode();
            }

            // Scroll: PageUp
            KeyCode::PageUp => {
                if self.auto_scroll {
                    self.auto_scroll = false;
                    // Set to max_scroll or at least 50 to start from bottom
                    self.scroll_offset = self.max_scroll.get().max(50);
                }
                self.scroll_offset = self.scroll_offset.saturating_sub(10);
            }

            // Scroll: PageDown
            KeyCode::PageDown => {
                if !self.auto_scroll {
                    self.scroll_offset = self.scroll_offset.saturating_add(10);
                    let max = self.max_scroll.get();
                    if max > 0 && self.scroll_offset >= max {
                        self.auto_scroll = true;
                        self.scroll_offset = 0;
                    }
                }
            }

            // Scroll: Alt+Up (or Up when not idle)
            KeyCode::Up if k.modifiers.contains(KeyModifiers::ALT) => {
                if self.auto_scroll {
                    self.auto_scroll = false;
                    // Set to max_scroll or at least 50 to start from bottom
                    self.scroll_offset = self.max_scroll.get().max(50);
                }
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }

            // Scroll: Alt+Down (or Down when not idle)
            KeyCode::Down if k.modifiers.contains(KeyModifiers::ALT) => {
                if !self.auto_scroll {
                    self.scroll_offset = self.scroll_offset.saturating_add(1);
                    let max = self.max_scroll.get();
                    if max > 0 && self.scroll_offset >= max {
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
    pub(crate) fn ensure_char_boundary(&mut self) {
        if !self.input.is_char_boundary(self.cursor_pos) {
            self.cursor_pos = self.input.char_indices()
                .rfind(|(i, _)| *i <= self.cursor_pos)
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    /// Sync approve_mode to the shared atomic and notify agent task.
    /// If switching to Auto and there's a pending approval, auto-approve it.
    pub(crate) fn sync_approve_mode(&mut self) {
        if let Some(ref shared) = self.shared_approve_mode {
            shared.store(self.approve_mode.to_u8(), std::sync::atomic::Ordering::Relaxed);
        }
        // If switching to auto and agent is waiting for approval, auto-approve
        if self.approve_mode == ApproveMode::Auto && self.waiting_for_ask
            && let Some(ref ask_tx) = self.ask_tx {
                ask_tx.try_send("y".to_string()).ok();
                self.waiting_for_ask = false;
            }
        self.tx.try_send(format!("/mode:{}", self.approve_mode.label())).ok();
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
        self.input[..self.cursor_pos].chars().count()
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
        let before_cursor = &self.input[..self.cursor_pos];
        let current_line_num = before_cursor.matches('\n').count() + 1;
        let total_lines = self.input.lines().count().max(1);
        let col_chars = before_cursor.rfind('\n')
            .map(|i| before_cursor[i+1..].chars().count())
            .unwrap_or_else(|| before_cursor.chars().count());
        (current_line_num, col_chars, total_lines)
    }

    pub(crate) fn send_input(&mut self) {
        self.show_welcome = false;
        let input = self.input.trim().to_string();
        self.input.clear();
        self.cursor_pos = 0;
        
        // Save to input history (skip duplicates of last entry)
        if !input.is_empty()
            && self.input_history.last().map(|s| s.as_str()) != Some(&input) {
                self.input_history.push(input.clone());
            }
        // Reset history browsing state
        self.history_index = None;
        self.history_draft.clear();

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
            self.request_start = Some(Instant::now());
            self.auto_scroll = true;
        } else {
            // Queue message (AI is processing)
            self.pending_messages.push(input.clone());
        }
    }

    pub(crate) fn on_paste(&mut self, text: &str) {
        self.ensure_char_boundary();
        self.input.insert_str(self.cursor_pos, text);
        self.cursor_pos += text.len();  // cursor_pos is byte position
    }
}
