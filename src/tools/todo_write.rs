use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

use super::{Tool, ToolDefinition};

pub struct TodoWriteTool;

#[async_trait]
impl Tool for TodoWriteTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "todo_write".to_string(),
            description:
                "Maintain a structured todo list to plan and track multi-step work. \
                 Each call REPLACES the full list. Use for non-trivial tasks (3+ \
                 steps) to show progress. Keep exactly one task 'in_progress' at \
                 a time and mark tasks 'completed' as soon as they finish."
                    .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "todos": {
                        "type": "array",
                        "description": "Full replacement list of todo items",
                        "items": {
                            "type": "object",
                            "properties": {
                                "content": {
                                    "type": "string",
                                    "description": "Imperative form (e.g. 'Run tests')"
                                },
                                "activeForm": {
                                    "type": "string",
                                    "description": "Present-continuous form (e.g. 'Running tests')"
                                },
                                "status": {
                                    "type": "string",
                                    "enum": ["pending", "in_progress", "completed"]
                                }
                            },
                            "required": ["content", "activeForm", "status"]
                        }
                    }
                },
                "required": ["todos"]
            }),
        }
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let todos = params["todos"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("missing 'todos' array"))?;

        // Show spinner while updating todo list
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::with_template("{spinner:.cyan} {msg}")
                .unwrap_or_else(|_| ProgressStyle::default_spinner())
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
        );
        spinner.set_message(format!("updating todos ({} items)", todos.len()));
        spinner.enable_steady_tick(Duration::from_millis(80));

        let mut in_progress_count = 0;
        let mut completed_count = 0;
        let mut pending_count = 0;
        let mut lines = Vec::with_capacity(todos.len() + 1);

        for (i, todo) in todos.iter().enumerate() {
            let content = todo["content"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("todo {}: missing 'content'", i))?;
            let active = todo["activeForm"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("todo {}: missing 'activeForm'", i))?;
            let status = todo["status"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("todo {}: missing 'status'", i))?;

            let (marker, display) = match status {
                "pending" => {
                    pending_count += 1;
                    ("[ ]", content)
                }
                "in_progress" => {
                    in_progress_count += 1;
                    ("[~]", active)
                }
                "completed" => {
                    completed_count += 1;
                    ("[x]", content)
                }
                other => anyhow::bail!("todo {}: invalid status '{}'", i, other),
            };

            lines.push(format!("  {} {}", marker, display));
        }

        if in_progress_count > 1 {
            spinner.finish_with_message("✗ multiple in_progress".to_string());
            anyhow::bail!(
                "at most one task may be 'in_progress' at a time (found {})",
                in_progress_count
            );
        }

        if lines.is_empty() {
            spinner.finish_with_message("✓ cleared".to_string());
            return Ok("Todo list cleared.".to_string());
        }

        let header = format!("Todos ({}):", lines.len());
        let mut out = String::with_capacity(header.len() + lines.iter().map(|l| l.len() + 1).sum::<usize>());
        out.push_str(&header);
        for l in &lines {
            out.push('\n');
            out.push_str(l);
        }

        spinner.finish_with_message(format!("✓ {} pending, {} in_progress, {} done", 
            pending_count, in_progress_count, completed_count));
        Ok(out)
    }
}