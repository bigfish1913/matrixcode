//! Agent tool execution implementation.

use anyhow::Result;
use std::sync::atomic::Ordering;
use tokio::time::{Duration, sleep};

use crate::approval::{ApproveMode, needs_approval};
use crate::event::{AgentEvent, EventData, EventType};
use crate::providers::{ChatResponse, ContentBlock, Message, MessageContent, Role};

use super::helpers::extract_tool_detail;
use super::types::Agent;

/// Wait for cancellation signal, checking periodically.
async fn wait_for_cancel(token: &crate::cancel::CancellationToken) {
    while !token.is_cancelled() {
        sleep(Duration::from_millis(50)).await;
    }
}

/// Analyze tool error and generate recovery guidance
fn analyze_tool_error(tool_name: &str, error_msg: &str) -> String {
    let guidance = match tool_name {
        "read" => {
            if error_msg.contains("not found") || error_msg.contains("does not exist") {
                "文件不存在。建议：\n1. 检查路径是否正确\n2. 使用 glob 或 ls 查找类似文件\n3. 确认文件是否已被删除或移动"
            } else if error_msg.contains("permission") {
                "权限不足。建议：\n1. 检查文件权限\n2. 尝试其他路径\n3. 询问用户是否有权限问题"
            } else {
                "读取失败。建议检查路径和文件状态，或尝试其他方法获取信息。"
            }
        }
        "write" | "edit" | "multi_edit" => {
            if error_msg.contains("not found") {
                "文件不存在。建议：\n1. 先确认文件路径\n2. 如需创建新文件，明确告知用户\n3. 使用 read 检查目录结构"
            } else if error_msg.contains("permission") {
                "写入权限不足。建议：\n1. 检查文件权限\n2. 询问用户是否需要权限调整\n3. 考虑其他写入位置"
            } else if error_msg.contains("unique") || error_msg.contains("not match") {
                "编辑匹配失败。建议：\n1. 先读取文件确认内容\n2. 检查 old_string 是否精确匹配\n3. 调整匹配内容或使用其他编辑方式"
            } else {
                "写入失败。建议检查路径、权限和内容，或询问用户如何处理。"
            }
        }
        "bash" => {
            if error_msg.contains("timeout") {
                "命令执行超时。建议：\n1. 检查命令是否需要更长时间\n2. 考虑分步执行\n3. 询问用户是否继续等待或调整方案"
            } else if error_msg.contains("permission") {
                "执行权限不足。建议：\n1. 检查命令权限要求\n2. 询问用户是否有 sudo 权限\n3. 考虑替代方案"
            } else if error_msg.contains("not found") {
                "命令或程序不存在。建议：\n1. 检查命令是否正确\n2. 确认程序是否已安装\n3. 考虑替代工具"
            } else {
                "命令执行失败。建议检查命令语法、环境和依赖，或询问用户如何处理。"
            }
        }
        "search" | "grep" | "glob" => {
            if error_msg.contains("not found") || error_msg.contains("no matches") {
                "未找到匹配结果。建议：\n1. 调整搜索模式\n2. 扩大搜索范围\n3. 检查搜索路径是否正确"
            } else {
                "搜索失败。建议调整搜索参数或尝试其他方法。"
            }
        }
        _ => "操作失败。建议分析错误原因，调整方案，或询问用户如何处理。"
    };

    format!("\n\n【错误分析】{}\n【错误详情】{}", guidance, error_msg)
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

                    let result = self.execute_tool(name, input.clone()).await;

                    let (content, is_error) = match result {
                        Ok(output) => (output, false),
                        Err(e) => {
                            // Add intelligent error guidance
                            let enhanced_error = analyze_tool_error(name, &e.to_string());
                            (enhanced_error, true)
                        }
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
}
