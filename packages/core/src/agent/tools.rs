//! Agent tool execution implementation.

use anyhow::Result;
use std::sync::atomic::Ordering;
use tokio::time::{Duration, sleep};

use crate::approval::{ApproveMode, needs_approval};
use crate::event::{AgentEvent, EventData, EventType};
use crate::providers::{ChatResponse, ContentBlock, Message, MessageContent, Role};
use crate::truncate::truncate_with_suffix;

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
                    if let Some(token) = &self.cancel_token
                        && token.is_cancelled()
                    {
                        return Err(anyhow::anyhow!("Operation cancelled"));
                    }

                    has_tool_use = true;

                    // Emit ToolUseStart event with full input before execution when the
                    // streaming phase did not already preview the complete input.
                    // This allows UI to display command details before result arrives.
                    if !self.previewed_tool_inputs.remove(id) {
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
            self.messages.push(Message {
                role: Role::Assistant,
                content: MessageContent::Blocks(assistant_content),
            });
        }

        for msg in tool_results {
            self.messages.push(msg);
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

        let tool = self.tools.iter().find(|t| t.definition().name == name);

        if let Some(tool) = tool {
            let current_mode = ApproveMode::from_u8(self.approve_mode.load(Ordering::Relaxed));

            log::debug!(
                "Tool '{}' approval check: mode={}, risk={}, needs_approval={}",
                name,
                current_mode,
                tool.risk_level(),
                needs_approval(current_mode, tool.risk_level())
            );

            if needs_approval(current_mode, tool.risk_level()) {
                if self.ask_rx.is_some() {
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
                        tool.risk_level(),
                        detail
                    );

                    self.emit(AgentEvent::with_data(
                        EventType::AskQuestion,
                        EventData::AskQuestion {
                            question,
                            options: None,
                        },
                    ))?;

                    if let Some(rx) = &mut self.ask_rx {
                        // Wait for approval with cancellation check
                        let answer = if let Some(token) = &self.cancel_token {
                            tokio::select! {
                                result = rx.recv() => result,
                                _ = wait_for_cancel(token) => {
                                    return Err(anyhow::anyhow!("Operation cancelled"));
                                }
                            }
                        } else {
                            rx.recv().await
                        };

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
                } else {
                    return Err(anyhow::anyhow!(
                        "Tool '{}' requires manual approval (risk: {}). Use --approve-mode auto to auto-approve.",
                        name,
                        tool.risk_level()
                    ));
                }
            }

            // Special handling for "ask" tool in TUI mode
            if name == "ask" && self.ask_rx.is_some() {
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

                    if let Some(rx) = &mut self.ask_rx {
                        // Wait for answer with cancellation check
                        let answer = if let Some(token) = &self.cancel_token {
                            tokio::select! {
                                result = rx.recv() => result,
                                _ = wait_for_cancel(token) => {
                                    return Err(anyhow::anyhow!("Operation cancelled"));
                                }
                            }
                        } else {
                            rx.recv().await
                        };

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

                    if let Some(rx) = &mut self.ask_rx {
                        // Wait for answer with cancellation check
                        let answer = if let Some(token) = &self.cancel_token {
                            tokio::select! {
                                result = rx.recv() => result,
                                _ = wait_for_cancel(token) => {
                                    return Err(anyhow::anyhow!("Operation cancelled"));
                                }
                            }
                        } else {
                            rx.recv().await
                        };

                        match answer {
                            Some(answer) => return Ok(answer),
                            None => return Err(anyhow::anyhow!("Ask channel closed")),
                        }
                    }
                }
            }

            self.emit(AgentEvent::progress(format!("Executing: {}", name), None))?;
            tool.execute(input).await
        } else {
            Err(anyhow::anyhow!("Tool '{}' not found", name))
        }
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
            let result = if let Some(token) = &self.cancel_token {
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
