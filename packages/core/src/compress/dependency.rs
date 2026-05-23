//! Message dependency tracking for preserving conversation coherence.
//!
//! Builds a graph of ToolUse ↔ ToolResult pairs to ensure they are
//! preserved together during compression.

use std::collections::HashMap;

use crate::providers::{ContentBlock, Message, MessageContent};

use super::types::{DependencyGraph, MessageDependency};

/// Builder for dependency graphs.
pub struct DependencyBuilder;

impl DependencyBuilder {
    /// Build a dependency graph from message history.
    pub fn build(messages: &[Message]) -> DependencyGraph {
        let mut dependencies: Vec<MessageDependency> = Vec::new();
        let mut pending_tool_use: HashMap<String, usize> = HashMap::new();
        let mut tool_names: HashMap<String, String> = HashMap::new();

        // Scan all messages to find ToolUse-ToolResult pairs
        for (idx, msg) in messages.iter().enumerate() {
            let blocks = get_content_blocks(&msg.content);

            for block in blocks {
                match block {
                    ContentBlock::ToolUse { id, name, .. } => {
                        // Record pending ToolUse
                        pending_tool_use.insert(id.clone(), idx);
                        tool_names.insert(id.clone(), name.clone());
                    }
                    ContentBlock::ToolResult { tool_use_id, .. } => {
                        // Match with pending ToolUse
                        if let Some(tool_use_idx) = pending_tool_use.remove(tool_use_id.as_str()) {
                            let tool_name = tool_names
                                .get(tool_use_id.as_str())
                                .cloned()
                                .unwrap_or_else(|| "unknown".to_string());

                            dependencies.push(MessageDependency {
                                tool_use_idx,
                                tool_result_idx: idx,
                                tool_name: tool_name.clone(),
                                is_critical: is_critical_tool(&tool_name),
                            });
                        }
                    }
                    _ => {}
                }
            }
        }

        // Build reverse index for quick lookup
        let message_to_deps = build_reverse_index(&dependencies);

        DependencyGraph {
            dependencies,
            message_to_deps,
        }
    }

    /// Build dependency graph with custom critical tool detection.
    pub fn build_with_custom_critical(
        messages: &[Message],
        critical_tools: &[&str],
    ) -> DependencyGraph {
        let mut graph = Self::build(messages);

        // Update is_critical based on custom list
        for dep in &mut graph.dependencies {
            dep.is_critical = critical_tools.contains(&dep.tool_name.as_str());
        }

        graph
    }
}

/// Extract content blocks from a message.
fn get_content_blocks(content: &MessageContent) -> Vec<&ContentBlock> {
    match content {
        MessageContent::Blocks(blocks) => blocks.iter().collect(),
        _ => Vec::new(),
    }
}

/// Check if a tool is considered critical (modifies state).
fn is_critical_tool(name: &str) -> bool {
    let critical_tools = ["write", "edit", "multi_edit", "bash"];
    critical_tools.contains(&name)
}

/// Build reverse index: message idx -> dependency indices.
fn build_reverse_index(dependencies: &[MessageDependency]) -> HashMap<usize, Vec<usize>> {
    let mut index: HashMap<usize, Vec<usize>> = HashMap::new();

    for (dep_idx, dep) in dependencies.iter().enumerate() {
        // Add ToolUse index
        index.entry(dep.tool_use_idx).or_default().push(dep_idx);

        // Add ToolResult index
        index.entry(dep.tool_result_idx).or_default().push(dep_idx);
    }

    index
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::Role;

    #[test]
    fn test_build_empty() {
        let messages: Vec<Message> = Vec::new();
        let graph = DependencyBuilder::build(&messages);
        assert_eq!(graph.dependencies.len(), 0);
    }

    #[test]
    fn test_build_single_pair() {
        let messages = vec![
            Message {
                role: Role::User,
                content: MessageContent::Text("Read file".to_string()),
            },
            Message {
                role: Role::Assistant,
                content: MessageContent::Blocks(vec![ContentBlock::ToolUse {
                    id: "t1".to_string(),
                    name: "read".to_string(),
                    input: serde_json::json!({"path": "test.rs"}),
                }]),
            },
            Message {
                role: Role::Tool,
                content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                    tool_use_id: "t1".to_string(),
                    content: "file content".to_string(),
                }]),
            },
        ];

        let graph = DependencyBuilder::build(&messages);
        assert_eq!(graph.dependencies.len(), 1);

        let dep = &graph.dependencies[0];
        assert_eq!(dep.tool_use_idx, 1);
        assert_eq!(dep.tool_result_idx, 2);
        assert_eq!(dep.tool_name, "read");
        assert!(!dep.is_critical);
    }

    #[test]
    fn test_build_critical_tool() {
        let messages = vec![
            Message {
                role: Role::Assistant,
                content: MessageContent::Blocks(vec![ContentBlock::ToolUse {
                    id: "t1".to_string(),
                    name: "write".to_string(),
                    input: serde_json::json!({"path": "test.rs"}),
                }]),
            },
            Message {
                role: Role::Tool,
                content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                    tool_use_id: "t1".to_string(),
                    content: "success".to_string(),
                }]),
            },
        ];

        let graph = DependencyBuilder::build(&messages);
        assert_eq!(graph.dependencies.len(), 1);
        assert!(graph.dependencies[0].is_critical);
    }

    #[test]
    fn test_build_multiple_tools_same_message() {
        let messages = vec![
            Message {
                role: Role::Assistant,
                content: MessageContent::Blocks(vec![
                    ContentBlock::ToolUse {
                        id: "t1".to_string(),
                        name: "read".to_string(),
                        input: serde_json::json!({"path": "a.rs"}),
                    },
                    ContentBlock::ToolUse {
                        id: "t2".to_string(),
                        name: "read".to_string(),
                        input: serde_json::json!({"path": "b.rs"}),
                    },
                ]),
            },
            Message {
                role: Role::Tool,
                content: MessageContent::Blocks(vec![
                    ContentBlock::ToolResult {
                        tool_use_id: "t1".to_string(),
                        content: "content a".to_string(),
                    },
                    ContentBlock::ToolResult {
                        tool_use_id: "t2".to_string(),
                        content: "content b".to_string(),
                    },
                ]),
            },
        ];

        let graph = DependencyBuilder::build(&messages);
        assert_eq!(graph.dependencies.len(), 2);

        // Check reverse index
        assert!(graph.message_to_deps.contains_key(&0));
        assert_eq!(graph.message_to_deps.get(&0).unwrap().len(), 2);
    }

    #[test]
    fn test_missing_tool_result() {
        let messages = vec![
            Message {
                role: Role::Assistant,
                content: MessageContent::Blocks(vec![ContentBlock::ToolUse {
                    id: "t1".to_string(),
                    name: "read".to_string(),
                    input: serde_json::json!({"path": "test.rs"}),
                }]),
            },
        ];

        let graph = DependencyBuilder::build(&messages);
        assert_eq!(graph.dependencies.len(), 0);
    }

    #[test]
    fn test_reverse_index() {
        let messages = vec![
            Message {
                role: Role::Assistant,
                content: MessageContent::Blocks(vec![ContentBlock::ToolUse {
                    id: "t1".to_string(),
                    name: "read".to_string(),
                    input: serde_json::json!({}),
                }]),
            },
            Message {
                role: Role::Tool,
                content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                    tool_use_id: "t1".to_string(),
                    content: "result".to_string(),
                }]),
            },
        ];

        let graph = DependencyBuilder::build(&messages);

        // Both indices should map to the same dependency
        assert!(graph.message_to_deps.contains_key(&0));
        assert!(graph.message_to_deps.contains_key(&1));
    }

    #[test]
    fn test_get_pair_indices() {
        let messages = vec![
            Message {
                role: Role::Assistant,
                content: MessageContent::Blocks(vec![ContentBlock::ToolUse {
                    id: "t1".to_string(),
                    name: "read".to_string(),
                    input: serde_json::json!({}),
                }]),
            },
            Message {
                role: Role::Tool,
                content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                    tool_use_id: "t1".to_string(),
                    content: "result".to_string(),
                }]),
            },
        ];

        let graph = DependencyBuilder::build(&messages);

        // Get pair indices for ToolUse
        let pairs = graph.get_pair_indices(0);
        assert_eq!(pairs, vec![1]);

        // Get pair indices for ToolResult
        let pairs = graph.get_pair_indices(1);
        assert_eq!(pairs, vec![0]);
    }
}