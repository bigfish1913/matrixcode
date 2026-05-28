use std::time::{Duration, Instant};

use matrixcode_core::cancel::CancellationToken;

use crate::app::{CronTask, LoopTask, TuiApp};
use crate::types::{Activity, ApproveMode, Message, Role};
use crate::utils::{fmt_tokens, truncate};

impl TuiApp {
    pub(crate) fn handle_command(&mut self, cmd: &str) {
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
                } else {
                    self.push_message(Message {
                        role: Role::System,
                        content: "⚠️ Cannot clear while AI is processing".into(),
                    });
                }
                self.auto_scroll = true;
            }
            "/history" => {
                // Count message types in single pass
                let (user_count, assistant_count, tool_count) =
                    self.messages
                        .iter()
                        .fold((0, 0, 0), |(u, a, t), m| match m.role {
                            Role::User => (u + 1, a, t),
                            Role::Assistant => (u, a + 1, t),
                            Role::Tool { .. } => (u, a, t + 1),
                            _ => (u, a, t),
                        });
                let queue_count = self.pending_messages.len();
                if self.debug_mode {
                    self.push_message(Message {
                        role: Role::System,
                        content: format!(
                            "📊 Session: {} user, {} assistant, {} tools, {} queued, {}tok output",
                            user_count,
                            assistant_count,
                            tool_count,
                            queue_count,
                            fmt_tokens(self.session_total_out)
                        ),
                    });
                }
                self.auto_scroll = true;
            }
            "/mode" => {
                if args.is_empty() {
                    // Status bar already shows mode, no message needed
                } else {
                    match args[0] {
                        "ask" => self.approve_mode = ApproveMode::Ask,
                        "auto" => self.approve_mode = ApproveMode::Auto,
                        "strict" => self.approve_mode = ApproveMode::Strict,
                        _ => {
                            self.push_message(Message {
                                role: Role::System,
                                content: "Invalid mode. Use: ask, auto, strict".into(),
                            });
                            return;
                        }
                    }
                    // Update shared atomic immediately (takes effect even during agent execution)
                    self.sync_approve_mode();
                }
                self.auto_scroll = true;
            }
            "/model" => {
                if args.is_empty() {
                    // Status bar already shows model info
                } else if self.activity == Activity::Idle {
                    let new_model = args.join(" ");
                    self.model = new_model.clone();
                } else {
                    self.push_message(Message {
                        role: Role::System,
                        content: "⚠️ Cannot change model while AI is processing".into(),
                    });
                }
                self.auto_scroll = true;
            }
            "/compact" | "/compress" => {
                self.tx.try_send("/compact".to_string()).ok();
                self.auto_scroll = true;
            }
            "/workflow" => {
                // Handle workflow commands in TUI (don't send to Agent)
                use matrixcode_core::workflow::WorkflowRegistry;

                let project_path = std::env::current_dir().ok();

                let response = if args.is_empty() || args[0] == "discover" || args[0] == "list" {
                    let registry = WorkflowRegistry::new(project_path.as_ref());
                    if registry.is_empty() {
                        "📋 No workflows found.\n\nCreate YAML files in:\n  - .matrix/workflows/ (project)\n  - ~/.matrix/workflows/ (global)".to_string()
                    } else {
                        registry.generate_summary()
                    }
                } else if args[0] == "match" && args.len() > 1 {
                    let query = args[1..].join(" ");
                    let registry = WorkflowRegistry::new(project_path.as_ref());
                    let matches = registry.match_workflows(&query);
                    if matches.is_empty() {
                        format!("❌ No workflows match '{}'", query)
                    } else {
                        let mut result = format!("🔍 Matching workflows for '{}':\n\n", query);
                        for info in matches.iter().take(5) {
                            result.push_str(&format!("• {} - {}\n", info.id, info.name));
                        }
                        result
                    }
                } else if args[0] == "run" && args.len() > 1 {
                    format!("⏳ Use CLI to run workflows:\n\n  matrixcode workflow run --file .matrix/workflows/{}.yaml", args[1])
                } else if args[0] == "help" {
                    "📋 /workflow commands:\n\n  discover - List available workflows\n  match <query> - Find matching workflows\n  run <id> - Hint for CLI execution".to_string()
                } else {
                    "Unknown command. Use '/workflow help' for available commands.".to_string()
                };

                self.push_message(Message {
                    role: Role::System,
                    content: response,
                });
                self.auto_scroll = true;
            }
            "/init" => {
                // Initialize project configuration
                if args.is_empty() {
                    // Generate project overview
                    self.tx.try_send("/init".to_string()).ok();
                } else if args[0] == "status" {
                    // Show project status
                    self.tx.try_send("/init status".to_string()).ok();
                } else if matches!(args[0], "reset" | "clear") {
                    // Reset configuration
                    self.tx.try_send("/init reset".to_string()).ok();
                } else {
                    self.push_message(Message {
                        role: Role::System,
                        content: "Unknown init command. Use: /init, /init status, /init reset"
                            .into(),
                    });
                }
                self.auto_scroll = true;
            }
            "/debug" => {
                // Toggle debug mode
                self.debug_mode = !self.debug_mode;
                self.auto_scroll = true;
            }
            "/retry" => {
                // Process pending queue
                if !self.pending_messages.is_empty() && self.activity == Activity::Idle {
                    let next_msg = self.pending_messages.remove(0);
                    self.push_message(Message {
                        role: Role::User,
                        content: next_msg.clone(),
                    });
                    self.tx.try_send(next_msg).ok();
                    self.activity = Activity::Thinking;
                    self.auto_scroll = true;
                } else if self.pending_messages.is_empty() {
                    self.push_message(Message {
                        role: Role::System,
                        content: "No pending messages to retry".into(),
                    });
                } else {
                    self.push_message(Message {
                        role: Role::System,
                        content: "AI is busy, please wait".into(),
                    });
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
                } else {
                    self.push_message(Message {
                        role: Role::System,
                        content: "⚠️ Cannot start new session while AI is processing".into(),
                    });
                }
                self.auto_scroll = true;
            }
            "/mcp" | "/mcp list" => {
                if self.mcp_servers.is_empty() {
                    self.push_message(Message {
                        role: Role::System,
                        content: "📋 未连接任何 MCP servers\n\n使用 --mcp 参数启动:\n  matrixcode-tui --mcp \"playwright:npx -y @playwright/mcp@latest\"".into(),
                    });
                } else {
                    let mut content = "📋 MCP Servers:\n".to_string();
                    for server in &self.mcp_servers {
                        let status = if server.is_started { "✓ 运行中" } else { "✗ 未启动" };
                        content.push_str(&format!(
                            "  • {} {} ({} 工具)\n",
                            server.name, status, server.tool_count
                        ));
                    }
                    self.push_message(Message {
                        role: Role::System,
                        content,
                    });
                }
                self.auto_scroll = true;
            }
            "/help" => {
                self.push_message(Message {
                    role: Role::System,
                    content: concat!(
                        "📖 Commands:\n",
                        "  /help     - Show this help\n",
                        "  /exit     - Exit MatrixCode\n",
                        "  /clear    - Clear messages\n",
                        "  /history  - Show session history\n",
                        "  /mode     - Change approve mode (ask/auto/strict)\n",
                        "  /model    - Show/change model\n",
                        "  /config   - Show current configuration\n",
                        "  /tools    - List available tools with descriptions\n",
                        "  /system   - Show system prompt and all details\n",
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
                        "  Enter=send │ Shift+Enter=newline │ Ctrl+V=paste\n",
                        "  ↑↓=history │ PgUp/PgDn=scroll │ Home/End=top/bot\n",
                        "  Alt+M=mode │ Alt+T=collapse │ Esc=interrupt │ Ctrl+D=exit"
                    )
                    .into(),
                });
                self.auto_scroll = true;
            }
            "/tools" => {
                // Send to backend for processing
                self.tx.try_send("/tools".to_string()).ok();
                self.auto_scroll = true;
            }
            "/system" => {
                // Send to backend for processing
                self.tx.try_send("/system".to_string()).ok();
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
            "/config" => {
                // Send to backend for processing
                self.tx.try_send("/config".to_string()).ok();
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
                    self.push_message(Message {
                        role: Role::System,
                        content: "/loop <message> [interval] [count] - Start loop\n/loop stop - Stop loop\n/loop status - Show status".into()
                    });
                } else if args[0] == "stop" {
                    // Take ownership to avoid borrow conflict
                    let task = self.loop_task.take();
                    if let Some(ref task) = task {
                        task.cancel_token.cancel();
                        self.push_message(Message {
                            role: Role::System,
                            content: format!("✓ Loop stopped (executed {} times)", task.count),
                        });
                        self.loop_task = None; // Already taken
                    } else {
                        self.push_message(Message {
                            role: Role::System,
                            content: "No active loop".into(),
                        });
                    }
                } else if args[0] == "status" {
                    if let Some(ref task) = self.loop_task {
                        self.push_message(Message {
                            role: Role::System,
                            content: format!(
                                "🔄 Loop active: '{}' every {}s, count {}{}",
                                truncate(&task.message, 30),
                                task.interval_secs,
                                task.count,
                                task.max_count
                                    .map(|m| format!(" (max {})", m))
                                    .unwrap_or_default()
                            ),
                        });
                    } else {
                        self.push_message(Message {
                            role: Role::System,
                            content: "No active loop".into(),
                        });
                    }
                } else {
                    // Start new loop: /loop "message" [interval] [max_count]
                    if self.loop_task.is_some() {
                        self.push_message(Message {
                            role: Role::System,
                            content: "⚠️ Loop already active. Use /loop stop first".into(),
                        });
                    } else {
                        let message = args[0].to_string();
                        let interval_secs: u64 =
                            args.get(1).and_then(|s| s.parse().ok()).unwrap_or(60);
                        let max_count: Option<u64> = args.get(2).and_then(|s| s.parse().ok());

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

                        self.push_message(Message {
                            role: Role::System,
                            content: format!(
                                "🔄 Loop started: '{}' every {}s{}",
                                truncate(&message, 30),
                                interval_secs,
                                max_count
                                    .map(|m| format!(" (max {})", m))
                                    .unwrap_or_default()
                            ),
                        });
                    }
                }
                self.auto_scroll = true;
            }
            "/cron" => {
                if args.is_empty() {
                    self.push_message(Message {
                        role: Role::System,
                        content: "/cron add <message> <minutes> - Add cron task\n/cron list - List tasks\n/cron remove <id> - Remove task\n/cron clear - Clear all".into()
                    });
                } else if args[0] == "list" {
                    if self.cron_tasks.is_empty() {
                        self.push_message(Message {
                            role: Role::System,
                            content: "No cron tasks".into(),
                        });
                    } else {
                        let list: Vec<String> = self
                            .cron_tasks
                            .iter()
                            .map(|t| {
                                format!(
                                    "#{}: '{}' every {}min",
                                    t.id,
                                    truncate(&t.message, 20),
                                    t.minute_interval
                                )
                            })
                            .collect();
                        self.push_message(Message {
                            role: Role::System,
                            content: format!("📋 Cron tasks:\n{}", list.join("\n")),
                        });
                    }
                } else if args[0] == "remove" || args[0] == "rm" {
                    let id: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                    if let Some(pos) = self.cron_tasks.iter().position(|t| t.id == id) {
                        let task = &self.cron_tasks[pos];
                        task.cancel_token.cancel();
                        self.cron_tasks.remove(pos);
                        self.push_message(Message {
                            role: Role::System,
                            content: format!("✓ Cron task #{} removed", id),
                        });
                    } else {
                        self.push_message(Message {
                            role: Role::System,
                            content: format!("Cron task #{} not found", id),
                        });
                    }
                } else if args[0] == "clear" {
                    for task in &self.cron_tasks {
                        task.cancel_token.cancel();
                    }
                    let count = self.cron_tasks.len();
                    self.cron_tasks.clear();
                    self.push_message(Message {
                        role: Role::System,
                        content: format!("✓ {} cron tasks cleared", count),
                    });
                } else if args[0] == "add" {
                    // /cron add "message" 5
                    if args.len() < 3 {
                        self.push_message(Message {
                            role: Role::System,
                            content: "Usage: /cron add <message> <minutes>".into(),
                        });
                    } else {
                        let message = args[1].to_string();
                        let minute_interval: u64 =
                            args.get(2).and_then(|s| s.parse().ok()).unwrap_or(5);

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

                        self.push_message(Message {
                            role: Role::System,
                            content: format!(
                                "✓ Cron #{} added: '{}' every {}min",
                                id,
                                truncate(&message, 30),
                                minute_interval
                            ),
                        });
                    }
                } else {
                    self.push_message(Message {
                        role: Role::System,
                        content: "Unknown cron command. Use: add, list, remove, clear".into(),
                    });
                }
                self.auto_scroll = true;
            }
            _ => {
                // Forward unknown commands to backend for skill handling
                // Backend will check if it matches a skill name (e.g., /om:debug)
                if self.activity == Activity::Idle {
                    self.push_message(Message {
                        role: Role::User,
                        content: cmd.to_string(),
                    });
                    self.tx.try_send(cmd.to_string()).ok();
                    self.activity = Activity::Thinking;
                    self.request_start = Some(Instant::now());
                } else {
                    // Queue message when AI is processing (same as regular messages)
                    self.pending_messages.push(cmd.to_string());
                }
                self.auto_scroll = true;
            }
        }
    }
}
