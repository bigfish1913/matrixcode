//! /load 命令实现
//!
//! 加载指定会话，包括完整消息历史和项目记忆。

use super::super::backend_context::BackendContext;
use super::super::command_trait::Command;
use crate::{AgentEvent, ContentBlock, EventData, EventType, HistoryMessage, MessageContent, Role};
use std::path::PathBuf;

/// Load 命令
///
/// 用法：
/// - /load <session_id> - 加载指定会话（包括消息历史和记忆）
pub struct Load;

impl Command for Load {
    fn name(&self) -> &'static str {
        "load"
    }

    fn help(&self) -> Option<&'static str> {
        Some("加载指定会话（包括消息历史和记忆）。用法: /load <session_id>")
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a mut BackendContext<'_>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + 'a>> {
        Box::pin(async move {
            let session_id = ctx.message.strip_prefix("/load ").unwrap_or("");

            if let Some(mgr) = ctx.session_mgr {
                if mgr.resume(session_id).is_ok() {
                    // Load full message history for display and agent
                    let full_messages = mgr
                        .full_messages()
                        .map(|msgs| msgs.to_vec())
                        .unwrap_or_default();

                    if !full_messages.is_empty() {
                        // Set full messages to agent
                        ctx.agent.set_messages(full_messages.clone());

                        // Load memory for this session's project path
                        let project_path: Option<PathBuf> = mgr
                            .current_metadata()
                            .and_then(|m| m.project_path.clone())
                            .map(PathBuf::from);

                        if let Some(ref path) = project_path {
                            // Reload memory storage with session's project path
                            if let Ok(new_storage) = crate::memory::MemoryStorage::new(Some(path)) {
                                *ctx.memory_storage = Some(new_storage);
                                log::info!("Loaded memory for project: {}", path.display());
                            }
                        }

                        // Convert messages to history format for TUI display
                        // Each message can produce multiple history entries (text + thinking)
                        let history: Vec<HistoryMessage> = full_messages
                            .iter()
                            .filter_map(|m| {
                                let base_role = match m.role {
                                    Role::User => "user",
                                    Role::Assistant => "assistant",
                                    _ => return None, // Skip system/tool messages
                                };

                                match &m.content {
                                    MessageContent::Text(text) => {
                                        Some(vec![HistoryMessage {
                                            role: base_role.to_string(),
                                            content: text.clone(),
                                            is_thinking: false,
                                        }])
                                    }
                                    MessageContent::Blocks(blocks) => {
                                        // Extract text and thinking from blocks
                                        // Thinking blocks become separate messages
                                        let entries: Vec<HistoryMessage> = blocks
                                            .iter()
                                            .filter_map(|b| match b {
                                                ContentBlock::Text { text } => Some(HistoryMessage {
                                                    role: base_role.to_string(),
                                                    content: text.clone(),
                                                    is_thinking: false,
                                                }),
                                                ContentBlock::Thinking { thinking, .. } => {
                                                    Some(HistoryMessage {
                                                        role: base_role.to_string(),
                                                        content: thinking.clone(),
                                                        is_thinking: true,
                                                    })
                                                }
                                                _ => None,
                                            })
                                            .collect();
                                        if entries.is_empty() {
                                            None
                                        } else {
                                            Some(entries)
                                        }
                                    }
                                }
                            })
                            .flatten()
                            .collect();

                        // Send history to TUI for display
                        if !history.is_empty() {
                            let _ = ctx
                                .event_tx
                                .send(AgentEvent::with_data(
                                    EventType::HistoryLoaded,
                                    EventData::HistoryMessages { messages: history },
                                ))
                                .await;
                        }

                        // Build success message
                        let msg = match project_path {
                            Some(path) => format!(
                                "✓ Session '{}' loaded: {} messages (project: {})",
                                session_id,
                                full_messages.len(),
                                path.display()
                            ),
                            None => format!(
                                "✓ Session '{}' loaded: {} messages",
                                session_id,
                                full_messages.len()
                            ),
                        };

                        let _ = ctx.event_tx.send(AgentEvent::progress(msg, None)).await;
                    } else {
                        let _ = ctx
                            .event_tx
                            .send(AgentEvent::progress(
                                format!("⚠️ Session '{}' has no messages", session_id),
                                None,
                            ))
                            .await;
                    }
                } else {
                    let _ = ctx
                        .event_tx
                        .send(AgentEvent::progress(
                            format!("❌ Session '{}' not found", session_id),
                            None,
                        ))
                        .await;
                }
            } else {
                let _ = ctx
                    .event_tx
                    .send(AgentEvent::progress(
                        "❌ Session manager not available",
                        None,
                    ))
                    .await;
            }

            false // 不转发给 agent
        })
    }
}