//! /memory command handler

use std::future::Future;
use std::pin::Pin;

use crate::command::{Command, BackendContext};
use crate::memory;
use crate::constants::DISPLAY_MEMORY_SEARCH_LIMIT;

pub struct MemoryCommand;

impl Command for MemoryCommand {
    fn name(&self) -> &'static str {
        "memory"
    }

    fn help(&self) -> Option<&'static str> {
        Some("Manage memory: /memory [search|add|stats]")
    }

    fn execute<'a>(&'a self, ctx: &'a mut BackendContext<'_>) 
        -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> 
    {
        Box::pin(async move {
            let msg = ctx.message;
            let parts: Vec<&str> = msg.split_whitespace().collect();
            let subcmd = parts.get(1).copied().unwrap_or("");

            if let Some(ms) = ctx.memory_storage {
                let response = match subcmd {
                    "" | "list" => {
                        if let Ok(mem) = ms.load_combined() {
                            if mem.entries.is_empty() {
                                "📝 No memories stored.".to_string()
                            } else {
                                mem.generate_statistics().format_summary()
                            }
                        } else {
                            "❌ Failed to load memories".to_string()
                        }
                    }
                    "stats" => {
                        if let Ok(mem) = ms.load_combined() {
                            mem.generate_statistics().format_summary()
                        } else {
                            "❌ Failed to get stats".to_string()
                        }
                    }
                    "search" => {
                        let query = parts.get(2..).map(|p| p.join(" ")).unwrap_or_default();
                        if query.is_empty() {
                            "Usage: /memory search <query>".to_string()
                        } else if let Ok(mem) = ms.load_combined() {
                            let results = mem.search_with_limit(&query, Some(DISPLAY_MEMORY_SEARCH_LIMIT));
                            if results.is_empty() {
                                format!("No memories found for '{}'", query)
                            } else {
                                format!("🔍 Found {} memories for '{}'", results.len(), query)
                            }
                        } else {
                            "❌ Failed to search".to_string()
                        }
                    }
                    "analyze" => {
                        if let Some(pp) = ctx.project_path {
                            let count = memory::generate_project_structure_memories(pp.as_path(), ms);
                            format!("✓ Generated {} structure memories", count)
                        } else {
                            "❌ No project path".to_string()
                        }
                    }
                    "merge" => {
                        if let Ok(mut mem) = ms.load_combined() {
                            let count = mem.smart_merge();
                            if let Err(e) = ms.save_global(&mem) {
                                log::warn!("Failed to save: {}", e);
                            }
                            format!("✓ Merged {} similar memories", count)
                        } else {
                            "❌ Failed to merge".to_string()
                        }
                    }
                    "help" => "Commands: list, stats, search, analyze, merge".to_string(),
                    _ => format!("Unknown command '{}'. Use '/memory help'", subcmd)
                };
                let _ = ctx.event_tx.send(crate::AgentEvent::progress(response, None)).await;
            } else {
                let _ = ctx.event_tx.send(crate::AgentEvent::progress(
                    "❌ Memory storage not available".to_string(),
                    None,
                )).await;
            }
            false
        })
    }
}