//! Input handling helpers for TuiApp
//!
//! Splits the large on_key method into smaller, focused handlers.

use ratatui::crossterm::event::KeyModifiers;

use crate::app::TuiApp;
use crate::types::{Activity, Message, Role};

impl TuiApp {
    /// Handle Enter key press
    pub(crate) fn handle_enter(&mut self, modifiers: KeyModifiers) {
        if modifiers.contains(KeyModifiers::SHIFT) {
            // Shift+Enter: insert newline
            self.ensure_char_boundary();
            self.input.insert(self.cursor_pos, '\n');
            self.cursor_pos += 1;
            self.multiline_confirm_send = false;
        } else if self.activity == Activity::Asking && self.waiting_for_ask {
            self.handle_ask_enter();
        } else if !self.input.trim().is_empty() {
            if self.input.contains('\n') && !self.multiline_confirm_send {
                self.multiline_confirm_send = true;
            } else {
                self.multiline_confirm_send = false;
                self.send_input();
            }
        } else {
            self.multiline_confirm_send = false;
        }
    }

    /// Handle Escape key press
    pub(crate) fn handle_escape(&mut self, modifiers: KeyModifiers) {
        if modifiers.contains(KeyModifiers::SHIFT) {
            // Shift+Esc: remove first pending message
            if !self.pending_messages.is_empty() {
                let removed = self.pending_messages.remove(0);
                self.push_message(Message {
                    role: Role::System,
                    content: format!("🗑️ Removed from queue: {}", crate::utils::truncate(&removed, 50)),
                });
            }
        } else if self.multiline_confirm_send {
            self.multiline_confirm_send = false;
        } else if self.ask_other_input_active {
            self.ask_other_input_active = false;
            self.input.clear();
            self.cursor_pos = 0;
            for opt in &mut self.ask_options {
                if opt.is_other { opt.selected = false; }
            }
        } else if self.activity == Activity::Asking {
            self.waiting_for_ask = false;
            self.activity = Activity::Idle;
            self.push_message(Message { role: Role::System, content: "⚠️ 已取消".into() });
            if let Some(ask_tx) = &self.ask_tx { ask_tx.try_send("abort".to_string()).ok(); }
        } else if self.activity != Activity::Idle {
            self.interrupt_activity();
        } else if !self.input.is_empty() {
            self.input.clear();
            self.cursor_pos = 0;
        }
    }

    /// Handle Ctrl+C interrupt
    pub(crate) fn handle_ctrl_c(&mut self) {
        if self.activity != Activity::Idle {
            self.interrupt_activity();
        }
    }

    /// Handle Ctrl+D exit
    pub(crate) fn handle_ctrl_d(&mut self) {
        self.exit = true;
    }

    /// Handle paste from clipboard
    pub(crate) fn handle_paste(&mut self) {
        if let Ok(mut clipboard) = arboard::Clipboard::new()
            && let Ok(text) = clipboard.get_text()
        {
            self.on_paste(&text);
        }
    }

    /// Handle backspace key
    pub(crate) fn handle_backspace(&mut self) {
        if self.cursor_pos > 0 {
            let prev_pos = self.prev_char_boundary();
            self.input.drain(prev_pos..self.cursor_pos);
            self.cursor_pos = prev_pos;
        }
    }

    /// Handle delete key
    pub(crate) fn handle_delete(&mut self) {
        if self.cursor_pos < self.input.len() {
            let next_pos = self.next_char_boundary();
            self.input.drain(self.cursor_pos..next_pos);
        }
    }

    /// Handle left arrow
    pub(crate) fn handle_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos = self.prev_char_boundary();
        }
    }

    /// Handle right arrow
    pub(crate) fn handle_right(&mut self) {
        if self.cursor_pos < self.input.len() {
            self.cursor_pos = self.next_char_boundary();
        }
    }

    /// Handle Home key
    pub(crate) fn handle_home(&mut self) {
        if !self.input.is_empty() {
            self.cursor_pos = 0;
        } else {
            self.auto_scroll = false;
            self.scroll_offset = 0;
        }
    }

    /// Handle End key
    pub(crate) fn handle_end(&mut self) {
        if !self.input.is_empty() {
            self.cursor_pos = self.input.len();
        } else {
            self.auto_scroll = true;
            self.scroll_offset = 0;
            self.new_message_while_scrolled.set(false);
        }
    }

    /// Handle character input
    pub(crate) fn handle_char(&mut self, c: char) {
        self.ensure_char_boundary();
        self.input.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
        if self.history_index.is_some() {
            self.history_index = None;
            self.history_draft.clear();
        }
    }

    /// Interrupt current activity
    fn interrupt_activity(&mut self) {
        self.activity = Activity::Idle;
        self.streaming.clear();
        self.thinking.clear();
        self.activity_input = None;
        self.activity_detail.clear();
        self.cancel.cancel();
        self.push_message(Message { role: Role::System, content: "⚡ 已中断".into() });
    }
}