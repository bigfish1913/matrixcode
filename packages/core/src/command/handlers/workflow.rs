//! /workflow command handler

use std::future::Future;
use std::pin::Pin;

use crate::command::{Command, BackendContext};
use crate::workflow::WorkflowRegistry;

pub struct WorkflowCommand;

impl Command for WorkflowCommand {
    fn name(&self) -> &'static str {
        "workflow"
    }

    fn help(&self) -> Option<&'static str> {
        Some("Discover and run workflows: /workflow [name]")
    }

    fn execute<'a>(&'a self, ctx: &'a mut BackendContext<'_>) 
        -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> 
    {
        Box::pin(async move {
            let msg = ctx.message;
            let parts: Vec<&str> = msg.split_whitespace().collect();
            let subcmd = parts.get(1).copied().unwrap_or("");

            let response = match subcmd {
                "" | "discover" | "list" => {
                    let registry = WorkflowRegistry::new(ctx.project_path);
                    if registry.is_empty() {
                        "📋 No workflows found.".to_string()
                    } else {
                        registry.generate_summary()
                    }
                }
                "match" => {
                    let query = parts.get(2..).map(|p| p.join(" ")).unwrap_or_default();
                    if query.is_empty() {
                        "Usage: /workflow match <query>".to_string()
                    } else {
                        let registry = WorkflowRegistry::new(ctx.project_path);
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
                    }
                }
                "run" => {
                    let workflow_id = parts.get(2).copied().unwrap_or("");
                    if workflow_id.is_empty() {
                        "Usage: /workflow run <workflow-id>".to_string()
                    } else {
                        format!("⏳ Workflow '{}' queued. Use CLI for full execution.", workflow_id)
                    }
                }
                _ => format!("Unknown subcommand '{}'.", subcmd)
            };

            let _ = ctx.event_tx.send(crate::AgentEvent::progress(response, None)).await;
            false
        })
    }
}