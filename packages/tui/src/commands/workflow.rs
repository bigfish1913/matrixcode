//! /workflow command

use crate::commands::{Command, CommandContext};
use matrixcode_core::workflow::WorkflowRegistry;

pub struct WorkflowCommand;

impl Command for WorkflowCommand {
    fn name(&self) -> &'static str {
        "workflow"
    }

    fn help(&self) -> Option<&'static str> {
        Some("Manage workflows")
    }

    fn execute(&self, ctx: &mut CommandContext, args: &[&str]) {
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
            format!(
                "⏳ Use CLI to run workflows:\n\n  matrixcode workflow run --file .matrix/workflows/{}.yaml",
                args[1]
            )
        } else if args[0] == "help" {
            "📋 /workflow commands:\n\n  discover - List available workflows\n  match <query> - Find matching workflows\n  run <id> - Hint for CLI execution".to_string()
        } else {
            "Unknown command. Use '/workflow help' for available commands.".to_string()
        };

        ctx.push_system(response);
        ctx.auto_scroll();
    }
}
