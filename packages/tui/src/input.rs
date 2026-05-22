use std::time::Instant;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::app::TuiApp;
use crate::types::{Activity, ApproveMode, AskOption, Message, Role, SubmitMode};

impl TuiApp {
    pub(crate) fn on_key(&mut self, k: KeyEvent) {
        if k.kind != KeyEventKind::Press {
            return;
        }

        match k.code {
            // Enter: send or newline
            KeyCode::Enter => {
                if k.modifiers.contains(KeyModifiers::SHIFT) {
                    // Shift+Enter: insert newline at cursor position
                    self.ensure_char_boundary();
                    self.input.insert(self.cursor_pos, '\n');
                    self.cursor_pos += 1; // '\n' is 1 byte
                } else if self.activity == Activity::Asking && self.waiting_for_ask {
                    // Handle ask confirmation or toggle selection in multi-select
                    self.handle_ask_enter();
                } else if !self.input.trim().is_empty() {
                    self.send_input();
                }
            }

            // Tab: switch between multiple questions or toggle approve mode
            KeyCode::Tab if !k.modifiers.contains(KeyModifiers::SHIFT) => {
                if self.activity == Activity::Asking
                    && self.waiting_for_ask
                    && self.ask_questions.len() > 1
                {
                    self.switch_to_next_question();
                }
            }

            // Escape: interrupt or clear input
            KeyCode::Esc => {
                // If in "Other" input mode, return to selection mode
                if self.ask_other_input_active {
                    self.ask_other_input_active = false;
                    self.input.clear();
                    self.cursor_pos = 0;
                    // Uncheck the "Other" option if it was checked
                    for opt in &mut self.ask_options {
                        if opt.is_other {
                            opt.selected = false;
                        }
                    }
                    return;
                }

                if self.activity == Activity::Asking {
                    // Abort approval request
                    self.waiting_for_ask = false;
                    self.activity = Activity::Idle;
                    self.messages.push(Message {
                        role: Role::System,
                        content: "⚠️ 已取消".into(),
                    });
                    if let Some(ask_tx) = &self.ask_tx {
                        ask_tx.try_send("abort".to_string()).ok();
                    }
                } else if self.activity != Activity::Idle {
                    // Signal cancellation - backend will respond with Error event
                    // The events.rs handler will then process queue
                    self.cancel.cancel();
                    self.messages.push(Message {
                        role: Role::System,
                        content: "⚡ 正在中断...".into(),
                    });
                } else {
                    self.input.clear();
                    self.cursor_pos = 0;
                }
            }

            // Ctrl+C: interrupt
            KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.activity != Activity::Idle {
                    self.cancel.cancel();
                    self.messages.push(Message {
                        role: Role::System,
                        content: "⚡ 正在中断...".into(),
                    });
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
                    && let Ok(text) = clipboard.get_text()
                {
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

            // Space: toggle selection in multi-select mode or insert space
            KeyCode::Char(' ')
                if !k.modifiers.contains(KeyModifiers::ALT)
                    && !k.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if self.activity == Activity::Asking
                    && self.waiting_for_ask
                    && self.ask_multi_select
                    && !self.ask_options.is_empty()
                    && !self.ask_other_input_active
                {
                    // Toggle current selection (only when not in Other input mode)
                    self.toggle_ask_selection();
                } else {
                    // Normal input: insert space
                    self.ensure_char_boundary();
                    self.input.insert(self.cursor_pos, ' ');
                    self.cursor_pos += 1;
                    if self.history_index.is_some() {
                        self.history_index = None;
                        self.history_draft.clear();
                    }
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

            // Up arrow: ask selection, history navigation, or multiline cursor
            KeyCode::Up if !k.modifiers.contains(KeyModifiers::ALT) => {
                // If in "Other" input mode, allow multiline navigation
                if self.ask_other_input_active && self.input.contains('\n') {
                    let (current_line_num, col_chars, _) = self.get_line_info();
                    if current_line_num > 1 {
                        let char_pos = self.byte_pos_to_char_pos();
                        let input_chars: Vec<char> = self.input.chars().collect();
                        let before_cursor_str: String = input_chars
                           [..char_pos.min(input_chars.len())]
                            .iter()
                            .collect();

                        // Previous line is before the last '\n' in before_cursor_str
                        let prev_lines_str =
                            &before_cursor_str[..before_cursor_str.rfind('\n').unwrap_or(0)];
                        let prev_line_start_char = prev_lines_str.chars().count();

                        // Find previous line length
                        let prev_line_end_char =
                            char_pos.saturating_sub(col_chars).saturating_sub(1); // -1 for the newline
                        let prev_line_len_chars =
                            prev_line_end_char.saturating_sub(prev_line_start_char);

                        // Move to same column (or end if shorter)
                        let target_char_pos =
                            prev_line_start_char + col_chars.min(prev_line_len_chars);
                        self.cursor_pos = self.char_pos_to_byte_pos(target_char_pos);
                    }
                } else if self.activity == Activity::Asking
                    && self.waiting_for_ask
                    && !self.ask_options.is_empty()
                    && !self.ask_other_input_active
                {
                    // Ask selection (only when not in Other input mode)
                    if self.ask_selected_index > 0 {
                        self.ask_selected_index -= 1;
                    }
                } else if self.input.contains('\n') {
                    let (current_line_num, col_chars, _) = self.get_line_info();
                    if current_line_num > 1 {
                        let char_pos = self.byte_pos_to_char_pos();
                        let input_chars: Vec<char> = self.input.chars().collect();
                        let before_cursor_str: String = input_chars
                            [..char_pos.min(input_chars.len())]
                            .iter()
                            .collect();

                        // Previous line is before the last '\n' in before_cursor_str
                        let prev_lines_str =
                            &before_cursor_str[..before_cursor_str.rfind('\n').unwrap_or(0)];
                        let prev_line_start_char = prev_lines_str.chars().count();

                        // Find previous line length
                        let prev_line_end_char =
                            char_pos.saturating_sub(col_chars).saturating_sub(1); // -1 for the newline
                        let prev_line_len_chars =
                            prev_line_end_char.saturating_sub(prev_line_start_char);

                        // Move to same column (or end if shorter)
                        let target_char_pos =
                            prev_line_start_char + col_chars.min(prev_line_len_chars);
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

            // Down arrow: ask selection, history navigation, or multiline cursor
            KeyCode::Down if !k.modifiers.contains(KeyModifiers::ALT) => {
                // If in "Other" input mode, allow multiline navigation
                if self.ask_other_input_active && self.input.contains('\n') {
                    let (current_line_num, col_chars, total_lines) = self.get_line_info();
                    if current_line_num < total_lines {
                        let char_pos = self.byte_pos_to_char_pos();
                        let input_chars: Vec<char> = self.input.chars().collect();

                        // Boundary check: char_pos must not exceed input_chars.len()
                        let safe_char_pos = char_pos.min(input_chars.len());

                        // Find next line start
                        let remaining_chars = &input_chars[safe_char_pos..];
                        let next_line_start_char = remaining_chars
                            .iter()
                            .position(|c| *c == '\n')
                            .map(|i| safe_char_pos + i + 1)
                            .unwrap_or_else(|| input_chars.len());

                        // Find next line end
                        let next_line_chars = &input_chars[next_line_start_char..];
                        let next_line_end_char = next_line_chars
                            .iter()
                            .position(|c| *c == '\n')
                            .map(|i| next_line_start_char + i)
                            .unwrap_or_else(|| input_chars.len());

                        let next_line_len_chars =
                            next_line_end_char.saturating_sub(next_line_start_char);

                        // Move to same column (or end if shorter)
                        let target_char_pos =
                            next_line_start_char + col_chars.min(next_line_len_chars);
                        self.cursor_pos = self.char_pos_to_byte_pos(target_char_pos);
                    }
                } else if self.activity == Activity::Asking
                    && self.waiting_for_ask
                    && !self.ask_options.is_empty()
                    && !self.ask_other_input_active
                {
                    // Ask selection (only when not in Other input mode)
                    if self.ask_selected_index < self.ask_options.len() - 1 {
                        self.ask_selected_index += 1;
                    }
                } else if self.input.contains('\n') {
                    let (current_line_num, col_chars, total_lines) = self.get_line_info();
                    if current_line_num < total_lines {
                        let char_pos = self.byte_pos_to_char_pos();
                        let input_chars: Vec<char> = self.input.chars().collect();

                        // Boundary check: char_pos must not exceed input_chars.len()
                        let safe_char_pos = char_pos.min(input_chars.len());

                        // Find next line start
                        let remaining_chars = &input_chars[safe_char_pos..];
                        let next_line_start_char = remaining_chars
                            .iter()
                            .position(|c| *c == '\n')
                            .map(|i| safe_char_pos + i + 1)
                            .unwrap_or_else(|| input_chars.len());

                        // Find next line end
                        let next_line_chars = &input_chars[next_line_start_char..];
                        let next_line_end_char = next_line_chars
                            .iter()
                            .position(|c| *c == '\n')
                            .map(|i| next_line_start_char + i)
                            .unwrap_or_else(|| input_chars.len());

                        let next_line_len_chars =
                            next_line_end_char.saturating_sub(next_line_start_char);

                        // Move to same column (or end if shorter)
                        let target_char_pos =
                            next_line_start_char + col_chars.min(next_line_len_chars);
                        self.cursor_pos = self.char_pos_to_byte_pos(target_char_pos);
                    }
                } else if let Some(idx) = self.history_index {
                    // Single-line: browse history forward
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
            KeyCode::Char(c)
                if !k.modifiers.contains(KeyModifiers::ALT)
                    && !k.modifiers.contains(KeyModifiers::CONTROL) =>
            {
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
            self.cursor_pos = self
                .input
                .char_indices()
                .rfind(|(i, _)| *i <= self.cursor_pos)
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    /// Sync approve_mode to the shared atomic and notify agent task.
    /// If switching to Auto and there's a pending approval, auto-approve it.
    pub(crate) fn sync_approve_mode(&mut self) {
        if let Some(ref shared) = self.shared_approve_mode {
            shared.store(
                self.approve_mode.to_u8(),
                std::sync::atomic::Ordering::Relaxed,
            );
        }
        // If switching to auto and agent is waiting for approval, auto-approve
        if self.approve_mode == ApproveMode::Auto
            && self.waiting_for_ask
            && let Some(ref ask_tx) = self.ask_tx
        {
            ask_tx.try_send("y".to_string()).ok();
            self.waiting_for_ask = false;
        }
        self.tx
            .try_send(format!("/mode:{}", self.approve_mode))
            .ok();
    }

    /// Find the byte position of the previous character boundary.
    /// Returns 0 if cursor is at the start.
    fn prev_char_boundary(&self) -> usize {
        self.input
            .char_indices()
            .rfind(|(i, _)| *i < self.cursor_pos)
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    /// Find the byte position of the next character boundary.
    /// Returns input.len() if cursor is at the end.
    fn next_char_boundary(&self) -> usize {
        self.input
            .char_indices()
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
        self.input
            .char_indices()
            .nth(char_pos)
            .map(|(i, _)| i)
            .unwrap_or_else(|| self.input.len())
    }

    /// Get current line info: (current_line_number, column_in_chars, total_lines)
    fn get_line_info(&self) -> (usize, usize, usize) {
        let before_cursor = &self.input[..self.cursor_pos];
        let current_line_num = before_cursor.matches('\n').count() + 1;
        let total_lines = self.input.lines().count().max(1);
        let col_chars = before_cursor
            .rfind('\n')
            .map(|i| before_cursor[i + 1..].chars().count())
            .unwrap_or_else(|| before_cursor.chars().count());
        (current_line_num, col_chars, total_lines)
    }

    pub(crate) fn send_input(&mut self) {
        self.show_welcome = false;
        let input = self.input.trim().to_string();
        self.input.clear();
        self.cursor_pos = 0;

        // Save to input history (skip duplicates of last entry)
        if !input.is_empty() && self.input_history.last().map(|s| s.as_str()) != Some(&input) {
            self.input_history.push(input.clone());
        }
        // Reset history browsing state
        self.history_index = None;
        self.history_draft.clear();

        if self.waiting_for_ask {
            // Respond to approval/ask question
            self.waiting_for_ask = false;
            self.messages.push(Message {
                role: Role::User,
                content: input.clone(),
            });
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
            self.messages.push(Message {
                role: Role::User,
                content: input.clone(),
            });
            self.tx.try_send(input).ok();
            self.activity = Activity::Thinking;
            self.request_start = Some(Instant::now());
            self.auto_scroll = true;
        } else {
            // Queue message (AI is processing)
            self.pending_messages.push(input.clone());
        }
    }

    /// Handle Enter key in Ask mode - toggle selection in multi-select or confirm
    fn handle_ask_enter(&mut self) {
        // If in "Other" input mode, send the custom text
        if self.ask_other_input_active {
            self.confirm_ask_selection();
            return;
        }

        // Check if current selection is "Other" option (single-select)
        if !self.ask_multi_select && !self.ask_options.is_empty() {
            let current_idx = self.ask_selected_index;
            if current_idx < self.ask_options.len() && self.ask_options[current_idx].is_other {
                // Enter "Other" input mode - user will type custom content
                self.ask_other_input_active = true;
                self.input.clear();
                self.cursor_pos = 0;
                return;
            }
        }

        if self.ask_multi_select && !self.ask_options.is_empty() {
            // Multi-select: toggle current selection
            self.toggle_ask_selection();
        } else {
            // Single-select: confirm
            self.confirm_ask_selection();
        }
    }

    /// Toggle current selection in multi-select mode (used by Space and Enter)
    fn toggle_ask_selection(&mut self) {
        if !self.ask_multi_select || self.ask_options.is_empty() {
            return;
        }

        let current_idx = self.ask_selected_index;
        if current_idx < self.ask_options.len() {
            let is_submit = self.ask_options[current_idx].is_submit;
            let is_other = self.ask_options[current_idx].is_other;
            // Toggle the selection
            self.ask_options[current_idx].selected = !self.ask_options[current_idx].selected;

            // If "Other" was just checked, enter text input mode
            if is_other && self.ask_options[current_idx].selected {
                self.ask_other_input_active = true;
                self.input.clear();
                self.cursor_pos = 0;
                return;
            }

            // If "Other" was just unchecked, exit text input mode
            if is_other && !self.ask_options[current_idx].selected {
                self.ask_other_input_active = false;
                self.input.clear();
                self.cursor_pos = 0;
                return;
            }

            // If Submit was just checked, confirm immediately in Option mode
            if is_submit
                && self.ask_options[current_idx].selected
                && self.ask_submit_mode == SubmitMode::Option
            {
                self.confirm_ask_selection();
            }
        }
    }

    /// Switch to next question in multi-question mode
    fn switch_to_next_question(&mut self) {
        if self.ask_questions.len() <= 1 {
            return;
        }

        // Save current question state
        self.save_current_question_state();

        // Move to next question
        self.current_question_idx = (self.current_question_idx + 1) % self.ask_questions.len();

        // Load next question state
        self.load_question_state();

        // Update the Ask message content to show new question
        self.update_ask_message_for_current_question();
    }

    /// Update the Ask message to display current question
    fn update_ask_message_for_current_question(&mut self) {
        if self.current_question_idx >= self.ask_questions.len() {
            return;
        }

        let q = &self.ask_questions[self.current_question_idx];
        let mut content = String::new();

        content.push_str("╔══════════════════════════════════════╗\n");
        content.push_str(&format!(
            "║  ⚡ 问题 {} / {} (Tab切换) ⚡        ║\n",
            self.current_question_idx + 1,
            self.ask_questions.len()
        ));
        content.push_str("╚══════════════════════════════════════╝\n\n");
        content.push_str(&q.question);

        // Add Submit option for Option mode if needed
        let mut display_options = q.options.clone();
        if q.multi_select && q.submit_mode == SubmitMode::Option {
            display_options.push(AskOption {
                id: "__submit__".into(),
                label: "✓ 提交".into(),
                description: Some("确认选择并提交".into()),
                selected: false,
                is_submit: true,
                is_other: false,
            });
        }

        content.push_str("\n\n─────────────────────────────────────\n");
        if q.multi_select {
            match q.submit_mode {
                SubmitMode::Direct => {
                    if self.current_question_idx < self.ask_questions.len() - 1 {
                        content.push_str("选项 (↑↓导航 Space/Enter切换 Enter下一题):\n");
                    } else {
                        content.push_str("选项 (↑↓导航 Space/Enter切换 Enter提交):\n");
                    }
                }
                SubmitMode::Option => {
                    content.push_str("选项 (↑↓导航 Space/Enter切换 选中[✓提交]):\n")
                }
                SubmitMode::Button => {
                    content.push_str("选项 (↑↓导航 Space/Enter切换 Enter提交):\n")
                }
            }
        } else {
            if self.current_question_idx < self.ask_questions.len() - 1 {
                content.push_str("选项 (↑↓选择 Enter下一题):\n");
            } else {
                content.push_str("选项 (↑↓选择 Enter提交):\n");
            }
        }

        for (i, opt) in display_options.iter().enumerate() {
            if opt.is_submit {
                // Submit option also shows as checkbox
                let marker = if opt.selected { "[✓]" } else { "[ ]" };
                content.push_str(&format!(
                    "  {} {}{}\n",
                    marker,
                    opt.label,
                    opt.format_description()
                ));
            } else {
                let marker = if q.multi_select {
                    if opt.selected {
                        "[✓]".to_string()
                    } else {
                        "[ ]".to_string()
                    }
                } else {
                    format!("[{}]", (b'A' + i as u8) as char)
                };
                content.push_str(&format!(
                    "  {} {}{}\n",
                    marker,
                    opt.label,
                    opt.format_description()
                ));
            }
        }

        // Update the last Ask message
        if let Some(last_msg) = self.messages.last_mut()
            && last_msg.role == Role::Ask
        {
            last_msg.content = content;
        }
    }

    /// Save current question state to ask_questions
    fn save_current_question_state(&mut self) {
        if self.current_question_idx < self.ask_questions.len() {
            let q = &mut self.ask_questions[self.current_question_idx];
            // Save options excluding the Submit option (it's dynamically added)
            q.options = self
                .ask_options
                .iter()
                .filter(|opt| !opt.is_submit)
                .cloned()
                .collect();
            q.selected_index = self
                .ask_selected_index
                .min(q.options.len().saturating_sub(1));
            q.multi_select = self.ask_multi_select;
            q.submit_mode = self.ask_submit_mode.clone();
            // Save "Other" input state
            if self.ask_other_input_active && !self.input.trim().is_empty() {
                q.other_input = Some(self.input.clone());
            }
        }
    }

    /// Load question state from ask_questions[current_idx]
    fn load_question_state(&mut self) {
        if self.current_question_idx < self.ask_questions.len() {
            let q = &self.ask_questions[self.current_question_idx];
            self.ask_options = q.options.clone();
            self.ask_selected_index = q.selected_index;
            self.ask_multi_select = q.multi_select;
            self.ask_submit_mode = q.submit_mode.clone();

            // Restore "Other" input state
            if q.other_input.is_some() {
                self.ask_other_input_active = true;
                self.input = q.other_input.clone().unwrap_or_default();
                self.cursor_pos = self.input.len();
            } else {
                self.ask_other_input_active = false;
                self.input.clear();
                self.cursor_pos = 0;
            }

            // Add Submit option for Option mode if needed
            if self.ask_multi_select && self.ask_submit_mode == SubmitMode::Option {
                self.ask_options.push(AskOption {
                    id: "__submit__".into(),
                    label: "提交".into(),
                    description: Some("确认并提交所有选择".into()),
                    selected: false,
                    is_submit: true,
                    is_other: false,
                });
            }
        }
    }

    /// Confirm ask selection - send selected option(s) or custom input
    pub(crate) fn confirm_ask_selection(&mut self) {
        if !self.waiting_for_ask {
            return;
        }

        // Handle "Other" input mode - send custom text
        if self.ask_other_input_active {
            if self.input.trim().is_empty() {
                // Empty input in Other mode - don't submit, stay in input mode
                return;
            }
            let custom_text = self.input.trim().to_string();
            // Find the "Other" option and get its id
            let other_id = self
                .ask_options
                .iter()
                .find(|opt| opt.is_other)
                .map(|opt| opt.id.clone())
                .unwrap_or("other".to_string());

            // In multi-select, include other_id with custom text
            if self.ask_multi_select {
                let mut selected_ids: Vec<String> = self
                    .ask_options
                    .iter()
                    .filter(|opt| opt.selected && !opt.is_submit && !opt.is_other)
                    .map(|opt| opt.id.clone())
                    .collect();
                selected_ids.push(other_id);

                let response = serde_json::to_string(&selected_ids).unwrap_or_else(|_| "[]".to_string());
                let mut display_labels: Vec<&str> = self
                    .ask_options
                    .iter()
                    .filter(|opt| opt.selected && !opt.is_submit && !opt.is_other)
                    .map(|opt| opt.label.as_str())
                    .collect();
                display_labels.push(&custom_text);
                let display_response = display_labels.join(", ");

                self.waiting_for_ask = false;
                self.activity = Activity::Thinking;
                self.auto_scroll = true;
                self.input.clear();
                self.cursor_pos = 0;
                self.ask_options.clear();
                self.ask_selected_index = 0;
                self.ask_multi_select = false;
                self.ask_submit_mode = SubmitMode::default();
                self.ask_other_input_active = false;

                self.messages.push(Message {
                    role: Role::User,
                    content: display_response,
                });
                if let Some(ask_tx) = &self.ask_tx {
                    ask_tx.try_send(response).ok();
                }
                return;
            } else {
                // Single-select with "Other" - send custom text directly
                self.waiting_for_ask = false;
                self.activity = Activity::Thinking;
                self.auto_scroll = true;

                self.messages.push(Message {
                    role: Role::User,
                    content: custom_text.clone(),
                });
                if let Some(ask_tx) = &self.ask_tx {
                    ask_tx.try_send(custom_text).ok();
                }

                self.input.clear();
                self.cursor_pos = 0;
                self.ask_options.clear();
                self.ask_selected_index = 0;
                self.ask_multi_select = false;
                self.ask_submit_mode = SubmitMode::default();
                self.ask_other_input_active = false;
                return;
            }
        }

        // In Option submit mode, only submit when Submit option is selected (checked)
        if self.ask_submit_mode == SubmitMode::Option && !self.ask_options.is_empty() {
            // Find the Submit option
            let submit_selected = self
                .ask_options
                .iter()
                .find(|opt| opt.is_submit)
                .map(|opt| opt.selected)
                .unwrap_or(false);

            if !submit_selected {
                // Submit not selected, don't submit
                return;
            }
        }

        // Multi-question mode: check if all questions answered
        if self.ask_questions.len() > 1 {
            // Save current question state first
            self.save_current_question_state();

            // Check if we're at the last question
            if self.current_question_idx < self.ask_questions.len() - 1 {
                // Not at last question, switch to next
                self.switch_to_next_question();
                return;
            }

            // At last question - collect all answers and submit
            let answers: std::collections::HashMap<String, serde_json::Value> = self
                .ask_questions
                .iter()
                .map(|q| {
                    let answer = if q.multi_select && !q.options.is_empty() {
                        // Multi-select: collect selected ids
                        let selected_ids: Vec<&str> = q
                            .options
                            .iter()
                            .filter(|opt| opt.selected && !opt.is_submit)
                            .map(|opt| opt.id.as_str())
                            .collect();
                        serde_json::json!(selected_ids)
                    } else if !q.options.is_empty() {
                        // Single select: use selected index
                        serde_json::json!(
                            q.options
                                .get(q.selected_index)
                                .map(|o| o.id.clone())
                                .unwrap_or_default()
                        )
                    } else {
                        serde_json::json!("")
                    };
                    (q.id.clone(), answer)
                })
                .collect();

            let response = serde_json::to_string(&answers).unwrap_or_else(|_| "{}".to_string());
            let display_response = format!("已回答 {} 个问题", self.ask_questions.len());

            // Clear state
            self.waiting_for_ask = false;
            self.activity = Activity::Thinking;
            self.auto_scroll = true;
            self.input.clear();
            self.cursor_pos = 0;
            self.ask_options.clear();
            self.ask_selected_index = 0;
            self.ask_multi_select = false;
            self.ask_submit_mode = SubmitMode::default();
            self.ask_other_input_active = false;
            self.ask_questions.clear();
            self.current_question_idx = 0;

            // Send response
            self.messages.push(Message {
                role: Role::User,
                content: display_response,
            });
            if let Some(ask_tx) = &self.ask_tx {
                ask_tx.try_send(response).ok();
            }
            return;
        }

        // Single question mode
        self.waiting_for_ask = false;
        self.activity = Activity::Thinking;
        self.auto_scroll = true;

        // Determine response based on mode
        let (response, display_response) = if self.ask_multi_select && !self.ask_options.is_empty()
        {
            // Multi-select: collect all selected options (exclude Submit option)
            let selected_ids: Vec<&str> = self
                .ask_options
                .iter()
                .filter(|opt| opt.selected && !opt.is_submit)
                .map(|opt| opt.id.as_str())
                .collect();

            // Send as JSON array
            let response =
                serde_json::to_string(&selected_ids).unwrap_or_else(|_| "[]".to_string());

            // Display as comma-separated labels
            let display_labels: Vec<&str> = self
                .ask_options
                .iter()
                .filter(|opt| opt.selected && !opt.is_submit)
                .map(|opt| opt.label.as_str())
                .collect();
            let display = if display_labels.is_empty() {
                "未选择".to_string()
            } else {
                display_labels.join(", ")
            };

            (response, display)
        } else if !self.ask_options.is_empty() && self.input.trim().is_empty() {
            // Single select: use selected option's id
            let selected = &self.ask_options[self.ask_selected_index];
            let response = selected.id.clone();
            let display = selected.label.clone();
            (response, display)
        } else if !self.input.trim().is_empty() {
            // Custom text input
            let text = self.input.trim().to_string();
            (text.clone(), text)
        } else if !self.ask_options.is_empty() {
            // Default: first option
            let selected = &self.ask_options[0];
            let response = selected.id.clone();
            let display = selected.label.clone();
            (response, display)
        } else {
            ("y".to_string(), "同意".to_string()) // Default approval
        };

        // Clear input and options
        self.input.clear();
        self.cursor_pos = 0;
        self.ask_options.clear();
        self.ask_selected_index = 0;
        self.ask_multi_select = false;
        self.ask_submit_mode = SubmitMode::default();
        self.ask_other_input_active = false;

        // Send response
        self.messages.push(Message {
            role: Role::User,
            content: display_response,
        });
        if let Some(ask_tx) = &self.ask_tx {
            ask_tx.try_send(response).ok();
        }
    }

    pub(crate) fn on_paste(&mut self, text: &str) {
        self.ensure_char_boundary();
        self.input.insert_str(self.cursor_pos, text);
        self.cursor_pos += text.len(); // cursor_pos is byte position
    }
}
