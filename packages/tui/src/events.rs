use matrixcode_core::{AgentEvent, EventData, EventType};
use serde_json::Value;

use crate::app::{TuiApp, TodoItem};
use crate::types::{Activity, Message, Role, SubmitMode};
use crate::utils::{extract_tool_detail, fmt_tokens};

impl TuiApp {
    /// Push a message and set notification flag if user is scrolled up.
    pub(crate) fn push_message(&mut self, msg: Message) {
        // If user has scrolled up, mark that new message arrived
        if !self.auto_scroll {
            self.new_message_while_scrolled.set(true);
        }
        self.messages.push(msg);
    }

    /// Update todo items from todo_write tool input
    pub(crate) fn update_todo_items(&mut self, input: &Value) {
        if let Some(todos) = input.get("todos").and_then(|t| t.as_array()) {
            self.todo_items = todos
                .iter()
                .map(|todo| TodoItem {
                    content: todo.get("content")
                        .and_then(|c| c.as_str())
                        .unwrap_or("")
                        .to_string(),
                    status: todo.get("status")
                        .and_then(|s| s.as_str())
                        .unwrap_or("pending")
                        .to_string(),
                })
                .collect();
        }
    }

    /// Get todo progress summary (completed/total)
    pub(crate) fn todo_progress(&self) -> (usize, usize) {
        let total = self.todo_items.len();
        let completed = self.todo_items.iter()
            .filter(|t| t.status == "completed")
            .count();
        (completed, total)
    }

    /// Flush partial content to messages.
    fn flush_partial_content(&mut self) {
        if !self.thinking.is_empty() {
            self.push_message(Message {
                role: Role::Thinking,
                content: self.thinking.clone(),
            });
            self.thinking.clear();
        }
        if !self.streaming.is_empty() {
            self.push_message(Message {
                role: Role::Assistant,
                content: self.streaming.clone(),
            });
            self.streaming.clear();
        }
    }

    /// Process pending message queue, returning true if a message was sent.
    fn process_pending_queue(&mut self) -> bool {
        if !self.pending_messages.is_empty() {
            let next_msg = self.pending_messages.remove(0);
            self.push_message(Message {
                role: Role::User,
                content: next_msg.clone(),
            });
            self.tx.try_send(next_msg).ok();
            self.activity = Activity::Thinking;
            self.auto_scroll = true;
            true
        } else {
            self.activity = Activity::Idle;
            false
        }
    }

    pub(crate) fn on_event(&mut self, e: AgentEvent) {
        match e.event_type {
            EventType::ThinkingStart => {
                self.activity = Activity::Thinking;
                self.thinking.clear();
                self.request_start = Some(std::time::Instant::now());
            }
            EventType::ThinkingDelta => {
                if let Some(EventData::Thinking { delta, .. }) = e.data {
                    self.thinking.push_str(&delta);
                    self.activity = Activity::Thinking;
                }
            }
            EventType::ThinkingEnd => {
                if !self.thinking.is_empty() {
                    self.push_message(Message {
                        role: Role::Thinking,
                        content: self.thinking.clone(),
                    });
                    self.thinking.clear();
                }
            }
            EventType::TextStart => {
                self.streaming.clear();
                self.activity = Activity::Thinking;
                self.request_start = Some(std::time::Instant::now());
            }
            EventType::TextDelta => {
                if let Some(EventData::Text { delta }) = e.data {
                    self.streaming.push_str(&delta);
                    self.activity = Activity::Thinking;
                }
            }
            EventType::TextEnd => {
                if !self.streaming.is_empty() {
                    self.push_message(Message {
                        role: Role::Assistant,
                        content: self.streaming.clone(),
                    });
                    self.streaming.clear();
                }
            }
            EventType::ToolUseStart => {
                if let Some(EventData::ToolUse { name, input, .. }) = e.data {
                    self.activity = Activity::from_tool(&name);
                    self.activity_detail = extract_tool_detail(&name, input.as_ref());
                    self.activity_input = input.clone(); // Save full input for display
                    // Reset tool_start for each new tool execution
                    self.tool_start = Some(std::time::Instant::now());
                    if self.request_start.is_none() {
                        self.request_start = Some(std::time::Instant::now());
                    }

                    // Track todo_write for progress display
                    if name == "todo_write" && let Some(ref input) = input {
                        self.update_todo_items(input);
                    }
                }
            }
            EventType::ToolResult => {
                if let Some(EventData::ToolResult {
                    content,
                    name,
                    detail,
                    is_error,
                    ..
                }) = e.data
                {
                    self.push_message(Message {
                        role: Role::Tool {
                            name,
                            detail,
                            is_error,
                        },
                        content, // Keep full content, draw.rs will summarize
                    });
                    self.tool_calls += 1;
                    self.activity = Activity::Thinking;
                    self.activity_detail.clear();
                    self.activity_input = None;
                }
            }
            EventType::SessionEnded => {
                // Flush remaining content - thinking first, then assistant
                self.flush_partial_content();

                // Clear current request tokens
                self.current_request_tokens = 0;

                // Process queue or go idle
                if !self.process_pending_queue() {
                    self.request_start = None;
                }
                self.activity_detail.clear();
                self.activity_input = None;
                self.cancel.reset(); // Reset cancel state for next request
            }
            EventType::SessionRestored => {
                if let Some(EventData::SessionRestore {
                    input_tokens,
                    total_output_tokens,
                    message_count: _,
                }) = e.data
                {
                    self.tokens_in = input_tokens;
                    self.session_total_out = total_output_tokens;
                }
            }
            EventType::Error => {
                if let Some(EventData::Error { message, .. }) = e.data {
                    // Check if this is a cancellation error
                    let is_cancelled = message == "Operation cancelled";

                    if is_cancelled {
                        // Flush partial content before showing cancel message
                        self.flush_partial_content();
                        self.push_message(Message {
                            role: Role::System,
                            content: "\u{26a1} Interrupted".into(),
                        });
                    } else {
                        self.push_message(Message {
                            role: Role::System,
                            content: format!("\u{274c} Error: {}", message),
                        });
                        self.streaming.clear();
                        self.thinking.clear();
                    }
                }
                self.activity_detail.clear();
                self.activity_input = None;
                self.request_start = None;
                self.cancel.reset(); // Reset cancel state for next request

                // Process queue after cancellation or error
                self.process_pending_queue();
            }
            EventType::Usage => {
                if let Some(EventData::Usage {
                    input_tokens,
                    output_tokens,
                    cache_creation_input_tokens,
                    cache_read_input_tokens,
                }) = e.data
                {
                    // Only update tokens_in if it's non-zero (real-time updates may have 0)
                    if input_tokens > 0 {
                        self.tokens_in = input_tokens;
                        // Only count as a new API call when we have full usage info
                        self.api_calls += 1;
                    }
                    self.tokens_out = output_tokens;
                    self.session_total_out += output_tokens;
                    self.current_request_tokens = output_tokens; // Real-time update

                    // Update cache stats (only when actually reported by API)
                    let cache_read = cache_read_input_tokens.unwrap_or(0);
                    let cache_created = cache_creation_input_tokens.unwrap_or(0);

                    // Only update cache if values are non-zero (final usage event)
                    if cache_read > 0 || cache_created > 0 {
                        self.cache_read += cache_read;
                        self.cache_created += cache_created;
                    }
                }
            }
            EventType::CompressionCompleted => {
                if let Some(EventData::Compression {
                    original_tokens,
                    compressed_tokens,
                    ratio,
                }) = e.data
                {
                    self.compressions += 1;
                    // Update token display to reflect compressed state
                    self.tokens_in = compressed_tokens;
                    // Show compression result to user (useful feedback)
                    self.push_message(Message {
                        role: Role::System,
                        content: format!(
                            "📦 Compressed: {} → {}tok ({:.0}% saved)",
                            fmt_tokens(original_tokens),
                            fmt_tokens(compressed_tokens),
                            (1.0 - ratio) * 100.0
                        ),
                    });
                    self.auto_scroll = true;
                }
            }
            EventType::CompressionTriggered => {
                if let Some(EventData::Progress { .. }) = e.data {
                    // Silent - usage bar already reflects compression state
                }
            }
            EventType::Progress => {
                if let Some(EventData::Progress { message, .. }) = e.data {
                    self.push_message(Message {
                        role: Role::System,
                        content: message,
                    });
                    self.auto_scroll = true;
                }
            }
            EventType::MemoryLoaded => {
                // Only update counter, don't show in message area
                // Debug info is already shown in debug panel via DebugLog events
                if let Some(EventData::Memory { entries_count, .. }) = e.data
                    && entries_count > 0
                {
                    self.memory_saves += 1;
                }
            }
            EventType::MemoryDetected => {
                // Only update counter, don't show in message area
                // Debug info is already shown in debug panel via DebugLog events
                if let Some(EventData::Memory { .. }) = e.data
                {
                    self.memory_saves += 1;
                }
            }
            EventType::KeywordsExtracted => {
                // Keywords extraction info only in debug panel now
                // Update activity detail briefly, no message in main area
                if let Some(EventData::Keywords { keywords, .. }) = e.data
                    && !keywords.is_empty()
                {
                    // Update activity detail to show keywords briefly
                    self.activity_detail = format!(
                        "keywords: {}",
                        keywords
                            .iter()
                            .take(3)
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
            }
            EventType::ContextSize => {
                // Update context size from provider for accurate display
                if let Some(EventData::ContextSize { context_size }) = e.data {
                    self.context_size = context_size;
                }
            }
            EventType::AskQuestion => {
                if let Some(EventData::AskQuestion { question, options }) = e.data {
                    // Check for multiple questions format: { "questions": [...] }
                    let has_multiple = options.as_ref().and_then(|o| o.get("questions")).is_some();

                    if has_multiple {
                        // Multi-question mode
                        self.handle_multiple_questions(&question, options);
                    } else {
                        // Single question mode
                        self.handle_single_question(&question, options);
                    }
                }
            }
            EventType::DebugLog => {
                // Add debug log to panel (if debug mode is on)
                if self.debug_mode
                    && let Some(EventData::DebugLog { category, message }) = e.data
                {
                    let timestamp = e.timestamp;
                    // Format: [HH:MM:SS] category: message
                    let secs = (timestamp / 1000) % 60;
                    let mins = (timestamp / 60000) % 60;
                    let hours = (timestamp / 3600000) % 24;
                    let log = format!("[{:02}:{:02}:{:02}] {}: {}", hours, mins, secs, category, message);
                    self.add_debug_log(log);
                }
            }
            EventType::ProxyToolRequest => {
                // Handle proxy tool request - execute externally and send response
                if let Some(EventData::ProxyToolRequest { request_id, tool_name, tool_input, metadata: _ }) = e.data {
                    log::info!(
                        "TUI received proxy tool request: id={}, tool={}",
                        request_id, tool_name
                    );

                    // For image_search, we need async execution
                    if tool_name == "image_search" {
                        // Extract query and max_results
                        let query = tool_input.get("query").and_then(|q| q.as_str()).unwrap_or("");
                        let max_results = tool_input.get("max_results").and_then(|m| m.as_u64()).unwrap_or(5) as u32;

                        if query.is_empty() {
                            // Send error response immediately
                            if let Some(tx) = &self.proxy_response_tx {
                                let response = matrixcode_core::tools::ProxyToolResponse {
                                    request_id,
                                    result: r#"{"error": "query is required"}"#.to_string(),
                                    is_error: true,
                                };
                                // Use try_send for immediate error response
                                if let Err(e) = tx.try_send(response) {
                                    log::error!("Failed to send proxy tool error response: {}", e);
                                }
                            }
                        } else {
                            // Spawn async task to call real APIs
                            let tx = self.proxy_response_tx.clone();
                            let query = query.to_string();

                            tokio::spawn(async move {
                                use crate::image_utils;

                                log::info!("Calling real image search APIs for: {}", query);
                                let results = image_utils::search_all(&query, max_results).await;

                                let response = match results {
                                    Ok(images) => {
                                        // Format results as JSON
                                        let json = serde_json::json!({
                                            "success": true,
                                            "query": query,
                                            "total": images.len(),
                                            "images": images
                                        });
                                        log::info!("Image search completed: {} results", images.len());
                                        matrixcode_core::tools::ProxyToolResponse {
                                            request_id,
                                            result: json.to_string(),
                                            is_error: false,
                                        }
                                    }
                                    Err(e) => {
                                        log::error!("Image search error: {}", e);
                                        matrixcode_core::tools::ProxyToolResponse {
                                            request_id,
                                            result: serde_json::json!({
                                                "success": false,
                                                "error": e.to_string()
                                            }).to_string(),
                                            is_error: true,
                                        }
                                    }
                                };

                                if let Some(tx) = tx {
                                    // Use async send() - wait for channel capacity
                                    if let Err(e) = tx.send(response).await {
                                        log::error!("Failed to send proxy tool response: {}", e);
                                    }
                                    log::info!("Proxy tool response sent successfully");
                                }
                            });
                        }
                    } else {
                        // Unknown tool
                        if let Some(tx) = &self.proxy_response_tx {
                            let response = matrixcode_core::tools::ProxyToolResponse {
                                request_id,
                                result: format!("{{\"error\": \"Unknown proxy tool: {}\"}}", tool_name),
                                is_error: true,
                            };
                            if let Err(e) = tx.try_send(response) {
                                log::error!("Failed to send proxy tool response: {}", e);
                            }
                        }
                    }
                }
            }
            EventType::LspServerStatus => {
                if let Some(EventData::LspServerStatus { servers }) = e.data {
                    log::info!("TUI received LspServerStatus event: {} servers", servers.len());
                    for server in &servers {
                        log::info!("  - {} ({}) {:?}", server.name, server.language, server.status);
                    }
                    self.lsp_servers = servers
                        .iter()
                        .map(|s| crate::app::LspServerInfo {
                            name: s.name.clone(),
                            language: s.language.clone(),
                            status: match &s.status {
                                matrixcode_core::lsp::LspServerStatus::NotStarted => {
                                    crate::app::LspServerStatus::NotStarted
                                }
                                matrixcode_core::lsp::LspServerStatus::Starting => {
                                    crate::app::LspServerStatus::Starting
                                }
                                matrixcode_core::lsp::LspServerStatus::Connected => {
                                    crate::app::LspServerStatus::Connected
                                }
                                matrixcode_core::lsp::LspServerStatus::Error(msg) => {
                                    crate::app::LspServerStatus::Error(msg.clone())
                                }
                            },
                        })
                        .collect();
                    log::info!("TUI lsp_servers updated: {} servers", self.lsp_servers.len());
                    self.dirty.set(true); // Mark as dirty to redraw
                }
            }
            EventType::LspServerAdded => {
                if let Some(EventData::LspServerAdded { name, language }) = e.data {
                    log::info!("TUI received LspServerAdded event: {} ({})", name, language);
                    // Add new server with NotStarted status
                    self.lsp_servers.push(crate::app::LspServerInfo {
                        name,
                        language,
                        status: crate::app::LspServerStatus::NotStarted,
                    });
                    self.dirty.set(true);
                }
            }
            _ => {}
        }
    }

    /// Handle single question
    fn handle_single_question(&mut self, question: &str, options: Option<Value>) {
        // Format the question with clear styling
        let mut content = String::new();

        // Header line - prominent
        content.push_str("┌──────────────────────────────────────┐\n");
        content.push_str("│            ⚡ 等待输入 ⚡            │\n");
        content.push_str("└──────────────────────────────────────┘\n\n");

        // Question content
        content.push_str(question);

        // Parse options or create default y/n for approval
        if let Some(ref opts) = options {
            let arr: Option<&Vec<Value>> =
                if let Some(arr) = opts.get("options").and_then(|o| o.as_array()) {
                    self.ask_multi_select = opts
                        .get("multiSelect")
                        .and_then(|m| m.as_bool())
                        .unwrap_or(false);
                    Some(arr)
                } else if let Some(arr) = opts.as_array() {
                    self.ask_multi_select = false;
                    Some(arr)
                } else {
                    None
                };

            match arr {
                Some(arr) if !arr.is_empty() => {
                    self.ask_options = arr
                        .iter()
                        .map(|opt| crate::types::AskOption {
                            id: opt["id"].as_str().unwrap_or("").to_string(),
                            label: opt["label"].as_str().unwrap_or("").to_string(),
                            description: opt["description"].as_str().map(|s| s.to_string()),
                            selected: opt
                                .get("selected")
                                .and_then(|s| s.as_bool())
                                .unwrap_or(false),
                            is_submit: false,
                            is_other: false,
                        })
                        .collect();
                    // Append "Other" option for custom input
                    self.ask_options
                        .push(crate::types::AskOption::other_option());
                    self.ask_selected_index = 0;

                    if self.ask_multi_select {
                        let opt_count = self.ask_options.len();
                        self.ask_submit_mode = SubmitMode::from_option_count(opt_count, true);

                        if self.ask_submit_mode == SubmitMode::Option {
                            self.ask_options.push(crate::types::AskOption {
                                id: "__submit__".into(),
                                label: "提交".into(),
                                description: Some("确认并提交所有选择".into()),
                                selected: false,
                                is_submit: true,
                                is_other: false,
                            });
                        }
                    } else {
                        self.ask_submit_mode = SubmitMode::Direct;
                    }

                    content.push_str("\n\n─────────────────────────────────────\n");
                    if self.ask_multi_select {
                        match self.ask_submit_mode {
                            SubmitMode::Direct => {
                                content.push_str("选项 (↑↓导航 Space/Enter切换 Enter确认):\n")
                            }
                            SubmitMode::Option => {
                                content.push_str("选项 (↑↓导航 Space/Enter切换 选中提交):\n")
                            }
                            SubmitMode::Button => {
                                content.push_str("选项 (↑↓导航 Space/Enter切换):\n")
                            }
                        }
                    } else {
                        content.push_str("选项 (↑↓选择 Enter确认):\n");
                    }
                    for (i, opt) in self.ask_options.iter().enumerate() {
                        if opt.is_submit {
                            // Submit 选项也显示为复选框
                            let marker = if opt.selected { "[✓]" } else { "[ ]" };
                            content.push_str(&format!(
                                "  {} {} - {}\n",
                                marker,
                                opt.label,
                                opt.description.as_deref().unwrap_or("")
                            ));
                        } else {
                            let marker = if self.ask_multi_select {
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
                    self.input.clear();
                    self.cursor_pos = 0;
                }
                _ if question.contains("(y/n)") || question.contains("Allow?") => {
                    self.ask_multi_select = false;
                    self.ask_submit_mode = SubmitMode::Direct;
                    self.ask_options = vec![
                        crate::types::AskOption {
                            id: "y".into(),
                            label: "同意".into(),
                            description: Some("允许此操作".into()),
                            selected: false,
                            is_submit: false,
                            is_other: false,
                        },
                        crate::types::AskOption {
                            id: "n".into(),
                            label: "拒绝".into(),
                            description: Some("拒绝此操作".into()),
                            selected: false,
                            is_submit: false,
                            is_other: false,
                        },
                        crate::types::AskOption::other_option(),
                    ];
                    self.ask_selected_index = 0;
                    content.push_str("\n\n─────────────────────────────────────\n");
                    content.push_str("选项 (↑↓选择 Enter确认):\n");
                    content.push_str("  [Y] 同意 - 允许此操作\n");
                    content.push_str("  [N] 拒绝 - 拒绝此操作\n");
                    self.input.clear();
                    self.cursor_pos = 0;
                }
                _ => {
                    self.ask_multi_select = false;
                    self.ask_submit_mode = SubmitMode::Direct;
                    self.ask_options.clear();
                    self.ask_selected_index = 0;
                }
            }
        } else {
            self.ask_multi_select = false;
            self.ask_submit_mode = SubmitMode::Direct;
            self.ask_options.clear();
            self.ask_selected_index = 0;
        }

        // Single question - clear multi-question state
        self.ask_questions.clear();
        self.current_question_idx = 0;

        self.push_message(Message {
            role: Role::Ask,
            content,
        });
        self.waiting_for_ask = true;
        self.activity = Activity::Asking;
        self.request_start = None; // Pause elapsed time during ask wait
        self.auto_scroll = true;
    }

    /// Handle multiple questions
    fn handle_multiple_questions(&mut self, _intro: &str, options: Option<Value>) {
        if let Some(ref opts) = options
            && let Some(arr) = opts
                .get("questions")
                .and_then(|q| q.as_array())
                .filter(|a| !a.is_empty())
        {
            // Parse each question
            self.ask_questions = arr
                .iter()
                .enumerate()
                .map(|(idx, q)| {
                    let id = q["id"].as_str().unwrap_or(&idx.to_string()).to_string();
                    let question = q["question"].as_str().unwrap_or("").to_string();
                    let multi_select = q
                        .get("options")
                        .and_then(|o| o.get("multiSelect").and_then(|m| m.as_bool()))
                        .unwrap_or(false);

                    let opts_arr = q
                        .get("options")
                        .and_then(|o| o.get("options"))
                        .and_then(|o| o.as_array())
                        .or_else(|| q.get("options").and_then(|o| o.as_array()));

                    let options: Vec<crate::types::AskOption> = opts_arr
                        .map(|arr| {
                            arr.iter()
                                .map(|opt| crate::types::AskOption {
                                    id: opt["id"].as_str().unwrap_or("").to_string(),
                                    label: opt["label"].as_str().unwrap_or("").to_string(),
                                    description: opt["description"].as_str().map(|s| s.to_string()),
                                    selected: opt
                                        .get("selected")
                                        .and_then(|s| s.as_bool())
                                        .unwrap_or(false),
                                    is_submit: false,
                                    is_other: false,
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    let opt_count = options.len();
                    let submit_mode = SubmitMode::from_option_count(opt_count, multi_select);

                    // Append "Other" option for custom input
                    let mut options = options;
                    options.push(crate::types::AskOption::other_option());

                    crate::types::AskQuestion {
                        id,
                        question,
                        options,
                        multi_select,
                        selected_index: 0,
                        submit_mode,
                        other_input: None,
                    }
                })
                .collect();

            self.current_question_idx = 0;

            // Build content for first question with navigation hint
            let first_q = &self.ask_questions[0];
            let mut content = String::new();

            content.push_str("┌──────────────────────────────────────┐\n");
            content.push_str(&format!(
                "│  ⚡ 问题 1 / {} (Tab切换) ⚡          │\n",
                self.ask_questions.len()
            ));
            content.push_str("└──────────────────────────────────────┘\n\n");
            content.push_str(&first_q.question);

            // Load first question state
            self.ask_options = first_q.options.clone();
            self.ask_selected_index = first_q.selected_index;
            self.ask_multi_select = first_q.multi_select;
            self.ask_submit_mode = first_q.submit_mode.clone();

            // Add Submit option for Option mode
            if self.ask_multi_select && self.ask_submit_mode == SubmitMode::Option {
                self.ask_options.push(crate::types::AskOption {
                    id: "__submit__".into(),
                    label: "✓ 提交".into(),
                    description: Some("确认选择并提交".into()),
                    selected: false,
                    is_submit: true,
                    is_other: false,
                });
            }

            content.push_str("\n\n─────────────────────────────────────\n");
            if self.ask_multi_select {
                match self.ask_submit_mode {
                    SubmitMode::Direct => {
                        content.push_str("选项 (↑↓导航 Space切换 Enter下一题):\n")
                    }
                    SubmitMode::Option => content.push_str("选项 (↑↓导航 Space切换 Enter提交):\n"),
                    SubmitMode::Button => content.push_str("选项 (↑↓导航 Space切换):\n"),
                }
            } else {
                content.push_str("选项 (↑↓选择 Enter下一题):\n");
            }

            for (i, opt) in self.ask_options.iter().enumerate() {
                if opt.is_submit {
                    content.push_str(&format!("  >>> {} <<<\n", opt.label));
                } else {
                    let marker = if self.ask_multi_select {
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

            self.input.clear();
            self.cursor_pos = 0;

            self.push_message(Message {
                role: Role::Ask,
                content,
            });
            self.waiting_for_ask = true;
            self.activity = Activity::Asking;
            self.request_start = None; // Pause elapsed time during ask wait
            self.auto_scroll = true;
        }
    }
}
