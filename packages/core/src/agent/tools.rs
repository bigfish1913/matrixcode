//! Agent tool execution implementation.

use anyhow::Result;
use std::sync::atomic::Ordering;
use tokio::time::{Duration, sleep};

use crate::approval::{ApproveMode, needs_approval, RiskLevel};
use crate::event::{AgentEvent, EventData, EventType};
use crate::providers::{ChatResponse, ContentBlock, Message, MessageContent, Role};
use crate::tools::MustReadFirstError;
use crate::truncate::truncate_with_suffix;
use crate::agent::core::state::MAX_SAME_ERROR_COUNT;

use super::helpers::extract_tool_detail;
use super::types::Agent;

/// Maximum tool result size (50KB) - prevents oversized responses
const MAX_TOOL_RESULT_SIZE: usize = 50_000;

/// Wait for cancellation signal, checking periodically.
async fn wait_for_cancel(token: &crate::cancel::CancellationToken) {
    while !token.is_cancelled() {
        sleep(Duration::from_millis(50)).await;
    }
}

impl Agent {
    /// Process response and handle tool_use
    pub(crate) async fn process_response(&mut self, response: &ChatResponse) -> Result<bool> {
        let mut has_tool_use = false;
        let mut assistant_content: Vec<ContentBlock> = Vec::new();
        let mut tool_results: Vec<Message> = Vec::new();

        for block in &response.content {
            match block {
                ContentBlock::Text { text } => {
                    assistant_content.push(ContentBlock::Text { text: text.clone() });
                }

                ContentBlock::Thinking {
                    thinking,
                    signature,
                } => {
                    assistant_content.push(ContentBlock::Thinking {
                        thinking: thinking.clone(),
                        signature: signature.clone(),
                    });
                }

                ContentBlock::ToolUse { id, name, input } => {
                    if self.session.is_cancelled() {
                        return Err(anyhow::anyhow!("Operation cancelled"));
                    }

                    has_tool_use = true;

                    // Emit ToolUseStart event with full input before execution when the
                    // streaming phase did not already preview the complete input.
                    // This allows UI to display command details before result arrives.
                    if !self.state.remove_previewed_tool_input(id) {
                        self.emit(AgentEvent::tool_use_start(
                            id.clone(),
                            name.clone(),
                            Some(input.clone()),
                        ))?;
                    }

                    log::info!("Agent: starting tool '{}' with id {}", name, id);
                    let result = self.execute_tool(name, input.clone()).await;
                    log::info!("Agent: tool '{}' completed", name);

                    let (content, is_error) = match result {
                        Ok(output) => (output, false),
                        Err(e) => (e.to_string(), true),
                    };

                    // Truncate oversized tool results to prevent API issues
                    let content = if content.len() > MAX_TOOL_RESULT_SIZE {
                        let truncated = truncate_with_suffix(&content, MAX_TOOL_RESULT_SIZE);
                        log::warn!(
                            "Tool '{}' result truncated: {} -> {} bytes",
                            name,
                            content.len(),
                            truncated.len()
                        );
                        format!(
                            "{}\n\n⚠️ Output truncated ({} bytes total)",
                            truncated,
                            content.len()
                        )
                    } else {
                        content
                    };

                    self.emit(AgentEvent::tool_result(
                        id.clone(),
                        name.clone(),
                        extract_tool_detail(name, input),
                        content.clone(),
                        is_error,
                    ))?;

                    assistant_content.push(ContentBlock::ToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        input: input.clone(),
                    });

                    tool_results.push(Message {
                        role: Role::User,
                        content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                            tool_use_id: id.clone(),
                            content: format!(
                                "{}: {}",
                                if is_error { "Error" } else { "Result" },
                                content
                            ),
                        }]),
                    });
                }

                _ => {}
            }
        }

        if !assistant_content.is_empty() {
            self.state.add_message(Message {
                role: Role::Assistant,
                content: MessageContent::Blocks(assistant_content),
            });
        }

        for msg in tool_results {
            self.state.add_message(msg);
        }

        Ok(has_tool_use)
    }

    /// Execute a tool
    pub(crate) async fn execute_tool(
        &mut self,
        name: &str,
        input: serde_json::Value,
    ) -> Result<String> {
        // 先检查是否是代理工具
        if self
            .proxy_tool_defs
            .iter()
            .any(|t| t.definition.name == name)
        {
            log::info!("Executing proxy tool: {}", name);
            return self.handle_proxy_tool(name, input).await;
        }

        // === Read history precondition check ===
        // For edit/write tools, check if the file has been read first
        if matches!(name, "edit" | "multi_edit" | "write") {
            let file_path = input["path"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("missing 'path' parameter for {} tool", name))?;

            // Check if file exists (only enforce read-first for existing files)
            let file_exists = tokio::fs::try_exists(file_path).await.unwrap_or(false);

            if file_exists && !self.state.read_history().has_read(file_path) {
                log::warn!(
                    "Tool '{}' rejected: file '{}' not read in this session",
                    name,
                    file_path
                );
                let error = MustReadFirstError::new(file_path);
                return Err(anyhow::anyhow!("{}", error.message()));
            }
        }

        // Find tool and extract info needed for approval check
        let tool = self.tools.iter().find(|t| t.definition().name == name);

        if tool.is_none() {
            return Err(anyhow::anyhow!("Tool '{}' not found", name));
        }

        let tool = tool.unwrap();
        let current_mode = ApproveMode::from_u8(self.approve_mode.load(Ordering::Relaxed));
        let tool_risk_level = tool.risk_level();
        let needs_approval_flag = needs_approval(current_mode, tool_risk_level);

        log::debug!(
            "Tool '{}' approval check: mode={}, risk={}, needs_approval={}",
            name,
            current_mode,
            tool_risk_level,
            needs_approval_flag
        );

        // Handle approval if needed (this requires mutable borrow, so we do it before tool.execute)
        if needs_approval_flag {
            self.handle_tool_approval(name, &input, tool_risk_level).await?;
        }

        // Handle "ask" tool special case (also requires mutable borrow)
        if name == "ask" && self.has_ask_channel() {
            return self.handle_ask_tool(&input).await;
        }

        // Now find tool again and execute (after all mutable borrows are released)
        let tool = self.tools.iter().find(|t| t.definition().name == name);
        if let Some(tool) = tool {
            self.emit(AgentEvent::progress(format!("Executing: {}", name), None))?;
            let result = tool.execute(input.clone()).await;

            // === Read history post-action ===
            // For read tool, mark the file as read on success
            if name == "read" && result.is_ok() {
                if let Some(file_path) = input["path"].as_str() {
                    self.state.read_history_mut().mark_read(file_path);
                    log::info!("File '{}' marked as read in session history", file_path);
                }
            }

            // === Error history tracking ===
            // Track errors to detect repeated mistakes
            if let Err(ref e) = result {
                let error_msg = e.to_string();
                let error_count = self.state.record_tool_error(name, &error_msg);

                // If error repeats, provide enhanced guidance
                if error_count >= MAX_SAME_ERROR_COUNT {
                    log::warn!(
                        "Tool '{}' error repeated {} times: {}",
                        name,
                        error_count,
                        &error_msg[..100.min(error_msg.len())]
                    );

                    // Provide enhanced error message with guidance
                    let enhanced_error = self.enhance_repeated_error(name, &error_msg, error_count);
                    return Err(anyhow::anyhow!("{}", enhanced_error));
                }
            }

            result
        } else {
            Err(anyhow::anyhow!("Tool '{}' not found", name))
        }
    }

    /// Enhance error message when it repeats multiple times.
    /// Provides specific guidance based on tool type and error pattern.
    fn enhance_repeated_error(&self, tool_name: &str, error_msg: &str, count: usize) -> String {
        let base_guidance = format!(
            "⚠️ 错误已重复 {} 次。请仔细阅读错误信息并采取不同的方法。\n\n",
            count
        );

        // Tool-specific guidance
        let tool_guidance = match tool_name {
            "edit" | "multi_edit" => {
                self.enhance_edit_error(error_msg)
            }
            "write" => {
                self.enhance_write_error(error_msg)
            }
            "read" => {
                self.enhance_read_error(error_msg)
            }
            "bash" => {
                self.enhance_bash_error(error_msg)
            }
            "grep" | "glob" => {
                self.enhance_search_error(error_msg)
            }
            _ => {
                format!(
                    "原始错误: {}\n\n建议：检查工具参数是否正确，或尝试其他工具。",
                    error_msg
                )
            }
        };

        format!("{}{}", base_guidance, tool_guidance)
    }

    /// Enhance edit/multi_edit tool error
    fn enhance_edit_error(&self, error_msg: &str) -> String {
        if error_msg.contains("not found") || error_msg.contains("old_string") {
            format!(
                "原始错误: {}\n\n\
                🔧 edit 工具错误指导：\n\
                1. 'old_string' 必须与文件内容**完全匹配**（包括空格、换行）\n\
                2. 请先用 read 工具读取文件，确认实际内容\n\
                3. 复制粘贴时注意不要遗漏或添加字符\n\
                4. 如果内容有多处匹配，需要扩大上下文使其唯一\n\
                5. Windows 文件注意换行符差异（CRLF vs LF）\n\
                \n\
                建议：先 read 文件，然后精确复制要替换的内容作为 old_string。",
                error_msg
            )
        } else if error_msg.contains("multiple") || error_msg.contains("unique") {
            format!(
                "原始错误: {}\n\n\
                🔧 多处匹配问题指导：\n\
                1. 'old_string' 在文件中找到多处匹配\n\
                2. 需要扩大上下文（前后添加更多行）使其唯一\n\
                3. 或者使用 multi_edit 工具处理多处修改\n\
                \n\
                建议：在 old_string 中添加更多上下文行。",
                error_msg
            )
        } else {
            format!(
                "原始错误: {}\n\n\
                🔧 建议先读取文件确认内容，然后精确匹配 old_string。",
                error_msg
            )
        }
    }

    /// Enhance write tool error
    fn enhance_write_error(&self, error_msg: &str) -> String {
        if error_msg.contains("must read") || error_msg.contains("read first") {
            format!(
                "原始错误: {}\n\n\
                📝 write 工具前置条件：\n\
                1. 写入已存在的文件前必须先用 read 工具读取\n\
                2. 这是为了防止意外覆盖重要内容\n\
                \n\
                建议：先执行 read 工具读取目标文件。",
                error_msg
            )
        } else if error_msg.contains("path") || error_msg.contains("traversal") {
            format!(
                "原始错误: {}\n\n\
                📝 路径安全限制：\n\
                1. 不允许写入系统关键目录\n\
                2. 不允许路径穿越（如 ../../../etc/passwd）\n\
                3. 必须在项目目录范围内\n\
                \n\
                建议：使用项目内的相对路径。",
                error_msg
            )
        } else {
            format!(
                "原始错误: {}\n\n\
                📝 建议检查路径和参数是否正确。",
                error_msg
            )
        }
    }

    /// Enhance read tool error
    fn enhance_read_error(&self, error_msg: &str) -> String {
        if error_msg.contains("not found") || error_msg.contains("does not exist") {
            format!(
                "原始错误: {}\n\n\
                📖 文件不存在指导：\n\
                1. 检查路径是否正确（相对路径 vs 绝对路径）\n\
                2. 使用 glob 工具搜索类似文件名\n\
                3. 检查文件扩展名是否正确\n\
                \n\
                建议：先用 ls 或 glob 确认文件位置。",
                error_msg
            )
        } else {
            format!(
                "原始错误: {}\n\n\
                📖 建议检查文件路径和权限。",
                error_msg
            )
        }
    }

    /// Enhance bash tool error
    fn enhance_bash_error(&self, error_msg: &str) -> String {
        if error_msg.contains("not found") || error_msg.contains("command") {
            format!(
                "原始错误: {}\n\n\
                💻 命令执行错误指导：\n\
                1. 检查命令是否存在（可能需要安装）\n\
                2. Windows 环境注意命令语法差异\n\
                3. 使用绝对路径或确认 PATH 环境变量\n\
                \n\
                建议：先确认命令可用，或使用替代命令。",
                error_msg
            )
        } else if error_msg.contains("permission") || error_msg.contains("access") {
            format!(
                "原始错误: {}\n\n\
                💻 权限错误指导：\n\
                1. 检查是否有执行权限\n\
                2. 可能需要管理员权限\n\
                3. 检查文件/目录权限设置\n\
                \n\
                建议：确认权限或使用其他目录。",
                error_msg
            )
        } else {
            format!(
                "原始错误: {}\n\n\
                💻 建议检查命令语法和参数。",
                error_msg
            )
        }
    }

    /// Enhance search tool error
    fn enhance_search_error(&self, error_msg: &str) -> String {
        format!(
            "原始错误: {}\n\n\
            🔍 搜索工具指导：\n\
            1. 检查路径参数是否正确\n\
            2. 检查 glob 模式语法（如 *.rs, **/*.ts）\n\
            3. 检查正则表达式语法是否正确\n\
            4. 确认搜索目录存在\n\
            \n\
            建议：先用 ls 确认目录结构。",
            error_msg
        )
    }

    /// Handle tool approval flow
    async fn handle_tool_approval(
        &mut self,
        name: &str,
        input: &serde_json::Value,
        tool_risk_level: RiskLevel,
    ) -> Result<()> {
        if !self.has_ask_channel() {
            return Err(anyhow::anyhow!(
                "Tool '{}' requires manual approval (risk: {}). Use --approve-mode auto to auto-approve.",
                name,
                tool_risk_level
            ));
        }

        let detail = match name {
            "bash" => format!("Command: {}", input["command"].as_str().unwrap_or("?")),
            "write" => format!("File: {}", input["path"].as_str().unwrap_or("?")),
            "edit" | "multi_edit" => {
                format!("File: {}", input["path"].as_str().unwrap_or("?"))
            }
            _ => format!("Tool: {}", name),
        };

        let question = format!(
            "⚠️ Tool '{}' requires approval (risk: {})\n{}\n\nAllow? (y/n)",
            name,
            tool_risk_level,
            detail
        );

        self.emit(AgentEvent::with_data(
            EventType::AskQuestion,
            EventData::AskQuestion {
                question,
                options: None,
            },
        ))?;

        // Check cancellation status before getting ask_channel
        if self.session.is_cancelled() {
            return Err(anyhow::anyhow!("Operation cancelled"));
        }

        if let Some(rx) = self.ask_channel() {
            // Simple receive with cancellation check before
            let answer = rx.recv().await;

            // Check cancellation after receive
            if self.session.is_cancelled() {
                return Err(anyhow::anyhow!("Operation cancelled"));
            }

            match answer {
                Some(answer) => {
                    let answer_lower = answer.trim().to_lowercase();
                    if matches!(
                        answer_lower.as_str(),
                        "a" | "abort" | "q" | "quit" | "stop"
                    ) {
                        self.emit(AgentEvent::with_data(
                            EventType::Error,
                            EventData::Error {
                                message: "Aborted by user".into(),
                                code: None,
                                source: None,
                            },
                        ))?;
                        return Err(anyhow::anyhow!("Session aborted by user"));
                    }
                    let approved = matches!(
                        answer_lower.as_str(),
                        "y" | "yes" | "ok" | "approve" | ""
                    );
                    if !approved {
                        return Err(anyhow::anyhow!(
                            "Tool '{}' rejected by user (answer: '{}')",
                            name,
                            answer_lower
                        ));
                    }
                }
                None => {
                    return Err(anyhow::anyhow!("Approval channel closed"));
                }
            }
        }

        Ok(())
    }

    /// Handle "ask" tool special case
    async fn handle_ask_tool(&mut self, input: &serde_json::Value) -> Result<String> {
        // Check cancellation before getting ask_channel
        if self.session.is_cancelled() {
            return Err(anyhow::anyhow!("Operation cancelled"));
        }

        if input
            .get("questions")
            .and_then(|q| q.as_array())
            .filter(|a| !a.is_empty())
            .is_some()
        {
            let intro = input.get("intro").and_then(|s| s.as_str()).unwrap_or("");
            let questions = input.get("questions").cloned();

            let options = serde_json::json!({
                "questions": questions
            });

            self.emit(AgentEvent::with_data(
                EventType::AskQuestion,
                EventData::AskQuestion {
                    question: intro.to_string(),
                    options: Some(options),
                },
            ))?;

            if let Some(rx) = self.ask_channel() {
                // Simple receive with cancellation check before
                let answer = rx.recv().await;

                // Check cancellation after receive
                if self.session.is_cancelled() {
                    return Err(anyhow::anyhow!("Operation cancelled"));
                }

                match answer {
                    Some(answer) => return Ok(answer),
                    None => return Err(anyhow::anyhow!("Ask channel closed")),
                }
            }
        } else {
            let question = input["question"].as_str().unwrap_or("").to_string();
            let options = input.get("options").cloned();

            self.emit(AgentEvent::with_data(
                EventType::AskQuestion,
                EventData::AskQuestion { question, options },
            ))?;

            if let Some(rx) = self.ask_channel() {
                // Simple receive with cancellation check before
                let answer = rx.recv().await;

                // Check cancellation after receive
                if self.session.is_cancelled() {
                    return Err(anyhow::anyhow!("Operation cancelled"));
                }

                match answer {
                    Some(answer) => return Ok(answer),
                    None => return Err(anyhow::anyhow!("Ask channel closed")),
                }
            }
        }

        Err(anyhow::anyhow!("Ask channel not available"))
    }

    /// 处理代理工具 - 直接调用 executor 执行
    async fn handle_proxy_tool(&mut self, name: &str, input: serde_json::Value) -> Result<String> {
        // 检查是否有执行器
        if let Some(executor) = &self.proxy_executor {
            log::info!("Proxy tool: calling executor for {}", name);

            // 获取超时时间
            let timeout_ms = self
                .proxy_tool_defs
                .iter()
                .find(|t| t.definition.name == name)
                .map(|t| t.timeout_ms)
                .unwrap_or(30000);

            // 执行代理工具（带超时和取消）
            let result = if let Some(token) = self.session.cancel_token() {
                tokio::select! {
                    result = tokio::time::timeout(
                        tokio::time::Duration::from_millis(timeout_ms),
                        executor.exec(name, input.clone())
                    ) => result,
                    _ = wait_for_cancel(token) => {
                        return Err(anyhow::anyhow!("Operation cancelled"));
                    }
                }
            } else {
                tokio::time::timeout(
                    tokio::time::Duration::from_millis(timeout_ms),
                    executor.exec(name, input.clone()),
                )
                .await
            };

            match result {
                Ok(inner_result) => {
                    log::info!("Proxy tool {} completed", name);
                    inner_result
                }
                Err(_) => Err(anyhow::anyhow!(
                    "Proxy tool '{}' timed out after {}ms",
                    name,
                    timeout_ms
                )),
            }
        } else {
            Err(anyhow::anyhow!(
                "Proxy tool '{}' requested but no executor configured. \
                 Use agent.set_proxy_executor() to configure.",
                name
            ))
        }
    }
}